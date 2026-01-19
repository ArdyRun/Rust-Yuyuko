use poise::serenity_prelude as serenity;
use tracing::{error, info};

use crate::models::guild::GuildConfig;
use crate::utils::config::colors;
use crate::{Context, Error};

/// Configuration options
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum ConfigKey {
    #[name = "Ayumi Channel"]
    AyumiChannel,
    #[name = "Quiz Channel"]
    QuizChannel,
    #[name = "Welcome Channel"]
    WelcomeChannel,
}

/// Manage bot configuration
#[poise::command(
    slash_command,
    prefix_command,
    required_permissions = "MANAGE_GUILD",
    subcommands("set", "get")
)]
pub async fn config(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Set a configuration value
#[poise::command(slash_command)]
pub async fn set(
    ctx: Context<'_>,
    #[description = "Setting to configure"] key: ConfigKey,
    #[description = "Channel to use"] channel: serenity::Channel,
) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => {
            ctx.say("This command can only be used in a server.").await?;
            return Ok(());
        }
    };

    ctx.defer().await?;

    let channel_id = channel.id().to_string();
    let data = ctx.data();

    // Fetch existing config or create new
    // Check cache first
    let mut config = if let Some(cached) = data.guild_configs.get(&guild_id) {
        cached.clone()
    } else {
        match data.firebase.get_document("guilds", &guild_id).await {
            Ok(Some(doc)) => serde_json::from_value::<GuildConfig>(doc).unwrap_or_default(),
            Ok(None) => GuildConfig::default(),
            Err(e) => {
                error!("Failed to fetch guild config: {:?}", e);
                ctx.say("Failed to fetch configuration.").await?;
                return Ok(());
            }
        }
    };

    // Update config
    match key {
        ConfigKey::AyumiChannel => config.ayumi_channel_id = Some(channel_id.clone()),
        ConfigKey::QuizChannel => config.quiz_channel_id = Some(channel_id.clone()),
        ConfigKey::WelcomeChannel => config.welcome_channel_id = Some(channel_id.clone()),
    }

    // Save back to Firebase
    let json_val = serde_json::to_value(&config)?;
    match data.firebase.set_document("guilds", &guild_id, &json_val).await {
        Ok(_) => {
            info!("Updated config for guild {}: {:?} -> {}", guild_id, key, channel_id);
            // Update cache
            data.guild_configs.insert(guild_id.clone(), config);
            
            let embed = serenity::CreateEmbed::new()
                .title("Configuration Updated")
                .description(format!("**{:?}** set to <#{}>", key, channel_id))
                .color(colors::SUCCESS);
            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
        Err(e) => {
            error!("Failed to save guild config: {:?}", e);
            ctx.say("Failed to save configuration.").await?;
        }
    }

    Ok(())
}

/// Get current configuration
#[poise::command(slash_command)]
pub async fn get(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id.to_string(),
        None => {
            ctx.say("This command can only be used in a server.").await?;
            return Ok(());
        }
    };

    ctx.defer().await?;
    let data = ctx.data();

    // Check cache first
    let config = if let Some(cached) = data.guild_configs.get(&guild_id) {
        cached.clone()
    } else {
        match data.firebase.get_document("guilds", &guild_id).await {
            Ok(Some(doc)) => {
                let cfg = serde_json::from_value::<GuildConfig>(doc).unwrap_or_default();
                // Populate cache
                data.guild_configs.insert(guild_id.clone(), cfg.clone());
                cfg
            },
            Ok(None) => GuildConfig::default(),
            Err(e) => {
                error!("Failed to fetch guild config: {:?}", e);
                ctx.say("Failed to fetch configuration.").await?;
                return Ok(());
            }
        }
    };

    let ayumi = config.ayumi_channel_id.map(|id| format!("<#{}>", id)).unwrap_or_else(|| "Not set".to_string());
    let quiz = config.quiz_channel_id.map(|id| format!("<#{}>", id)).unwrap_or_else(|| "Not set".to_string());
    let welcome = config.welcome_channel_id.map(|id| format!("<#{}>", id)).unwrap_or_else(|| "Not set".to_string());

    let embed = serenity::CreateEmbed::new()
        .title("Server Configuration")
        .field("Ayumi Channel", ayumi, true)
        .field("Quiz Channel", quiz, true)
        .field("Welcome Channel", welcome, true)
        .color(colors::INFO);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}
