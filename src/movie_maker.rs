use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct MovieMaker {
    output_dir: PathBuf,
    frame_rate: u32,
    file_extension: String,
    output_width: u32,
    output_height: u32,
    ffmpeg: String,
}

impl MovieMaker {
    pub fn new(
        output_dir: &Path,
        file_extension: &str,
        output_width: u32,
        output_height: u32,
        ffmpeg: &str,
    ) -> MovieMaker {
        MovieMaker {
            output_dir: PathBuf::from(output_dir),
            frame_rate: ((9 * 60 * 60) / 20) / 60,
            file_extension: file_extension.to_string(),
            output_width,
            output_height,
            ffmpeg: ffmpeg.to_string(),
        }
    }

    pub fn make_movie_from(&self, input_dir: &Path) {
        self.fix_missing_frames(input_dir);

        let mut ancestors = input_dir.ancestors();
        let day = ancestors
            .next()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let month = ancestors
            .next()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let year = ancestors
            .next()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let out_f = format!("ompd-{}-{}-{}.mkv", year, month, day);

        let mut to_run = Command::new(&self.ffmpeg);
        to_run.args([
            // Frame rate to generate
            "-r",
            &self.frame_rate.to_string(),
            // Where to find input frames and what format to expect
            "-i",
            &input_dir
                .join(format!("%05d.{}", self.file_extension))
                .to_string_lossy(),
            // Output size
            "-s",
            &format!("{}x{}", self.output_width, self.output_height),
            // Pixel format -- maybe only relevant on MacOS?
            "-pix_fmt",
            "yuv420p",
            // Clobber existing files
            "-y",
            // Where to store the output
            &self.output_dir.join(out_f).to_string_lossy(),
        ]);

        debug!("{:?}", to_run);

        let output = to_run.output().expect("Failed to run ffmpeg :(");
        debug!("Finished with: {:?}", output.status);

        if !output.status.success() {
            panic!("Some issue running ffmpeg! Abort abort abort!");
        }
    }

    pub fn fix_missing_frames(&self, in_dir: &Path) {
        let expected_extension = self.file_extension.clone();

        let mut found_frames = Vec::new();

        debug!("Gathering up frames in {in_dir:?}");
        for entry_maybe in fs::read_dir(in_dir).unwrap() {
            // }.into_iter().filter_map(|e| e.ok()) {
            let entry = match entry_maybe {
                Ok(e) => e,
                Err(e) => {
                    warn!("Issue walking directory, trying to continue: {e:?}");
                    continue;
                }
            };

            let extension = match entry.path().extension() {
                Some(e) => e.to_os_string(),
                None => {
                    debug!("No extension on {entry:?} eh? carry on!");
                    continue;
                }
            };

            if extension.to_string_lossy() == expected_extension {
                found_frames.push(entry.path());
            }
        }

        debug!("Sorting, to be safe");
        found_frames.sort_by(|a, b| a.file_name().unwrap().cmp(b.file_name().unwrap()));

        let expected_first_frame = in_dir.join(format!("00000.{expected_extension}"));
        if found_frames[0] != expected_first_frame {
            debug!(
                "Looks like {expected_first_frame:?} was missing, copying earliest into position"
            );
            fs::copy(&found_frames[0], &expected_first_frame)
                .expect("Couldn't create first frame?!");
            found_frames.insert(0, expected_first_frame);
        }

        let mut expected_file: PathBuf;
        let mut prev_file: PathBuf;

        debug!("Checking for any missing frames");
        for expected_index in 0..found_frames.len() {
            expected_file = in_dir.join(format!("{expected_index:05}.{expected_extension}"));

            if !expected_file.exists() {
                prev_file = in_dir.join(format!("{:05}.{expected_extension}", expected_index - 1));
                info!("Missing {expected_file:?}. Copying {prev_file:?} into place");
                fs::copy(&prev_file, &expected_file).expect("Couldn't copy a missing frame?!");
            }
        }
    }
}