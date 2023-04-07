use anyhow::{anyhow, Error};
use screenshots::{Screen, Image};
use std::collections::HashMap;
use serde::Deserialize;
use wmi::connection::WMIConnection;
use wmi::query::FilterValue;
use wmi::COMLibrary;

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_Process")]
#[serde(rename_all = "PascalCase")]
struct Process {
    process_id: u32,
}

pub fn get_screenshot(screen: Screen) -> Result<Image, Error> {
    /*
    First, just try to capture the screen. The main reason we wouldn't be able to is that the screen
    saver is running. Even running as administrator you can't capture the screen saver.
    */

    let capture = screen.capture()?;

    /*
    OK, that worked. Now let's make sure we didn't capture anything strange because the lock screen
    is active. Once the lock screen activates and the display goes into standby, we get a simple
    solid color. So just to be safe, if LogonUI.exe is running, return an Error.
    */
    let wmi_con = WMIConnection::new(COMLibrary::new().unwrap()).unwrap();
    let logons: Vec<Process> = wmi_con
        .filtered_query(&HashMap::from([(
            "Name".to_owned(),
            FilterValue::Str("LogonUI.exe"),
        )]))
        .unwrap();
    if !logons.is_empty() {
        return Err(anyhow!(
            "Lock screen is active (pid = {:?}), do not want.",
            logons.get(0).unwrap().process_id
        ));
    }

    Ok(capture)
}

pub fn ctrl_c_exit() {
    std::process::exit(0x13a);
}
