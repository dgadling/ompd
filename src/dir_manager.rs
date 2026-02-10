use anyhow::Error;
use chrono::{Datelike, Local, NaiveDate};
use csv::{Reader, Writer};
use glob::glob;
use log::{debug, error, info, warn};
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

    /// Remove old shot directories that have already been converted to videos.
    ///
    /// Discovers all shot directories under `shot_root`, excludes `today`, sorts
    /// by date descending, keeps the most recent `keep_count` entries, and deletes
    /// the rest if a corresponding video file exists with non-zero size.
    pub fn cleanup_old_shot_dirs(
        shot_root: &Path,
        vid_dir: &Path,
        video_type: &str,
        keep_count: u32,
        today: NaiveDate,
    ) {
        info!(
            "Checking for old shot dirs to clean up (keeping {} days)",
            keep_count
        );

        let shot_glob = shot_root
            .join("[0-9][0-9][0-9][0-9]")
            .join("[0-1][0-9]")
            .join("[0-3][0-9]");

        let ok_matches = match glob(shot_glob.to_str().unwrap()) {
            Ok(paths) => paths.filter_map(Result::ok),
            Err(e) => {
                warn!("Failed to glob shot directories: {e}");
                return;
            }
        };

        let mut dated_dirs: Vec<(NaiveDate, PathBuf)> = Vec::new();

        for entry in ok_matches {
            if !entry.is_dir() {
                continue;
            }

            if let Some((year, month, day)) = Self::parse_date_from_shot_dir(&entry) {
                if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32) {
                    if date == today {
                        continue;
                    }
                    dated_dirs.push((date, entry));
                }
            }
        }

        // Sort descending (most recent first)
        dated_dirs.sort_by(|a, b| b.0.cmp(&a.0));

        // Skip the first keep_count entries
        let to_delete = dated_dirs.into_iter().skip(keep_count as usize);

        for (date, shot_dir) in to_delete {
            let video_file = vid_dir.join(format!(
                "ompd-{}-{:02}-{:02}.{}",
                date.year(),
                date.month(),
                date.day(),
                video_type
            ));

            match std::fs::metadata(&video_file) {
                Ok(meta) if meta.len() > 0 => {
                    info!(
                        "Cleaning up shot dir {} (video exists at {})",
                        shot_dir.display(),
                        video_file.display()
                    );
                    Self::cleanup_shot_dir(&shot_dir, shot_root);
                }
                Ok(_) => {
                    debug!("Skipping {} — video file is empty", shot_dir.display());
                }
                Err(_) => {
                    debug!(
                        "Skipping {} — no video at {}",
                        shot_dir.display(),
                        video_file.display()
                    );
                }
            }
        }
    }

    /// Remove a shot directory and clean up empty parent directories up to shot_root.
    fn cleanup_shot_dir(shot_dir: &Path, shot_root: &Path) {
        if let Err(e) = std::fs::remove_dir_all(shot_dir) {
            warn!("Failed to remove shot dir {}: {e}", shot_dir.display());
            return;
        }

        // Walk parents up to (but not including) shot_root, removing empty dirs
        let mut current = shot_dir.parent();
        while let Some(parent) = current {
            if parent == shot_root {
                break;
            }
            // remove_dir only succeeds if the directory is empty
            if std::fs::remove_dir(parent).is_err() {
                break;
            }
            current = parent.parent();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    /// Helper to create a shot directory structure YYYY/MM/DD under root
    fn create_shot_dir(root: &Path, year: u16, month: u8, day: u8) -> PathBuf {
        let dir = DirManager::shot_dir_for_date(root, year, month, day);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Helper to create a video file with non-zero content
    fn create_video(vid_dir: &Path, year: u16, month: u8, day: u8, ext: &str) {
        let path = vid_dir.join(format!("ompd-{year}-{month:02}-{day:02}.{ext}"));
        let mut f = File::create(path).unwrap();
        f.write_all(b"fake video data").unwrap();
    }

    #[test]
    fn test_cleanup_shot_dir_removes_dir_and_empty_parents() {
        let temp = tempfile::tempdir().unwrap();
        let shot_root = temp.path().join("shots");
        let shot_dir = create_shot_dir(&shot_root, 2024, 3, 15);

        // Put a file in the shot dir
        File::create(shot_dir.join("00000.jpeg")).unwrap();

        DirManager::cleanup_shot_dir(&shot_dir, &shot_root);

        assert!(!shot_dir.exists(), "Shot dir should be removed");
        // Month dir (03) should be removed since it's now empty
        assert!(
            !shot_root.join("2024").join("03").exists(),
            "Empty month dir should be removed"
        );
        // Year dir (2024) should be removed since it's now empty
        assert!(
            !shot_root.join("2024").exists(),
            "Empty year dir should be removed"
        );
        // shot_root itself should still exist
        assert!(shot_root.exists(), "Shot root should not be removed");
    }

    #[test]
    fn test_cleanup_shot_dir_preserves_nonempty_parents() {
        let temp = tempfile::tempdir().unwrap();
        let shot_root = temp.path().join("shots");

        // Create two days in the same month
        let dir_15 = create_shot_dir(&shot_root, 2024, 3, 15);
        let dir_16 = create_shot_dir(&shot_root, 2024, 3, 16);

        File::create(dir_15.join("00000.jpeg")).unwrap();
        File::create(dir_16.join("00000.jpeg")).unwrap();

        // Only remove day 15
        DirManager::cleanup_shot_dir(&dir_15, &shot_root);

        assert!(!dir_15.exists(), "Shot dir 15 should be removed");
        assert!(dir_16.exists(), "Shot dir 16 should still exist");
        assert!(
            shot_root.join("2024").join("03").exists(),
            "Month dir should still exist (has day 16)"
        );
    }

    #[test]
    fn test_cleanup_old_shot_dirs_keeps_n_most_recent() {
        let temp = tempfile::tempdir().unwrap();
        let shot_root = temp.path().join("shots");
        let vid_dir = temp.path().join("vids");
        std::fs::create_dir_all(&vid_dir).unwrap();

        let today = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();

        // Create 5 shot dirs (not including today)
        for day in 15..=19 {
            create_shot_dir(&shot_root, 2024, 3, day);
            create_video(&vid_dir, 2024, 3, day, "mp4");
        }

        // Keep 2 most recent
        DirManager::cleanup_old_shot_dirs(&shot_root, &vid_dir, "mp4", 2, today);

        // Days 18 and 19 should remain (most recent 2)
        assert!(
            DirManager::shot_dir_for_date(&shot_root, 2024, 3, 19).exists(),
            "Day 19 should be kept"
        );
        assert!(
            DirManager::shot_dir_for_date(&shot_root, 2024, 3, 18).exists(),
            "Day 18 should be kept"
        );
        // Days 15, 16, 17 should be deleted
        assert!(
            !DirManager::shot_dir_for_date(&shot_root, 2024, 3, 15).exists(),
            "Day 15 should be deleted"
        );
        assert!(
            !DirManager::shot_dir_for_date(&shot_root, 2024, 3, 16).exists(),
            "Day 16 should be deleted"
        );
        assert!(
            !DirManager::shot_dir_for_date(&shot_root, 2024, 3, 17).exists(),
            "Day 17 should be deleted"
        );
    }

    #[test]
    fn test_cleanup_old_shot_dirs_skips_dirs_without_video() {
        let temp = tempfile::tempdir().unwrap();
        let shot_root = temp.path().join("shots");
        let vid_dir = temp.path().join("vids");
        std::fs::create_dir_all(&vid_dir).unwrap();

        let today = NaiveDate::from_ymd_opt(2024, 3, 20).unwrap();

        // Create 3 shot dirs, but only 1 has a video
        create_shot_dir(&shot_root, 2024, 3, 15);
        create_shot_dir(&shot_root, 2024, 3, 16);
        create_video(&vid_dir, 2024, 3, 16, "mp4");
        create_shot_dir(&shot_root, 2024, 3, 17);

        // Keep 0 (would delete everything old)
        DirManager::cleanup_old_shot_dirs(&shot_root, &vid_dir, "mp4", 0, today);

        // Day 16 should be deleted (has video)
        assert!(
            !DirManager::shot_dir_for_date(&shot_root, 2024, 3, 16).exists(),
            "Day 16 should be deleted (has video)"
        );
        // Days 15, 17 should still exist (no video)
        assert!(
            DirManager::shot_dir_for_date(&shot_root, 2024, 3, 15).exists(),
            "Day 15 should remain (no video)"
        );
        assert!(
            DirManager::shot_dir_for_date(&shot_root, 2024, 3, 17).exists(),
            "Day 17 should remain (no video)"
        );
    }

    #[test]
    fn test_cleanup_old_shot_dirs_never_deletes_today() {
        let temp = tempfile::tempdir().unwrap();
        let shot_root = temp.path().join("shots");
        let vid_dir = temp.path().join("vids");
        std::fs::create_dir_all(&vid_dir).unwrap();

        let today = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();

        create_shot_dir(&shot_root, 2024, 3, 15);
        create_video(&vid_dir, 2024, 3, 15, "mp4");

        // Keep 0, but today should still survive
        DirManager::cleanup_old_shot_dirs(&shot_root, &vid_dir, "mp4", 0, today);

        assert!(
            DirManager::shot_dir_for_date(&shot_root, 2024, 3, 15).exists(),
            "Today's directory should never be deleted"
        );
    }
}
