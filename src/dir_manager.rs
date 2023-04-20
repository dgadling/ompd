use chrono::{Datelike, Local};
use log::{debug, warn};
use std::fs::{create_dir_all, read_dir, remove_file};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use zstd::DEFAULT_COMPRESSION_LEVEL;

const COMPRESSED_FILE_EXTENSION: &str = "zst";

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

    pub fn decompress(target: &Path) {
        debug!("Going to iterate_and_operate({target:?}, {COMPRESSED_FILE_EXTENSION}, Self::actually_decompress)");

        Self::iterate_and_operate(target, COMPRESSED_FILE_EXTENSION, Self::actually_decompress)
    }

    pub fn compress(target: &Path, target_extension: &str) {
        Self::iterate_and_operate(target, target_extension, Self::actually_compress)
    }

    fn iterate_and_operate(
        target: &Path,
        target_extension: &str,
        op: fn(&Path) -> Result<(), anyhow::Error>,
    ) {
        for entry_maybe in read_dir(target).unwrap() {
            let entry = match entry_maybe {
                Ok(e) => e,
                Err(e) => {
                    debug!("{e:?}");
                    continue;
                }
            };

            if entry.file_type().unwrap().is_symlink() {
                continue;
            }

            let entry_path = entry.path();
            let extension_maybe = entry_path.extension();
            let extension = match extension_maybe {
                Some(e) => e.to_os_string(),
                None => {
                    debug!("No extension on {entry:?} eh? carry on!");
                    continue;
                }
            };

            if extension != target_extension {
                continue;
            }

            let done = op(entry_path.as_path());
            if let Err(e) = done {
                warn!("Some issue with {entry:?}: {e:?}");
            };
        }
    }

    fn actually_compress(entry: &Path) -> Result<(), anyhow::Error> {
        let mut new_file_name = entry.as_os_str().to_owned();
        new_file_name.push(".");
        new_file_name.push(COMPRESSED_FILE_EXTENSION);

        {
            let orig_file = std::fs::File::open(entry)?;
            let reader = BufReader::new(&orig_file);

            let compressed_file = std::fs::File::create(&new_file_name)?;
            let writer = BufWriter::new(&compressed_file);

            zstd::stream::copy_encode(reader, writer, DEFAULT_COMPRESSION_LEVEL)?;
        }

        remove_file(entry)?;

        Ok(())
    }

    fn actually_decompress(entry: &Path) -> Result<(), anyhow::Error> {
        let new_file_name = entry
            .as_os_str()
            .to_os_string()
            .into_string()
            .unwrap()
            .replace(COMPRESSED_FILE_EXTENSION, "");

        {
            let orig_file = std::fs::File::open(entry)?;
            let reader = BufReader::new(&orig_file);

            let compressed_file = std::fs::File::create(new_file_name)?;
            let writer = BufWriter::new(&compressed_file);

            zstd::stream::copy_decode(reader, writer)?;
        }

        remove_file(entry)?;

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
