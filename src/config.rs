use home::home_dir;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;
use std::fs::File;
use which::which;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub interval: u64,
    pub max_sleep_secs: i64,
    pub shot_output_dir: String,
    pub vid_output_dir: String,
    pub ffmpeg: String,
    pub handle_old_dirs_on_startup: bool,
    pub vid_width: u32,
    pub vid_height: u32,
    pub shot_type: String,
}

impl Config {
    pub fn get_config() -> Config {
        let home = home_dir().expect("Couldn't figure out our home directory?!");

        create_dir_all(home.join("ompd")).expect("Couldn't create our own directory?!");

        let config_path = home.join("ompd").join("config.json");
        let mut write_config = true;

        if config_path.exists() {
            if config_path.is_file() {
                let config_file = File::open(config_path).expect("Failed to open config.json");
                let config: Config =
                    serde_json::from_reader(config_file).expect("Failed to read config file");
                debug!("Read config of: {config:?}");

                assert!(
                    config.max_sleep_secs > 0,
                    "max_sleep_secs must be greater than zero. No sleeping backwards!"
                );

                return config;
            } else {
                warn!("{config_path:?} isn't a file. Going to use default config and NOT save it.");
                write_config = false;
            }
        }

        debug!("Making new base config");
        #[cfg(target_os = "windows")]
        let ffmpeg_path_maybe = which("ffmpeg.exe");

        #[cfg(not(target_os = "windows"))]
        let ffmpeg_path_maybe = which("ffmpeg");

        let ffmpeg_path = match ffmpeg_path_maybe {
            Err(_) => {
                warn!("Couldn't find a path to ffmpeg, making one up! You should update {config_path:?}");
                "FIND SOMETHING TO PUT HERE".to_string()
            }
            Ok(p) => p.to_str().unwrap().to_string(),
        };

        let new_config = Config {
            interval: 20,
            max_sleep_secs: 180,
            shot_output_dir: home
                .join("ompd")
                .join("shots")
                .into_os_string()
                .into_string()
                .unwrap(),
            vid_output_dir: home
                .join("ompd")
                .join("videos")
                .into_os_string()
                .into_string()
                .unwrap(),
            ffmpeg: ffmpeg_path,
            handle_old_dirs_on_startup: true,
            vid_width: 860,
            vid_height: 360,
            shot_type: "png".to_string(),
        };

        if write_config {
            let wrote_config = std::fs::write(
                config_path,
                serde_json::to_string_pretty(&new_config).unwrap(),
            );
            if let Err(e) = wrote_config {
                error!("Couldn't write config file! Will have to try again next time: {e:?}");
            }
        }

        new_config
    }
}
