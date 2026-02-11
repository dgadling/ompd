use crate::config::Config;
use crate::dir_manager::DirManager;
use crate::movie_maker::MovieMaker;

use anyhow::Error;
use chrono::{DateTime, Datelike, Local, NaiveDate};
use glob::glob;
use log::{info, warn};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::result::Result;

pub struct BackFiller {
    config: Config,
    today: NaiveDate,
}

impl BackFiller {
    pub fn new(config: Config, today: DateTime<Local>) -> BackFiller {
        BackFiller {
            config,
            today: NaiveDate::from_ymd_opt(today.year(), today.month(), today.day())
                .expect("Invalid date from DateTime<Local>"),
        }
    }

    pub fn run(&self) {
        let mut vid_coverage = match self.discover_vids() {
            Ok(r) => r,
            Err(e) => {
                warn!("Couldn't discover videos, giving up!: {e}");
                return;
            }
        };

        // Throw in today's video so that when we find the directory below we don't try to start the video process early
        vid_coverage.insert(self.today);

        let shot_coverage = match self.discover_shots() {
            Ok(r) => r,
            Err(e) => {
                warn!("Couldn't discover videos, giving up!: {e}");
                return;
            }
        };

        let to_process = shot_coverage.difference(&vid_coverage);

        let m = MovieMaker::new(self.config.clone());

        let root_shot_dir = PathBuf::from(&self.config.shot_output_dir);
        for date in to_process {
            let shot_dir = DirManager::shot_dir_for_date(
                &root_shot_dir,
                date.year() as u16,
                date.month() as u8,
                date.day() as u8,
            );
            info!("Launching movie maker for {date}");

            // Generate metadata for old directories that may not have it
            let metadata_csv = shot_dir.join("frame_metadata.csv");
            if !metadata_csv.exists() {
                info!("Generating missing metadata for {}", shot_dir.display());
                if let Err(e) = DirManager::generate_metadata(&shot_dir, &self.config.shot_type) {
                    warn!(
                        "Failed to generate metadata for {}: {e}",
                        shot_dir.display()
                    );
                }
            }

            m.make_movie_from(&shot_dir);
        }

        info!("Done backfilling movies");

        if let Some(keep_count) = self.config.keep_shots_days {
            DirManager::cleanup_old_shot_dirs(
                Path::new(&self.config.shot_output_dir),
                Path::new(&self.config.vid_output_dir),
                &self.config.video_type,
                keep_count,
                self.today,
            );
        }
    }

    fn discover_vids(&self) -> Result<HashSet<NaiveDate>, Error> {
        let mut discovered = HashSet::new();

        let video_glob = PathBuf::from(&self.config.vid_output_dir).join(format!(
            "ompd-[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9].{}",
            self.config.video_type
        ));
        let ok_matches = glob(video_glob.to_str().unwrap())
            .unwrap()
            .filter_map(Result::ok);

        for entry in ok_matches {
            if !entry.is_file() {
                info!("Found {entry:?} which apparently isn't a file, skipping");
                continue;
            }

            let file_name = entry.file_stem().unwrap().to_string_lossy();
            let file_parts: Vec<&str> = file_name.split('-').collect();

            // Remember that the first bit is "ompd"
            if let Some(date) = NaiveDate::from_ymd_opt(
                file_parts[1].parse::<i32>().unwrap(),
                file_parts[2].parse::<u32>().unwrap(),
                file_parts[3].parse::<u32>().unwrap(),
            ) {
                discovered.insert(date);
            }
        }

        Ok(discovered)
    }

    fn discover_shots(&self) -> Result<HashSet<NaiveDate>, Error> {
        let mut discovered = HashSet::new();

        let shot_glob = PathBuf::from(&self.config.shot_output_dir)
            .join("[0-9][0-9][0-9][0-9]")
            .join("[0-1][0-9]")
            .join("[0-3][0-9]");

        let ok_matches = glob(shot_glob.to_str().unwrap())
            .unwrap()
            .filter_map(Result::ok);

        for entry in ok_matches {
            if !entry.is_dir() {
                info!("Found {entry:?} which apparently isn't a directory, skipping");
                continue;
            }

            if let Some((year, month, day)) = DirManager::parse_date_from_shot_dir(&entry) {
                if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32) {
                    discovered.insert(date);
                }
            }
        }

        Ok(discovered)
    }
}
