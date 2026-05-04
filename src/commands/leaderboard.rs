// Leaderboard command - view community rankings
// Ported from commands/leaderboard.js

use crate::utils::config::colors;
use crate::utils::points::calculate_points;
use crate::{Context, Error};
use chrono::{DateTime, Datelike, Duration, NaiveDate};
use poise::serenity_prelude as serenity;
use serde_json::Value;
use tracing::error;

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
    #[description = "Time period for the leaderboard"] timestamp: TimePeriod,
    #[description = "Media type for the leaderboard"] media_type: LeaderboardMediaType,
    #[description = "Month (for monthly leaderboard)"] month: Option<MonthChoice>,
    #[description = "Year"]
    #[min = 2020]
    year: Option<i32>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let media_type_filter = media_type.as_str();
    let effective_date = crate::utils::config::get_effective_date();
    let period_filter = PeriodFilter::new(timestamp, month, year, effective_date);
    let title = period_filter.title();

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

    let mut leaderboard: Vec<LeaderboardEntry> = Vec::new();

    for user_doc in users {
        let user_id = user_doc.get("_id").and_then(|v| v.as_str()).unwrap_or("");
        if user_id.is_empty() {
            continue;
        }

        let profile = user_doc.get("profile");
        let display_name = profile
            .and_then(|p| p.get("displayName"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                profile
                    .and_then(|p| p.get("username"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("Unknown");

        let total_points = if matches!(timestamp, TimePeriod::AllTime) {
            calculate_all_time_points(&user_doc, media_type_filter)
        } else {
            calculate_interval_points(data, user_id, &period_filter, media_type_filter).await
        };

        if total_points > 0.0 {
            leaderboard.push(LeaderboardEntry {
                display_name: display_name.to_string(),
                points: total_points,
            });
        }
    }

    leaderboard.sort_by(|a, b| {
        b.points
            .partial_cmp(&a.points)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if leaderboard.is_empty() {
        let embed = serenity::CreateEmbed::new()
            .title(format!("{} ({})", title, media_type.label()))
            .description(format!(
                "No immersion data found for the **{}** period and **{}** media type.",
                timestamp.label(),
                media_type.label()
            ))
            .color(colors::INFO);

        ctx.send(poise::CreateReply::default().embed(embed)).await?;
        return Ok(());
    }

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

fn calculate_all_time_points(user_doc: &Value, media_type_filter: Option<&str>) -> f64 {
    let stats = match user_doc.get("stats") {
        Some(s) if s.is_object() => s,
        _ => return 0.0,
    };

    let mut total_points = 0.0;
    if let Some(stats_obj) = stats.as_object() {
        for (media_type, data) in stats_obj {
            if let Some(filter) = media_type_filter {
                if media_type != filter {
                    continue;
                }
            }

            let amount = data.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if amount > 0.0 {
                total_points += calculate_points(media_type, amount) as f64;
            }
        }
    }

    total_points
}

async fn calculate_interval_points(
    data: &crate::Data,
    user_id: &str,
    period_filter: &PeriodFilter,
    media_type_filter: Option<&str>,
) -> f64 {
    let logs = match data
        .firebase
        .query_subcollection("users", user_id, "immersion_logs")
        .await
    {
        Ok(logs) => logs,
        Err(e) => {
            error!(
                "Failed to fetch immersion logs for user {}: {:?}",
                user_id, e
            );
            return 0.0;
        }
    };

    let mut total_points = 0.0;

    for log in logs {
        if !period_filter.matches_log(&log) {
            continue;
        }

        let activity = match log.get("activity") {
            Some(activity) if activity.is_object() => activity,
            _ => continue,
        };

        let log_media_type = match activity.get("type").and_then(|v| v.as_str()) {
            Some(media_type) => media_type,
            None => continue,
        };

        if let Some(filter) = media_type_filter {
            if log_media_type != filter {
                continue;
            }
        }

        let amount = activity
            .get("amount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        if amount > 0.0 {
            total_points += calculate_points(log_media_type, amount) as f64;
        }
    }

    total_points
}

fn extract_log_date(log: &Value) -> Option<NaiveDate> {
    let timestamps = log.get("timestamps")?;

    if let Some(date) = timestamps.get("date").and_then(|v| v.as_str()) {
        if let Ok(parsed) = NaiveDate::parse_from_str(date, "%Y-%m-%d") {
            return Some(parsed);
        }
    }

    if let Some(created) = timestamps.get("created").and_then(|v| v.as_str()) {
        if let Ok(created_utc) = DateTime::parse_from_rfc3339(created) {
            let wib_offset = chrono::FixedOffset::east_opt(7 * 3600)?;
            return Some(created_utc.with_timezone(&wib_offset).date_naive());
        }
    }

    None
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

struct PeriodFilter {
    period: TimePeriod,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    month: Option<(i32, u32)>,
    year: Option<i32>,
}

impl PeriodFilter {
    fn new(
        period: TimePeriod,
        month: Option<MonthChoice>,
        year: Option<i32>,
        effective_date: NaiveDate,
    ) -> Self {
        match period {
            TimePeriod::Weekly => Self {
                period,
                start: Some(effective_date - Duration::days(6)),
                end: Some(effective_date),
                month: None,
                year: None,
            },
            TimePeriod::Monthly => Self {
                period,
                start: None,
                end: None,
                month: Some((
                    year.unwrap_or(effective_date.year()),
                    month.map(|m| m as u32).unwrap_or(effective_date.month()),
                )),
                year: None,
            },
            TimePeriod::Yearly => Self {
                period,
                start: None,
                end: None,
                month: None,
                year: Some(year.unwrap_or(effective_date.year())),
            },
            TimePeriod::AllTime => Self {
                period,
                start: None,
                end: None,
                month: None,
                year: None,
            },
        }
    }

    fn title(&self) -> String {
        match self.period {
            TimePeriod::Weekly => {
                let start = self.start.expect("weekly period missing start date");
                let end = self.end.expect("weekly period missing end date");
                format!(
                    "Weekly Leaderboard - {} to {}",
                    start.format("%Y-%m-%d"),
                    end.format("%Y-%m-%d")
                )
            }
            TimePeriod::Monthly => {
                let (year, month) = self.month.expect("monthly period missing month");
                format!("Monthly Leaderboard - {} {}", month_name(month), year)
            }
            TimePeriod::Yearly => {
                let year = self.year.expect("yearly period missing year");
                format!("Yearly Leaderboard - {}", year)
            }
            TimePeriod::AllTime => "All-time Leaderboard".to_string(),
        }
    }

    fn matches_log(&self, log: &Value) -> bool {
        match self.period {
            TimePeriod::AllTime => true,
            TimePeriod::Weekly => {
                let date = match extract_log_date(log) {
                    Some(date) => date,
                    None => return false,
                };

                let start = self.start.expect("weekly period missing start date");
                let end = self.end.expect("weekly period missing end date");
                date >= start && date <= end
            }
            TimePeriod::Monthly => {
                let date = match extract_log_date(log) {
                    Some(date) => date,
                    None => return false,
                };

                let (year, month) = self.month.expect("monthly period missing month");
                date.year() == year && date.month() == month
            }
            TimePeriod::Yearly => {
                let date = match extract_log_date(log) {
                    Some(date) => date,
                    None => return false,
                };

                let year = self.year.expect("yearly period missing year");
                date.year() == year
            }
        }
    }
}

struct LeaderboardEntry {
    display_name: String,
    points: f64,
}
