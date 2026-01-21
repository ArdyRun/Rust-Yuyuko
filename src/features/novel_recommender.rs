use serde::{Deserialize, Serialize};
use rand::prelude::IndexedRandom;
use std::sync::OnceLock;
use tracing::{error, info, debug};
use unicode_normalization::UnicodeNormalization;

use crate::Data;
use crate::api::llm::completion_gemini;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Novel {
    pub id: String,
    pub title: String,
    pub url: String,
    pub size: String,
    pub format: String,
}

static NOVELS: OnceLock<Vec<Novel>> = OnceLock::new();

/// Load novels from JSON file (Lazy loaded)
pub fn get_novels() -> &'static [Novel] {
    NOVELS.get_or_init(|| {
        let paths = [
            "Yuyuko/utils/novelList.json",
            "src/data/novelList.json",
            "data/novelList.json",
        ];
        
        for path in &paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                match serde_json::from_str::<Vec<Novel>>(&content) {
                    Ok(novels) => {
                        info!("Novel recommender loaded {} novels from {}", novels.len(), path);
                        return novels;
                    },
                    Err(e) => {
                        error!("Failed to parse {}: {:?}", path, e);
                    }
                }
            }
        }
        
        error!("Failed to load novelList.json from any path");
        Vec::new()
    })
}

/// Normalize string for matching (lowercase, no diacritics, no punctuation)
fn normalize_string(s: &str) -> String {
    s.nfd()
        .filter(|c| !c.is_ascii_punctuation() && !matches!(c, '\u{0300}'..='\u{036f}'))
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detect JLPT level from user message
fn detect_jlpt_level(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    
    if lower.contains("n5") || lower.contains("pemula") || lower.contains("beginner") {
        Some("N5 (beginner)")
    } else if lower.contains("n4") || lower.contains("elementary") {
        Some("N4 (elementary)")
    } else if lower.contains("n3") || lower.contains("menengah") || lower.contains("intermediate") {
        Some("N3 (intermediate)")
    } else if lower.contains("n2") || lower.contains("upper") {
        Some("N2 (upper intermediate)")
    } else if lower.contains("n1") || lower.contains("advanced") || lower.contains("mahir") {
        Some("N1 (advanced)")
    } else {
        None
    }
}

/// Detect genre from user message
fn detect_genre(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    
    let genres = [
        ("romance", "romance"),
        ("romantic", "romance"),
        ("cinta", "romance"),
        ("yuri", "yuri"),
        ("yaoi", "yaoi"),
        ("isekai", "isekai"),
        ("fantasy", "fantasy"),
        ("fantasi", "fantasy"),
        ("sci-fi", "sci-fi"),
        ("scifi", "sci-fi"),
        ("action", "action"),
        ("aksi", "action"),
        ("adventure", "adventure"),
        ("petualangan", "adventure"),
        ("comedy", "comedy"),
        ("komedi", "comedy"),
        ("horror", "horror"),
        ("horor", "horror"),
        ("mystery", "mystery"),
        ("misteri", "mystery"),
        ("slice of life", "slice of life"),
        ("drama", "drama"),
        ("psychological", "psychological"),
        ("supernatural", "supernatural"),
    ];
    
    for (keyword, genre) in genres {
        if lower.contains(keyword) {
            return Some(genre);
        }
    }
    None
}

/// Random recommendation (fallback)
pub fn recommend_novels(count: usize) -> String {
    let novels = get_novels();
    if novels.is_empty() {
        return "Maaf, aku belum menemukan daftar novelnya... Sepertinya ada yang salah.".to_string();
    }

    let mut rng = rand::rng();
    let selected: Vec<_> = novels.choose_multiple(&mut rng, count).collect();

    let mut response = "**Rekomendasi Novel untukmu:**\n\n".to_string();
    for (i, novel) in selected.iter().enumerate() {
        response.push_str(&format!("{}. [{}]({})\n   Format: {} | Size: {}\n\n", 
            i + 1, novel.title, novel.url, novel.format, novel.size));
    }
    
    response.push_str("Semoga suka ya! Jangan lupa baca~");
    response
}

/// Smart novel search using LLM to get suggestions
pub async fn smart_novel_search(data: &Data, query: &str) -> String {
    let novels = get_novels();
    if novels.is_empty() {
        return "Maaf, database novel belum tersedia.".to_string();
    }

    // Detect JLPT level or genre
    let level = detect_jlpt_level(query);
    let genre = detect_genre(query);
    
    debug!("Smart novel search - Level: {:?}, Genre: {:?}", level, genre);

    // Build LLM prompt based on detected intent
    let prompt = if let Some(lvl) = level {
        format!(
            "Suggest 5 popular Japanese light novel titles that are appropriate for {} level learners. Only respond with the titles in Japanese, one per line, no additional text or numbering.",
            lvl
        )
    } else if let Some(g) = genre {
        format!(
            "Suggest 5 popular Japanese light novel titles in the {} genre. Only respond with the titles in Japanese, one per line, no additional text or numbering.",
            g
        )
    } else {
        // Check if it looks like a title search
        format!(
            "What is the original Japanese title for the light novel or anime '{}'? Only respond with the Japanese title, no additional text.",
            query
        )
    };

    // Call LLM for suggestions
    let suggested_titles = match completion_gemini(data, &prompt).await {
        Ok(response) => {
            response
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with(|c: char| c.is_ascii_digit()))
                .map(|l| l.trim_start_matches(|c: char| c == '-' || c == '.' || c == ' '))
                .map(|l| l.trim_matches(|c: char| c == '"' || c == '\'' || c == '「' || c == '」'))
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        }
        Err(e) => {
            error!("LLM suggestion failed: {:?}", e);
            return recommend_novels(5); // Fallback to random
        }
    };

    debug!("LLM suggested titles: {:?}", suggested_titles);

    // Match suggested titles with database
    let normalized_suggestions: Vec<String> = suggested_titles.iter()
        .map(|t| normalize_string(t))
        .collect();

    let mut results: Vec<&Novel> = novels.iter()
        .filter(|novel| {
            let norm_title = normalize_string(&novel.title);
            normalized_suggestions.iter().any(|s| norm_title.contains(s) || s.contains(&norm_title))
        })
        .take(10)
        .collect();

    // If no matches from LLM, try direct search
    if results.is_empty() {
        let norm_query = normalize_string(query);
        results = novels.iter()
            .filter(|novel| normalize_string(&novel.title).contains(&norm_query))
            .take(10)
            .collect();
    }

    // Still no results? Random fallback
    if results.is_empty() {
        return format!(
            "Tidak ada novel yang cocok dengan pencarian '{}'. Berikut rekomendasi acak:\n\n{}",
            query,
            recommend_novels(5)
        );
    }

    // Build response
    let title = if level.is_some() {
        format!("**Rekomendasi Novel untuk Level {}:**\n\n", level.unwrap())
    } else if genre.is_some() {
        format!("**Rekomendasi Novel Genre {}:**\n\n", genre.unwrap())
    } else {
        format!("**Hasil Pencarian '{}':**\n\n", query)
    };

    let mut response = title;
    for (i, novel) in results.iter().enumerate() {
        response.push_str(&format!(
            "{}. [{}]({})\n   Format: {} | Size: {}\n\n",
            i + 1, novel.title, novel.url, novel.format, novel.size
        ));
    }

    response.push_str("Semoga suka ya! Jangan lupa baca~");
    response
}
