use anyhow::Error;
use chrono::{Datelike, Local};
use ctrlc;
use env_logger::Builder;
use image::{ImageBuffer, Rgba};
use log::{debug, info, LevelFilter};
use rusttype::{Font, Scale};
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::{create_dir_all, File};
use std::path::PathBuf;
use std::thread;
use symlink::symlink_file;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::{ctrl_c_exit, get_screenshot};

#[cfg(not(target_os = "windows"))]
mod not_windows;
#[cfg(not(target_os = "windows"))]
use not_windows::{ctrl_c_exit, get_screenshot};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    interval: u64,
    max_sleep_secs: u64,
    output_dir: String,
}

type FrameCounter = u32;

fn main() {
    ctrlc::set_handler(move || {
        ctrl_c_exit();
    })
    .expect("Couldn't set a clean exit handler!");

    Builder::new()
        .filter_level(LevelFilter::max())
        .filter_module("wmi", LevelFilter::Error)
        .init();

    let config_file = File::open("config.json").expect("Failed to open config.json");
    let config: Config = serde_json::from_reader(config_file).expect("Failed to read config file");
    debug!("Read config of: {:?}", config);
    let sleep_interval = std::time::Duration::from_secs(config.interval);

    let starting_time = Local::now();
    let mut last_time = starting_time.clone();
    let year = starting_time.year().to_string();
    let month = format!("{:02}", starting_time.month());
    let day = format!("{:02}", starting_time.day());

    let output_dir = std::path::Path::new(&config.output_dir)
        .join(&year)
        .join(&month)
        .join(&day);

    create_dir_all(&output_dir).expect("Failed to create output directory");
    let mut curr_frame = get_curr_frame(&output_dir).expect("Failed to count files");
    let screen = Screen::all().unwrap().first().unwrap().to_owned();

    loop {
        let capture_result = get_screenshot(screen);
        match capture_result {
            Err(e) => {
                info!("Couldn't get a good screenshot ({:?}), skip this frame", e);
                thread::sleep(sleep_interval);
                continue;
            }
            _ => (),
        }

        let now = Local::now();

        // NOTE: Timezone changes are handled correctly, so this can only go backwards if the timezone doesn't
        // change but the system clock goes backwards somehow.
        let elapsed_since_last_shot = (now - last_time).num_seconds() as i64;

        if elapsed_since_last_shot > config.max_sleep_secs as i64 {
            // At this point we know we went *forward* in time since max_sleep_secs can only be
            // positive, so it's safe to cast the i64 to a u64.
            curr_frame = deal_with_blackout(
                elapsed_since_last_shot as u64,
                &output_dir,
                curr_frame,
                &sleep_interval,
            )
            .unwrap();
        }

        let filename = format!("{:05}.png", curr_frame);
        let filepath = output_dir.join(filename);

        let capture = capture_result.unwrap();
        debug!("Writing out a file to {:?}", filepath);
        fs::write(&filepath, capture.buffer()).expect("Failed to write PNG data to file");
        curr_frame += 1;
        last_time = now;

        thread::sleep(sleep_interval);
    }
}

fn get_curr_frame(dir: &std::path::Path) -> std::io::Result<FrameCounter> {
    debug!("Examining {:?}", dir);
    let mut count: FrameCounter = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        /*
        if !(entry.file_type()?.is_file()) {
            continue;
        }
        */
        if !(entry.path().extension().unwrap() == "png") {
            continue;
        }
        count += 1;
    }
    debug!("Found {:?} existing PNGs", count);
    Ok(count)
}

fn create_filler_frame(
    duration_secs: u64,
    width: u32,
    height: u32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let mut img = ImageBuffer::from_pixel(width, height, black);

    let duration_str = format!("{:#} go by", human_duration(duration_secs));
    let font_data = include_bytes!("Ubuntu-Regular.ttf");
    let font = Font::try_from_bytes(font_data as &[u8]).unwrap();
    let font_size = 80.0;
    let scale = Scale::uniform(font_size);
    let (text_w, text_h) = imageproc::drawing::text_size(scale, &font, &duration_str);
    let offset_x = (width as f32 / 2.0) - (text_w as f32 / 2.0);
    let offset_y = (height as f32 / 2.0) - (text_h as f32 / 2.0);

    // Write the text to the image
    imageproc::drawing::draw_text_mut(
        &mut img,
        white,
        offset_x as i32,
        offset_y as i32,
        scale,
        &font,
        &duration_str,
    );

    // return the resulting image
    img
}

fn deal_with_blackout(
    elapsed_secs: u64,
    output_dir: &PathBuf,
    curr_frame: FrameCounter,
    sleep_interval: &std::time::Duration,
) -> Result<FrameCounter, Error> {
    info!(
        "Looks like we've been away for a while ({:?} seconds).",
        elapsed_secs
    );

    let filler_frame_path = output_dir.join(format!("{:05}.png", curr_frame));

    info!("Creating filler frame @ {:?}", filler_frame_path);
    create_filler_frame(elapsed_secs, 860, 360)
        .save(&filler_frame_path)
        .expect("Couldn't create filler frame!");

    let missed_frames = (elapsed_secs / sleep_interval.as_secs()) as u32;
    debug!("Going to create {:?} frames", missed_frames);
    for n in 1..missed_frames {
        symlink_file(
            &filler_frame_path,
            output_dir.join(format!("{:05}.png", curr_frame + n)),
        )?;
    }

    debug!("New curr_frame = {:?}", curr_frame + missed_frames);
    Ok(curr_frame + missed_frames)
}

fn human_duration(duration_secs: u64) -> String {
    let cleaned;
    let unit;

    if duration_secs >= 3600 {
        cleaned = duration_secs / 3600;
        unit = "hr";
    } else if duration_secs > 60 {
        cleaned = duration_secs / 60;
        unit = "min";
    } else {
        cleaned = duration_secs;
        unit = "sec";
    }

    if cleaned > 1 {
        format!("~ {} {}s", cleaned, unit)
    } else {
        format!("~ {} {}", cleaned, unit)
    }
}
