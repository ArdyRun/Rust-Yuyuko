// AniList GraphQL API client
// For anime/manga metadata

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Media type for AniList
#[derive(Debug, Clone, Copy)]
pub enum MediaType {
    Anime,
    Manga,
}

impl MediaType {
    fn as_str(&self) -> &'static str {
        match self {
            MediaType::Anime => "ANIME",
            MediaType::Manga => "MANGA",
        }
    }
}

/// AniList media info
#[derive(Debug, Clone)]
pub struct AniListMedia {
    pub id: i32,
    pub title: String,
    pub title_romaji: Option<String>,
    pub image: Option<String>,
    pub url: String,
}

/// Search for media on AniList
pub async fn search_media(
    client: &reqwest::Client,
    query: &str,
    media_type: MediaType,
    limit: usize,
) -> Result<Vec<AniListMedia>> {
    let graphql_query = r#"
        query ($search: String, $type: MediaType) {
            Page(perPage: 25) {
                media(search: $search, type: $type) {
                    id
                    title {
                        romaji
                        english
                        native
                    }
                    coverImage {
                        large
                    }
                    siteUrl
                }
            }
        }
    "#;

    let variables = serde_json::json!({
        "search": query,
        "type": media_type.as_str()
    });

    let response = client
        .post("https://graphql.anilist.co")
        .json(&GraphQLRequest {
            query: graphql_query.to_string(),
            variables,
        })
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("ERROR: AniList API error: status={}, body={}", status, body);
        return Ok(vec![]);
    }

    let data: AniListResponse = response.json().await?;
    
    let results = data
        .data
        .page
        .media
        .into_iter()
        .take(limit)
        .map(|m| AniListMedia {
            id: m.id,
            title: m.title.english
                .or(m.title.romaji.clone())
                .or(m.title.native)
                .unwrap_or_else(|| "Unknown".to_string()),
            title_romaji: m.title.romaji,
            image: m.cover_image.map(|c| c.large),
            url: m.site_url,
        })
        .collect();

    Ok(results)
}

/// Get media info by ID
pub async fn get_media_by_id(
    client: &reqwest::Client,
    id: i32,
    media_type: MediaType,
) -> Result<Option<AniListMedia>> {
    let graphql_query = r#"
        query ($id: Int, $type: MediaType) {
            Media(id: $id, type: $type) {
                id
                title {
                    romaji
                    english
                    native
                }
                coverImage {
                    large
                }
                siteUrl
            }
        }
    "#;

    let variables = serde_json::json!({
        "id": id,
        "type": media_type.as_str()
    });

    let response = client
        .post("https://graphql.anilist.co")
        .json(&GraphQLRequest {
            query: graphql_query.to_string(),
            variables,
        })
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let data: AniListSingleResponse = response.json().await?;
    
    if let Some(m) = data.data.media {
        Ok(Some(AniListMedia {
            id: m.id,
            title: m.title.english
                .or(m.title.romaji.clone())
                .or(m.title.native)
                .unwrap_or_else(|| "Unknown".to_string()),
            title_romaji: m.title.romaji,
            image: m.cover_image.map(|c| c.large),
            url: m.site_url,
        }))
    } else {
        Ok(None)
    }
}

// Request/Response structures
#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AniListResponse {
    data: AniListData,
}

#[derive(Debug, Deserialize)]
struct AniListData {
    #[serde(rename = "Page")]
    page: AniListPage,
}

#[derive(Debug, Deserialize)]
struct AniListPage {
    media: Vec<AniListMediaItem>,
}

#[derive(Debug, Deserialize)]
struct AniListSingleResponse {
    data: AniListSingleData,
}

#[derive(Debug, Deserialize)]
struct AniListSingleData {
    #[serde(rename = "Media")]
    media: Option<AniListMediaItem>,
}

#[derive(Debug, Deserialize)]
struct AniListMediaItem {
    id: i32,
    title: AniListTitle,
    #[serde(rename = "coverImage")]
    cover_image: Option<AniListCoverImage>,
    #[serde(rename = "siteUrl")]
    site_url: String,
}

#[derive(Debug, Deserialize)]
struct AniListTitle {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AniListCoverImage {
    large: String,
}
