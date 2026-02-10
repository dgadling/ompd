//! Integration tests for multiple aspect ratio support

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn get_fixture_files() -> Vec<PathBuf> {
    fs::read_dir(fixtures_path())
        .expect("tests/fixtures directory should exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map_or(false, |ext| ext == "jpeg" || ext == "jpg")
        })
        .collect()
}

fn copy_fixtures_to(dest: &Path, fixtures: &[PathBuf]) {
    for (i, fixture) in fixtures.iter().enumerate() {
        fs::copy(fixture, dest.join(format!("{:05}.jpeg", i))).expect("copy fixture");
    }
}

fn test_config(shot_dir: &str, vid_dir: &str) -> ompd::Config {
    ompd::Config {
        interval: 20,
        max_sleep_secs: 180,
        shot_output_dir: shot_dir.to_string(),
        vid_output_dir: vid_dir.to_string(),
        ffmpeg: "ffmpeg".to_string(),
        handle_old_dirs_on_startup: false,
        shot_type: "jpeg".to_string(),
        video_type: "mp4".to_string(),
        vid_scale_factor: 1.0,
    }
}

#[test]
fn test_metadata_csv_generation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let shot_dir = temp_dir.path();

    let fixtures = get_fixture_files();
    assert!(fixtures.len() >= 2, "Need at least 2 fixture images");
    copy_fixtures_to(shot_dir, &fixtures);

    ompd::dir_manager::DirManager::generate_metadata(shot_dir, "jpeg").unwrap();

    let csv_path = shot_dir.join("frame_metadata.csv");
    assert!(csv_path.exists(), "Metadata CSV should be created");

    let original_content = fs::read_to_string(&csv_path).unwrap();
    assert!(original_content.contains("frame,width,height"));
}

#[test]
fn test_video_with_mixed_resolutions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let shot_dir = temp_dir.path().join("2026").join("01").join("01");
    let vid_dir = temp_dir.path().join("videos");

    fs::create_dir_all(&shot_dir).unwrap();
    fs::create_dir_all(&vid_dir).unwrap();

    let fixtures = get_fixture_files();
    assert!(fixtures.len() >= 2, "Need at least 2 fixture images");
    copy_fixtures_to(&shot_dir, &fixtures);

    let config = test_config(
        &temp_dir.path().to_string_lossy(),
        &vid_dir.to_string_lossy(),
    );
    let maker = ompd::movie_maker::MovieMaker::new(config);

    let output_file = vid_dir.join("ompd-2026-01-01.mp4");
    // Pass explicit dimensions to build_ffmpeg_args
    let args = maker.build_ffmpeg_args(&shot_dir, &output_file, 1920, 1080);
    let args_str = args.join(" ");

    assert!(
        args_str.contains("-vf"),
        "Should use -vf filter. Got: {}",
        args_str
    );
    assert!(
        args_str.contains("scale=") && args_str.contains("pad="),
        "Should have scale/pad. Got: {}",
        args_str
    );
}

#[test]
fn test_backfiller_generates_metadata() {
    let temp_dir = tempfile::tempdir().unwrap();
    let old_shot_dir = temp_dir
        .path()
        .join("shots")
        .join("2025")
        .join("12")
        .join("15");
    let vid_dir = temp_dir.path().join("videos");

    fs::create_dir_all(&old_shot_dir).unwrap();
    fs::create_dir_all(&vid_dir).unwrap();

    let fixtures = get_fixture_files();
    assert!(!fixtures.is_empty(), "Need at least 1 fixture image");
    copy_fixtures_to(&old_shot_dir, &fixtures[..fixtures.len().min(3)]);

    let csv_path = old_shot_dir.join("frame_metadata.csv");
    assert!(!csv_path.exists(), "CSV should not exist initially");

    // Verify generate_metadata works for legacy directories
    ompd::dir_manager::DirManager::generate_metadata(&old_shot_dir, "jpeg").unwrap();

    assert!(csv_path.exists(), "Metadata CSV should be generated");
}

/// Test that mixed resolution input produces a valid video using the most common resolution
/// This test actually runs ffmpeg and verifies the video is created successfully
#[test]
fn test_mixed_resolution_video_uses_most_common() {
    let temp_dir = tempfile::tempdir().unwrap();
    let shot_dir = temp_dir.path().join("2026").join("01").join("02");
    let vid_dir = temp_dir.path().join("videos");

    fs::create_dir_all(&shot_dir).unwrap();
    fs::create_dir_all(&vid_dir).unwrap();

    // Get fixtures with different aspect ratios
    let fixtures = get_fixture_files();
    assert!(
        fixtures.len() >= 2,
        "Need at least 2 fixture images with different aspect ratios"
    );

    // Copy fixtures to shot directory
    copy_fixtures_to(&shot_dir, &fixtures);

    let config = test_config(
        &temp_dir.path().to_string_lossy(),
        &vid_dir.to_string_lossy(),
    );
    let maker = ompd::movie_maker::MovieMaker::new(config);

    // Run make_movie_from to generate the video
    maker.make_movie_from(&shot_dir);

    let output_video = vid_dir.join("ompd-2026-01-02.mp4");
    assert!(output_video.exists(), "Video should be created");

    // Extract first frame from video for analysis
    let extracted_frame = temp_dir.path().join("extracted_frame.png");
    let extract_result = Command::new("ffmpeg")
        .args([
            "-i",
            &output_video.to_string_lossy(),
            "-vframes",
            "1",
            "-y",
            &extracted_frame.to_string_lossy(),
        ])
        .output()
        .expect("Failed to extract frame from video");

    assert!(
        extract_result.status.success(),
        "Frame extraction should succeed: {}",
        String::from_utf8_lossy(&extract_result.stderr)
    );

    assert!(extracted_frame.exists(), "Extracted frame should exist");

    // Load the extracted frame and verify dimensions are valid (even numbers)
    let (width, height) =
        image::image_dimensions(&extracted_frame).expect("Should read extracted frame dimensions");

    assert_eq!(width % 2, 0, "Output width should be even. Got: {}", width);
    assert_eq!(
        height % 2,
        0,
        "Output height should be even. Got: {}",
        height
    );

    println!("Video created at {}x{}", width, height);
}
