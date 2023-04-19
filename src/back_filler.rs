use crate::config::Config;
use crate::movie_maker::MovieMaker;

use anyhow::Error;
use chrono::{DateTime, Datelike, Local};
use glob::glob;
use log::{info, warn};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
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
        root_dir
            .join(format!("{}", self.year))
            .join(format!("{:02}", self.month))
            .join(format!("{:02}", self.day))
    }
}

impl BackFiller {
    pub fn new(config: Config, today: DateTime<Local>) -> BackFiller {
        //shots_root_dir: &Path, vids_root_dir: &Path, ffmpeg: &str, vid_width: u32, vid_height: u32) -> BackFiller {
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
            info!("Launching movie maker for {dir:?}");
            m.make_movie_from(&dir.to_shot_dir_in(&root_shot_dir));
        }

        info!("Done backfilling movies");
    }

    fn discover_vids(&self) -> Result<HashSet<Discovered>, Error> {
        let mut discovered = HashSet::new();

        let video_glob = PathBuf::from(&self.config.vid_output_dir).join("ompd-*-*-*.mkv");
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

            // NOTE: We reverse the path to make life easier since Rust doesn't like negative indexing.
            let dir_parts: Vec<Component> = entry.components().rev().collect();

            let day = dir_parts[0].as_os_str().to_str().unwrap();
            let month = dir_parts[1].as_os_str().to_str().unwrap();
            let year = dir_parts[2].as_os_str().to_str().unwrap();

            discovered.insert(Discovered {
                year: year.parse::<u16>().unwrap(),
                month: month.parse::<u8>().unwrap(),
                day: day.parse::<u8>().unwrap(),
            });
        }

        Ok(discovered)
    }
}
