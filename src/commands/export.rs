// Export command - export immersion logs as text file
// Ported from commands/export.js

use poise::serenity_prelude as serenity;
use chrono::{DateTime, Utc, Duration};
use tracing::error;

use crate::utils::config::get_media_label;
use crate::{Context, Error};

/// Timeframe options for export
#[derive(Debug, poise::ChoiceParameter)]
pub enum Timeframe {
    #[name = "Last 24 Hours"]
    Day,
    #[name = "Last 7 Days"]
    Week,
    #[name = "Last 30 Days"]
    Month,
    #[name = "Last 365 Days"]
    Year,
    #[name = "All Time"]
    All,
}

impl Timeframe {
    fn as_str(&self) -> &'static str {
        match self {
            Timeframe::Day => "Last 24 Hours",
            Timeframe::Week => "Last 7 Days",
            Timeframe::Month => "Last 30 Days",
            Timeframe::Year => "Last 365 Days",
            Timeframe::All => "All Time",
        }
    }

    fn get_start_date(&self) -> DateTime<Utc> {
        let now = Utc::now();
        match self {
            Timeframe::Day => now - Duration::days(1),
            Timeframe::Week => now - Duration::days(7),
            Timeframe::Month => now - Duration::days(30),
            Timeframe::Year => now - Duration::days(365),
            Timeframe::All => DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        }
    }
}

/// Media type filter for export
#[derive(Debug, poise::ChoiceParameter)]
pub enum ExportMediaType {
    #[name = "All Types"]
    All,
    #[name = "Visual Novel"]
    VisualNovel,
    #[name = "Book"]
    Book,
    #[name = "Reading"]
    Reading,
    #[name = "Reading Time"]
    ReadingTime,
    #[name = "Manga"]
    Manga,
    #[name = "Anime"]
    Anime,
    #[name = "Listening"]
    Listening,
}

impl ExportMediaType {
    fn as_str(&self) -> Option<&'static str> {
        match self {
            ExportMediaType::All => None,
            ExportMediaType::VisualNovel => Some("visual_novel"),
            ExportMediaType::Book => Some("book"),
            ExportMediaType::Reading => Some("reading"),
            ExportMediaType::ReadingTime => Some("reading_time"),
            ExportMediaType::Manga => Some("manga"),
            ExportMediaType::Anime => Some("anime"),
            ExportMediaType::Listening => Some("listening"),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            ExportMediaType::All => "All Types",
            ExportMediaType::VisualNovel => "Visual Novel",
            ExportMediaType::Book => "Book",
            ExportMediaType::Reading => "Reading",
            ExportMediaType::ReadingTime => "Reading Time",
            ExportMediaType::Manga => "Manga",
            ExportMediaType::Anime => "Anime",
            ExportMediaType::Listening => "Listening",
        }
    }
}

/// Export your immersion logs as a text file
#[poise::command(slash_command, prefix_command)]
pub async fn export(
    ctx: Context<'_>,
    #[description = "Timeframe to export logs"]
    timeframe: Timeframe,
    #[description = "Filter by media type (optional)"]
    mediatype: Option<ExportMediaType>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let user = ctx.author();
    let user_id = user.id.to_string();
    let firebase = &ctx.data().firebase;
    let media_filter = mediatype.unwrap_or(ExportMediaType::All);

    // Fetch user logs from Firebase subcollection
    let logs_result = firebase.query_subcollection("users", &user_id, "immersion_logs").await;
    
    let all_logs: Vec<serde_json::Value> = match logs_result {
        Ok(logs) => logs,
        Err(e) => {
            error!("Failed to fetch logs: {:?}", e);
            ctx.say("Failed to export logs. Please try again later.").await?;
            return Ok(());
        }
    };

    // Filter logs by timeframe and media type
    let start_date = timeframe.get_start_date();
    let media_type_str = media_filter.as_str();

    let filtered_logs: Vec<&serde_json::Value> = all_logs
        .iter()
        .filter(|log| {
            // Filter by timestamp
            let created = log
                .get("timestamps")
                .and_then(|t| t.get("created"))
                .and_then(|c| c.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(Utc::now());

            if created < start_date {
                return false;
            }

            // Filter by media type
            if let Some(filter_type) = media_type_str {
                let log_type = log
                    .get("activity")
                    .and_then(|a| a.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                
                if log_type != filter_type {
                    return false;
                }
            }

            true
        })
        .collect();

    // Generate export content
    let content = generate_export_content(&filtered_logs, &timeframe, &media_filter, &user.name);

    // Create filename
    let timeframe_label = match timeframe {
        Timeframe::Day => "24h",
        Timeframe::Week => "7d",
        Timeframe::Month => "30d",
        Timeframe::Year => "365d",
        Timeframe::All => "all",
    };
    let media_label = media_filter.as_str().unwrap_or("all");
    let filename = format!("immersion_logs_{}_{}_{}_{}.txt", 
        user.name, 
        timeframe_label, 
        media_label,
        Utc::now().format("%Y%m%d")
    );

    // Create attachment
    let attachment = serenity::CreateAttachment::bytes(content.as_bytes().to_vec(), filename);

    // Send file
    let media_type_text = if media_filter.as_str().is_some() {
        format!(" ({})", media_filter.label())
    } else {
        String::new()
    };

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                "**{}'s** immersion log export for {}{}:",
                user.name,
                timeframe.as_str(),
                media_type_text
            ))
            .attachment(attachment)
    ).await?;

    Ok(())
}

