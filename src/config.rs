use home::home_dir;
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub interval: u64,
    pub max_sleep_secs: i64,
    pub output_dir: String,
}

impl Config {
    pub fn get_config() -> Config {
        let config_path = Path::new("config.json");
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
        let home = home_dir().expect("Couldn't figure out our home directory?!");
        let new_config = Config {
            interval: 20,
            max_sleep_secs: 180,
            output_dir: home
                .join("Pictures")
                .join("ompd")
                .into_os_string()
                .into_string()
                .unwrap(),
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