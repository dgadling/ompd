use log::info;

pub fn ctrl_c_exit() {
    info!("And we're done!");
    std::process::exit(0x13a);
}
