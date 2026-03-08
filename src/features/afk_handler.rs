// AFK Handler - handles AFK status detection
// Ported from events/afkHandler.js

use poise::serenity_prelude as serenity;
use tracing::{debug, error, info};

use crate::commands::afk::{get_afk_data, is_afk, remove_afk};

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
    let author_id = msg.author.id.get();
    if is_afk(author_id).await {
        info!(
            "[AFK] User {} ({}) sent a message while AFK, removing status",
            msg.author.name, author_id
        );
        if let Some(_afk_data) = remove_afk(author_id).await {
            let embed = serenity::CreateEmbed::new()
                .color(0x2ecc71) // Green
                .author(
                    serenity::CreateEmbedAuthor::new(&msg.author.name).icon_url(
                        msg.author
                            .avatar_url()
                            .unwrap_or_else(|| msg.author.default_avatar_url()),
                    ),
                )
                .title("Selamat Datang Kembali")
                .description("Status AFK kamu telah dihapus")
                .timestamp(serenity::Timestamp::now());

            match msg
                .channel_id
                .send_message(
                    &ctx.http,
                    serenity::CreateMessage::new()
                        .embed(embed)
                        .reference_message(msg),
                )
                .await
            {
                Ok(_reply) => {
                    info!("[AFK] Sent welcome back message for {}", msg.author.name);
                }
                Err(e) => {
                    error!(
                        "[AFK] Failed to send welcome back message for {}: {:?}",
                        msg.author.name, e
                    );
                }
            }
        } else {
            debug!(
                "[AFK] Race condition: AFK already removed for {} ({})",
                msg.author.name, author_id
            );
        }
    }

    // Check for mentions of AFK users
    for mentioned_user in &msg.mentions {
        // Skip bot mentions
        if mentioned_user.bot {
            continue;
        }

        let mentioned_id = mentioned_user.id.get();
        if let Some(afk_data) = get_afk_data(mentioned_id).await {
            debug!(
                "[AFK] User {} mentioned AFK user {} ({})",
                msg.author.name, afk_data.username, mentioned_id
            );
            let embed = serenity::CreateEmbed::new()
                .color(0xe67e22) // Orange
                .author(
                    serenity::CreateEmbedAuthor::new(&afk_data.username)
                        .icon_url(&afk_data.avatar_url),
                )
                .title(format!("{} sedang AFK", afk_data.username))
                .description(format!(
                    "**Alasan:** {}\n**Sejak:** <t:{}:R>",
                    afk_data.reason, afk_data.timestamp
                ))
                .timestamp(serenity::Timestamp::now());

            if let Err(e) = msg
                .channel_id
                .send_message(
                    &ctx.http,
                    serenity::CreateMessage::new()
                        .embed(embed)
                        .reference_message(msg),
                )
                .await
            {
                error!(
                    "[AFK] Failed to send AFK mention notification for {}: {:?}",
                    afk_data.username, e
                );
            }
        }
    }

    Ok(())
}
