// Novel search command - search and download light novels
// Ported from commands/downNovel.js

use poise::serenity_prelude as serenity;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use once_cell::sync::Lazy;
use tracing::{error, info};

use crate::utils::config::colors;
use crate::{Context, Error};

/// Novel entry from novelList.json
#[derive(Debug, Clone, Deserialize)]
pub struct NovelEntry {
    pub id: String,
    pub title: String,
    pub url: String,
    pub size: String,
    pub format: String,
}

/// Global novel database (loaded once at startup)
static NOVELS: Lazy<Vec<NovelEntry>> = Lazy::new(|| {
    load_novels().unwrap_or_else(|e| {
        error!("Failed to load novel database: {:?}", e);
        Vec::new()
    })
});

/// Load novels from JSON file
fn load_novels() -> Result<Vec<NovelEntry>, Box<dyn std::error::Error + Send + Sync>> {
    // Try multiple possible paths
    let paths = [
        "Yuyuko/utils/novelList.json",
        "src/data/novelList.json",
        "data/novelList.json",
    ];

    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let novels: Vec<NovelEntry> = serde_json::from_str(&content)?;
            info!("Loaded {} novels from {}", novels.len(), path);
            return Ok(novels);
        }
    }

    Err("Could not find novelList.json".into())
}

/// Get total novel count
pub fn get_novel_count() -> usize {
    NOVELS.len()
}

const PAGE_SIZE: usize = 10;

/// Search and download light novels
#[poise::command(slash_command, prefix_command)]
pub async fn novel(
    ctx: Context<'_>,
    #[description = "Judul light novel (kanji/kana/romaji)"] title: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    // Check if novels loaded
    if NOVELS.is_empty() {
        ctx.say("Gagal memuat data novel. Silakan hubungi administrator.").await?;
        return Ok(());
    }

    // Search novels
    let query = title.to_lowercase();
    let results: Vec<&NovelEntry> = NOVELS
        .iter()
        .filter(|n| n.title.to_lowercase().contains(&query))
        .collect();

    if results.is_empty() {
        ctx.say("Tidak ditemukan novel dengan judul tersebut.").await?;
        return Ok(());
    }

    let total_results = results.len();
    let total_pages = (total_results + PAGE_SIZE - 1) / PAGE_SIZE;

    // Create initial embed and buttons
    let embed = create_embed(&results, 0, total_results);
    let components = create_buttons(0, total_pages);

    let reply = ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .components(components)
    ).await?;

    // Handle button interactions
    let msg = reply.message().await?;
    let mut current_page: usize = 0;

    // Create collector for button interactions
    let mut collector = msg
        .await_component_interactions(ctx)
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(60))
        .stream();

    use futures::StreamExt;
    while let Some(interaction) = collector.next().await {
        match interaction.data.custom_id.as_str() {
            "novel_prev" => {
                if current_page > 0 {
                    current_page -= 1;
                }
            }
            "novel_next" => {
                if current_page < total_pages - 1 {
                    current_page += 1;
                }
            }
            _ => continue,
        }

        let new_embed = create_embed(&results, current_page, total_results);
        let new_components = create_buttons(current_page, total_pages);

        interaction
            .create_response(
                ctx,
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(new_embed)
                        .components(new_components)
                ),
            )
            .await?;
    }

    // Disable buttons after timeout - update the original reply
    let disabled_components = create_disabled_buttons();
    let _ = reply.edit(
        ctx,
        poise::CreateReply::default()
            .embed(create_embed(&results, current_page, total_results))
            .components(disabled_components)
    ).await;

    Ok(())
}

/// Create embed for current page
fn create_embed(results: &[&NovelEntry], page: usize, total: usize) -> serenity::CreateEmbed {
    let start = page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(results.len());
    let current_results = &results[start..end];

    let description = current_results
        .iter()
        .enumerate()
        .map(|(i, novel)| {
            format!(
                "**{}.** [{}]({})\nSize: {} • Format: {}",
                start + i + 1,
                truncate_title(&novel.title, 60),
                novel.url,
                novel.size,
                novel.format
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    serenity::CreateEmbed::new()
        .title("Hasil Pencarian Light Novel")
        .description(description)
        .color(colors::INFO)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Menampilkan {}-{} dari {}",
            start + 1,
            end,
            total
        )))
        .timestamp(serenity::Timestamp::now())
}

/// Truncate title if too long
fn truncate_title(title: &str, max_len: usize) -> String {
    if title.chars().count() <= max_len {
        title.to_string()
    } else {
        format!("{}...", title.chars().take(max_len - 3).collect::<String>())
    }
}

/// Create navigation buttons
fn create_buttons(current_page: usize, total_pages: usize) -> Vec<serenity::CreateActionRow> {
    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new("novel_prev")
            .label("⬅️ Prev")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(current_page == 0),
        serenity::CreateButton::new("novel_next")
            .label("Next ➡️")
            .style(serenity::ButtonStyle::Primary)
            .disabled(current_page >= total_pages - 1),
    ])]
}

/// Create disabled buttons (after timeout)
fn create_disabled_buttons() -> Vec<serenity::CreateActionRow> {
    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new("novel_prev")
            .label("⬅️ Prev")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(true),
        serenity::CreateButton::new("novel_next")
            .label("Next ➡️")
            .style(serenity::ButtonStyle::Primary)
            .disabled(true),
    ])]
}
