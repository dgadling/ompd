use anyhow::{anyhow, Error};
use screenshots::{Image, Screen};

#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use which::which;

pub fn get_screenshot(screen: Screen) -> Result<Image, Error> {
    if !have_graphics() {
        return Err(anyhow!("Don't appear to have graphics!"));
    }

    screen.capture()
}

#[cfg(target_os = "macos")]
fn have_graphics() -> bool {
    let pmset_path = which("pmset").unwrap_or_else(|_| "/usr/bin/pmset".into());
    let output = Command::new(pmset_path)
        .args(["-g", "systemstate"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.lines().any(|line| line.contains("Graphics"))
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "macos"))]
fn have_graphics() -> bool {
    true
}
