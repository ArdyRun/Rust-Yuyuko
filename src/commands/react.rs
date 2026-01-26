// React command - react to messages with animated emojis
// Ported from commands/react.js

use poise::serenity_prelude as serenity;
use tracing::error;

use crate::{Context, Error};
use crate::utils::config::colors;
use crate::utils::emojis::{EMOJIS, get_emoji_by_id};

const EMOJIS_PER_PAGE: usize = 20;
const BUTTONS_PER_ROW: usize = 5;

/// Parse message link to extract channel_id and message_id
fn parse_message_link(input: &str) -> Option<(u64, u64)> {
    // Format: https://discord.com/channels/GUILD_ID/CHANNEL_ID/MESSAGE_ID
    if input.contains("/channels/") {
        let parts: Vec<&str> = input.split('/').collect();
        if parts.len() >= 3 {
            // Last 2 parts should be channel_id and message_id
            let msg_id = parts.last()?.parse().ok()?;
            let ch_id = parts.get(parts.len() - 2)?.parse().ok()?;
            return Some((ch_id, msg_id));
        }
    }
    None
}

/// React ke pesan dengan emoji animasi
#[poise::command(slash_command, prefix_command)]
pub async fn react(
    ctx: Context<'_>,
    #[description = "ID atau link pesan yang ingin direact"]
    pesan: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    // Parse message link or ID
    let (channel_id, message_id): (u64, u64) = if let Some((ch_id, msg_id)) = parse_message_link(&pesan) {
        (ch_id, msg_id)
    } else {
        // Try as message ID only
        let msg_id: u64 = pesan.trim().parse().unwrap_or(0);
        (ctx.channel_id().get(), msg_id)
    };

    if message_id == 0 {
        ctx.say("ID pesan tidak valid. Gunakan ID 17-19 digit atau link pesan.").await?;
        return Ok(());
    }

    // Fetch channel
    let channel = match ctx.http().get_channel(serenity::ChannelId::new(channel_id)).await {
        Ok(ch) => ch,
        Err(_) => {
            ctx.say("Channel tidak ditemukan atau bot tidak memiliki akses.").await?;
            return Ok(());
        }
    };

    let guild_channel = match channel.guild() {
        Some(gc) => gc,
        None => {
            ctx.say("Command ini hanya bisa digunakan di server.").await?;
            return Ok(());
        }
    };

    // Fetch message
    let message = match guild_channel.message(ctx.http(), serenity::MessageId::new(message_id)).await {
        Ok(msg) => msg,
        Err(_) => {
            ctx.say(format!("Pesan tidak ditemukan di <#{}>.", channel_id)).await?;
            return Ok(());
        }
    };

    // Check message age (14 days limit)
    let age = chrono::Utc::now().timestamp() - message.timestamp.unix_timestamp();
    if age > 14 * 24 * 60 * 60 {
        ctx.say("Pesan terlalu lama (lebih dari 14 hari) untuk direact.").await?;
        return Ok(());
    }

    // Build emoji selection embed
    let embed = serenity::CreateEmbed::new()
        .title("Pilih Emoji untuk React")
        .description(format!(
            "Klik emoji di bawah untuk mereact [pesan ini]({})",
            message.link()
        ))
        .color(colors::INFO);

    // Generate emoji buttons
    let mut current_page = 0;
    let components = generate_emoji_rows(current_page, channel_id, message_id);

    let reply = ctx.send(
        poise::CreateReply::default()
            .embed(embed.clone())
            .components(components)
    ).await?;

    let msg = reply.message().await?;

    // Collect button interactions
    let mut collector = msg.await_component_interactions(ctx.serenity_context())
        .timeout(std::time::Duration::from_secs(60))
        .author_id(ctx.author().id)
        .stream();

    use futures::StreamExt;

    while let Some(interaction) = collector.next().await {
        let custom_id = &interaction.data.custom_id;

        if custom_id.starts_with("react_") {
            // Extract emoji ID
            let parts: Vec<&str> = custom_id.split('_').collect();
            if parts.len() >= 2 {
                let emoji_id = parts[1];
                
                // Try to react
                let reaction = serenity::ReactionType::Custom {
                    animated: true,
                    id: serenity::EmojiId::new(emoji_id.parse().unwrap_or(0)),
                    name: get_emoji_by_id(emoji_id).map(|e| e.name.to_string()),
                };

                match message.react(ctx.http(), reaction).await {
                    Ok(_) => {
                        let emoji_name = get_emoji_by_id(emoji_id)
                            .map(|e| e.name)
                            .unwrap_or("emoji");

                        let success_embed = serenity::CreateEmbed::new()
                            .title("React Berhasil")
                            .description(format!("Pesan berhasil direact dengan emoji **{}**", emoji_name))
                            .color(0x00FF00)
                            .image(format!("https://cdn.discordapp.com/emojis/{}.gif", emoji_id))
                            .footer(serenity::CreateEmbedFooter::new(format!("Emoji ID: {}", emoji_id)));

                        let _ = interaction.create_response(
                            ctx.http(),
                            serenity::CreateInteractionResponse::UpdateMessage(
                                serenity::CreateInteractionResponseMessage::new()
                                    .embed(success_embed)
                                    .components(vec![])
                            )
                        ).await;
                        break;
                    }
                    Err(e) => {
                        error!("Failed to react: {:?}", e);
                        let _ = interaction.create_response(
                            ctx.http(),
                            serenity::CreateInteractionResponse::Message(
                                serenity::CreateInteractionResponseMessage::new()
                                    .content("Gagal menambahkan react. Bot mungkin tidak punya permission.")
                                    .ephemeral(true)
                            )
                        ).await;
                    }
                }
            }
        } else if custom_id.starts_with("page_") {
            // Pagination
            let parts: Vec<&str> = custom_id.split('_').collect();
            if parts.len() >= 2 {
                current_page = parts[1].parse().unwrap_or(0);
                
                let components = generate_emoji_rows(current_page, channel_id, message_id);
                
                let _ = interaction.create_response(
                    ctx.http(),
                    serenity::CreateInteractionResponse::UpdateMessage(
                        serenity::CreateInteractionResponseMessage::new()
                            .embed(embed.clone())
                            .components(components)
                    )
                ).await;
            }
        }
    }

    Ok(())
}

