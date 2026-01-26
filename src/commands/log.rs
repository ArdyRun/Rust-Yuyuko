// Log command - view and manage immersion logs
// Full implementation ported from commands/log.js

use poise::serenity_prelude as serenity;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use futures::StreamExt;
use tracing::{debug, error};

use crate::{Context, Error};
use crate::utils::config::get_media_label;

// ============ Data Structures ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmersionLog {
    #[serde(default)]
    pub id: String,
    pub activity: LogActivity,
    pub timestamps: LogTimestamps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogActivity {
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(rename = "typeLabel", default)]
    pub type_label: String,
    pub amount: f64,
    pub unit: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTimestamps {
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum LogTimeframe {
    #[name = "Last 24 Hours"]
    Day,
    #[name = "Last 7 Days"]
    Week,
}

impl LogTimeframe {
    fn to_string(&self) -> &'static str {
        match self {
            LogTimeframe::Day => "24h",
            LogTimeframe::Week => "7d",
        }
    }
}

const LOGS_PER_PAGE: usize = 10;

// ============ Main Command ============

/// View and manage your immersion logs
#[poise::command(slash_command, prefix_command)]
pub async fn log(
    ctx: Context<'_>,
    #[description = "Timeframe to view"] 
    timeframe: LogTimeframe,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    
    let timeframe_str = timeframe.to_string();
    
    // Show media type selection
    let embed = create_media_selection_embed(timeframe_str, &ctx.author().name);
    let components = create_media_selection_buttons(timeframe_str);
    
    let reply = ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .components(components)
            .ephemeral(true)
    ).await?;
    
    let msg = reply.message().await?.into_owned();
    
    // Handle button interactions
    handle_log_interactions(ctx, &msg, timeframe_str).await?;
    
    Ok(())
}

// ============ Embed Builders ============

fn create_media_selection_embed(timeframe: &str, username: &str) -> serenity::CreateEmbed {
    let timeframe_label = if timeframe == "24h" { "Last 24 Hours" } else { "Last 7 Days" };
    
    serenity::CreateEmbed::new()
        .color(0x2b2d31)
        .title("Select Media Type")
        .description(format!(
            "**Timeframe:** {}\n\nChoose which type of immersion logs you want to view:",
            timeframe_label
        ))
        .field(
            "Available Media Types",
            "**Visual Novel** - Characters read\n\
             **Book** - Pages read\n\
             **Reading** - Characters read\n\
             **Reading Time** - Minutes spent\n\
             **Manga** - Pages read\n\
             **Anime** - Episodes watched\n\
             **Listening** - Minutes spent\n\
             **All Types** - Show everything",
            false
        )
        .footer(serenity::CreateEmbedFooter::new(format!(
            "{} • This session expires in 60 seconds",
            username
        )))
        .timestamp(Utc::now())
}

fn create_media_selection_buttons(timeframe: &str) -> Vec<serenity::CreateActionRow> {
    let row1 = serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(format!("log_media_visual_novel_{}", timeframe))
            .label("Visual Novel")
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(format!("log_media_book_{}", timeframe))
            .label("Book")
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(format!("log_media_reading_{}", timeframe))
            .label("Reading")
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(format!("log_media_reading_time_{}", timeframe))
            .label("Reading Time")
            .style(serenity::ButtonStyle::Secondary),
    ]);
    
    let row2 = serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(format!("log_media_manga_{}", timeframe))
            .label("Manga")
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(format!("log_media_anime_{}", timeframe))
            .label("Anime")
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(format!("log_media_listening_{}", timeframe))
            .label("Listening")
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(format!("log_media_all_{}", timeframe))
            .label("All Types")
            .style(serenity::ButtonStyle::Primary),
    ]);
    
    vec![row1, row2]
}

