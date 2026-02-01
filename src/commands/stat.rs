// Stat command - view immersion statistics
// Ported from commands/stat.js

use poise::serenity_prelude as serenity;
use tracing::error;

use crate::utils::config::{colors, get_media_label, get_unit};
use crate::utils::points::calculate_points;
use crate::utils::streak;
use crate::utils::visualizations::{generate_bar_chart, generate_heatmap, BarData};
use crate::{Context, Error};
use chrono::{DateTime, Datelike};

/// Visualization type choices
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum VisualType {
    #[name = "Bar Chart"]
    Barchart,
    #[name = "Heatmap"]
    Heatmap,
}

/// Days choice for bar chart
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum DaysChoice {
    #[name = "7 days"]
    SevenDays = 7,
    #[name = "30 days"]
    ThirtyDays = 30,
}

/// View your immersion statistics
#[poise::command(slash_command, prefix_command)]
pub async fn stat(
    ctx: Context<'_>,
    #[description = "Pilih jenis visualisasi"] visual_type: Option<VisualType>,
    #[description = "Periode waktu (7 atau 30 hari)"] _days: Option<DaysChoice>,
    #[description = "Tahun untuk heatmap (default: tahun ini)"]
    #[min = 2020]
    #[max = 2030]
    _year: Option<i32>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let user = ctx.author();
    let data = ctx.data();
    let user_id = user.id.to_string();

    // Fetch user data from Firebase
    let user_doc = match data.firebase.get_document("users", &user_id).await {
        Ok(doc) => doc,
        Err(e) => {
            error!("Failed to fetch user data: {:?}", e);
            ctx.say("Failed to fetch your data. Please try again.")
                .await?;
            return Ok(());
        }
    };

    // Check if user has data
    let user_data = match user_doc {
        Some(doc) => doc,
        None => {
            let embed = serenity::CreateEmbed::new()
                .title(format!("Immersion Stats - {}", user.name))
                .description("**Total Points: 0** | **Total Sessions: 0**\n\n*Tip: Use `/stat visual_type:barchart` or `/stat visual_type:heatmap` to see visualizations!*")
                .color(colors::SUCCESS)
                .field("No data", "Start logging with `/immersion`!", false)
                .thumbnail(user.face());

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
            return Ok(());
        }
    };

    // Get stats object
    let stats = match user_data.get("stats") {
        Some(s) if s.is_object() => s,
        _ => {
            let embed = serenity::CreateEmbed::new()
                .title(format!("Immersion Stats - {}", user.name))
                .description("**Total Points: 0** | **Total Sessions: 0**\n\n*Tip: Use `/stat visual_type:barchart` or `/stat visual_type:heatmap` to see visualizations!*")
                .color(colors::SUCCESS)
                .field("No data", "Start logging with `/immersion`!", false)
                .thumbnail(user.face());

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
            return Ok(());
        }
    };

    // Get profile info
    let profile = user_data.get("profile");
    let display_name = profile
        .and_then(|p| p.get("displayName"))
        .and_then(|v| v.as_str())
        .unwrap_or(&user.name);
    let avatar = profile
        .and_then(|p| p.get("avatar"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Handle visualization types
    match visual_type {
        Some(VisualType::Heatmap) => {
            // Get immersion logs to calculate daily points
            let logs = match data
                .firebase
                .query_subcollection("users", &user_id, "immersion_logs")
                .await
            {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to fetch logs for heatmap: {:?}", e);
                    ctx.say("Failed to generate heatmap.").await?;
                    return Ok(());
                }
            };

            // Aggregate points by date - calculate from activity data
            let mut daily_points: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for log in &logs {
                // Get date (smart JST conversion)
                // Get date with fallback logic (Legacy Node.js behavior)
                let timestamps = log.get("timestamps");

                let date = if let Some(d) = timestamps
                    .and_then(|t| t.get("date"))
                    .and_then(|d| d.as_str())
                {
                    Some(d.to_string())
                } else if let Some(c) = timestamps
                    .and_then(|t| t.get("created"))
                    .and_then(|s| s.as_str())
                {
                    // Fallback to 'created' timestamp for legacy logs (UTC+7)
                    if let Ok(utc) = DateTime::parse_from_rfc3339(c) {
                        let wib_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
                        Some(
                            utc.with_timezone(&wib_offset)
                                .format("%Y-%m-%d")
                                .to_string(),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Get activity type and amount to calculate points
                let activity = log.get("activity");
                let media_type = activity
                    .and_then(|a| a.get("type"))
                    .and_then(|t| t.as_str());
                let amount = activity
                    .and_then(|a| a.get("amount"))
                    .and_then(|a| a.as_f64());

                if let (Some(date), Some(media_type), Some(amount)) = (date, media_type, amount) {
                    let points = calculate_points(media_type, amount);
                    *daily_points.entry(date.to_string()).or_insert(0) += points;
                }
            }

            // Debug: log how many entries we found
            tracing::debug!(
                "Heatmap: Found {} logs, {} unique dates with {} total points",
                logs.len(),
                daily_points.len(),
                daily_points.values().sum::<i64>()
            );

            let year = _year.unwrap_or_else(|| chrono::Utc::now().year());

            match generate_heatmap(&daily_points, year, display_name) {
                Ok(png_bytes) => {
                    let attachment = serenity::CreateAttachment::bytes(png_bytes, "heatmap.png");
                    let embed = serenity::CreateEmbed::new()
                        .title(format!("Immersion Heatmap {} - {}", year, display_name))
                        .color(colors::SUCCESS)
                        .image("attachment://heatmap.png");

                    ctx.send(
                        poise::CreateReply::default()
                            .embed(embed)
                            .attachment(attachment),
                    )
                    .await?;
                }
                Err(e) => {
                    error!("Heatmap generation failed: {}", e);
                    ctx.say("Failed to generate heatmap image.").await?;
                }
            }
            return Ok(());
        }
        Some(VisualType::Barchart) => {
            // Get days filter (default to all-time if not specified)
            let days_filter = _days.map(|d| d as i64);

            // Calculate date threshold for filtering
            let cutoff_date = days_filter.map(|d| {
                let now = chrono::Utc::now();
                (now - chrono::Duration::days(d))
                    .format("%Y-%m-%d")
                    .to_string()
            });

            // Fetch immersion logs
            let logs = match data
                .firebase
                .query_subcollection("users", &user_id, "immersion_logs")
                .await
            {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to fetch logs for barchart: {:?}", e);
                    ctx.say("Failed to generate chart.").await?;
                    return Ok(());
                }
            };

            // Aggregate points by media type with date filter
            let mut media_points: std::collections::HashMap<String, f64> =
                std::collections::HashMap::new();

            for log in &logs {
                // Get date (smart JST conversion)
                // Get date with fallback logic (Legacy Node.js behavior)
                let timestamps = log.get("timestamps");

                let date = if let Some(d) = timestamps
                    .and_then(|t| t.get("date"))
                    .and_then(|d| d.as_str())
                {
                    Some(d.to_string())
                } else if let Some(c) = timestamps
                    .and_then(|t| t.get("created"))
                    .and_then(|s| s.as_str())
                {
                    // Fallback to 'created' timestamp for legacy logs (UTC+7)
                    if let Ok(utc) = DateTime::parse_from_rfc3339(c) {
                        let wib_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
                        Some(
                            utc.with_timezone(&wib_offset)
                                .format("%Y-%m-%d")
                                .to_string(),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Apply date filter if specified
                if let Some(ref cutoff) = cutoff_date {
                    if let Some(ref log_date) = date {
                        if log_date.as_str() < cutoff.as_str() {
                            continue; // Skip logs before cutoff
                        }
                    }
                }

                // Get activity type and amount
                let activity = log.get("activity");
                let media_type = activity
                    .and_then(|a| a.get("type"))
                    .and_then(|t| t.as_str());
                let amount = activity
                    .and_then(|a| a.get("amount"))
                    .and_then(|a| a.as_f64());

                if let (Some(media_type), Some(amount)) = (media_type, amount) {
                    let points = calculate_points(media_type, amount) as f64;
                    *media_points.entry(media_type.to_string()).or_insert(0.0) += points;
                }
            }

            // All supported media types - initialize with 0
            let all_media_types = [
                "anime",
                "listening",
                "reading",
                "manga",
                "visual_novel",
                "book",
                "reading_time",
            ];
            for mt in &all_media_types {
                media_points.entry(mt.to_string()).or_insert(0.0);
            }

            // Build bar chart data (include all, even 0 values)
            let mut bar_data: Vec<BarData> = media_points
                .iter()
                .map(|(media_type, &value)| BarData {
                    label: get_media_label(media_type).to_string(),
                    value,
                    media_type: media_type.clone(),
                })
                .collect();

            // Sort by value descending
            bar_data.sort_by(|a, b| {
                b.value
                    .partial_cmp(&a.value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Check if there's ANY data at all
            let has_any_data = bar_data.iter().any(|d| d.value > 0.0);
            if !has_any_data {
                let period_text = match days_filter {
                    Some(7) => "last 7 days",
                    Some(30) => "last 30 days",
                    _ => "all time",
                };
                ctx.say(format!("No immersion data found for {}.", period_text))
                    .await?;
                return Ok(());
            }

            // Create title with period info
            let title = match days_filter {
                Some(7) => format!("Stats (7 Days) - {}", display_name),
                Some(30) => format!("Stats (30 Days) - {}", display_name),
                _ => format!("Stats - {}", display_name),
            };

            match generate_bar_chart(&bar_data, &title, "Points") {
                Ok(png_bytes) => {
                    let attachment = serenity::CreateAttachment::bytes(png_bytes, "chart.png");
                    let embed = serenity::CreateEmbed::new()
                        .title(format!("Immersion Chart - {}", display_name))
                        .color(colors::SUCCESS)
                        .image("attachment://chart.png");

                    ctx.send(
                        poise::CreateReply::default()
                            .embed(embed)
                            .attachment(attachment),
                    )
                    .await?;
                }
                Err(e) => {
                    error!("Bar chart generation failed: {}", e);
                    ctx.say("Failed to generate chart image.").await?;
                }
            }
            return Ok(());
        }
        None => {
            // Default: show text stats
        }
    }

    // Calculate stats
    let stats_obj = stats.as_object().unwrap();
    let mut total_points: i64 = 0;
    let mut total_sessions: i64 = 0;
    let mut stat_entries: Vec<StatEntry> = Vec::new();

    for (media_type, media_stats) in stats_obj {
        let total = media_stats
            .get("total")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let sessions = media_stats
            .get("sessions")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if total > 0.0 {
            let points = calculate_points(media_type, total);
            total_points += points;
            total_sessions += sessions;

            stat_entries.push(StatEntry {
                label: get_media_label(media_type).to_string(),
                total,
                unit: get_unit(media_type).to_string(),
                sessions,
                points,
            });
        }
    }

    // Sort by points (highest first)
    stat_entries.sort_by(|a, b| b.points.cmp(&a.points));

    // Calculate streaks
    let (current_streak, longest_streak) = {
        let logs = match data
            .firebase
            .query_subcollection("users", &user_id, "immersion_logs")
            .await
        {
            Ok(l) => l,
            Err(_) => Vec::new(),
        };

        let dates: Vec<String> = logs
            .iter()
            .filter_map(|log| {
                let timestamps = log.get("timestamps")?;

                // Try explicit date first
                if let Some(d) = timestamps.get("date").and_then(|v| v.as_str()) {
                    return Some(d.to_string());
                }

                // Fallback to created timestamp (UTC+7)
                if let Some(c) = timestamps.get("created").and_then(|v| v.as_str()) {
                    if let Ok(utc) = DateTime::parse_from_rfc3339(c) {
                        let wib_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
                        return Some(
                            utc.with_timezone(&wib_offset)
                                .format("%Y-%m-%d")
                                .to_string(),
                        );
                    }
                }

                None
            })
            .collect();

        let result = streak::calculate_streak(&dates);
        (result.current, result.longest)
    };

    // Build stats text (grouped in one field)
    let mut stats_text = String::new();
    for stat in &stat_entries {
        stats_text.push_str(&format!(
            "**{}**: {} {}\n",
            stat.label,
            format_number_f64(stat.total),
            stat.unit
        ));
    }

    if stats_text.is_empty() {
        stats_text = "*No data yet*".to_string();
    }

    // Build embed
    let mut embed = serenity::CreateEmbed::new()
        .title(format!("Immersion Stats - {}", display_name))
        .description(format!(
            "**{}** pts | **{}** sessions\nStreak: **{}** days | Best: **{}** days",
            format_number(total_points),
            total_sessions,
            current_streak,
            longest_streak
        ))
        .field("Stats", stats_text, false)
        .color(colors::SUCCESS);

    // Add avatar
    if let Some(ref avatar_url) = avatar {
        if !avatar_url.is_empty() {
            embed = embed.thumbnail(avatar_url);
        }
    } else {
        embed = embed.thumbnail(user.face());
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}

#[derive(Debug)]
struct StatEntry {
    label: String,
    total: f64,
    unit: String,
    #[allow(dead_code)]
    sessions: i64,
    points: i64,
}

/// Format a number with locale-aware thousands separators
fn format_number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 && *c != '-' {
            result.push(',');
        }
        result.push(*c);
    }

    result
}

/// Format a float number
fn format_number_f64(n: f64) -> String {
    if n == n.trunc() {
        format_number(n as i64)
    } else {
        format!("{:.1}", n)
    }
}

// Local calculate_user_streaks removed in favor of utils::streak::calculate_streak
