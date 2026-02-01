// VNDB API client
// For visual novel metadata

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// VNDB visual novel info
#[derive(Debug, Clone)]
pub struct VnInfo {
    pub id: String,
    pub title: String,
    pub image: Option<String>,
    pub url: String,
    pub developer: Option<String>,
    pub released: Option<String>,
    pub length: Option<i32>,
    pub description: Option<String>,
}

/// Search for visual novels on VNDB
pub async fn search_vns(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<VnInfo>> {
    let request = VndbRequest {
        filters: vec!["search".to_string(), "=".to_string(), query.to_string()],
        fields: "id, title, image.url, released, length, developers.name".to_string(),
        results: limit.min(25) as i32,
    };

    let response = client
        .post("https://api.vndb.org/kana/vn")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let data: VndbResponse = response.json().await?;

    let results = data
        .results
        .into_iter()
        .map(|v| VnInfo {
            id: v.id.clone(),
            title: v.title,
            image: v.image.map(|i| i.url),
            url: format!("https://vndb.org/{}", v.id),
            developer: v.developers.first().map(|d| d.name.clone()),
            released: v.released,
            length: v.length,
            description: None,
        })
        .collect();

    Ok(results)
}

/// Get visual novel info by ID
pub async fn get_vn_by_id(client: &reqwest::Client, id: &str) -> Result<Option<VnInfo>> {
    let request = VndbRequest {
        filters: vec!["id".to_string(), "=".to_string(), id.to_string()],
        fields: "id, title, image.url, released, length, developers.name, description".to_string(),
        results: 1,
    };

    let response = client
        .post("https://api.vndb.org/kana/vn")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let data: VndbResponse = response.json().await?;

    if let Some(v) = data.results.first() {
        Ok(Some(VnInfo {
            id: v.id.clone(),
            title: v.title.clone(),
            image: v.image.as_ref().map(|i| i.url.clone()),
            url: format!("https://vndb.org/{}", v.id),
            developer: v.developers.first().map(|d| d.name.clone()),
            released: v.released.clone(),
            length: v.length,
            description: v.description.clone(),
        }))
    } else {
        Ok(None)
    }
}

// Request/Response structures
#[derive(Debug, Serialize)]
struct VndbRequest {
    filters: Vec<String>,
    fields: String,
    results: i32,
}

#[derive(Debug, Deserialize)]
struct VndbResponse {
    results: Vec<VndbVn>,
}

#[derive(Debug, Deserialize)]
struct VndbVn {
    id: String,
    title: String,
    image: Option<VndbImage>,
    released: Option<String>,
    length: Option<i32>,
    developers: Vec<VndbDeveloper>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VndbImage {
    url: String,
}

#[derive(Debug, Deserialize)]
struct VndbDeveloper {
    name: String,
}