fn create_log_embed(
    logs: &[ImmersionLog],
    page: usize,
    total_pages: usize,
    timeframe: &str,
    media_type: Option<&str>,
    username: &str,
) -> serenity::CreateEmbed {
    let timeframe_label = if timeframe == "24h" { "Last 24 Hours" } else { "Last 7 Days" };
    let media_label = media_type
        .map(|m| get_media_label(m).to_string())
        .unwrap_or_else(|| "All Types".to_string());
    
    let start_idx = page * LOGS_PER_PAGE;
    let end_idx = (start_idx + LOGS_PER_PAGE).min(logs.len());
    let page_logs = &logs[start_idx..end_idx];
    
    let mut embed = serenity::CreateEmbed::new()
        .color(0x0099ff)
        .title(format!("Immersion Logs - {}", timeframe_label))
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Page {}/{} • {} total logs • {}",
            page + 1, total_pages, logs.len(), username
        )))
        .timestamp(Utc::now());
    
    if page_logs.is_empty() {
        embed = embed.description(format!("**{}**\n\n_No immersion logs found for this timeframe._", media_label));
    } else {
        let mut description = format!("**{}**\n\n", media_label);
        
        for (i, log) in page_logs.iter().enumerate() {
            let log_num = start_idx + i + 1;
            let activity = &log.activity;
            let time = log.timestamps.created.format("%Y-%m-%d %H:%M").to_string();
            
            let title_line = if let Some(ref title) = activity.title {
                if title != "-" && !title.is_empty() {
                    let truncated = if title.len() > 50 {
                        format!("{}...", &title[..50])
                    } else {
                        title.clone()
                    };
                    format!("*{}*\n", truncated)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            
            description.push_str(&format!(
                "**{}.** {} {} of {}\n{}{}\n\n",
                log_num, activity.amount, activity.unit, activity.type_label, title_line, time
            ));
        }
        
        embed = embed.description(description);
    }
    
    embed
}

fn create_navigation_buttons(
    page: usize,
    total_pages: usize,
    timeframe: &str,
    media_type: Option<&str>,
    logs: &[ImmersionLog],
) -> Vec<serenity::CreateActionRow> {
    let mut rows = Vec::new();
    let media = media_type.unwrap_or("all");
    
    // Navigation row
    let nav_buttons = vec![
        serenity::CreateButton::new(format!("log_prev_{}_{}_{}", page, timeframe, media))
            .label("Previous")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(page == 0),
        serenity::CreateButton::new("log_page_info")
            .label(format!("{}/{}", page + 1, total_pages))
            .style(serenity::ButtonStyle::Primary)
            .disabled(true),
        serenity::CreateButton::new(format!("log_next_{}_{}_{}", page, timeframe, media))
            .label("Next")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(page >= total_pages.saturating_sub(1)),
        serenity::CreateButton::new(format!("log_back_{}", timeframe))
            .label("Back to Selection")
            .style(serenity::ButtonStyle::Secondary),
    ];
    rows.push(serenity::CreateActionRow::Buttons(nav_buttons));
    
    // Delete buttons for current page logs
    let start_idx = page * LOGS_PER_PAGE;
    let end_idx = (start_idx + LOGS_PER_PAGE).min(logs.len());
    let page_logs = &logs[start_idx..end_idx];
    
    if !page_logs.is_empty() {
        // Max 5 buttons per row
        for chunk in page_logs.chunks(5) {
            let delete_buttons: Vec<serenity::CreateButton> = chunk.iter()
                .enumerate()
                .map(|(i, log)| {
                    let global_idx = start_idx + i + 1;
                    serenity::CreateButton::new(format!("log_delete_{}", log.id))
                        .label(format!("Delete {}", global_idx))
                        .style(serenity::ButtonStyle::Danger)
                })
                .collect();
            rows.push(serenity::CreateActionRow::Buttons(delete_buttons));
        }
    }
    
    rows
}

// ============ Interaction Handler ============

async fn handle_log_interactions(
    ctx: Context<'_>,
    msg: &serenity::Message,
    initial_timeframe: &str,
) -> Result<(), Error> {
    let data = ctx.data();
    let user_id = ctx.author().id.get().to_string();
    let username = ctx.author().name.clone();
    
    let mut collector = msg.await_component_interactions(ctx.serenity_context())
        .timeout(std::time::Duration::from_secs(60))
        .author_id(ctx.author().id)
        .stream();
    
    let mut current_timeframe = initial_timeframe.to_string();
    let mut current_media: Option<String> = None;
    let mut current_page: usize = 0;
    let mut current_logs: Vec<ImmersionLog> = Vec::new();
    
    while let Some(interaction) = collector.next().await {
        let custom_id = &interaction.data.custom_id;
        debug!("Log button interaction: {}", custom_id);
        
        if custom_id.starts_with("log_media_") {
            // Media type selection
            let parts: Vec<&str> = custom_id.split('_').collect();
            
            // Parse media type and timeframe from button ID
            let (media_type, timeframe) = if parts.len() >= 5 && parts[2] == "reading" && parts[3] == "time" {
                (Some("reading_time".to_string()), parts[4].to_string())
            } else if parts.len() >= 5 && parts[2] == "visual" && parts[3] == "novel" {
                (Some("visual_novel".to_string()), parts[4].to_string())
            } else if parts.len() >= 4 {
                let mt = if parts[2] == "all" { None } else { Some(parts[2].to_string()) };
                (mt, parts[3].to_string())
            } else {
                (None, current_timeframe.clone())
            };
            
            current_timeframe = timeframe;
            current_media = media_type;
            current_page = 0;
            
            // Fetch logs from Firebase
            current_logs = fetch_user_logs(data, &user_id, &current_timeframe, current_media.as_deref()).await;
            
            let total_pages = (current_logs.len() + LOGS_PER_PAGE - 1) / LOGS_PER_PAGE;
            let total_pages = if total_pages == 0 { 1 } else { total_pages };
            
            let embed = create_log_embed(
                &current_logs, current_page, total_pages, 
                &current_timeframe, current_media.as_deref(), &username
            );
            let components = if current_logs.is_empty() {
                vec![serenity::CreateActionRow::Buttons(vec![
                    serenity::CreateButton::new(format!("log_back_{}", current_timeframe))
                        .label("Back to Selection")
                        .style(serenity::ButtonStyle::Secondary)
                ])]
            } else {
                create_navigation_buttons(
                    current_page, total_pages, &current_timeframe,
                    current_media.as_deref(), &current_logs
                )
            };
            
            let _ = interaction.create_response(
                ctx.http(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components)
                )
            ).await;
            
        } else if custom_id.starts_with("log_back_") {
            // Back to media selection
            let timeframe = custom_id.strip_prefix("log_back_").unwrap_or(&current_timeframe);
            current_timeframe = timeframe.to_string();
            
            let embed = create_media_selection_embed(&current_timeframe, &username);
            let components = create_media_selection_buttons(&current_timeframe);
            
            let _ = interaction.create_response(
                ctx.http(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components)
                )
            ).await;
            
        } else if custom_id.starts_with("log_prev_") || custom_id.starts_with("log_next_") {
            // Pagination
            let parts: Vec<&str> = custom_id.split('_').collect();
            if parts.len() >= 5 {
                let old_page: usize = parts[2].parse().unwrap_or(0);
                let is_next = custom_id.starts_with("log_next_");
                
                current_page = if is_next { old_page + 1 } else { old_page.saturating_sub(1) };
                current_timeframe = parts[3].to_string();
                current_media = if parts[4] == "all" { None } else { Some(parts[4].to_string()) };
                
                let total_pages = (current_logs.len() + LOGS_PER_PAGE - 1) / LOGS_PER_PAGE;
                let total_pages = if total_pages == 0 { 1 } else { total_pages };
                
                let embed = create_log_embed(
                    &current_logs, current_page, total_pages,
                    &current_timeframe, current_media.as_deref(), &username
                );
                let components = create_navigation_buttons(
                    current_page, total_pages, &current_timeframe,
                    current_media.as_deref(), &current_logs
                );
                
                let _ = interaction.create_response(
                    ctx.http(),
                    serenity::CreateInteractionResponse::UpdateMessage(
                        serenity::CreateInteractionResponseMessage::new()
                            .embed(embed)
                            .components(components)
                    )
                ).await;
            }
            
        } else if custom_id.starts_with("log_delete_") {
            // Delete log
            let log_id = custom_id.strip_prefix("log_delete_").unwrap_or("");
            
            if let Some(pos) = current_logs.iter().position(|l| l.id == log_id) {
                let deleted_log = current_logs.remove(pos);
                
                // Delete from Firebase
                if let Err(e) = delete_log_from_firebase(data, &user_id, log_id, &deleted_log.activity).await {
                    error!("Failed to delete log: {:?}", e);
                }
                
                // Respond with confirmation
                let _ = interaction.create_response(
                    ctx.http(),
                    serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content(format!(
                                "Deleted log: **{} {} of {}**{}",
                                deleted_log.activity.amount,
                                deleted_log.activity.unit,
                                deleted_log.activity.type_label,
                                deleted_log.activity.title.as_ref()
                                    .filter(|t| t != &"-" && !t.is_empty())
                                    .map(|t| format!(" - {}", t))
                                    .unwrap_or_default()
                            ))
                            .ephemeral(true)
                    )
                ).await;
                
                // Update the view
                let total_pages = (current_logs.len() + LOGS_PER_PAGE - 1) / LOGS_PER_PAGE;
                let total_pages = if total_pages == 0 { 1 } else { total_pages };
                
                // Adjust page if needed
                if current_page >= total_pages && current_page > 0 {
                    current_page = total_pages - 1;
                }
                
                let embed = create_log_embed(
                    &current_logs, current_page, total_pages,
                    &current_timeframe, current_media.as_deref(), &username
                );
                let components = if current_logs.is_empty() {
                    vec![serenity::CreateActionRow::Buttons(vec![
                        serenity::CreateButton::new(format!("log_back_{}", current_timeframe))
                            .label("Back to Selection")
                            .style(serenity::ButtonStyle::Secondary)
                    ])]
                } else {
                    create_navigation_buttons(
                        current_page, total_pages, &current_timeframe,
                        current_media.as_deref(), &current_logs
                    )
                };
                
                // Edit original message
                let _ = ctx.http().edit_message(
                    msg.channel_id,
                    msg.id,
                    &serenity::EditMessage::new()
                        .embed(embed)
                        .components(components),
                    vec![],
                ).await;
            }
        }
    }
    
    // Session expired
    let expired_embed = serenity::CreateEmbed::new()
        .color(0x5865f2)
        .title("Session Expired")
        .description("This immersion log session has expired due to inactivity.\n\nUse `/log` to start a new session.")
        .footer(serenity::CreateEmbedFooter::new("Session automatically closed after 60 seconds"))
        .timestamp(Utc::now());
    
    let _ = ctx.http().edit_message(
        msg.channel_id,
        msg.id,
        &serenity::EditMessage::new()
            .embed(expired_embed)
            .components(vec![]),
        vec![],
    ).await;
    
    Ok(())
}

