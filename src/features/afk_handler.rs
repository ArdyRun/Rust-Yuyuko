// AFK Handler - handles AFK status detection
// Ported from events/afkHandler.js

use poise::serenity_prelude as serenity;
use tracing::error;

use crate::commands::afk::{get_afk_data, remove_afk, is_afk};

/// Handle AFK-related events on message create
pub async fn handle_afk_message(
    ctx: &serenity::Context,
    msg: &serenity::Message,
) -> Result<(), anyhow::Error> {
    // Ignore bots
    if msg.author.bot {
        return Ok(());
    }

    // Check if the message author is AFK - remove their status
    if is_afk(msg.author.id.get()).await {
        if let Some(afk_data) = remove_afk(msg.author.id.get()).await {
            let embed = serenity::CreateEmbed::new()
                .color(0x2ecc71) // Green
                .author(serenity::CreateEmbedAuthor::new(&msg.author.name)
                    .icon_url(msg.author.avatar_url().unwrap_or_else(|| msg.author.default_avatar_url())))
                .title("Selamat Datang Kembali")
                .description("Status AFK kamu telah dihapus")
                .timestamp(serenity::Timestamp::now());

            let reply = msg.channel_id.send_message(
                &ctx.http,
                serenity::CreateMessage::new()
                    .embed(embed)
                    .reference_message(msg)
            ).await?;

            // Delete the welcome back message after 5 seconds
            let http = ctx.http.clone();
            let channel_id = msg.channel_id;
            let message_id = reply.id;
            
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                let _ = channel_id.delete_message(&http, message_id).await;
            });
        }
    }

    // Check for mentions of AFK users
    for mentioned_user in &msg.mentions {
        if let Some(afk_data) = get_afk_data(mentioned_user.id.get()).await {
            let embed = serenity::CreateEmbed::new()
                .color(0xe67e22) // Orange
                .author(serenity::CreateEmbedAuthor::new(&afk_data.username)
                    .icon_url(&afk_data.avatar_url))
                .title(format!("{} sedang AFK", afk_data.username))
                .description(format!(
                    "**Alasan:** {}\n**Sejak:** <t:{}:R>",
                    afk_data.reason,
                    afk_data.timestamp
                ))
                .timestamp(serenity::Timestamp::now());

            msg.channel_id.send_message(
                &ctx.http,
                serenity::CreateMessage::new()
                    .embed(embed)
                    .reference_message(msg)
            ).await?;
        }
    }

    Ok(())
}
