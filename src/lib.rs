mod capturer;
pub mod config;
mod dir_manager;

use capturer::Capturer;
use chrono::Local;
use config::Config;
use dir_manager::DirManager;
use log::{error, info};
use std::thread;

pub fn run(config: Config) {
    let sleep_interval = std::time::Duration::from_secs(config.interval);
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
