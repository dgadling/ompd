use crate::Config;
use crate::DirManager;
use crate::FrameMetadata;
use crate::DEFAULT_FRAME_DIMENSIONS;
use anyhow::Error;
use chrono::{Datelike, Local, NaiveDate};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Frame rate for generated videos: ~27 fps yields a ~1 minute video from 9 hours of 20-second captures
const FRAME_RATE: u32 = ((9 * 60 * 60) / 20) / 60;

pub struct MovieMaker {
    config: Config,
}

impl MovieMaker {
    pub fn new(config: Config) -> MovieMaker {
        MovieMaker { config }
    }

    pub fn has_muxer(ffmpeg: &str, extension: &str) -> Result<bool, Error> {
        debug!("Asking {} for its muxers", ffmpeg);

        let output = Command::new(ffmpeg)
            .arg("-muxers")
            .output()
            .unwrap_or_else(|_| panic!("Couldn't ask {} for muxers!", ffmpeg));

        let stdout_raw = String::from_utf8_lossy(&output.stdout);
        let needle = format!(" {extension}");

        if stdout_raw.lines().any(|line| line.contains(&needle)) {
            return Ok(true);
        }

        Err(anyhow::anyhow!(
            "Invalid video type, ffmpeg doesn't know how to make '{extension}' files"
        ))
    }

    pub fn make_movie_from(&self, input_dir: &Path) {
        self.fix_missing_frames(input_dir);

        // Analyze frames and determine target dimensions
        let metadata = match DirManager::get_or_generate_metadata(input_dir, &self.config.shot_type)
        {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to get metadata, using default dimensions: {}", e);
                FrameMetadata { frames: vec![] }
            }
        };
        let (target_width, target_height) = self.analyze_frame_dimensions(&metadata);

        let (year, month, day) = DirManager::parse_date_from_shot_dir(input_dir)
            .expect("input_dir should be a valid shot directory path");

        let out_f = format!(
            "ompd-{}-{:02}-{:02}.{}",
            year, month, day, &self.config.video_type
        );
        let output_file = PathBuf::from(&self.config.vid_output_dir).join(out_f);

        let args = self.build_ffmpeg_args(input_dir, &output_file, target_width, target_height);
        let mut to_run = Command::new(&self.config.ffmpeg);
        to_run.args(&args);

        debug!("{:?}", to_run);

        let output = to_run.output().expect("Failed to run ffmpeg :(");
        debug!("Finished with: {:?}", output.status);

        let stdout_raw = String::from_utf8_lossy(&output.stdout);
        let stderr_raw = String::from_utf8_lossy(&output.stderr);

        // Log ffmpeg output no matter what
        if let Err(e) = fs::write(input_dir.join("ffmpeg-stdout.log"), stdout_raw.as_bytes()) {
            warn!("Couldn't write ffmpeg stdout to file: {e}");
        }

        if let Err(e) = fs::write(input_dir.join("ffmpeg-stderr.log"), stderr_raw.as_bytes()) {
            warn!("Couldn't write ffmpeg stderr to file: {e}");
        }

        if !output.status.success() {
            let last_line = stderr_raw.lines().last().unwrap_or("(no stderr)");
            let err = format!("Issue with ffmpeg - last line of stderr: {}", last_line);
            error!("{}", &err);
            panic!("{}", &err);
        }

        if let Some(keep_count) = self.config.keep_shots_days {
            let today = Local::now();
            let today_date = NaiveDate::from_ymd_opt(today.year(), today.month(), today.day())
                .expect("Invalid date from Local::now()");

            DirManager::cleanup_old_shot_dirs(
                Path::new(&self.config.shot_output_dir),
                Path::new(&self.config.vid_output_dir),
                &self.config.video_type,
                keep_count,
                today_date,
            );
        }

