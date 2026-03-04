use std::time::SystemTime;

fn main() {
    // --- version ---
    let version = match std::env::var("OMPD_BUILD_VERSION") {
        Ok(v) if !v.is_empty() => v,
        _ => env!("CARGO_PKG_VERSION").to_string(),
    };
    println!("cargo:rustc-env=OMPD_VERSION={version}");
    println!("cargo:rerun-if-env-changed=OMPD_BUILD_VERSION");

    // --- build timestamp (UTC) using Hinnant civil calendar algorithm ---
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();

    let day_secs = secs % 86400;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // Days since 1970-01-01
    let z = (secs / 86400) as i64 + 719468; // shift to 0000-03-01 epoch
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    let build_time = format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z");
    println!("cargo:rustc-env=OMPD_BUILD_TIME={build_time}");
}
