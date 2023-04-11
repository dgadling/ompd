use anyhow::Error;
use screenshots::{Image, Screen};
use std::process;

pub fn get_screenshot(screen: Screen) -> Result<Image, Error> {
    /*
    NOTE: On OS X we can use
        /usr/bin/pmset -g systemstate | grep -q Graphics
    to see if the display is on or not.
    if that exits dirty, there's no graphics. We should just sleep & continue
    */
    screen.capture()
}
