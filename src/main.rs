// `Write` trait is needed for the `writeln!` macro in the logger format closure.
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;
use clap::Parser;
use env_logger::Builder;
use log::{info, LevelFilter};

use ompd::config::Config;

const OMPD_VERSION: &str = env!("OMPD_VERSION");
const OMPD_BUILD_TIME: &str = env!("OMPD_BUILD_TIME");

#[derive(Parser)]
#[command(
    name = "ompd",
    about = "One Movie Per Day — time-lapse screen recorder",
    version = const_format_version(),
)]
struct Cli {
    /// Log verbosity level (trace, debug, info, warn, error, off)
    #[arg(short = 'l', long = "log-level")]
    log_level: Option<LevelFilter>,

    /// Path to config file (default: ~/.ompd-config.json)
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,
}

const fn const_format_version() -> &'static str {
    // clap needs a &'static str; we build it at compile time via concat!
    concat!(
        env!("OMPD_VERSION"),
        " (built ",
        env!("OMPD_BUILD_TIME"),
        ")"
    )
}

#[cfg(target_os = "windows")]
const EXIT_CODE: i32 = 0x13a;

#[cfg(not(target_os = "windows"))]
const EXIT_CODE: i32 = 130;

fn ctrl_c_exit() {
    info!("And we're done!");
    std::process::exit(EXIT_CODE);
}

fn main() {
    let cli = Cli::parse();

    ctrlc::set_handler(move || {
        ctrl_c_exit();
    })
    .expect("Couldn't set a clean exit handler!");

    let level_filter = cli.log_level.unwrap_or(if cfg!(debug_assertions) {
        LevelFilter::max()
    } else {
        LevelFilter::Info
    });

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

    info!("ompd {} built @ {}", OMPD_VERSION, OMPD_BUILD_TIME);

    let config = Config::get_config(cli.config.as_deref());
    ompd::run(config, cli.config);
}
