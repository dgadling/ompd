use chrono::{Datelike, Local};
use log::{debug, warn};
use std::fs::create_dir_all;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zstd::DEFAULT_COMPRESSION_LEVEL;

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

    pub fn get_current_shot_dir(&self) -> PathBuf {
        self.current_shot_dir.clone()
    }

    pub fn compress(target: &Path) {
        for entry in WalkDir::new(target).into_iter().filter_map(|e| e.ok()) {
            if entry.path_is_symlink() {
                debug!("Found a link {entry:?}, skipping!");
                continue;
            }

            let extension_maybe = entry.path().extension();
            let extension = match extension_maybe {
                Some(e) => e.to_os_string(),
                None => {
                    debug!("No extension on {entry:?} eh? carry on!");
                    continue;
                }
            };

            if extension != "png" {
                debug!("Found non-png {entry:?}, skip!");
                continue;
            }

            let compressed = Self::actually_compress(entry.path());
            match compressed {
                Err(e) => {
                    warn!("Some issue with {entry:?}: {e:?}");
                }
                Ok(_) => {
                    debug!("Compressed!");
                }
            }
        }
    }

    fn actually_compress(entry: &Path) -> Result<(), anyhow::Error> {
        let mut new_file_name = entry.as_os_str().to_owned();
        new_file_name.push(".zst");

        let orig_file = std::fs::File::open(entry)?;
        let reader = BufReader::new(&orig_file);

        let compressed_file = std::fs::File::create(&new_file_name)?;
        let writer = BufWriter::new(&compressed_file);

        zstd::stream::copy_encode(reader, writer, DEFAULT_COMPRESSION_LEVEL)?;

        Ok(())
    }

    fn get_current_shot_dir_in(root_dir: &Path) -> PathBuf {
        let now = Local::now();

        root_dir
            .join(now.year().to_string())
            .join(format!("{:02}", now.month()))
            .join(format!("{:02}", now.day()))
    }
}
