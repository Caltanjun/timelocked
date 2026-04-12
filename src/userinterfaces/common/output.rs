use serde_json::Value;

pub fn emit_json_line(value: Value) {
    println!("{}", value);
}

pub fn format_binary_size(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < UNITS.len().saturating_sub(1) {
        value /= 1024.0;
        unit_index += 1;
    }

    format!("{value:.2} {}", UNITS[unit_index])
}

pub fn format_eta(seconds: u64) -> String {
    let mut remaining = seconds;
    let days = remaining / 86_400;
    remaining %= 86_400;
    let hours = remaining / 3_600;
    remaining %= 3_600;
    let minutes = remaining / 60;
    let secs = remaining % 60;

    if days > 0 {
        format!("~{}d {}h", days, hours)
    } else if hours > 0 {
        format!("~{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("~{}m {}s", minutes, secs)
    } else {
        format!("~{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::format_binary_size;

    #[test]
    fn formats_bytes_without_scaling() {
        assert_eq!(format_binary_size(0), "0 B");
        assert_eq!(format_binary_size(999), "999 B");
    }

    #[test]
    fn formats_scaled_binary_units() {
        assert_eq!(format_binary_size(1024), "1.00 KiB");
        assert_eq!(format_binary_size(1_048_576), "1.00 MiB");
        assert_eq!(format_binary_size(3_234_022_377), "3.01 GiB");
    }
}
