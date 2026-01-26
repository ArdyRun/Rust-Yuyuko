// Immersion log data model
// Matches Firebase immersion_logs subcollection structure

use serde::{Deserialize, Serialize};

/// User info embedded in log
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct LogUser {
    pub id: String,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub avatar: Option<String>,
}

/// Activity information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Activity {
    #[serde(rename = "type")]
    pub media_type: String,
    #[serde(rename = "typeLabel")]
    pub type_label: String,
    pub amount: f64,
    pub unit: String,
    pub title: String,
    pub comment: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "anilistUrl")]
    pub anilist_url: Option<String>,
    #[serde(rename = "vndbUrl")]
    pub vndb_url: Option<String>,
}

/// Metadata from external APIs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct LogMetadata {
    pub thumbnail: Option<String>,
    pub duration: Option<i32>,
    pub source: String,
    #[serde(rename = "vndbInfo")]
    pub vndb_info: Option<VndbInfo>,
}

/// VNDB info embedded in metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VndbInfo {
    pub developer: Option<String>,
    pub released: Option<String>,
    pub length: Option<i32>,
    pub description: Option<String>,
}

/// Timestamp information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Timestamps {
    pub created: String,
    pub date: String,
    pub month: String,
    pub year: i32,
}

/// Full immersion log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ImmersionLog {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub user: LogUser,
    pub activity: Activity,
    pub metadata: LogMetadata,
    pub timestamps: Timestamps,
}

impl ImmersionLog {
    /// Create a new immersion log
    #[allow(dead_code)]
    pub fn new(
        user_id: &str,
        username: &str,
        display_name: Option<&str>,
        avatar: Option<&str>,
        media_type: &str,
        type_label: &str,
        amount: f64,
        unit: &str,
        title: &str,
        comment: Option<&str>,
    ) -> Self {
        use crate::utils::config::{get_effective_date, get_effective_date_string};
        
        let now = chrono::Utc::now();
        let effective_date = get_effective_date();
        
        Self {
            id: None,
            user: LogUser {
                id: user_id.to_string(),
                username: username.to_string(),
                display_name: display_name.map(|s| s.to_string()),
                avatar: avatar.map(|s| s.to_string()),
            },
            activity: Activity {
                media_type: media_type.to_string(),
                type_label: type_label.to_string(),
                amount,
                unit: unit.to_string(),
                title: title.to_string(),
                comment: comment.map(|s| s.to_string()),
                url: None,
                anilist_url: None,
                vndb_url: None,
            },
            metadata: LogMetadata {
                source: "manual".to_string(),
                ..Default::default()
            },
            timestamps: Timestamps {
                created: now.to_rfc3339(),
                date: get_effective_date_string(),
                month: effective_date.format("%Y-%m").to_string(),
                year: effective_date.year(),
            },
        }
    }
}

use chrono::Datelike;