        info!("All done with {input_dir:?}!");
    }

    fn fix_missing_frames(&self, in_dir: &Path) {
        let expected_extension = self.config.shot_type.as_str();

        let mut found_frames = Vec::new();

        debug!("Gathering up frames in {in_dir:?}");
        for entry_maybe in fs::read_dir(in_dir).unwrap() {
            let entry = match entry_maybe {
                Ok(e) => e,
                Err(e) => {
                    warn!("Issue walking directory, trying to continue: {e:?}");
                    continue;
                }
            };

            if entry.file_type().unwrap().is_symlink() {
                continue;
            }

            let extension = match entry.path().extension() {
                Some(e) => e.to_os_string(),
                None => {
                    debug!("No extension on {entry:?} eh? carry on!");
                    continue;
                }
            };

            if extension == expected_extension {
                found_frames.push(entry.path());
            }
        }

        if found_frames.is_empty() {
            panic!("Uhh, no frames AT ALL in {in_dir:?}?!");
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

    /// Analyze frame dimensions and return target dimensions for video
    /// Returns (width, height) - the most common resolution, scaled and rounded to even numbers
    /// This minimizes the number of frames that need letterboxing/pillarboxing
    pub fn analyze_frame_dimensions(&self, metadata: &FrameMetadata) -> (u32, u32) {
        // Count occurrences of each (width, height) pair
        let mut resolution_counts: HashMap<(u32, u32), usize> = HashMap::new();
        for (_, w, h) in &metadata.frames {
            *resolution_counts.entry((*w, *h)).or_insert(0) += 1;
        }

        // Find the most common resolution
        let total_frames = metadata.frames.len();
        let (most_common_width, most_common_height) = resolution_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|((w, h), count)| {
                let percentage = (count as f64 / total_frames as f64) * 100.0;
                info!(
                    "Most common resolution: {}x{} ({:.1}% of frames)",
                    w, h, percentage
                );
                (w, h)
            })
            .unwrap_or(DEFAULT_FRAME_DIMENSIONS); // Default if no frames

        // Apply scale factor
        let scaled_width = (most_common_width as f32 * self.config.vid_scale_factor) as u32;
        let scaled_height = (most_common_height as f32 * self.config.vid_scale_factor) as u32;

        // Round to nearest even number (required for video encoding)
        let final_width = (scaled_width + 1) & !1;
        let final_height = (scaled_height + 1) & !1;

        info!(
            "Target video dimensions: {}x{} (scale factor: {})",
            final_width, final_height, self.config.vid_scale_factor
        );

        (final_width, final_height)
    }

    /// Build the ffmpeg command arguments for video generation
    /// Uses scale and pad filters to handle mixed resolution input
    pub fn build_ffmpeg_args(
        &self,
        input_dir: &Path,
        output_file: &Path,
        target_width: u32,
        target_height: u32,
    ) -> Vec<String> {
        // Use scale and pad filter for mixed resolution handling
        let filter = format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black",
            target_width, target_height, target_width, target_height
        );

        vec![
            "-r".to_string(),
            FRAME_RATE.to_string(),
            "-i".to_string(),
            input_dir
                .join(format!("%05d.{}", self.config.shot_type))
                .to_string_lossy()
                .to_string(),
            "-vf".to_string(),
            filter,
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-y".to_string(),
            output_file.to_string_lossy().to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_with_scale(scale_factor: f32) -> crate::Config {
        let mut config = crate::Config::for_testing();
        config.vid_scale_factor = scale_factor;
        config
    }

    fn test_metadata() -> FrameMetadata {
        // 1920x1080 is most common (2 frames), others have 1 each
        FrameMetadata {
            frames: vec![
                (0, 1920, 1080),
                (1, 1920, 1080),
                (2, 2560, 1440),
                (3, 3420, 2224),
            ],
        }
    }

    #[test]
    fn test_analyze_dimensions_finds_most_common() {
        let maker = MovieMaker::new(test_config_with_scale(1.0));
        let metadata = test_metadata();

        let (width, height) = maker.analyze_frame_dimensions(&metadata);

        // 1920x1080 is the most common resolution (2 frames)
        assert_eq!(width, 1920, "Should return most common width");
        assert_eq!(height, 1080, "Should return most common height");
    }

    #[test]
    fn test_analyze_dimensions_applies_scale_and_rounds_even() {
        let maker = MovieMaker::new(test_config_with_scale(0.5));
        let metadata = test_metadata();

        let (width, height) = maker.analyze_frame_dimensions(&metadata);

        // Most common is 1920x1080, scaled by 0.5 = 960x540
        assert_eq!(width, 960, "Should apply scale factor to width");
        assert_eq!(height, 540, "Should apply scale factor to height");
        assert_eq!(width % 2, 0, "Width should be even");
        assert_eq!(height % 2, 0, "Height should be even");
    }

    // NOTE: test_generate_metadata_creates_csv is in integration tests (requires fixtures)

    #[test]
    fn test_ffmpeg_uses_scale_pad_filter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let maker = MovieMaker::new(test_config_with_scale(1.0));

        let input_dir = temp_dir.path().join("input");
        let output_file = temp_dir.path().join("output.mp4");
        let args = maker.build_ffmpeg_args(&input_dir, &output_file, 1920, 1080);
        let args_str = args.join(" ");

        assert!(
            args_str.contains("-vf"),
            "Should use -vf filter, not -s. Got: {}",
            args_str
        );
        assert!(
            args_str.contains("scale=1920:1080"),
            "Should include scale filter with dimensions. Got: {}",
            args_str
        );
        assert!(
            args_str.contains("pad=1920:1080"),
            "Should include pad filter with dimensions. Got: {}",
            args_str
        );
        assert!(
            !args_str.contains("-s "),
            "Should NOT use -s flag. Got: {}",
            args_str
        );
    }

    #[test]
    fn test_generate_metadata_fails_on_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();

        let result = DirManager::generate_metadata(temp_dir.path(), "webp");

        assert!(result.is_err(), "Should error when no frames found");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("No frames found"),
            "Error should mention no frames. Got: {}",
            err
        );
    }
}
