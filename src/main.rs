use chrono::Local;
use env_logger::Builder;
use log::{debug, error, info, LevelFilter};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::thread;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::ctrl_c_exit;

#[cfg(not(target_os = "windows"))]
mod not_windows;
#[cfg(not(target_os = "windows"))]
use not_windows::ctrl_c_exit;

mod capturer;
use capturer::Capturer;

mod dir_manager;
use dir_manager::DirManager;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    interval: u64,
    max_sleep_secs: i64,
    output_dir: String,
}

fn main() {
    ctrlc::set_handler(move || {
        ctrl_c_exit();
    })
    .expect("Couldn't set a clean exit handler!");

    Builder::new()
        .filter_level(LevelFilter::max())
        .filter_module("wmi", LevelFilter::Error)
        .init();

    let config_file = File::open("config.json").expect("Failed to open config.json");
    let config: Config = serde_json::from_reader(config_file).expect("Failed to read config file");
    debug!("Read config of: {config:?}");
    let sleep_interval = std::time::Duration::from_secs(config.interval);

    assert!(
        config.max_sleep_secs > 0,
        "max_sleep_secs must be greater than zero. No sleeping backwards!"
    );

    let mut d = DirManager::new(&config.output_dir);
    let mut c = Capturer::new(&sleep_interval);

    let starting_time = Local::now();
    let mut last_time = starting_time;

    let made_output_d = d.make_output_dir();
    if let Err(e) = made_output_d {
        error!("Couldn't make an output directory: {e:?}");
        panic!("Couldn't make an output directory!");
    }

    c.discover_current_frame(&mut d);

    loop {
        let capture_result = c.capture_screen();
        if let Err(e) = capture_result {
            info!("Couldn't get a good screenshot ({:?}), skip this frame", e);
            thread::sleep(sleep_interval);
            continue;
        }

        let now = Local::now();

        // NOTE: Timezone changes are handled correctly in subtraction, so this can only go
        // backwards if the timezone doesn't change but the system clock goes backwards.
        if (now - last_time).num_seconds() > config.max_sleep_secs {

            // At this point we know we went *forward* in time since max_sleep_secs can only be
            // positive.
            let change_result = c.deal_with_change(&mut d, &last_time, &now);
            if let Err(e) = change_result {
                error!("Some issue dealing with a decent time gap: {e:?}");
                info!("Going to sleep and try again");
                thread::sleep(sleep_interval);
                continue;
            }
        }

        c.store(capture_result.unwrap(), d.current_dir());
        last_time = now;

        thread::sleep(sleep_interval);
    }
}
