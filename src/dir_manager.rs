use chrono::{Datelike, Local};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

pub struct DirManager {
    current_dir: PathBuf,
    root_dir: PathBuf,
}

impl DirManager {
    pub fn new(root_dir: &String) -> DirManager {
        let root_path = PathBuf::from(root_dir);

        DirManager {
            current_dir: Self::get_current_dir_in(&root_path),
            root_dir: root_path,
        }
    }

    pub fn make_output_dir(&mut self) -> std::io::Result<&Path> {
        self.current_dir = Self::get_current_dir_in(&self.root_dir);

        create_dir_all(&self.current_dir).expect("Couldn't create output directory!");
        Ok(self.current_dir.as_path())
    }

    pub fn current_dir(&self) -> &Path {
        self.current_dir.as_path()
    }

    fn get_current_dir_in(root_dir: &Path) -> PathBuf {
        let now = Local::now();

        root_dir
            .join(now.year().to_string())
            .join(format!("{:02}", now.month()))
            .join(format!("{:02}", now.day()))
    }
}
