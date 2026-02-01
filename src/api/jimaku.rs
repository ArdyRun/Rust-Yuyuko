// Jimaku.cc API client
// For searching and downloading anime subtitles

use anyhow::Result;
use serde::Deserialize;

pub const JIMAKU_API_BASE: &str = "https://jimaku.cc/api";

#[derive(Debug, Clone, Deserialize)]
pub struct JimakuEntry {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
    pub english_name: Option<String>,
    pub japanese_name: Option<String>,
    pub anilist_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JimakuFile {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub last_modified: String,
}

pub async fn search_anime(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
) -> Result<Vec<JimakuEntry>> {
    let response = client
        .get(format!("{}/entries/search", JIMAKU_API_BASE))
        .header("Authorization", api_key)
        .query(&[("query", query), ("anime", "true")])
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let entries: Vec<JimakuEntry> = response.json().await?;
    Ok(entries)
}

pub async fn get_entry(
    client: &reqwest::Client,
    api_key: &str,
    entry_id: i32,
) -> Result<Option<JimakuEntry>> {
    let response = client
        .get(format!("{}/entries/{}", JIMAKU_API_BASE, entry_id))
        .header("Authorization", api_key)
        .send()
        .await?;

    if response.status() == 404 {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Ok(None);
    }

    let entry: JimakuEntry = response.json().await?;
    Ok(Some(entry))
}

pub async fn get_files(
    client: &reqwest::Client,
    api_key: &str,
    entry_id: i32,
    episode: Option<i32>,
) -> Result<Vec<JimakuFile>> {
    let mut request = client
        .get(format!("{}/entries/{}/files", JIMAKU_API_BASE, entry_id))
        .header("Authorization", api_key);

    if let Some(ep) = episode {
        request = request.query(&[("episode", ep)]);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let files: Vec<JimakuFile> = response.json().await?;
    Ok(files)
}

pub async fn download_file(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}
