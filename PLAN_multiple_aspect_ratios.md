# Plan: Support Multiple Aspect Ratio Stills

## Overview
Auto-detect the largest frame dimensions from all captured stills, apply a configurable scale factor, and use ffmpeg filters to letterbox/pillarbox frames that don't match the target dimensions.

## Implementation Steps

### 1. Add dimension tracking during capture
**File**: `src/capturer.rs`

- Create CSV metadata file: `frame_metadata.csv` with header: `frame,width,height`
- In `store()` (line 89-111):
  - Extract image dimensions after decoding
  - Append row to CSV: `{frame_number},{width},{height}\n`
  - Use append mode to avoid reading entire file
- Update `create_filler_frame()` signature to accept `width: u32, height: u32` parameters (currently hardcoded at line 126 as 860x360)
- Add new method: `get_current_dimensions(&self, dir_manager: &DirManager) -> (u32, u32)`
  - Read metadata CSV if it exists and get the last frame's dimensions
  - Otherwise, read the most recent actual screenshot file's dimensions
  - Otherwise, fall back to sensible default (1920x1080)
- Update `deal_with_blackout()` (line 114-146):
  - Call `get_current_dimensions()` to determine filler frame size
  - Pass dimensions to `create_filler_frame()`

**Logging**:
- Debug level: "Creating filler frame with dimensions WxH" (in `deal_with_blackout`)

### 2. Add scale factor to configuration
**File**: `src/config.rs`

- Add field to `Config` struct: `pub vid_scale_factor: f32`
- Default value: `1.0` in `new_config` (line 90-112)
- Add validation after line 47: `assert!(config.vid_scale_factor > 0.0, "vid_scale_factor must be positive")`
- Keep `vid_width` and `vid_height` fields unchanged (no auto-migration)

### 3. Implement dimension analysis in MovieMaker
**File**: `src/movie_maker.rs`

Add new data structure:
```rust
struct FrameMetadata {
    frames: Vec<(u32, u32, u32)>, // (frame_num, width, height)
}
```

Add new methods:
- `generate_metadata(in_dir: &Path, extension: &str) -> Result<FrameMetadata, Error>`
  - Scan all image files in directory
  - Read dimensions from each
  - Write to `frame_metadata.csv`
  - Return FrameMetadata structure
  - **Logging**: Info level: "Generating metadata from N frames"

- `get_metadata(&self, in_dir: &Path) -> Result<FrameMetadata, Error>`
  - Decompress `frame_metadata.csv.zst` if it exists
  - If `frame_metadata.csv` exists, read and parse it
  - If not, call `generate_metadata()` to create it
  - Return FrameMetadata structure
  - **Logging**: Info level: "Detected dimensions range: {min_w}x{min_h} to {max_w}x{max_h}"

- `analyze_frame_dimensions(&self, metadata: &FrameMetadata) -> (u32, u32)`
  - Find max width and max height across all frames
  - Multiply by `self.vid_scale_factor` (from config)
  - Round to nearest even number
  - Return `(target_width, target_height)`
  - **Logging**: Info level: "Target video dimensions: {w}x{h} (scale factor: {f}, rounded to even)"

Update `make_movie_from()` (line 61-144):
- After `fix_missing_frames()`, call `get_metadata()` and `analyze_frame_dimensions()`
- Store target dimensions for use in ffmpeg command

### 4. Update ffmpeg command to handle multiple aspect ratios
**File**: `src/movie_maker.rs`, in `make_movie_from()`

Replace the `-s` argument (line 100-101) with `-vf` filter:
```
"-vf",
&format!("scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black",
    target_width, target_height, target_width, target_height)
```

This filter:
- Scales frames to fit within target dimensions while preserving aspect ratio
- Pads with black bars to reach exact target dimensions
- Centers the content

### 5. Add metadata compression
**File**: `src/dir_manager.rs`

Update `compress()` method (line 50-52):
- After compressing shot files with `iterate_and_operate()`, add special handling for `frame_metadata.csv`
- Check if `target.join("frame_metadata.csv")` exists
- If yes, call `actually_compress()` on it

Update `decompress()` method (line 44-48):
- After decompressing shot files with `iterate_and_operate()`, add special handling for `frame_metadata.csv.zst`
- Check if `target.join("frame_metadata.csv.zst")` exists
- If yes, call `actually_decompress()` on it

### 6. Update BackFiller to generate missing metadata
**File**: `src/back_filler.rs`

Update `run()` method (line 53-84):
- In the loop over `to_process` (line 78-81), before calling `make_movie_from()`:
  - Get the full shot directory path
  - Call `MovieMaker::generate_metadata()` if `frame_metadata.csv` doesn't exist
  - This ensures backward compatibility with old directories

**Logging**: Info level: "Generating metadata for {dir}" (if metadata missing)

## Critical Files to Modify

1. `src/capturer.rs` - dimension tracking, filler frame sizing
2. `src/config.rs` - add `vid_scale_factor` field
3. `src/movie_maker.rs` - metadata handling, dimension analysis, ffmpeg filter
4. `src/dir_manager.rs` - compress/decompress metadata CSV
5. `src/back_filler.rs` - generate metadata for old directories

## Metadata Format

CSV file: `frame_metadata.csv`
```
frame,width,height
0,1920,1080
1,1920,1080
2,2560,1440
...
```

Simple to append, parse, and compress.

## Testing Scenarios

1. Single monitor, consistent resolution
2. Switching between different monitor resolutions
3. Display rotation (landscape ↔ portrait)
4. Blackout/filler frames with varying dimensions
5. Old directories without metadata (BackFiller migration)
6. Various scale factors (1.0, 0.5, 0.25)
7. Metadata compression/decompression
