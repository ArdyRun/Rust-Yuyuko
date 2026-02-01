// Custom Prompt Manager
// Allows users to set custom system prompts for Ayumi from Rentry.co URLs

use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::{debug, error};

/// User's custom prompt data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPromptData {
    pub user_id: String,
    pub prompt: String,
    pub timestamp: String,
    pub last_updated: u64,
}

/// Rate limit data
struct RateLimitEntry {
    count: u32,
    timestamp: Instant,
}

// Rate limit configuration
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60); // 1 minute
const MAX_REQUESTS_PER_WINDOW: u32 = 3;

// Custom prompt directory
static PROMPT_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let dir = PathBuf::from("data/custom_prompts");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
});

// Rate limit storage
static RATE_LIMITS: Lazy<RwLock<HashMap<u64, RateLimitEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Check if user is rate limited
pub fn is_rate_limited(user_id: u64) -> Result<bool, u64> {
    let now = Instant::now();
    let mut limits = RATE_LIMITS.write().unwrap();

    if let Some(entry) = limits.get_mut(&user_id) {
        if now.duration_since(entry.timestamp) > RATE_LIMIT_WINDOW {
            // Reset window
            entry.count = 1;
            entry.timestamp = now;
            Ok(false)
        } else if entry.count >= MAX_REQUESTS_PER_WINDOW {
            // Rate limited
            let time_left =
                RATE_LIMIT_WINDOW.as_secs() - now.duration_since(entry.timestamp).as_secs();
            Err(time_left)
        } else {
            entry.count += 1;
            Ok(false)
        }
    } else {
        limits.insert(
            user_id,
            RateLimitEntry {
                count: 1,
                timestamp: now,
            },
        );
        Ok(false)
    }
}

/// Get user's custom prompt from local file
pub fn get_user_custom_prompt(user_id: u64) -> Option<String> {
    let path = PROMPT_DIR.join(format!("{}.json", user_id));

    if !path.exists() {
        return None;
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<UserPromptData>(&content) {
            Ok(data) => Some(data.prompt),
            Err(e) => {
                error!("Failed to parse prompt file for user {}: {:?}", user_id, e);
                None
            }
        },
        Err(e) => {
            error!("Failed to read prompt file for user {}: {:?}", user_id, e);
            None
        }
    }
}

/// Save user's custom prompt to local file
pub fn save_user_custom_prompt(user_id: u64, prompt: &str) -> bool {
    let path = PROMPT_DIR.join(format!("{}.json", user_id));

    let data = UserPromptData {
        user_id: user_id.to_string(),
        prompt: prompt.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    };

    match serde_json::to_string_pretty(&data) {
        Ok(json) => match fs::write(&path, json) {
            Ok(_) => {
                debug!("Saved custom prompt for user {}", user_id);
                true
            }
            Err(e) => {
                error!("Failed to write prompt file for user {}: {:?}", user_id, e);
                false
            }
        },
        Err(e) => {
            error!("Failed to serialize prompt for user {}: {:?}", user_id, e);
            false
        }
    }
}

/// Delete user's custom prompt
pub fn delete_user_custom_prompt(user_id: u64) -> bool {
    let path = PROMPT_DIR.join(format!("{}.json", user_id));

    if path.exists() {
        match fs::remove_file(&path) {
            Ok(_) => {
                debug!("Deleted custom prompt for user {}", user_id);
                true
            }
            Err(e) => {
                error!("Failed to delete prompt file for user {}: {:?}", user_id, e);
                false
            }
        }
    } else {
        false
    }
}

/// Validate if URL is a valid Rentry URL
pub fn is_valid_rentry_url(url: &str) -> bool {
    url.starts_with("https://rentry.co/") || url.starts_with("https://www.rentry.co/")
}

/// Extract Rentry code from URL
fn extract_rentry_code(url: &str) -> Option<String> {
    let url_parts: Vec<&str> = url.trim_end_matches('/').split('/').collect();
    url_parts.last().map(|s| s.to_string())
}

/// Fetch prompt content from Rentry URL
pub async fn fetch_prompt_from_rentry(
    client: &reqwest::Client,
    rentry_url: &str,
) -> Result<String> {
    let code =
        extract_rentry_code(rentry_url).ok_or_else(|| anyhow::anyhow!("Invalid Rentry URL"))?;

    // Try raw endpoint first
    let raw_url = format!("https://rentry.co/{}/raw", code);

    let response = client
        .get(&raw_url)
        .header("Accept", "text/plain")
        .header("User-Agent", "Mozilla/5.0 (compatible; DiscordBot/1.0)")
        .send()
        .await?;

    if response.status().is_success() {
        let content = response.text().await?;

        // Check if it's an error page
        if content.to_lowercase().contains("access code")
            || content.to_lowercase().contains("<!doctype")
            || content.to_lowercase().contains("<html")
        {
            anyhow::bail!("Rentry page requires access code or is not accessible");
        }

        Ok(content.trim().to_string())
    } else {
        anyhow::bail!("Failed to fetch Rentry content: {}", response.status())
    }
}

/// Validate prompt content
pub fn validate_prompt_content(content: &str) -> Result<()> {
    if content.len() < 10 {
        anyhow::bail!("Prompt is too short (minimum 10 characters)");
    }

    if content.len() > 10000 {
        anyhow::bail!("Prompt is too long (maximum 10,000 characters)");
    }

    // Basic security check
    let harmful_patterns = [
        "eval(",
        "Function(",
        "setTimeout(",
        "setInterval(",
        "require(",
    ];
    for pattern in harmful_patterns {
        if content.contains(pattern) {
            anyhow::bail!("Prompt contains potentially harmful content");
        }
    }

    Ok(())
}
