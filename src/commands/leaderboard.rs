// Leaderboard command - view community rankings
// Ported from commands/leaderboard.js

use poise::serenity_prelude as serenity;
use tracing::error;
use crate::utils::config::colors;
use crate::utils::points::calculate_points;
use crate::{Context, Error};

/// Time period for leaderboard
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum TimePeriod {
    #[name = "Weekly"]
    Weekly,
    #[name = "Monthly"]
    Monthly,
    #[name = "Yearly"]
    Yearly,
    #[name = "All-time"]
    AllTime,
}

impl TimePeriod {
    fn label(&self) -> &'static str {
        match self {
            TimePeriod::Weekly => "Weekly",
            TimePeriod::Monthly => "Monthly",
            TimePeriod::Yearly => "Yearly",
            TimePeriod::AllTime => "All-time",
        }
    }
}

/// Media type filter for leaderboard
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum LeaderboardMediaType {
    #[name = "All Media"]
    All,
    #[name = "Visual Novel"]
    VisualNovel,
    #[name = "Manga"]
    Manga,
    #[name = "Anime"]
    Anime,
    #[name = "Book"]
    Book,
    #[name = "Reading Time"]
    ReadingTime,
    #[name = "Listening"]
    Listening,
    #[name = "Reading"]
    Reading,
}

impl LeaderboardMediaType {
    fn as_str(&self) -> Option<&'static str> {
        match self {
            LeaderboardMediaType::All => None,
            LeaderboardMediaType::VisualNovel => Some("visual_novel"),
            LeaderboardMediaType::Manga => Some("manga"),
            LeaderboardMediaType::Anime => Some("anime"),
            LeaderboardMediaType::Book => Some("book"),
            LeaderboardMediaType::ReadingTime => Some("reading_time"),
            LeaderboardMediaType::Listening => Some("listening"),
            LeaderboardMediaType::Reading => Some("reading"),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            LeaderboardMediaType::All => "All Media",
            LeaderboardMediaType::VisualNovel => "Visual Novel",
            LeaderboardMediaType::Manga => "Manga",
            LeaderboardMediaType::Anime => "Anime",
            LeaderboardMediaType::Book => "Book",
            LeaderboardMediaType::ReadingTime => "Reading Time",
            LeaderboardMediaType::Listening => "Listening",
            LeaderboardMediaType::Reading => "Reading",
        }
    }
}

/// Month choice for leaderboard
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum MonthChoice {
    #[name = "January"]
    January = 1,
    #[name = "February"]
    February = 2,
    #[name = "March"]
    March = 3,
    #[name = "April"]
    April = 4,
    #[name = "May"]
    May = 5,
    #[name = "June"]
    June = 6,
    #[name = "July"]
    July = 7,
    #[name = "August"]
    August = 8,
    #[name = "September"]
    September = 9,
    #[name = "October"]
    October = 10,
    #[name = "November"]
    November = 11,
    #[name = "December"]
    December = 12,
}

