// Novel search command — search via Anna's Archive, direct download via Libgen.li
// Download links: libgen.li/get.php?md5={md5} — no session/timer needed, instant redirect

use futures::StreamExt;
use once_cell::sync::Lazy;
use poise::serenity_prelude as serenity;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

use crate::utils::config::colors;
use crate::{Context, Error};

const ANNAS_BASE_URL: &str = "https://annas-archive.gl";
const PAGE_SIZE: usize = 10;

// ── Structs ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnnaResult {
    pub title: String,
    pub author: Option<String>,
    pub format: Option<String>,
    pub size: Option<String>,
    pub detail_url: String,
}

// ── Search ────────────────────────────────────────────────────────

pub async fn search_annas_archive(
    query: &str,
) -> Result<Vec<AnnaResult>, Box<dyn std::error::Error + Send + Sync>> {
    let encoded = urlencoding::encode(query);
    let url = format!("{}/search?q={}&lang=ja&ext=epub", ANNAS_BASE_URL, encoded);
    info!("Searching Anna's Archive: {}", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()?;

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }
    parse_search_results(&resp.text().await?)
}

fn parse_search_results(
    html: &str,
) -> Result<Vec<AnnaResult>, Box<dyn std::error::Error + Send + Sync>> {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("a.js-vim-focus").unwrap();
    let icon_sel = Selector::parse("span[class*='icon-[mdi--user-edit]']").unwrap();

    let mut results = Vec::new();

    for title_el in doc.select(&title_sel) {
        let href = match title_el.value().attr("href") {
            Some(h) if h.starts_with("/md5/") => h,
            _ => continue,
        };

        let md5 = href.trim_start_matches("/md5/").to_string();
        let title = title_el.text().collect::<String>().trim().to_string();
        if title.is_empty() || md5.is_empty() {
            continue;
        }

        let detail_url = format!("{}{}", ANNAS_BASE_URL, href);
        let mut author = None;
        let mut format = None;
        let mut size = None;

        // Walk up to the result card container
        if let Some(parent) = title_el.parent() {
            if let Some(gp) = parent.parent() {
                if let Some(container) = scraper::ElementRef::wrap(gp) {
                    // Author
                    for icon in container.select(&icon_sel) {
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
                    for line in container.text().collect::<String>().lines() {
                        let t = line.trim();
                        if t.contains(" · ")
                            && (t.contains("Japanese") || t.contains("[ja]"))
                        {
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

        // Only include epub and pdf
        let is_valid = format
            .as_deref()
            .map(|f| f == "EPUB" || f == "PDF")
            .unwrap_or(false);
        if !is_valid {
            continue;
        }

        results.push(AnnaResult {
            title,
            author,
            format,
            size,
            detail_url,
        });
    }

    info!("Parsed {} results (epub/pdf only)", results.len());
    Ok(results)
}

// ── Local fallback: novelList.json ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct NovelEntry {
    #[allow(dead_code)]
    pub id: String,
    pub title: String,
    pub url: String,
    pub size: String,
    pub format: String,
}

static NOVELS: Lazy<Vec<NovelEntry>> = Lazy::new(|| {
    let paths = [
        "Ayumi/utils/novelList.json",
        "src/data/novelList.json",
        "data/novelList.json",
    ];
    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(novels) = serde_json::from_str::<Vec<NovelEntry>>(&content) {
                info!("Loaded {} local novels from {}", novels.len(), path);
                return novels;
            }
        }
    }
    warn!("novelList.json not found — local fallback unavailable");
    Vec::new()
});

fn search_local(query: &str) -> Vec<AnnaResult> {
    let q = query.to_lowercase();
    NOVELS
        .iter()
        .filter(|n| n.title.to_lowercase().contains(&q))
        .map(|n| AnnaResult {
            title: n.title.clone(),
            author: None,
            format: Some(n.format.clone()),
            size: Some(n.size.clone()),
            detail_url: n.url.clone(),
        })
        .collect()
}

// ── Slash command ─────────────────────────────────────────────────

/// Cari dan download light novel via Anna's Archive
#[poise::command(slash_command, prefix_command)]
pub async fn novel(
    ctx: Context<'_>,
    #[description = "Judul light novel (kanji/kana/romaji)"] title: String,
) -> Result<(), Error> {
    info!("Novel command by {} — query: {}", ctx.author().id, title);
    ctx.defer().await?;

    let (results, source) = match search_annas_archive(&title).await {
        Ok(r) if !r.is_empty() => (r, "Anna's Archive"),
        Ok(_) => {
            let local = search_local(&title);
            if local.is_empty() {
                ctx.say("Tidak ditemukan novel dengan judul tersebut.").await?;
                return Ok(());
            }
            (local, "Local Database")
        }
        Err(e) => {
            warn!("Anna's Archive error: {:?}", e);
            let local = search_local(&title);
            if local.is_empty() {
                ctx.say("Anna's Archive tidak dapat dijangkau dan tidak ada hasil di database lokal.")
                    .await?;
                return Ok(());
            }
            (local, "Local Database")
        }
    };

    let total = results.len();
    let total_pages = (total + PAGE_SIZE - 1) / PAGE_SIZE;

    let reply = ctx
        .send(
            poise::CreateReply::default()
                .embed(build_embed(&results, 0, total, source))
                .components(nav_buttons(0, total_pages)),
        )
        .await?;

    // Pagination — no async work during button handling, so interactions complete instantly
    let msg = reply.message().await?;
    let mut current_page: usize = 0;

    let mut collector = msg
        .await_component_interactions(ctx)
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(120))
        .stream();

    while let Some(interaction) = collector.next().await {
        match interaction.data.custom_id.as_str() {
            "novel_prev" if current_page > 0 => current_page -= 1,
            "novel_next" if current_page < total_pages - 1 => current_page += 1,
            _ => continue,
        }

        interaction
            .create_response(
                ctx,
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(build_embed(&results, current_page, total, source))
                        .components(nav_buttons(current_page, total_pages)),
                ),
            )
            .await?;
    }

    // Disable buttons on timeout
    let _ = reply
        .edit(
            ctx,
            poise::CreateReply::default()
                .embed(build_embed(&results, current_page, total, source))
                .components(disabled_buttons()),
        )
        .await;

    Ok(())
}

// ── Embed & buttons ───────────────────────────────────────────────

fn build_embed(
    results: &[AnnaResult],
    page: usize,
    total: usize,
    source: &str,
) -> serenity::CreateEmbed {
    let start = page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(results.len());

    let description = results[start..end]
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let author = r.author.as_deref().unwrap_or("Unknown");
            let fmt = r.format.as_deref().unwrap_or("?");
            let sz = r.size.as_deref().unwrap_or("?");

            format!(
                "**{}.** [Link]({})\n{}\nAuthor: {} | Format: {} | Size: {}",
                start + i + 1,
                r.detail_url,
                truncate(&r.title, 60),
                author,
                fmt,
                sz,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    serenity::CreateEmbed::new()
        .title("Light Novel Search")
        .description(description)
        .color(colors::INFO)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Source: {} | {}-{} of {}",
            source,
            start + 1,
            end,
            total
        )))
        .timestamp(serenity::Timestamp::now())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    }
}

fn nav_buttons(page: usize, total_pages: usize) -> Vec<serenity::CreateActionRow> {
    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new("novel_prev")
            .label("< Prev")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(page == 0),
        serenity::CreateButton::new("novel_next")
            .label("Next >")
            .style(serenity::ButtonStyle::Primary)
            .disabled(page >= total_pages - 1),
    ])]
}

fn disabled_buttons() -> Vec<serenity::CreateActionRow> {
    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new("novel_prev")
            .label("< Prev")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(true),
        serenity::CreateButton::new("novel_next")
            .label("Next >")
            .style(serenity::ButtonStyle::Primary)
            .disabled(true),
    ])]
}
