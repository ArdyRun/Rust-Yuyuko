use rand::prelude::IndexedRandom;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use unicode_normalization::UnicodeNormalization;

use crate::api::llm::completion_gemini;
use crate::Data;

const ANNAS_BASE_URL: &str = "https://annas-archive.gl";

// ── Local novel database ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Novel {
    pub id: String,
    pub title: String,
    pub url: String,
    pub size: String,
    pub format: String,
}

static NOVELS: OnceLock<Vec<Novel>> = OnceLock::new();

pub fn get_novels() -> &'static [Novel] {
    NOVELS.get_or_init(|| {
        let paths = [
            "Ayumi/utils/novelList.json",
            "src/data/novelList.json",
            "data/novelList.json",
        ];

        for path in &paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                match serde_json::from_str::<Vec<Novel>>(&content) {
                    Ok(novels) => {
                        info!(
                            "Novel recommender loaded {} novels from {}",
                            novels.len(),
                            path
                        );
                        return novels;
                    }
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

// ── Anna's Archive integration ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnnaNovelResult {
    pub title: String,
    pub author: Option<String>,
    pub format: Option<String>,
    pub size: Option<String>,
    pub url: String,
}

/// Search Anna's Archive for novels
async fn search_annas(query: &str) -> Option<Vec<AnnaNovelResult>> {
    let encoded = urlencoding::encode(query);
    let url = format!("{}/search?q={}&lang=ja", ANNAS_BASE_URL, encoded);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .ok()?;

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let html = resp.text().await.ok()?;
    let document = Html::parse_document(&html);
    let title_sel = Selector::parse("a.js-vim-focus").ok()?;
    let author_icon_sel = Selector::parse("span[class*='icon-[mdi--user-edit]']").ok()?;

    let mut results = Vec::new();

    for title_el in document.select(&title_sel) {
        let href = match title_el.value().attr("href") {
            Some(h) if h.starts_with("/md5/") => h,
            _ => continue,
        };

        let title = title_el.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let full_url = format!("{}{}", ANNAS_BASE_URL, href);
        let mut author = None;
        let mut format = None;
        let mut size = None;

        if let Some(parent) = title_el.parent() {
            if let Some(gp) = parent.parent() {
                if let Some(container) = scraper::ElementRef::wrap(gp) {
                    // Author
                    for icon in container.select(&author_icon_sel) {
                        if let Some(p) = icon.parent() {
                            if let Some(el) = scraper::ElementRef::wrap(p) {
                                let txt = el.text().collect::<String>().trim().to_string();
                                if !txt.is_empty() {
                                    author = Some(txt);
                                    break;
                                }
                            }
                        }
                    }

                    // Metadata line: "Japanese [ja] · PDF · 86.7MB · 2024"
                    let all_text = container.text().collect::<String>();
                    for line in all_text.lines() {
                        let t = line.trim();
                        if t.contains(" · ") && (t.contains("Japanese") || t.contains("[ja]")) {
                            let parts: Vec<&str> = t.split(" · ").collect();
                            if parts.len() >= 3 {
                                format = Some(parts[1].trim().to_uppercase());
                                size = Some(parts[2].trim().to_string());
                            }
                            break;
                        }
                    }
                }
            }
        }

        results.push(AnnaNovelResult {
            title,
            author,
            format,
            size,
            url: full_url,
        });
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

// ── Helpers ───────────────────────────────────────────────────────

fn normalize_string(s: &str) -> String {
    s.nfd()
        .filter(|c| !c.is_ascii_punctuation() && !matches!(c, '\u{0300}'..='\u{036f}'))
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

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

// ── Public API ────────────────────────────────────────────────────

/// Random recommendation from local database (fallback)
pub fn recommend_novels(count: usize) -> String {
    let novels = get_novels();
    if novels.is_empty() {
        return "Maaf, database novel kosong.".to_string();
    }

    let mut rng = rand::rng();
    let selected: Vec<_> = novels.choose_multiple(&mut rng, count).collect();

    let mut response = "**Rekomendasi Novel untukmu:**\n\n".to_string();
    for (i, novel) in selected.iter().enumerate() {
        response.push_str(&format!(
            "{}. [{}]({})\n   Format: {} | Size: {}\n\n",
            i + 1,
            novel.title,
            novel.url,
            novel.format,
            novel.size
        ));
    }

    response.push_str("Semoga suka ya! Jangan lupa baca~");
    response
}

/// Smart novel search — uses Anna's Archive first, then local DB
pub async fn smart_novel_search(data: &Data, query: &str) -> String {
    // Detect JLPT level or genre
    let level = detect_jlpt_level(query);
    let genre = detect_genre(query);

    debug!(
        "Smart novel search — Level: {:?}, Genre: {:?}",
        level, genre
    );

    // Build LLM prompt for title resolution
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
        format!(
            "What is the original Japanese title for the light novel or anime '{}'? Only respond with the Japanese title, no additional text.",
            query
        )
    };

    // Get LLM suggestions
    let suggested_titles = match completion_gemini(data, &prompt).await {
        Ok(response) => response
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with(|c: char| c.is_ascii_digit()))
            .map(|l| l.trim_start_matches(|c: char| c == '-' || c == '.' || c == ' '))
            .map(|l| l.trim_matches(|c: char| c == '"' || c == '\'' || c == '「' || c == '」'))
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        Err(e) => {
            error!("LLM suggestion failed: {:?}", e);
            return recommend_novels(5);
        }
    };

    debug!("LLM suggested titles: {:?}", suggested_titles);

    // Try Anna's Archive for each suggested title
    let mut anna_results: Vec<AnnaNovelResult> = Vec::new();

    for title in &suggested_titles {
        if anna_results.len() >= 10 {
            break;
        }
        match search_annas(title).await {
            Some(mut results) => {
                let take = (10 - anna_results.len()).min(results.len());
                anna_results.extend(results.drain(..take));
            }
            None => {
                debug!("No Anna's Archive results for: {}", title);
            }
        }
    }

    // If Anna's Archive has results, use them
    if !anna_results.is_empty() {
        let title_str = if let Some(lvl) = level {
            format!(
                "**Rekomendasi Novel untuk Level {} (via Anna's Archive):**\n\n",
                lvl
            )
        } else if let Some(g) = genre {
            format!(
                "**Rekomendasi Novel Genre {} (via Anna's Archive):**\n\n",
                g
            )
        } else {
            format!("**Hasil Pencarian '{}' (via Anna's Archive):**\n\n", query)
        };

        let mut response = title_str;
        for (i, r) in anna_results.iter().enumerate() {
            let author = r.author.as_deref().unwrap_or("Unknown");
            let fmt = r.format.as_deref().unwrap_or("?");
            let sz = r.size.as_deref().unwrap_or("?");
            response.push_str(&format!(
                "{}. [{}]({})\n   ✍️ {} | 📄 {} | 💾 {}\n\n",
                i + 1,
                r.title,
                r.url,
                author,
                fmt,
                sz
            ));
        }
        response.push_str("Semoga suka ya! Jangan lupa baca~");
        return response;
    }

    // Fallback: search local database
    warn!("Anna's Archive returned no results, falling back to local DB");
    let novels = get_novels();
    if novels.is_empty() {
        return "Maaf, database novel belum tersedia.".to_string();
    }

    // Match LLM suggestions against local DB
    let normalized_suggestions: Vec<String> = suggested_titles
        .iter()
        .map(|t| normalize_string(t))
        .collect();

    let mut results: Vec<&Novel> = novels
        .iter()
        .filter(|novel| {
            let norm_title = normalize_string(&novel.title);
            normalized_suggestions
                .iter()
                .any(|s| norm_title.contains(s) || s.contains(&norm_title))
        })
        .take(10)
        .collect();

    // Direct search if no LLM match
    if results.is_empty() {
        let norm_query = normalize_string(query);
        results = novels
            .iter()
            .filter(|novel| normalize_string(&novel.title).contains(&norm_query))
            .take(10)
            .collect();
    }

    if results.is_empty() {
        return format!(
            "Tidak ada novel yang cocok dengan '{}'. Berikut rekomendasi acak:\n\n{}",
            query,
            recommend_novels(5)
        );
    }

    let title_str = if level.is_some() {
        format!("**Rekomendasi Novel untuk Level {}:**\n\n", level.unwrap())
    } else if genre.is_some() {
        format!("**Rekomendasi Novel Genre {}:**\n\n", genre.unwrap())
    } else {
        format!("**Hasil Pencarian '{}':**\n\n", query)
    };

    let mut response = title_str;
    for (i, novel) in results.iter().enumerate() {
        response.push_str(&format!(
            "{}. [{}]({})\n   Format: {} | Size: {}\n\n",
            i + 1,
            novel.title,
            novel.url,
            novel.format,
            novel.size
        ));
    }

    response.push_str("Semoga suka ya! Jangan lupa baca~");
    response
}
