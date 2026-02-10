use anyhow::Error;
use chrono::{Datelike, Local};
use csv::{Reader, Writer};
use log::{error, info, warn};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

use crate::{FrameMetadata, FrameRecord};

pub struct DirManager {
    current_shot_dir: PathBuf,
    shot_dir: PathBuf,
}

impl DirManager {
    pub fn new(shot_dir: &String, vid_dir: &String) -> DirManager {
        let shot_dir = PathBuf::from(shot_dir);
        let vid_dir = PathBuf::from(vid_dir);

        create_dir_all(&shot_dir).expect("Couldn't create directory for shots!");
        create_dir_all(vid_dir).expect("Couldn't create directory for videos!");

        DirManager {
            current_shot_dir: Self::get_current_shot_dir_in(&shot_dir),
            shot_dir,
        }
    }

    pub fn make_shot_output_dir(&mut self) -> std::io::Result<&Path> {
        self.current_shot_dir = Self::get_current_shot_dir_in(&self.shot_dir);

        create_dir_all(&self.current_shot_dir).expect("Couldn't create output directory!");
        Ok(self.current_shot_dir.as_path())
    }

    pub fn current_shot_dir(&self) -> &Path {
        self.current_shot_dir.as_path()
    }

    fn get_current_shot_dir_in(root_dir: &Path) -> PathBuf {
        let now = Local::now();
        Self::shot_dir_for_date(
            root_dir,
            now.year() as u16,
            now.month() as u8,
            now.day() as u8,
        )
    }

    /// Build a shot directory path for a given year/month/day
    pub fn shot_dir_for_date(root_dir: &Path, year: u16, month: u8, day: u8) -> PathBuf {
        root_dir
            .join(year.to_string())
            .join(format!("{:02}", month))
            .join(format!("{:02}", day))
    }

    /// Parse year/month/day from a shot directory path like /root/2024/01/15
    /// Returns None if the path doesn't have enough components or they can't be parsed
    pub fn parse_date_from_shot_dir(path: &Path) -> Option<(u16, u8, u8)> {
        let mut components = path.components().rev();
        let day: u8 = components.next()?.as_os_str().to_str()?.parse().ok()?;
        let month: u8 = components.next()?.as_os_str().to_str()?.parse().ok()?;
        let year: u16 = components.next()?.as_os_str().to_str()?.parse().ok()?;
        Some((year, month, day))
    }

    /// Generate metadata CSV from image files in a directory
    pub fn generate_metadata(in_dir: &Path, extension: &str) -> Result<FrameMetadata, Error> {
        let mut frames = Vec::new();

        // Find all image files matching the extension
        let mut image_files: Vec<_> = std::fs::read_dir(in_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == extension))
            .filter(|e| !e.file_type().map_or(true, |ft| ft.is_symlink()))
            .collect();

        image_files.sort_by_key(|a| a.file_name());

        if image_files.is_empty() {
            return Err(anyhow::anyhow!(
                "No frames found in {} with extension .{}",
                in_dir.display(),
                extension
            ));
        }

        info!("Generating metadata from {} frames", image_files.len());

        for entry in image_files {
            let path = entry.path();
            if let Some(stem) = path.file_stem() {
                if let Ok(frame_num) = stem.to_string_lossy().parse::<u32>() {
                    match image::image_dimensions(&path) {
                        Ok((width, height)) => {
                            frames.push((frame_num, width, height));
                        }
                        Err(e) => {
                            warn!("Failed to read dimensions from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        // Write to CSV using csv crate
        let csv_path = in_dir.join("frame_metadata.csv");
        let mut wtr = Writer::from_path(&csv_path)?;
        for (frame, width, height) in &frames {
            wtr.serialize(FrameRecord {
                frame: *frame,
                width: *width,
                height: *height,
            })?;
        }
        wtr.flush()?;

        let min_w = frames.iter().map(|(_, w, _)| *w).min().unwrap_or(0);
        let min_h = frames.iter().map(|(_, _, h)| *h).min().unwrap_or(0);
        let max_w = frames.iter().map(|(_, w, _)| *w).max().unwrap_or(0);
        let max_h = frames.iter().map(|(_, _, h)| *h).max().unwrap_or(0);

        if min_w < 860 || min_h < 360 {
            error!(
                "Unusually small frame dimensions detected: {}x{}",
                min_w, min_h
            );
        }

        info!(
            "Detected dimensions range: {}x{} to {}x{}",
            min_w, min_h, max_w, max_h
        );

        Ok(FrameMetadata { frames })
    }

    /// Read metadata from an existing CSV file
    pub fn read_metadata_from_csv(csv_path: &Path) -> Result<FrameMetadata, Error> {
        let mut rdr = Reader::from_path(csv_path)?;
        let mut frames = Vec::new();

        for result in rdr.deserialize() {
            let record: FrameRecord = result?;
            frames.push((record.frame, record.width, record.height));
        }

        Ok(FrameMetadata { frames })
    }

    /// Get metadata from CSV file or generate it if missing
    pub fn get_or_generate_metadata(
        in_dir: &Path,
        extension: &str,
    ) -> Result<FrameMetadata, Error> {
        let csv_path = in_dir.join("frame_metadata.csv");

        // Try to read from CSV
        if csv_path.exists() {
            return Self::read_metadata_from_csv(&csv_path);
        }

        // Generate if missing
        Self::generate_metadata(in_dir, extension)
    }
}
