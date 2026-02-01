// Formatting utilities

/// Format a number with locale-aware thousands separators
#[allow(dead_code)]
pub fn format_number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 && *c != '-' {
            result.push(',');
        }
        result.push(*c);
    }

    result
}

/// Format duration in minutes to human readable (e.g., "2h 30m")
#[allow(dead_code)]
pub fn format_duration(minutes: i64) -> String {
    if minutes < 60 {
        format!("{}m", minutes)
    } else {
        let hours = minutes / 60;
        let mins = minutes % 60;
        if mins > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}h", hours)
        }
    }
}

/// Format points with suffix (e.g., "1.2k", "3.5M")
#[allow(dead_code)]
pub fn format_points_short(points: i64) -> String {
    if points >= 1_000_000 {
        format!("{:.1}M", points as f64 / 1_000_000.0)
    } else if points >= 1_000 {
        format!("{:.1}k", points as f64 / 1_000.0)
    } else {
        points.to_string()
    }
}

/// Truncate string to max length with ellipsis
#[allow(dead_code)]
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Format relative time (e.g., "2 hours ago")
#[allow(dead_code)]
pub fn format_relative_time(seconds_ago: i64) -> String {
    if seconds_ago < 60 {
        "just now".to_string()
    } else if seconds_ago < 3600 {
        let mins = seconds_ago / 60;
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if seconds_ago < 86400 {
        let hours = seconds_ago / 3600;
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = seconds_ago / 86400;
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1000000), "1,000,000");
        assert_eq!(format_number(123), "123");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30m");
        assert_eq!(format_duration(90), "1h 30m");
        assert_eq!(format_duration(120), "2h");
    }

    #[test]
    fn test_format_points_short() {
        assert_eq!(format_points_short(500), "500");
        assert_eq!(format_points_short(1500), "1.5k");
        assert_eq!(format_points_short(1500000), "1.5M");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
    }
}
