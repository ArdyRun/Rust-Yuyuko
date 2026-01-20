// Subtitle download command - download anime subtitles from Jimaku
// Ported from commands/downSubs.js

use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};

use crate::api::jimaku::{search_anime, get_entry, get_files, download_file};
use crate::api::anilist::{search_media, MediaType};
use crate::{Context, Error};

const MAX_FILE_SIZE: u64 = 8 * 1024 * 1024; // 8MB Discord limit

/// Download anime subtitles from Jimaku
#[poise::command(slash_command, prefix_command)]
pub async fn subs(
    ctx: Context<'_>,
    #[description = "Anime name or Jimaku ID"]
    #[autocomplete = "autocomplete_anime"]
    name: String,
    #[description = "Episode number (optional)"]
    episode: Option<i32>,
) -> Result<(), Error> {
    let api_key = match env::var("JIMAKU_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            ctx.say("Jimaku API Key not configured!").await?;
            return Ok(());
        }
    };

    ctx.defer().await?;

    let http_client = &ctx.data().http_client;

    // Determine if input is an ID or search term
    let entry_id: i32 = if name.chars().all(|c| c.is_ascii_digit()) {
        name.parse().unwrap_or(0)
    } else {
        // Search for the anime
        let results = search_anime(http_client, &api_key, &name).await?;
        
        if results.is_empty() {
            ctx.say(format!("No anime found with keyword: **{}**", name)).await?;
            return Ok(());
        }

        results[0].id
    };

    // Get entry info
    let entry = match get_entry(http_client, &api_key, entry_id).await? {
        Some(e) => e,
        None => {
            ctx.say("Anime not found!").await?;
            return Ok(());
        }
    };

    // Get anime cover image from AniList if available
    let cover_image: Option<String> = if entry.anilist_id.is_some() {
        match search_media(http_client, &entry.name, MediaType::Anime, 1).await {
            Ok(results) if !results.is_empty() => results[0].image.clone(),
            _ => None,
        }
    } else {
        None
    };

    // Get files
    let files = get_files(http_client, &api_key, entry_id, episode).await?;

    if files.is_empty() {
        let episode_text = episode.map(|e| format!(" episode {}", e)).unwrap_or_default();
        ctx.say(format!("No subtitle files found for **{}**{}", entry.name, episode_text)).await?;
        return Ok(());
    }

    // Create title with episode number if specified
    let title = match episode {
        Some(ep) => format!("{} ep {}", entry.name, ep),
        None => entry.name.clone(),
    };

    // Create main embed for channel
    let mut channel_embed = serenity::CreateEmbed::new()
        .title(&title)
        .color(0x00ff00)
        .timestamp(serenity::Timestamp::now())
        .footer(serenity::CreateEmbedFooter::new("Jimaku API"));

    if let Some(ref img) = cover_image {
        channel_embed = channel_embed.thumbnail(img);
    }

    if let Some(ref english) = entry.english_name {
        channel_embed = channel_embed.field("English Name", english, true);
    }
    if let Some(ref japanese) = entry.japanese_name {
        channel_embed = channel_embed.field("Japanese Name", japanese, true);
    }

    // Send info embed to channel
    ctx.send(poise::CreateReply::default().embed(channel_embed)).await?;

    // Create DM embed with file list
    let mut dm_embed = serenity::CreateEmbed::new()
        .title(format!("Subtitle: {}", entry.name))
        .color(0x0099ff)
        .timestamp(serenity::Timestamp::now());

    if let Some(ref img) = cover_image {
        dm_embed = dm_embed.thumbnail(img);
    }

    if let Some(ref english) = entry.english_name {
        dm_embed = dm_embed.field("English", english, true);
    }
    if let Some(ref japanese) = entry.japanese_name {
        dm_embed = dm_embed.field("Japanese", japanese, true);
    }

    dm_embed = dm_embed.field("Entry ID", format!("`{}`", entry_id), true);

    // Build file list and download files
    let limited_files = files.iter().take(4).collect::<Vec<_>>();
    let mut file_list = String::new();
    let mut attachments: Vec<serenity::CreateAttachment> = Vec::new();

    for file in &limited_files {
        let file_size_kb = file.size as f64 / 1024.0;
        file_list.push_str(&format!("**{}**\n", file.name));
        file_list.push_str(&format!("Size: {:.2} KB\n", file_size_kb));
        
        // Download file if not too large
        if file.size < MAX_FILE_SIZE {
            match download_file(http_client, &file.url).await {
                Ok(data) => {
                    let attachment = serenity::CreateAttachment::bytes(data, &file.name);
                    attachments.push(attachment);
                }
                Err(e) => {
                    error!("Error downloading file {}: {:?}", file.name, e);
                    file_list.push_str("*Error downloading this file*\n");
                }
            }
        } else {
            file_list.push_str("*File too large for Discord upload*\n");
            file_list.push_str(&format!("[Manual Download]({})\n", file.url));
        }
        file_list.push('\n');
    }

    dm_embed = dm_embed.description(&file_list);

    if files.len() > 4 {
        dm_embed = dm_embed.field(
            "Info",
            format!("Showing 4 of {} files. Use Entry ID `{}` for specific downloads or use episode parameter.", files.len(), entry_id),
            false,
        );
    }

    // Send to user's DM
    let user = ctx.author();
    let dm_channel = match user.create_dm_channel(ctx).await {
        Ok(ch) => ch,
        Err(e) => {
            error!("Cannot create DM channel: {:?}", e);
            ctx.say("Cannot send DM. Please check your privacy settings and try again.").await?;
            return Ok(());
        }
    };

    // Build message with attachments
    let mut dm_message = serenity::CreateMessage::new().embed(dm_embed);
    
    for attachment in attachments {
        dm_message = dm_message.add_file(attachment);
    }

    match dm_channel.send_message(ctx, dm_message).await {
        Ok(_) => {
            info!("Sent subtitle files to user {} via DM", user.name);
        }
        Err(e) => {
            error!("Error sending DM: {:?}", e);
            ctx.say("Cannot send DM. Please check your privacy settings and try again.").await?;
        }
    }

    Ok(())
}

/// Autocomplete for anime search
async fn autocomplete_anime<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> + 'a {
    let results = async move {
        if partial.len() < 2 {
            return vec![];
        }

        let api_key = match env::var("JIMAKU_API_KEY") {
            Ok(key) => key,
            Err(_) => return vec![],
        };

        let http_client = &ctx.data().http_client;

        match search_anime(http_client, &api_key, partial).await {
            Ok(results) => {
                results
                    .into_iter()
                    .take(25)
                    .map(|anime| {
                        let display = if let Some(ref eng) = anime.english_name {
                            format!("{} ({})", anime.name, eng)
                        } else {
                            anime.name.clone()
                        };
                        // Truncate to 100 chars for Discord limit
                        let display = if display.len() > 100 {
                            format!("{}...", &display[..97])
                        } else {
                            display
                        };
                        serenity::AutocompleteChoice::new(display, anime.id.to_string())
                    })
                    .collect()
            }
            Err(_) => vec![],
        }
    };
    
    results.await.into_iter()
}
