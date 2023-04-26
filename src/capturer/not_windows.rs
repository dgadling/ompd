use anyhow::{anyhow, Error};
use screenshots::{Image, Screen};

#[cfg(target_os = "macos")]
use std::process::Command;

pub fn get_screenshot(screen: Screen) -> Result<Image, Error> {
    if !have_graphics() {
        return Err(anyhow!("Don't appear to have graphics!"));
    }

    screen.capture()
}

#[cfg(target_os = "macos")]
fn have_graphics() -> bool {
    let mut to_run = Command::new("/usr/bin/pmset");
    to_run.args(["-g", "systemstate"]);

    let output = to_run.output().expect("Failed to run pmset! :(");
    let stdout_raw = String::from_utf8(output.stdout).unwrap();
    let stdout = stdout_raw.lines().collect::<Vec<_>>();

    for line in stdout {
        if line.contains("Graphics") {
            return true;
        }
    }

    false
}

#[cfg(not(target_os = "macos"))]
fn have_graphics() -> bool {
    true
}