fn generate_export_content(
    logs: &[&serde_json::Value],
    timeframe: &Timeframe,
    media_type: &ExportMediaType,
    username: &str,
) -> String {
    let mut content = String::new();
    
    content.push_str("Immersion Logs Export\n");
    content.push_str("====================\n\n");
    content.push_str(&format!("User: {}\n", username));
    content.push_str(&format!("Timeframe: {}\n", timeframe.as_str()));
    content.push_str(&format!("Media Type: {}\n", media_type.label()));
    content.push_str(&format!("Total Logs: {}\n", logs.len()));
    content.push_str(&format!("Export Date: {}\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

    if logs.is_empty() {
        content.push_str("No immersion logs found for the selected timeframe and media type.\n");
        return content;
    }

    // Summary statistics
    use std::collections::HashMap;
    let mut stats: HashMap<String, (i32, f64)> = HashMap::new();

    for log in logs {
        if let Some(activity) = log.get("activity") {
            let log_type = activity.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
            let amount = activity.get("amount").and_then(|a| a.as_f64()).unwrap_or(0.0);

            let entry = stats.entry(log_type.to_string()).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += amount;
        }
    }

    content.push_str("Summary Statistics:\n");
    content.push_str("------------------\n");
    for (type_name, (count, total)) in &stats {
        let label = get_media_label(type_name);
        let unit = get_unit_for_type(type_name);
        content.push_str(&format!("{}: {} sessions, {:.1} total {}\n", label, count, total, unit));
    }
    content.push_str("\n\n");

    // Detailed logs
    content.push_str("Detailed Logs:\n");
    content.push_str("-------------\n");

    for (index, log) in logs.iter().enumerate() {
        if let Some(activity) = log.get("activity") {
            let amount = activity.get("amount").and_then(|a| a.as_f64()).unwrap_or(0.0);
            let unit = activity.get("unit").and_then(|u| u.as_str()).unwrap_or("");
            let type_label = activity.get("typeLabel").and_then(|t| t.as_str()).unwrap_or("Unknown");
            let title = activity.get("title").and_then(|t| t.as_str()).unwrap_or("-");

            content.push_str(&format!("{}. {:.0} {} of {}\n", index + 1, amount, unit, type_label));
            
            if title != "-" && !title.is_empty() {
                content.push_str(&format!("   Title: {}\n", title));
            }

            if let Some(timestamps) = log.get("timestamps") {
                if let Some(created) = timestamps.get("created").and_then(|c| c.as_str()) {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(created) {
                        content.push_str(&format!("   Date: {}\n", dt.format("%Y-%m-%d %H:%M")));
                    }
                }
            }

            if let Some(note) = log.get("note").and_then(|n| n.as_str()) {
                if !note.is_empty() {
                    content.push_str(&format!("   Note: {}\n", note));
                }
            }

            content.push_str("\n");
        }
    }

    content
}

fn get_unit_for_type(media_type: &str) -> &'static str {
    match media_type {
        "anime" => "episodes",
        "manga" => "pages",
        "visual_novel" | "book" | "reading" => "characters",
        "reading_time" | "listening" => "minutes",
        _ => "units",
    }
}
