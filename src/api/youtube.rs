// YouTube Data API client
// For fetching video metadata

use anyhow::Result;
use serde::Deserialize;

/// YouTube video information
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub title: String,
    pub duration_seconds: i32,
    pub thumbnail: Option<String>,
    #[allow(dead_code)]
    pub channel: String,
}

/// Extract video ID from YouTube URL or direct ID
pub fn extract_video_id(input: &str) -> Option<String> {
    // Direct video ID (11 characters)
    if input.len() == 11
        && input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some(input.to_string());
    }

    // youtube.com/watch?v=xxx
    if input.contains("youtube.com/watch") {
        if let Some(v_param) = input.split("v=").nth(1) {
            let id = v_param.split('&').next().unwrap_or(v_param);
            if id.len() >= 11 {
                return Some(id[..11].to_string());
            }
        }
    }

    // youtu.be/xxx
    if input.contains("youtu.be/") {
        if let Some(path) = input.split("youtu.be/").nth(1) {
            let id = path.split(['?', '&', '/'].as_ref()).next().unwrap_or(path);
            if id.len() >= 11 {
                return Some(id[..11].to_string());
            }
        }
    }

    None
}

/// Normalize YouTube URL to standard format
pub fn normalize_url(video_id: &str) -> String {
    format!("https://youtube.com/watch?v={}", video_id)
}

/// Fetch video info from YouTube API
pub async fn get_video_info(
    client: &reqwest::Client,
    api_key: &str,
    video_id: &str,
) -> Result<Option<VideoInfo>> {
    let url = format!(
        "https://www.googleapis.com/youtube/v3/videos?part=snippet,contentDetails&id={}&key={}",
        video_id, api_key
    );

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let data: YouTubeResponse = response.json().await?;

    if data.items.is_empty() {
        return Ok(None);
    }

    let item = &data.items[0];
    let duration = parse_iso8601_duration(&item.content_details.duration);

    Ok(Some(VideoInfo {
        title: item.snippet.title.clone(),
        duration_seconds: duration,
        thumbnail: item
            .snippet
            .thumbnails
            .get("high")
            .or_else(|| item.snippet.thumbnails.get("medium"))
            .or_else(|| item.snippet.thumbnails.get("default"))
            .map(|t| t.url.clone()),
        channel: item.snippet.channel_title.clone(),
    }))
}

/// Parse ISO 8601 duration (PT1H30M45S) to seconds
fn parse_iso8601_duration(duration: &str) -> i32 {
    let mut seconds = 0;
    let mut current_num = String::new();

    for c in duration.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else {
            if let Ok(num) = current_num.parse::<i32>() {
                match c {
                    'H' => seconds += num * 3600,
                    'M' => seconds += num * 60,
                    'S' => seconds += num,
                    _ => {}
                }
            }
            current_num.clear();
        }
    }

    seconds
}

// YouTube API response structures
#[derive(Debug, Deserialize)]
struct YouTubeResponse {
    items: Vec<YouTubeVideoItem>,
}

#[derive(Debug, Deserialize)]
struct YouTubeVideoItem {
    snippet: YouTubeSnippet,
    #[serde(rename = "contentDetails")]
    content_details: YouTubeContentDetails,
}

#[derive(Debug, Deserialize)]
struct YouTubeSnippet {
    title: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    thumbnails: std::collections::HashMap<String, YouTubeThumbnail>,
}

#[derive(Debug, Deserialize)]
struct YouTubeContentDetails {
    duration: String,
}

#[derive(Debug, Deserialize)]
struct YouTubeThumbnail {
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_video_id() {
        assert_eq!(
            extract_video_id("dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_iso8601_duration("PT1H30M45S"), 5445);
        assert_eq!(parse_iso8601_duration("PT10M"), 600);
        assert_eq!(parse_iso8601_duration("PT45S"), 45);
    }
}
