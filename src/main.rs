// Yuyuko Bot - Rust Edition
// A lightweight Discord bot for Japanese immersion tracking

mod commands;
mod api;
mod models;
mod utils;

use std::env;
use std::sync::Arc;

use poise::serenity_prelude as serenity;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::api::firebase::FirebaseClient;

/// User data shared across all commands
pub struct Data {
    pub http_client: reqwest::Client,
    pub firebase: Arc<FirebaseClient>,
}

// Manual Debug impl since FirebaseClient doesn't impl Debug
impl std::fmt::Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("http_client", &"reqwest::Client")
            .field("firebase", &"FirebaseClient")
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
    info!("Firebase client initialized");

    // Setup framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: get_commands(),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("y!".into()),
                ..Default::default()
            },
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            error!("Command error: {:?}", error);
                            let _ = ctx.say(format!("❌ Error: {}", error)).await;
                        }
                        err => {
                            error!("Framework error: {:?}", err);
                        }
                    }
                })
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                info!("Bot is ready! Registering commands...");
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                info!("Commands registered successfully!");
                
                Ok(Data {
                    http_client,
                    firebase,
                })
            })
        })
        .build();

    // Build client - note: MESSAGE_CONTENT is privileged, enable in Discord Dev Portal if needed
    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MESSAGES;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("Failed to create client");

    // Run with graceful shutdown
    let shard_manager = client.shard_manager.clone();
    
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
