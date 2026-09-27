//! Human-readable sizes, speeds and times for the UI and the CLI.

use std::time::Duration;

/// "148.2 MB" (binary units, labelled the way Windows does).
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut value = n as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// "4.2 MB/s"
pub fn rate(bytes_per_sec: f64) -> String {
    format!("{}/s", bytes(bytes_per_sec as u64))
}

/// "~40 s", "~3 min", "~1 h 5 min" - rounded up, never "~0 s".
pub fn eta(left: Duration) -> String {
    let secs = left.as_secs_f64().ceil() as u64;
    if secs < 60 {
        let rounded = secs.div_ceil(5).max(1) * 5;
        if rounded < 60 {
            return format!("~{rounded} s");
        }
    }
    let mins = secs.div_ceil(60);
    if mins < 60 {
        format!("~{mins} min")
    } else {
        format!("~{} h {} min", mins / 60, mins % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(155_398_963), "148.2 MB");
        assert_eq!(rate(4.2 * 1024.0 * 1024.0), "4.2 MB/s");
        assert_eq!(eta(Duration::from_secs(0)), "~5 s");
        assert_eq!(eta(Duration::from_secs(38)), "~40 s");
        assert_eq!(eta(Duration::from_secs(58)), "~1 min");
        assert_eq!(eta(Duration::from_secs(150)), "~3 min");
        assert_eq!(eta(Duration::from_secs(3900)), "~1 h 5 min");
    }
}
