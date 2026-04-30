// Ayumu API client for JLPT exam sessions, profiles, and leaderboards

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;

pub struct AyumuClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionResponse {
    pub session_code: String,
    pub url: String,
    pub question_count: i32,
    pub expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileResponse {
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub rank: String,
    pub total_xp: i32,
    pub current_streak: i32,
    pub total_exams: i32,
    pub achievements: Vec<Achievement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Achievement {
    pub code: String,
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub username: String,
    pub avatar_url: Option<String>,
    pub total_score: i32,
    pub total_exams: i32,
    pub best_score: i32,
}

impl AyumuClient {
    pub fn new(client: Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
        }
    }

    pub async fn create_session(
        &self,
        discord_id: &str,
        level: &str,
        template_id: &str,
    ) -> Result<SessionResponse> {
        let response = self
            .client
            .post(format!("{}/api/sessions", self.base_url))
            .header("X-Discord-User-Id", discord_id)
            .json(&serde_json::json!({
                "level": level,
                "template_id": template_id
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("Ayumu API error: {}", text));
        }

        Ok(response.json().await?)
    }

    pub async fn get_profile(&self, discord_id: &str) -> Result<ProfileResponse> {
        let response = self
            .client
            .get(format!("{}/api/profile", self.base_url))
            .header("X-Discord-User-Id", discord_id)
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("Ayumu API error: {}", text));
        }

        Ok(response.json().await?)
    }

    pub async fn get_leaderboard(
        &self,
        level: Option<&str>,
        period: Option<&str>,
    ) -> Result<Vec<LeaderboardEntry>> {
        let mut url = format!("{}/api/leaderboard", self.base_url);
        let mut params = vec![];
        if let Some(l) = level {
            params.push(format!("level={}", l));
        }
        if let Some(p) = period {
            params.push(format!("period={}", p));
        }
        if !params.is_empty() {
            url.push_str(&format!("?{}", params.join("&")));
        }

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let text = response.text().await?;
            return Err(anyhow!("Ayumu API error: {}", text));
        }

        #[derive(Deserialize)]
        struct LeaderboardResponse {
            entries: Vec<LeaderboardEntry>,
        }

        let data: LeaderboardResponse = response.json().await?;
        Ok(data.entries)
    }
}
