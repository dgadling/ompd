#[cfg(not(target_os = "windows"))]
mod not_windows;

#[cfg(target_os = "windows")]
mod windows;

use env_logger::Builder;
use log::LevelFilter;

#[cfg(not(target_os = "windows"))]
use not_windows::ctrl_c_exit;

#[cfg(target_os = "windows")]
use windows::ctrl_c_exit;

use ompd::config::Config;

fn main() {
    ctrlc::set_handler(move || {
        ctrl_c_exit();
    })
    .expect("Couldn't set a clean exit handler!");

    Builder::new()
        .filter_level(LevelFilter::max())
        .filter_module("wmi", LevelFilter::Error)
        .init();

    let config = Config::get_config();
    ompd::run(config);
}
