// AFK command - set AFK status
// Ported from commands/afk.js

use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use once_cell::sync::Lazy;

use crate::utils::config::colors;
use crate::{Context, Error};

/// AFK user data
#[derive(Debug, Clone)]
pub struct AfkData {
    pub username: String,
    pub reason: String,
    pub timestamp: u64,
    pub avatar_url: String,
}

/// Global AFK users map (User ID -> AFK Data)
pub static AFK_USERS: Lazy<Arc<RwLock<HashMap<u64, AfkData>>>> = Lazy::new(|| {
    Arc::new(RwLock::new(HashMap::new()))
});

/// Set your AFK status
#[poise::command(slash_command, prefix_command)]
pub async fn afk(
    ctx: Context<'_>,
    #[description = "Alasan AFK (opsional)"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or_else(|| "AFK".to_string());
    let user = ctx.author();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Store AFK data
    {
        let mut afk_users = AFK_USERS.write().await;
        afk_users.insert(user.id.get(), AfkData {
            username: user.name.clone(),
            reason: reason.clone(),
            timestamp,
            avatar_url: user.avatar_url().unwrap_or_else(|| user.default_avatar_url()),
        });
    }

    let embed = serenity::CreateEmbed::new()
        .color(colors::INFO)
        .author(serenity::CreateEmbedAuthor::new(&user.name)
            .icon_url(user.avatar_url().unwrap_or_else(|| user.default_avatar_url())))
        .title("💤 AFK")
        .description(format!(
            "User lain akan diberitahu kalau kamu sedang AFK.\n**Alasan:** {}",
            reason
        ))
        .footer(serenity::CreateEmbedFooter::new("Kirim pesan lagi untuk menghapus status AFK"))
        .timestamp(serenity::Timestamp::now());

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Check if user is AFK and return their data
pub async fn get_afk_data(user_id: u64) -> Option<AfkData> {
    let afk_users = AFK_USERS.read().await;
    afk_users.get(&user_id).cloned()
}

/// Remove user from AFK
pub async fn remove_afk(user_id: u64) -> Option<AfkData> {
    let mut afk_users = AFK_USERS.write().await;
    afk_users.remove(&user_id)
}

/// Check if user is AFK
pub async fn is_afk(user_id: u64) -> bool {
    let afk_users = AFK_USERS.read().await;
    afk_users.contains_key(&user_id)
}
