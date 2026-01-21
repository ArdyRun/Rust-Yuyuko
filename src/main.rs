// Yuyuko Bot - Rust Edition
// A lightweight Discord bot for Japanese immersion tracking

mod commands;
mod api;
mod models;
mod utils;
mod features;

use std::env;
use std::sync::Arc;

use poise::serenity_prelude as serenity;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use dashmap::DashMap;

use crate::models::guild::GuildConfig;

use crate::api::firebase::FirebaseClient;

/// User data shared across all commands
pub struct Data {
    pub http_client: reqwest::Client,
    pub firebase: Arc<FirebaseClient>,
    pub guild_configs: Arc<DashMap<String, GuildConfig>>,
    pub role_rank_sessions: Arc<DashMap<serenity::UserId, crate::features::role_rank::QuizSession>>,
}

// Manual Debug impl since FirebaseClient doesn't impl Debug
impl std::fmt::Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("http_client", &"reqwest::Client")
            .field("firebase", &"FirebaseClient")
            .field("guild_configs", &"DashMap")
            .finish()
    }
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// Register all slash commands
fn get_commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        commands::immersion::immersion(),
        commands::stat::stat(),
        commands::leaderboard::leaderboard(),
        commands::log::log(),
        commands::help::help(),
        commands::config::config(),
        commands::register::register(),
        commands::novel::novel(),
        commands::afk::afk(),
        commands::subs::subs(),
        commands::export::export(),
        commands::react::react(),
        commands::prompt::prompt(),
        commands::role_rank::role_rank(),
    ]
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "yuyuko_rs=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
    let firebase_project_id = env::var("FIREBASE_PROJECT_ID")
        .unwrap_or_else(|_| "yuyuko-bot".to_string());
    let owner_id = env::var("BOT_OWNER_ID").ok();

    info!("Starting Yuyuko Bot (Rust Edition)...");

    // Build HTTP client for API calls
    let http_client = reqwest::Client::builder()
        .user_agent("Yuyuko-Bot/1.0")
        .build()
        .expect("Failed to create HTTP client");

    // Initialize Firebase client
    let firebase = FirebaseClient::from_file(http_client.clone(), "firebase-key.json")
        .expect("Failed to load Firebase credentials");
    let firebase = Arc::new(firebase);
    let guild_configs = Arc::new(DashMap::new());
    let role_rank_sessions = Arc::new(DashMap::new());
    info!("Firebase client initialized");

    // Setup framework
    let guild_configs_clone = guild_configs.clone();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: get_commands(),
            // ... (rest of options)
            owners: if let Some(id) = owner_id.clone() {
                let mut owners = std::collections::HashSet::new();
                if let Ok(uid) = id.parse() {
                    owners.insert(uid);
                }
                owners
            } else {
                Default::default()
            },
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("y!".into()),
                ..Default::default()
            },
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            error!("Command error: {:?}", error);
                            let _ = ctx.say(format!("Error: {}", error)).await;
                        }
                        poise::FrameworkError::MissingUserPermissions { missing_permissions, ctx, .. } => {
                            let msg = if let Some(perms) = missing_permissions {
                                format!("You need the **{:?}** permission to use this command.", perms)
                            } else {
                                "You do not have the required permissions to use this command.".to_string()
                            };
                            let _ = ctx.send(poise::CreateReply::default().content(msg).ephemeral(true)).await;
                        }
                        poise::FrameworkError::MissingBotPermissions { missing_permissions, ctx, .. } => {
                             let msg = format!("I need the **{:?}** permission to execute this command.", missing_permissions);
                             let _ = ctx.send(poise::CreateReply::default().content(msg).ephemeral(true)).await;
                        }
                        err => {
                            error!("Framework error: {:?}", err);
                            // Try to notify the user if possible about the unexpected error
                            if let Some(ctx) = err.ctx() {
                                let _ = ctx.send(poise::CreateReply::default().content("An unexpected error occurred.").ephemeral(true)).await;
                            }
                        }
                    }
                })
            },
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    if let serenity::FullEvent::Message { new_message } = event {
                        // Handle AFK status
                        if let Err(e) = features::afk_handler::handle_afk_message(ctx, new_message).await {
                            error!("Error in AFK handler: {:?}", e);
                        }
                        
                        // Handle Role Rank Messages (Kotoba Bot listener)
                        if let Err(e) = features::role_rank::handle_message(ctx, new_message, data).await {
                             error!("Error in Role Rank message handler: {:?}", e);
                        }

                        // Handle Ayumi AI
                        if let Err(e) = features::ayumi::handle_message(ctx, new_message, data).await {
                            error!("Error in Ayumi handler: {:?}", e);
                        }
                    }
                    else if let serenity::FullEvent::InteractionCreate { interaction } = event {
                        if let serenity::Interaction::Component(component) = interaction {
                             if let Err(e) = features::role_rank::handle_interaction(ctx, component, data).await {
                                  error!("Error in Role Rank interaction handler: {:?}", e);
                             }
                        }
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                info!("Bot is ready!");

                // Register in all guilds (Instant updates)
                for guild in &_ready.guilds {
                    let guild_id = guild.id;
                    info!("Registering commands in guild: {}", guild_id);
                    if let Err(e) = poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await {
                        error!("Failed to register commands in guild {}: {:?}", guild_id, e);
                    }
                }
                
                // Also register globally as a fallback
                // poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(Data {
                    http_client,
                    firebase,
                    guild_configs: guild_configs_clone,
                    role_rank_sessions: role_rank_sessions.clone(),
                })
            })
        })
        .build();

    // Build client - note: MESSAGE_CONTENT is privileged, enable in Discord Dev Portal if needed
    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("Failed to create client");

    // Run with graceful shutdown
    let shard_manager = client.shard_manager.clone();
    
    // Background Task: Quiz Selector Refresh
    let http = client.http.clone();
    let configs = guild_configs.clone(); // This clone works if guild_configs is available.
    // BUT guild_configs was moved into setup() at line 174 (original view).
    // Wait, in line 94: let guild_configs = Arc::new(DashMap::new());
    // In setup(): ... guild_configs: guild_configs.clone() ... this moves the Arc clone? No, the variable itself if captured.
    
    // Add imports at top of file needed for this: use futures::StreamExt;
    
    tokio::spawn(async move {
        use futures::StreamExt;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // Check every 5 minutes
        
        loop {
            interval.tick().await;
            
            // Snapshot the configs to avoid holding locks during async operations
            // We collect only what we need: channel IDs
            let channels_to_check: Vec<String> = configs.iter()
                .filter_map(|entry| entry.value().quiz_channel_id.clone())
                .collect();

            // Create a stream of futures for concurrent processing
            let tasks = futures::stream::iter(channels_to_check)
                .map(|channel_id_str| {
                    let http = http.clone();
                    
                    async move {
                        if let Ok(channel_id) = channel_id_str.parse::<u64>().map(serenity::ChannelId::new) {
                            // Check last message in channel
                            match channel_id.messages(&http, serenity::GetMessages::new().limit(1)).await {
                                Ok(messages) => {
                                    let needs_refresh = if let Some(last_msg) = messages.first() {
                                        !last_msg.author.bot 
                                    } else {
                                        true 
                                    };

                                    if needs_refresh {
                                        // Find and delete old bot messages to clean up
                                        if let Ok(history) = channel_id.messages(&http, serenity::GetMessages::new().limit(10)).await {
                                            for msg in history {
                                                if msg.author.bot && msg.embeds.iter().any(|e| e.title.as_deref() == Some("Quiz Selector")) {
                                                    let _ = msg.delete(&http).await;
                                                }
                                            }
                                        }
                                        
                                        // Send new selector
                                        if let Err(e) = crate::commands::role_rank::send_quiz_selector(&http, channel_id).await {
                                            error!("Failed to auto-refresh quiz selector: {:?}", e);
                                        }
                                    }
                                },
                                Err(e) => error!("Failed to check quiz channel messages: {:?}", e),
                            }
                        }
                    }
                })
                .buffer_unordered(10); // Process 10 guilds concurrently

            tasks.collect::<Vec<_>>().await;
        }
    });

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to register Ctrl+C handler");
        info!("Shutting down...");
        shard_manager.shutdown_all().await;
    });

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    info!("Goodbye!");
}
