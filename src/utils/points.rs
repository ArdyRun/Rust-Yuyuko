// Points calculation system
// Ported from utils/points.js

use std::collections::HashMap;

/// Points multipliers for each media type
/// These values determine how much 1 unit of activity is worth in points
pub fn points_multipliers() -> HashMap<&'static str, f64> {
    HashMap::from([
        ("visual_novel", 0.0028571428571429),  // ~350 chars = 1 point
        ("manga", 0.25),                        // 4 pages = 1 point
        ("anime", 13.0),                        // 1 episode = 13 points
        ("book", 1.0),                          // 1 page = 1 point
        ("reading_time", 0.67),                 // ~1.5 min = 1 point
        ("listening", 0.67),                    // ~1.5 min = 1 point
        ("reading", 0.0028571428571429),        // ~350 chars = 1 point
    ])
}

/// Calculate points for a given media type and amount
pub fn calculate_points(media_type: &str, amount: f64) -> i64 {
    let multiplier = points_multipliers()
        .get(media_type)
        .copied()
        .unwrap_or(1.0);
    
    (amount * multiplier).round() as i64
}

/// Get the multiplier for a media type
pub fn get_multiplier(media_type: &str) -> f64 {
    points_multipliers()
        .get(media_type)
        .copied()
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_anime_points() {
        assert_eq!(calculate_points("anime", 1.0), 13);
        assert_eq!(calculate_points("anime", 12.0), 156);
    }
    
    #[test]
    fn test_manga_points() {
        assert_eq!(calculate_points("manga", 100.0), 25);
    }
    
    #[test]
    fn test_vn_points() {
        // ~350 chars should be ~1 point
        assert_eq!(calculate_points("visual_novel", 350.0), 1);
        assert_eq!(calculate_points("visual_novel", 35000.0), 100);
    }
    
    #[test]
    fn test_unknown_type() {
        // Unknown types get multiplier of 1.0
        assert_eq!(calculate_points("unknown", 100.0), 100);
    }
}
