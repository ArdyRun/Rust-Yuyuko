// Streak calculation system
// Ported from utils/streak.js

use chrono::{Duration, NaiveDate};
use std::collections::HashSet;

use super::config::get_effective_date;

/// Result of streak calculation
#[derive(Debug, Clone, Default)]
pub struct StreakResult {
    pub current: i32,
    pub longest: i32,
}

/// Calculate streak from a list of activity dates
/// Dates should be in YYYY-MM-DD format and sorted ascending
pub fn calculate_streak(dates: &[String]) -> StreakResult {
    if dates.is_empty() {
        return StreakResult::default();
    }

    let mut current_streak = 0;

    // Parse dates into NaiveDate
    let parsed_dates: Vec<NaiveDate> = dates
        .iter()
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .collect();

    if parsed_dates.is_empty() {
        return StreakResult::default();
    }

    let date_set: HashSet<NaiveDate> = parsed_dates.iter().cloned().collect();

    // Get today and yesterday with day offset
    let today = get_effective_date();
    let yesterday = today - Duration::days(1);

    // Calculate current streak from today backwards
    let mut check_date = if date_set.contains(&today) {
        today
    } else if date_set.contains(&yesterday) {
        // If no activity today but yesterday, streak is still alive
        yesterday
    } else {
        // No recent activity, current streak is 0
        return calculate_longest_only(&parsed_dates);
    };

    // Count backwards from check_date
    while date_set.contains(&check_date) {
        current_streak += 1;
        check_date -= Duration::days(1);
    }

    // Calculate longest streak
    let mut longest_streak = calculate_longest_streak(&parsed_dates);

    // Current can never be longer than longest
    if current_streak > longest_streak {
        longest_streak = current_streak;
    }

    StreakResult {
        current: current_streak,
        longest: longest_streak,
    }
}

/// Calculate only the longest streak (when current is 0)
fn calculate_longest_only(dates: &[NaiveDate]) -> StreakResult {
    StreakResult {
        current: 0,
        longest: calculate_longest_streak(dates),
    }
}

/// Calculate the longest consecutive streak in the dates
fn calculate_longest_streak(dates: &[NaiveDate]) -> i32 {
    if dates.is_empty() {
        return 0;
    }

    let mut sorted_dates: Vec<NaiveDate> = dates.to_vec();
    sorted_dates.sort();
    sorted_dates.dedup();

    if sorted_dates.len() == 1 {
        return 1;
    }

    let mut longest = 1;
    let mut current = 1;

    for i in 1..sorted_dates.len() {
        let prev = sorted_dates[i - 1];
        let curr = sorted_dates[i];

        if curr == prev + Duration::days(1) {
            current += 1;
            if current > longest {
                longest = current;
            }
        } else {
            current = 1;
        }
    }

    longest
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn today_str() -> String {
        get_effective_date().format("%Y-%m-%d").to_string()
    }

    fn yesterday_str() -> String {
        (get_effective_date() - Duration::days(1))
            .format("%Y-%m-%d")
            .to_string()
    }

    #[test]
    fn test_empty_dates() {
        let result = calculate_streak(&[]);
        assert_eq!(result.current, 0);
        assert_eq!(result.longest, 0);
    }

    #[test]
    fn test_single_day_today() {
        let dates = vec![today_str()];
        let result = calculate_streak(&dates);
        assert_eq!(result.current, 1);
        assert_eq!(result.longest, 1);
    }

    #[test]
    fn test_streak_yesterday() {
        // No activity today but yesterday - streak should still be 1
        let dates = vec![yesterday_str()];
        let result = calculate_streak(&dates);
        assert_eq!(result.current, 1);
    }

    #[test]
    fn test_consecutive_days() {
        let today = get_effective_date();
        let dates: Vec<String> = (0..5)
            .map(|i| (today - Duration::days(i)).format("%Y-%m-%d").to_string())
            .collect();

        let result = calculate_streak(&dates);
        assert_eq!(result.current, 5);
        assert_eq!(result.longest, 5);
    }

    #[test]
    fn test_broken_streak() {
        let today = get_effective_date();
        // Gap of 5 days ago
        let dates = vec![
            today.format("%Y-%m-%d").to_string(),
            (today - Duration::days(1)).format("%Y-%m-%d").to_string(),
            (today - Duration::days(5)).format("%Y-%m-%d").to_string(),
            (today - Duration::days(6)).format("%Y-%m-%d").to_string(),
            (today - Duration::days(7)).format("%Y-%m-%d").to_string(),
        ];

        let result = calculate_streak(&dates);
        assert_eq!(result.current, 2);
        assert_eq!(result.longest, 3);
    }
}
