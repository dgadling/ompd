use crate::config::Config;
use crate::dir_manager::DirManager;
use crate::movie_maker::MovieMaker;

use anyhow::Error;
use chrono::{DateTime, Datelike, Local};
use glob::glob;
use log::{info, warn};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::result::Result;

pub struct BackFiller {
    config: Config,
    today: Discovered,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Discovered {
    year: u16,
    month: u8,
    day: u8,
}

impl Discovered {
    fn to_shot_dir_in(&self, root_dir: &Path) -> PathBuf {
        DirManager::shot_dir_for_date(root_dir, self.year, self.month, self.day)
    }
}

impl fmt::Display for Discovered {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl BackFiller {
    pub fn new(config: Config, today: DateTime<Local>) -> BackFiller {
        BackFiller {
            config,
            today: Discovered {
                year: today.year() as u16,
                month: today.month() as u8,
                day: today.day() as u8,
            },
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
        vid_coverage.insert(self.today.clone());

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
        for dir in to_process {
            let shot_dir = dir.to_shot_dir_in(&root_shot_dir);
            info!("Launching movie maker for {dir}");

            // Generate metadata for old directories that may not have it
            let metadata_csv = shot_dir.join("frame_metadata.csv");
            if !metadata_csv.exists() {
                DirManager::decompress(&shot_dir);
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
    }

    fn discover_vids(&self) -> Result<HashSet<Discovered>, Error> {
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

            discovered.insert(Discovered {
                // Remember that the first bit is "ompd"
                year: file_parts[1].parse::<u16>().unwrap(),
                month: file_parts[2].parse::<u8>().unwrap(),
                day: file_parts[3].parse::<u8>().unwrap(),
            });
        }

        Ok(discovered)
    }

    fn discover_shots(&self) -> Result<HashSet<Discovered>, Error> {
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
                discovered.insert(Discovered { year, month, day });
            }
        }

        Ok(discovered)
    }
}
