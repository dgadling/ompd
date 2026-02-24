use std::io::Write;

use chrono::Local;
use env_logger::Builder;
use log::{info, LevelFilter};

use ompd::config::Config;

#[cfg(target_os = "windows")]
const EXIT_CODE: i32 = 0x13a;

#[cfg(not(target_os = "windows"))]
const EXIT_CODE: i32 = 130;

fn ctrl_c_exit() {
    info!("And we're done!");
    std::process::exit(EXIT_CODE);
}

fn main() {
    ctrlc::set_handler(move || {
        ctrl_c_exit();
    })
    .expect("Couldn't set a clean exit handler!");

    let level_filter = if cfg!(debug_assertions) {
        LevelFilter::max()
    } else {
        LevelFilter::Info
    };

    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {:5} {}] {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter_level(level_filter)
        .filter_module("wmi", LevelFilter::Error)
        .init();

    let config = Config::get_config();
    ompd::run(config);
}
