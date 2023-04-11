use anyhow::Error;
use chrono::{DateTime, Datelike, Local};
use image::{ImageBuffer, Rgba};
use log::{debug, error, info};
use rusttype::{Font, Scale};
use screenshots::{Image, Screen};
use std::fs;
use std::path::Path;
use symlink::symlink_file;

#[cfg(target_os = "windows")]
use crate::windows::get_screenshot;

#[cfg(not(target_os = "windows"))]
mod not_windows;
#[cfg(not(target_os = "windows"))]
use not_windows::{ctrl_c_exit, get_screenshot};

use crate::dir_manager::DirManager;

pub type FrameCounter = u32;

pub struct Capturer {
    screen: Screen,
    sleep_interval: std::time::Duration,
    curr_frame: u32,
}

impl Capturer {
    pub fn new(sleep_interval: &std::time::Duration) -> Capturer {
        Capturer {
            screen: Screen::all().unwrap().first().unwrap().to_owned(),
            sleep_interval: sleep_interval.to_owned(),
            curr_frame: 0,
        }
    }

    pub fn deal_with_change(
        &mut self,
        dir_manager: &mut DirManager,
        prev_time: &DateTime<Local>,
        curr_time: &DateTime<Local>,
    ) -> Result<(), Error> {
        if (curr_time.ordinal() > prev_time.ordinal()) || (curr_time.year() > prev_time.year()) {
            // Obviously this could be a new month, or even new year. Whatever, we'll be fine either way!
            self.deal_with_new_day(dir_manager)
        } else {
            // Same day, so there was just a blackout. nbd.
            self.deal_with_blackout((*curr_time - *prev_time).num_seconds() as u64, dir_manager)
        }
    }

    pub fn discover_current_frame(&mut self, dir_manager: &mut DirManager) {
        let curr_frame = self.get_curr_frame(dir_manager);
        match curr_frame {
            Ok(new_curr_frame) => {
                self.curr_frame = new_curr_frame;
            }
            Err(e) => {
                error!("Issue getting current frame: {e:?}");
                self.curr_frame = 0;
            }
        }
    }

    fn deal_with_new_day(&mut self, dir_manager: &mut DirManager) -> Result<(), Error> {
        info!("Brand new day! Let's goooooo");

        // TODO: Fire up a resizer, gap filler, and movie maker for the previous day. Do this before
        // getting ready for today to make sure we have the right path to make movies in.

        dir_manager.make_output_dir()?;
        self.curr_frame = 0;

        Ok(())
    }

    pub fn capture_screen(&self) -> Result<Image, anyhow::Error> {
        get_screenshot(self.screen)
    }

    pub fn store(&mut self, capture_result: Image, dir: &Path) {
        let filename = format!("{:05}.png", self.curr_frame);
        let filepath = dir.join(filename);

        assert!(!filepath.exists(), "I'm trying to overwrite myself!");

        let capture = capture_result;
        debug!("Writing out a file to {filepath:?}");
        fs::write(&filepath, capture.buffer()).expect("Failed to write PNG data to file");
        self.curr_frame += 1;
    }

    fn deal_with_blackout(
        &mut self,
        elapsed_secs: u64,
        dir_manager: &mut DirManager,
    ) -> Result<(), Error> {
        info!("Looks like we've been away for a while ({elapsed_secs:?} seconds).");

        let filler_frame_path = dir_manager
            .current_dir()
            .join(format!("{:05}.png", self.curr_frame));

        info!("Creating filler frame @ {filler_frame_path:?}");
        Self::create_filler_frame(elapsed_secs, 860, 360)
            .save(&filler_frame_path)
            .expect("Couldn't create filler frame!");

        let missed_frames = (elapsed_secs / self.sleep_interval.as_secs()) as u32;
        debug!("Going to create {missed_frames:?} frames");
        for n in 1..missed_frames {
            symlink_file(
                &filler_frame_path,
                dir_manager
                    .current_dir()
                    .join(format!("{:05}.png", self.curr_frame + n)),
            )?;
        }

        debug!("New curr_frame = {:?}", self.curr_frame + missed_frames);
        self.curr_frame += missed_frames;
        Ok(())
    }

    fn get_curr_frame(&self, dir_manager: &mut DirManager) -> std::io::Result<FrameCounter> {
        let dir = dir_manager.current_dir();

        debug!("Examining {dir:?}");
        let mut count: FrameCounter = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().unwrap() != "png" {
                continue;
            }
            count += 1;
        }
        debug!("Found {count:?} existing PNGs");
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

        let duration_str = format!("{:#} go by", Self::human_duration(duration_secs));
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
            format!("~ {cleaned} {unit}s")
        } else {
            format!("~ {cleaned} {unit}")
        }
    }
}