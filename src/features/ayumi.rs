use poise::serenity_prelude as serenity;
use tracing::{error, info, debug};
use std::sync::Arc;
use tokio::sync::Mutex;
use lru::LruCache;
use std::num::NonZeroUsize;
use once_cell::sync::Lazy;

use crate::features::novel_recommender::recommend_novels;
use crate::Data;
use crate::models::guild::GuildConfig;
use crate::api::llm::{completion_openrouter, completion_gemini_vision, ChatMessage};
use crate::utils::ayumi_prompt::AYUMI_SYSTEM_PROMPT;

// Global conversation history cache (User ID -> List of Messages)
// Capacity: 100 users, 10 messages each
type HistoryCache = LruCache<u64, Vec<ChatMessage>>;

static CONVERSATION_HISTORY: Lazy<Arc<Mutex<HistoryCache>>> = Lazy::new(|| {
    Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())))
});

/// Handle incoming messages for Ayumi
pub async fn handle_message(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    data: &Data,
) -> Result<(), anyhow::Error> {
    // 1. Ignore bots and self
    if msg.author.bot {
        return Ok(());
    }

    // 2. Check if in a guild
    let guild_id = match msg.guild_id {
        Some(gid) => gid.to_string(),
        None => return Ok(()), // Ignore DMs for now or handle differently
    };

    // 3. Get Guild Config (Check Cache first)
    let config = if let Some(cached) = data.guild_configs.get(&guild_id) {
        cached.clone()
    } else {
        // Fallback: Fetch from Firebase and Cache
        match data.firebase.get_document("guilds", &guild_id).await {
            Ok(Some(doc)) => {
                let cfg = serde_json::from_value::<GuildConfig>(doc).unwrap_or_default();
                data.guild_configs.insert(guild_id.clone(), cfg.clone());
                cfg
            },
            Ok(None) => return Ok(()), // No config, do nothing
            Err(e) => {
                error!("Failed to fetch guild config for {}: {:?}", guild_id, e);
                return Ok(());
            }
        }
    };

    // 4. Check if channel matches Ayumi Channel
    let ayumi_channel = match config.ayumi_channel_id {
        Some(id) => id,
        None => return Ok(()), // Not configured
    };

    if msg.channel_id.to_string() != ayumi_channel {
        return Ok(()); // Wrong channel
    }

    // 5. Respond!
    let _typing = msg.channel_id.start_typing(&ctx.http);

    // Prepare context
    let user_id = msg.author.id.get();
    let history_clone = {
        let mut cache = CONVERSATION_HISTORY.lock().await;
        cache.get(&user_id).cloned().unwrap_or_default()
    };

    // Build messages payload
    let mut messages = history_clone;
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: msg.content.clone(),
    });

    // Check for attachments (Images)
    let attachment = msg.attachments.iter().find(|a| {
        a.content_type.as_ref().map_or(false, |ct| ct.starts_with("image/"))
    });

    let response = if let Some(att) = attachment {
        // Handle Image Analysis
        debug!("Processing image attachment for user {}", msg.author.name);
        
        // Download image
        let image_data = match att.download().await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to download attachment: {:?}", e);
                msg.reply(ctx, "Gagal mengunduh gambar...").await?;
                return Ok(());
            }
        };

        // Determine prompt (use message content or default)
        let prompt = if msg.content.trim().is_empty() {
            "Deskripsikan gambar ini dengan gaya bahasa Ayumi."
        } else {
            &msg.content
        };

        let mime_type = att.content_type.as_deref().unwrap_or("image/jpeg");

        // Call Gemini Vision
        match completion_gemini_vision(data, prompt, &image_data, mime_type).await {
            Ok(res) => res,
            Err(e) => {
                error!("Ayumi Gemini Vision error: {:?}", e);
                "Maaf, mataku agak buram... Gak bisa liat gambarnya jelas.".to_string()
            }
        }
    } else if msg.content.to_lowercase().contains("rekomendasi novel") || msg.content.to_lowercase().contains("novel saran") {
        // Handle Novel Recommendation
        debug!("Processing novel recommendation for user {}", msg.author.name);
        recommend_novels(5) // Suggest 5 novels
    } else {
        // Standard Text Chat
        // Call LLM
        debug!("Calling Ayumi LLM for user {}", msg.author.name);
        match completion_openrouter(data, AYUMI_SYSTEM_PROMPT, messages.clone()).await {
            Ok(res) => res,
            Err(e) => {
                error!("Ayumi LLM error: {:?}", e);
                "Maaf, Ayumi lagi pusing... Coba lagi nanti ya.".to_string()
            }
        }
    };

    // Send reply
    // TODO: Split message if > 2000 chars (Basic implementation for now)
    if response.len() > 2000 {
        let chunks = response.chars().collect::<Vec<char>>().chunks(2000)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<String>>();
        
        for chunk in chunks {
            msg.channel_id.say(&ctx.http, chunk).await?;
        }
    } else {
        msg.reply(ctx, &response).await?;
    }

    // Update History
    {
        let mut cache = CONVERSATION_HISTORY.lock().await;
        // Re-fetch to ensure we have latest (if modified concurrently, though locking prevents this for single user)
        // Add bot response
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: response,
        });
        
        // Trim history to last 10 turns (20 messages)
        if messages.len() > 20 {
            messages = messages.iter().rev().take(20).rev().cloned().collect();
        }
        
        cache.put(user_id, messages);
    }

    Ok(())
}
