// Centralized configuration for Yuyuko Bot

use std::collections::HashMap;

/// Day offset - day ends at 2:00 AM instead of midnight
/// Activity at 1:30 AM on Jan 16 will count as Jan 15
pub const DAY_END_HOUR: u32 = 2;

/// Media type labels for display
pub fn media_type_labels() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("visual_novel", "Visual Novel"),
        ("manga", "Manga"),
        ("anime", "Anime"),
        ("book", "Book"),
        ("reading_time", "Reading Time"),
        ("listening", "Listening"),
        ("reading", "Reading"),
        ("all", "All Media Types"),
    ])
}

/// Units for each media type
pub fn unit_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("visual_novel", "characters"),
        ("manga", "pages"),
        ("anime", "episodes"),
        ("book", "pages"),
        ("reading_time", "minutes"),
        ("listening", "minutes"),
        ("reading", "characters"),
    ])
}

/// Get media type label
pub fn get_media_label(media_type: &str) -> &'static str {
    match media_type {
        "visual_novel" => "Visual Novel",
        "manga" => "Manga",
        "anime" => "Anime",
        "book" => "Book",
        "reading_time" => "Reading Time",
        "listening" => "Listening",
        "reading" => "Reading",
        "all" => "All Media Types",
        _ => "Unknown",
    }
}

/// Get unit for media type
pub fn get_unit(media_type: &str) -> &'static str {
    match media_type {
        "visual_novel" => "characters",
        "manga" => "pages",
        "anime" => "episodes",
        "book" => "pages",
        "reading_time" => "minutes",
        "listening" => "minutes",
        "reading" => "characters",
        _ => "units",
    }
}

/// Discord embed colors
pub mod colors {
    pub const PRIMARY: u32 = 0x00bfff;
    pub const SUCCESS: u32 = 0x2ecc71;
    pub const ERROR: u32 = 0xff0000;
    pub const WARNING: u32 = 0xffa500;
    pub const INFO: u32 = 0x3498db;
    pub const IMMERSION: u32 = 0x00d4aa;
}

/// Get effective date with day offset applied (JST: UTC+9)
/// If current time is before DAY_END_HOUR (e.g., 2 AM), return yesterday's date
pub fn get_effective_date() -> chrono::NaiveDate {
    use chrono::{Utc, Timelike, Duration};
    
    // JST is UTC+9
    let now_utc = Utc::now();
    let now_jst = now_utc + Duration::hours(9);
    let hours = now_jst.hour();
    
    if hours < DAY_END_HOUR {
        now_jst.date_naive() - Duration::days(1)
    } else {
        now_jst.date_naive()
    }
}

/// Get effective date string in YYYY-MM-DD format
pub fn get_effective_date_string() -> String {
    get_effective_date().format("%Y-%m-%d").to_string()
}



#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_media_labels() {
        assert_eq!(get_media_label("anime"), "Anime");
        assert_eq!(get_media_label("visual_novel"), "Visual Novel");
    }
    
    #[test]
    fn test_units() {
        assert_eq!(get_unit("anime"), "episodes");
        assert_eq!(get_unit("manga"), "pages");
    }
}
