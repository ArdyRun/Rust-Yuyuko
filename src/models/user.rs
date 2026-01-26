// User data model
// Matches Firebase user document structure

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User profile information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    #[serde(rename = "lastSeen")]
    pub last_seen: Option<String>,
}

/// Per-media-type statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct MediaStats {
    pub total: f64,
    pub sessions: i32,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
    #[serde(rename = "currentStreak")]
    pub current_streak: i32,
    #[serde(rename = "bestStreak")]
    pub best_streak: i32,
    pub unit: String,
    pub label: String,
}

/// User summary data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct UserSummary {
    #[serde(rename = "totalSessions")]
    pub total_sessions: i32,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
    #[serde(rename = "joinDate")]
    pub join_date: Option<String>,
    #[serde(rename = "activeTypes")]
    pub active_types: Vec<String>,
}

/// Streak information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct StreakInfo {
    pub current: i32,
    pub longest: i32,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
}

/// Full user document
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct User {
    pub profile: UserProfile,
    pub stats: HashMap<String, MediaStats>,
    pub summary: UserSummary,
    pub streaks: Option<StreakInfo>,
}

impl User {
    /// Create a new user with basic info
    #[allow(dead_code)]
    pub fn new(id: &str, username: &str, display_name: Option<&str>, avatar: Option<&str>) -> Self {
        Self {
            profile: UserProfile {
                id: id.to_string(),
                username: username.to_string(),
                display_name: display_name.map(|s| s.to_string()),
                avatar: avatar.map(|s| s.to_string()),
                last_seen: Some(chrono::Utc::now().to_rfc3339()),
            },
            stats: HashMap::new(),
            summary: UserSummary::default(),
            streaks: None,
        }
    }

    /// Get total points across all media types
    #[allow(dead_code)]
    pub fn total_points(&self) -> i64 {
        use crate::utils::points::calculate_points;
        
        self.stats
            .iter()
            .map(|(media_type, stats)| calculate_points(media_type, stats.total))
            .sum()
    }

    /// Get total sessions
    #[allow(dead_code)]
    pub fn total_sessions(&self) -> i32 {
        self.stats.values().map(|s| s.sessions).sum()
    }
}