// ============ Firebase Functions ============

async fn fetch_user_logs(
    data: &crate::Data,
    user_id: &str,
    timeframe: &str,
    media_type: Option<&str>,
) -> Vec<ImmersionLog> {
    let now = Utc::now();
    let start_date = if timeframe == "24h" {
        now - Duration::hours(24)
    } else {
        now - Duration::days(7)
    };
    
    // Query Firebase
    let _collection_path = format!("users/{}/immersion_logs", user_id);
    
    match data.firebase.query_subcollection_with_ids("users", user_id, "immersion_logs").await {
        Ok(docs) => {
            let mut logs: Vec<ImmersionLog> = docs.into_iter()
                .filter_map(|(id, value)| {
                    let mut log: ImmersionLog = serde_json::from_value(value).ok()?;
                    log.id = id;
                    
                    // Filter by time
                    if log.timestamps.created < start_date {
                        return None;
                    }
                    
                    // Filter by media type
                    if let Some(mt) = media_type {
                        if log.activity.activity_type != mt {
                            return None;
                        }
                    }
                    
                    Some(log)
                })
                .collect();
            
            // Sort by created date (newest first)
            logs.sort_by(|a, b| b.timestamps.created.cmp(&a.timestamps.created));
            
            logs
        }
        Err(e) => {
            error!("Failed to fetch immersion logs: {:?}", e);
            Vec::new()
        }
    }
}