fn generate_emoji_rows(page: usize, _channel_id: u64, _message_id: u64) -> Vec<serenity::CreateActionRow> {
    let start = page * EMOJIS_PER_PAGE;
    let page_emojis: Vec<_> = EMOJIS.iter().skip(start).take(EMOJIS_PER_PAGE).collect();
    
    let mut rows = Vec::new();
    
    // Emoji buttons (5 per row)
    for chunk in page_emojis.chunks(BUTTONS_PER_ROW) {
        let buttons: Vec<serenity::CreateButton> = chunk.iter().map(|emoji| {
            serenity::CreateButton::new(format!("react_{}", emoji.id))
                .style(serenity::ButtonStyle::Secondary)
                .emoji(serenity::ReactionType::Custom {
                    animated: true,
                    id: serenity::EmojiId::new(emoji.id.parse().unwrap_or(0)),
                    name: Some(emoji.name.to_string()),
                })
        }).collect();
        
        rows.push(serenity::CreateActionRow::Buttons(buttons));
    }
    
    // Navigation buttons
    let total_pages = (EMOJIS.len() + EMOJIS_PER_PAGE - 1) / EMOJIS_PER_PAGE;
    if total_pages > 1 {
        let nav_buttons = vec![
            serenity::CreateButton::new(format!("page_{}", page.saturating_sub(1)))
                .label("Prev")
                .style(serenity::ButtonStyle::Secondary)
                .disabled(page == 0),
            serenity::CreateButton::new(format!("page_{}", page + 1))
                .label("Next")
                .style(serenity::ButtonStyle::Secondary)
                .disabled(page >= total_pages - 1),
        ];
        rows.push(serenity::CreateActionRow::Buttons(nav_buttons));
    }
    
    rows
}
