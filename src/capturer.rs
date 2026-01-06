#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
mod not_windows;

use anyhow::Error;
use chrono::{DateTime, Datelike, Local};
use csv::{Reader as CsvReader, Writer as CsvWriter};
use image::io::Reader as ImageReader;
use image::{ImageBuffer, Rgba};
use log::{debug, error, info};
use rusttype::{Font, Scale};
use screenshots::Screen;
use std::fs::OpenOptions;
use std::io::Cursor;
use std::path::Path;
use symlink::symlink_file;

use crate::dir_manager::DirManager;
use crate::FrameRecord;
use crate::DEFAULT_FRAME_DIMENSIONS;

#[cfg(target_os = "windows")]
use windows::get_screenshot;

#[cfg(not(target_os = "windows"))]
use not_windows::get_screenshot;

pub type FrameCounter = u32;

pub struct Capturer {
    sleep_interval: std::time::Duration,
    curr_frame: u32,
    shot_type: String,
}

pub enum ChangeType {
    Nop,
    NewDay,
}

impl Capturer {
    pub fn new(sleep_interval: &std::time::Duration, shot_type: &str) -> Capturer {
        Capturer {
            sleep_interval: sleep_interval.to_owned(),
            curr_frame: 0,
            shot_type: shot_type.to_string(),
        }
    }

    pub fn deal_with_change(
        &mut self,
        dir_manager: &DirManager,
        prev_time: &DateTime<Local>,
        curr_time: &DateTime<Local>,
    ) -> Result<ChangeType, Error> {
        if curr_time.ordinal() != prev_time.ordinal() {
            // Obviously this could be a new month, or even new year. Whatever, we'll be fine either way!
            // The point is it simply not the same day as it was last time we did something.
            Ok(ChangeType::NewDay)
        } else {
            // Same day, so there was just a blackout. nbd.
            self.deal_with_blackout((*curr_time - *prev_time).num_seconds() as u64, dir_manager)?;
            Ok(ChangeType::Nop)
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

    pub fn set_current_frame(&mut self, new_curr_frame: u32) {
        self.curr_frame = new_curr_frame;
    }

    pub fn capture_screen(&self) -> Result<screenshots::Image, anyhow::Error> {
        // At any given point we may not have the same primary screen as we
        // did. For example, we may have switched from an external display to a
        // laptop primary display. So, reacquire the screen with (0, 0) every time.
        get_screenshot(Screen::from_point(0, 0).expect("Couldn't get screen at origin?!"))
    }

    pub fn store(&mut self, capture_result: screenshots::Image, dir: &Path) {
        debug!("Going to store a screenshots::Image");
        let filename = format!("{:05}.{}", self.curr_frame, self.shot_type);
        let filepath = dir.join(filename);

        assert!(!filepath.exists(), "I'm trying to overwrite myself!");

        // We know that the screenshots::Image is a PNG, that's hard-coded.
        // So, it's safe to decode it as such.
        let image_reader = ImageReader::with_format(
            Cursor::new(capture_result.buffer()),
            image::ImageFormat::Png,
        );
        debug!("Made a reader");

        let new_img = image_reader
            .decode()
            .expect("decoding shouldn't be able to fail at this point!");
        debug!("Done decoding it");

        // Get dimensions before saving
        let width = new_img.width();
        let height = new_img.height();

        debug!("Writing out a file to {filepath:?}");
        new_img.save(&filepath).expect("Couldn't save screenshot!");

        // Append frame metadata to CSV
        self.append_metadata(dir, self.curr_frame, width, height);

        self.curr_frame += 1;
    }

    /// Append frame metadata to CSV file
    fn append_metadata(&self, dir: &Path, frame: u32, width: u32, height: u32) {
        let csv_path = dir.join("frame_metadata.csv");
        let needs_header = !csv_path.exists();

        let file = OpenOptions::new().create(true).append(true).open(&csv_path);

        match file {
            Ok(f) => {
                let mut wtr = CsvWriter::from_writer(f);
                if needs_header {
                    if let Err(e) = wtr.write_record(["frame", "width", "height"]) {
                        error!("Failed to write CSV header: {e}");
                        return;
                    }
                }
                // Use write_record instead of serialize to avoid automatic header writing
                if let Err(e) =
                    wtr.write_record([frame.to_string(), width.to_string(), height.to_string()])
                {
                    error!("Failed to write frame metadata: {e}");
                }
            }
            Err(e) => {
                error!("Failed to open metadata CSV: {e}");
            }
        }
    }

    fn deal_with_blackout(
        &mut self,
        elapsed_secs: u64,
        dir_manager: &DirManager,
    ) -> Result<(), Error> {
        info!("Looks like we've been away for a while ({elapsed_secs:?} seconds).");

        let filler_frame_path = dir_manager
            .current_shot_dir()
            .join(format!("{:05}.{}", self.curr_frame, self.shot_type));

        // Get dimensions from current context
        let (width, height) = self.get_current_dimensions(dir_manager);
        debug!("Creating filler frame with dimensions {}x{}", width, height);

        info!("Creating filler frame @ {filler_frame_path:?}");
        Self::create_filler_frame(elapsed_secs, width, height)
            .save(&filler_frame_path)
            .expect("Couldn't create filler frame!");

        let missed_frames = (elapsed_secs / self.sleep_interval.as_secs()) as u32;
        debug!("Going to create {missed_frames:?} frames");
        for n in 1..missed_frames {
            symlink_file(
                &filler_frame_path,
                dir_manager.current_shot_dir().join(format!(
                    "{:05}.{}",
                    self.curr_frame + n,
                    self.shot_type
                )),
            )?;
        }

        debug!("New curr_frame = {:?}", self.curr_frame + missed_frames);
        self.curr_frame += missed_frames;
        Ok(())
    }

    fn get_curr_frame(&self, dir_manager: &mut DirManager) -> std::io::Result<FrameCounter> {
        let dir = dir_manager.current_shot_dir();

        debug!("Examining {dir:?}");
        let count = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == self.shot_type)
            })
            .count() as FrameCounter;

