use chrono::{Datelike, Local};
use image::{ImageBuffer, Rgba};
use rusttype::{Font, Scale};
use screenshots::Screen;
use serde::{Deserialize, Serialize};
// use std::alloc::System;
use duration_human::DurationHuman;
use env_logger::Builder;
use log::{debug, info, LevelFilter, warn};
use std::fs;
use std::fs::{create_dir_all, File};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    interval: u64,
    max_sleep_secs: u64,
    output_dir: String,
}

fn main() {
    Builder::new().filter_level(LevelFilter::max()).init();
    let config_file = File::open("config.json").expect("Failed to open config.json");
    let config: Config = serde_json::from_reader(config_file).expect("Failed to read config file");
    debug!("Read config of: {:?}", config);

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
    let sleep_interval = Duration::from_secs(config.interval);

    loop {
        let now = Local::now();
        let elapsed_since_last_shot = Duration::new((now.timestamp() - last_time.timestamp()) as u64, 0);
        if elapsed_since_last_shot.as_secs() > config.max_sleep_secs {
            create_filler_frame(elapsed_since_last_shot, 860, 360).save(output_dir.join("test-frame.png")).expect("Couldn't create filler frame!");
            // TODO: Handle the day rolling over, or needing a new directory to work in
            panic!("Uhhh, day rolled over, I should do something smarter here!");
        }

        let filename = format!("{:05}.png", curr_frame);
        let filepath = output_dir.join(filename);

        let capture_result = screen.capture();
        match capture_result {
            Ok(capture) => {
                fs::write(&filepath, capture.buffer()).expect("Failed to write PNG data to file");
                curr_frame += 1;
                last_time = now;
            },
            Err(error) => {
                warn!("Trouble capturing screen: {:?}", error);
            }
        }

        thread::sleep(sleep_interval);
    }
}

fn get_curr_frame(dir: &std::path::Path) -> std::io::Result<usize> {
    debug!("Examining {:?}", dir);
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !(entry.file_type()?.is_file()) {
            continue;
        }
        if !(entry.path().extension().unwrap() == "png") {
            continue;
        }
        count += 1;
    }
    debug!("Found {:?} existing PNGs", count);
    Ok(count)
}

fn create_filler_frame(
    duration: Duration,
    width: u32,
    height: u32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let mut img = ImageBuffer::from_pixel(width, height, black);

    let duration_str = format!("{:#} go by", DurationHuman::try_from(duration).unwrap());
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
