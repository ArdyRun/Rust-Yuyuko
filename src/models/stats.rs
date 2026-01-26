// Statistics models for leaderboard and rankings

use serde::{Deserialize, Serialize};

/// Stat entry for leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct LeaderboardEntry {
    pub user_id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub points: i64,
    pub amount: f64,
    pub sessions: i32,
    pub media_type: Option<String>,
}

/// Aggregated stats for a user
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct AggregatedStats {
    pub total_points: i64,
    pub total_sessions: i32,
    pub by_media: std::collections::HashMap<String, MediaTypeStats>,
}

/// Stats for a single media type
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct MediaTypeStats {
    pub total: f64,
    pub sessions: i32,
    pub points: i64,
}

/// Time period for leaderboard queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TimePeriod {
    Weekly,
    Monthly,
    Yearly,
    AllTime,
}

impl TimePeriod {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            TimePeriod::Weekly => "Weekly",
            TimePeriod::Monthly => "Monthly",
            TimePeriod::Yearly => "Yearly",
            TimePeriod::AllTime => "All-time",
        }
    }
}

impl std::str::FromStr for TimePeriod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "weekly" => Ok(TimePeriod::Weekly),
            "monthly" => Ok(TimePeriod::Monthly),
            "yearly" => Ok(TimePeriod::Yearly),
            "all_time" | "alltime" | "all-time" => Ok(TimePeriod::AllTime),
            _ => Err(format!("Unknown time period: {}", s)),
        }
    }
}