        debug!("Found {count:?} existing {}s", self.shot_type);
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

        // Format duration as human-readable string
        let duration_str = {
            let (value, unit) = if duration_secs >= 3600 {
                (duration_secs as f32 / 3600.0, "hr")
            } else if duration_secs > 60 {
                (duration_secs as f32 / 60.0, "min")
            } else {
                (duration_secs as f32, "sec")
            };
            let plural = if value > 1.0 { "s" } else { "" };
            if unit == "hr" {
                format!("~ {:.1} {}{} go by", value, unit, plural)
            } else {
                format!("~ {:.0} {}{} go by", value, unit, plural)
            }
        };

        let font_data = include_bytes!("Ubuntu-Regular.ttf");
        let font = Font::try_from_bytes(font_data as &[u8]).unwrap();

        // Start with font size as ~20% of height
        let initial_font_size = height as f32 / 5.0;

        // Measure text width at initial size
        let initial_scale = Scale::uniform(initial_font_size);
        let (text_w, _) = imageproc::drawing::text_size(initial_scale, &font, &duration_str);

        // If text is wider than 80% of frame, scale down proportionally
        let max_text_width = width as f32 * 0.8;
        let font_size = if text_w as f32 > max_text_width {
            initial_font_size * (max_text_width / text_w as f32)
        } else {
            initial_font_size
        };

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

        img
    }

    /// Get current dimensions for filler frames.
    /// Checks metadata CSV first, then falls back to default (3420x2224).
    fn get_current_dimensions(&self, dir_manager: &DirManager) -> (u32, u32) {
        let dir = dir_manager.current_shot_dir();
        let csv_path = dir.join("frame_metadata.csv");

        // Try to read from metadata CSV
        if csv_path.exists() {
            if let Some((width, height)) = self.read_last_metadata_line(&csv_path) {
                debug!("Got dimensions from metadata CSV: {}x{}", width, height);
                return (width, height);
            }
        }

        // Fallback: default dimensions
        debug!(
            "Using default dimensions: {}x{}",
            DEFAULT_FRAME_DIMENSIONS.0, DEFAULT_FRAME_DIMENSIONS.1
        );
        DEFAULT_FRAME_DIMENSIONS
    }

    /// Read the last line from the metadata CSV to get most recent dimensions
    fn read_last_metadata_line(&self, csv_path: &Path) -> Option<(u32, u32)> {
        let mut rdr = CsvReader::from_path(csv_path).ok()?;

        rdr.deserialize::<FrameRecord>()
            .filter_map(Result::ok) // Only keep successful deserializations
            .last() // Get the last valid record, returns Option<FrameRecord>
            .map(|r| (r.width, r.height)) // Map to the desired tuple format
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_appends_to_metadata_csv() {
        let temp_dir = tempfile::tempdir().unwrap();
        let csv_path = temp_dir.path().join("frame_metadata.csv");

        // Create a Capturer and call append_metadata directly
        // (since store() requires a screenshots::Image which is hard to mock)
        let capturer = Capturer::new(&std::time::Duration::from_secs(20), "jpeg");

        // Append first frame metadata
        capturer.append_metadata(temp_dir.path(), 0, 1920, 1080);

        assert!(
            csv_path.exists(),
            "frame_metadata.csv should be created by append_metadata()"
        );

        let contents = std::fs::read_to_string(&csv_path).unwrap();
        assert!(
            contents.contains("frame,width,height"),
            "CSV should contain header"
        );
        assert!(
            contents.contains("0,1920,1080"),
            "CSV should contain frame 0 data"
        );

        // Append second frame with different dimensions
        capturer.append_metadata(temp_dir.path(), 1, 2560, 1440);

        let contents = std::fs::read_to_string(&csv_path).unwrap();
        assert!(
            contents.contains("1,2560,1440"),
            "CSV should contain frame 1 data"
        );

        // Header should only appear once
        assert_eq!(
            contents.matches("frame,width,height").count(),
            1,
            "Header should only appear once"
        );
    }

    #[test]
    fn test_get_current_dimensions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let shot_dir = temp_dir.path().to_string_lossy().into_owned();
        let vid_dir = temp_dir.path().join("vids").to_string_lossy().into_owned();

        let mut dir_manager = DirManager::new(&shot_dir, &vid_dir);
        dir_manager.make_shot_output_dir().unwrap();

        let capturer = Capturer::new(&std::time::Duration::from_secs(20), "jpeg");
        let (width, height) = capturer.get_current_dimensions(&dir_manager);

        // Without any frames or metadata, should return DEFAULT_FRAME_DIMENSIONS
        assert_eq!(
            width, DEFAULT_FRAME_DIMENSIONS.0,
            "Default width should be {}",
            DEFAULT_FRAME_DIMENSIONS.0
        );
        assert_eq!(
            height, DEFAULT_FRAME_DIMENSIONS.1,
            "Default height should be {}",
            DEFAULT_FRAME_DIMENSIONS.1
        );
    }
}
