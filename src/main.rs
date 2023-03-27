use chrono::{Datelike, Local};
use image::{ImageBuffer, Rgba};
use rusttype::{Font, Scale};
use screenshots::Screen;
use serde::{Deserialize, Serialize};
// use std::alloc::System;
use std::fs;
use std::fs::{create_dir_all, File};
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    interval: u64,
    max_sleep_secs: u64,
    output_dir: String,
}

fn main() {
    let config_file = File::open("config.json").expect("Failed to open config.json");
    let config: Config = serde_json::from_reader(config_file).expect("Failed to read config file");

    let mut last_dir_date = String::new();
    // let last_screenshot_time = SystemTime::now();

    loop {
        let now = Local::now();
        let year = now.year().to_string();
        let month = format!("{:02}", now.month());
        let day = format!("{:02}", now.day());
        let dir_date = format!("{}-{}-{}", year, month, day);

        let output_dir = std::path::Path::new(&config.output_dir)
            .join(&year)
            .join(&month)
            .join(&day);

        if dir_date > last_dir_date {
            create_dir_all(&output_dir).expect("Failed to create output directory");
            last_dir_date = dir_date.clone();
        }

        let num_files = count_files(&output_dir).expect("Failed to count files");

        let filename = format!("{:05}.png", num_files);
        let filepath = output_dir.join(filename);

        let buffer = Screen::all()
            .unwrap()
            .first()
            .unwrap()
            .capture()
            .unwrap()
            .buffer()
            .to_owned();

        fs::write(&filepath, buffer).expect("Failed to write PNG data to file");
        print!("Presumably just wrote to {:?}", filepath);

        thread::sleep(Duration::from_secs(config.interval));
    }
}

fn count_files(dir: &std::path::Path) -> std::io::Result<usize> {
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        }
    }
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