/// View the immersion leaderboard
#[poise::command(slash_command, prefix_command)]
pub async fn leaderboard(
    ctx: Context<'_>,
    #[description = "Time period for the leaderboard"]
    timestamp: TimePeriod,
    #[description = "Media type for the leaderboard"]
    media_type: LeaderboardMediaType,
    #[description = "Month (for monthly leaderboard)"]
    month: Option<MonthChoice>,
    #[description = "Year"]
    #[min = 2020]
    year: Option<i32>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let media_type_filter = media_type.as_str();

    // Build title
    let mut title = format!("{} Leaderboard", timestamp.label());
    if let Some(m) = month {
        let month_names = ["", "January", "February", "March", "April", "May", "June",
                           "July", "August", "September", "October", "November", "December"];
        let y = year.unwrap_or_else(|| chrono::Utc::now().year());
        title = format!("{} - {} {}", title, month_names[m as usize], y);
    } else if let Some(y) = year {
        if matches!(timestamp, TimePeriod::Yearly) {
            title = format!("{} - {}", title, y);
        }
    }

    // Fetch all users
    let users = match data.firebase.get_all_users().await {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to fetch users: {:?}", e);
            ctx.say("Failed to fetch leaderboard data.").await?;
            return Ok(());
        }
    };

    if users.is_empty() {
        ctx.say("No immersion data recorded yet.").await?;
        return Ok(());
    }

    // For all_time, use stats from user doc directly
    let mut leaderboard: Vec<LeaderboardEntry> = Vec::new();

    for user_doc in users {
        let _user_id = user_doc.get("_id").and_then(|v| v.as_str()).unwrap_or("");
        let profile = user_doc.get("profile");
        let display_name = profile
            .and_then(|p| p.get("displayName"))
            .and_then(|v| v.as_str())
            .or_else(|| profile.and_then(|p| p.get("username")).and_then(|v| v.as_str()))
            .unwrap_or("Unknown");

        if matches!(timestamp, TimePeriod::AllTime) {
            // Use aggregated stats
            let stats = match user_doc.get("stats") {
                Some(s) if s.is_object() => s,
                _ => continue,
            };

            let mut total_points: f64 = 0.0;
            let mut total_amount: f64 = 0.0;

            if let Some(stats_obj) = stats.as_object() {
                for (mt, data) in stats_obj {
                    // Filter by media type if specified
                    if let Some(filter) = media_type_filter {
                        if mt != filter {
                            continue;
                        }
                    }

                    let amount = data.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if amount > 0.0 {
                        total_points += calculate_points(mt, amount) as f64;
                        if media_type_filter.is_some() {
                            total_amount += amount;
                        }
                    }
                }
            }

            if total_points > 0.0 {
                leaderboard.push(LeaderboardEntry {
                    display_name: display_name.to_string(),
                    points: total_points,
                    amount: total_amount,
                });
            }
        } else {
            // For weekly/monthly/yearly, we would need to query immersion_logs
            // For now, use all_time stats as placeholder
            let stats = match user_doc.get("stats") {
                Some(s) if s.is_object() => s,
                _ => continue,
            };

            let mut total_points: f64 = 0.0;

            if let Some(stats_obj) = stats.as_object() {
                for (mt, data) in stats_obj {
                    if let Some(filter) = media_type_filter {
                        if mt != filter {
                            continue;
                        }
                    }

                    let amount = data.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if amount > 0.0 {
                        total_points += calculate_points(mt, amount) as f64;
                    }
                }
            }

            if total_points > 0.0 {
                leaderboard.push(LeaderboardEntry {
                    display_name: display_name.to_string(),
                    points: total_points,
                    amount: 0.0,
                });
            }
        }
    }

    // Sort by points
    leaderboard.sort_by(|a, b| b.points.partial_cmp(&a.points).unwrap_or(std::cmp::Ordering::Equal));

    if leaderboard.is_empty() {
        let embed = serenity::CreateEmbed::new()
            .title(format!("{} ({})", title, media_type.label()))
            .description(format!("No immersion data found for the **{}** period and **{}** media type.", 
                timestamp.label(), media_type.label()))
            .color(colors::INFO);

        ctx.send(poise::CreateReply::default().embed(embed)).await?;
        return Ok(());
    }

    // Build leaderboard description
    let mut description = String::from("Here's the list of top immersionists:\n\n");
    let top_count = leaderboard.len().min(10);

    for (i, entry) in leaderboard.iter().take(top_count).enumerate() {
        description.push_str(&format!(
            "**#{}. {}**: {:.2} Pts\n",
            i + 1,
            entry.display_name,
            entry.points
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("{} ({})", title, media_type.label()))
        .description(description)
        .color(colors::PRIMARY);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}

use chrono::Datelike;

struct LeaderboardEntry {
    display_name: String,
    points: f64,
    #[allow(dead_code)]
    amount: f64,
}
