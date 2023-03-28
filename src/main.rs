use chrono::{Datelike, Local};
// use image::{ImageBuffer, Rgba};
// use rusttype::{Font, Scale};
use screenshots::Screen;
use serde::{Deserialize, Serialize};
// use std::alloc::System;
use env_logger::Builder;
use log::{debug, LevelFilter};
use std::fs;
use std::fs::{create_dir_all, File};
use std::thread;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    interval: u64,
    max_sleep_secs: u16,
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

    loop {
        let now = Local::now();
        if now.timestamp() - last_time.timestamp() > config.max_sleep_secs.into() {
            // TODO: Handle the day rolling over, or needing a new directory to work in
            panic!("Uhhh, day rolled over, I should do something smarter here!");
        }

        let filename = format!("{:05}.png", curr_frame);
        let filepath = output_dir.join(filename);

        let capture = screen.capture().unwrap();

        fs::write(&filepath, capture.buffer()).expect("Failed to write PNG data to file");

        curr_frame += 1;
        last_time = now;
        thread::sleep(Duration::from_secs(config.interval));
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

/*
fn create_filler_frame(
    duration: Duration,
    width: Option<u32>,
    height: Option<u32>,
) -> image::DynamicImage {
    let _width = width.unwrap_or(1920);
    let _height = height.unwrap_or(1080);
    let black = Rgba([0, 0, 0, 255]);
    let img = ImageBuffer::from_pixel(_width, _height, black);


    let duration_str = format!("{:?}", duration);
    let font_data = include_bytes!("Ubuntu-Regular.ttf");
    let font = Font::try_from_bytes(font_data as &[u8]).unwrap();
    let font_size = 120.0;
    let scale = Scale::uniform(font_size);
    let v_metrics = font.v_metrics(scale);
    let offset_x = ((_width as f32 - font_size * duration_str.len() as f32) / 2.0) as f32;
    let offset_y = ((_height as f32 - font_size) / 2.0 + v_metrics.ascent) as f32;

    // Write the text to the image
    let draw_text = imageproc::drawing::draw_text_mut;
    draw_text(
        &mut img,
        Rgba([255, 255, 255, 255]),
        offset_x as i32,
        offset_y as i32,
        scale,
        &font,
        &duration_str,
    );

    // return the resulting image
    img.
}
*/
