use chrono::{Datelike, Local};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

pub struct DirManager {
    current_shot_dir: PathBuf,
    shot_dir: PathBuf,
    vid_dir: PathBuf,
}

impl DirManager {
    pub fn new(shot_dir: &String, vid_dir: &String) -> DirManager {
        let shot_dir = PathBuf::from(shot_dir);
        let vid_dir = PathBuf::from(vid_dir);

        create_dir_all(&shot_dir).expect("Couldn't create directory for shots!");
        create_dir_all(&vid_dir).expect("Couldn't create directory for videos!");

        DirManager {
            current_shot_dir: Self::get_current_shot_dir_in(&shot_dir),
            shot_dir,
            vid_dir,
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

    pub fn get_current_shot_dir(&self) -> PathBuf {
        self.current_shot_dir.clone()
    }

    pub fn get_vid_output_dir(&self) -> PathBuf {
        self.vid_dir.clone()
    }

    fn get_current_shot_dir_in(root_dir: &Path) -> PathBuf {
        let now = Local::now();

        root_dir
            .join(now.year().to_string())
            .join(format!("{:02}", now.month()))
            .join(format!("{:02}", now.day()))
    }
}