async fn delete_log_from_firebase(
    data: &crate::Data,
    user_id: &str,
    log_id: &str,
    activity: &LogActivity,
) -> Result<(), anyhow::Error> {
    // Delete the log document
    data.firebase.delete_document(
        &format!("users/{}/immersion_logs", user_id),
        log_id
    ).await?;
    
    // Update user stats (subtract the deleted amount)
    // Fetch current stats
    if let Ok(Some(user_doc)) = data.firebase.get_document("users", user_id).await {
        let mut user_data: serde_json::Value = user_doc;
        
        if let Some(stats) = user_data.get_mut("stats") {
            if let Some(type_stats) = stats.get_mut(&activity.activity_type) {
                if let Some(total) = type_stats.get_mut("total") {
                    if let Some(t) = total.as_f64() {
                        *total = serde_json::json!(f64::max(0.0, t - activity.amount));
                    }
                }
                if let Some(sessions) = type_stats.get_mut("sessions") {
                    if let Some(s) = sessions.as_i64() {
                        *sessions = serde_json::json!(i64::max(0, s - 1));
                    }
                }
            }
        }
        
        // Update timestamps
        if let Some(timestamps) = user_data.get_mut("timestamps") {
            timestamps["updated"] = serde_json::json!(Utc::now().to_rfc3339());
        }
        
        // Save updated stats
        data.firebase.set_document("users", user_id, &user_data).await?;
    }
    
    Ok(())
}
