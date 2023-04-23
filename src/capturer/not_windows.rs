use anyhow::Error;
use screenshots::{Image, Screen};

pub fn get_screenshot(screen: Screen) -> Result<Image, Error> {
    screen.capture()
}
