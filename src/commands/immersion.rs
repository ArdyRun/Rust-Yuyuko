// Immersion command - log immersion activities
// Ported from commands/immersion.js

use chrono::Datelike;
use poise::serenity_prelude as serenity;
use serde_json::json;
use tracing::{debug, error};

use crate::api::{anilist, vndb, youtube};
use crate::utils::config::{colors, get_effective_date, get_media_label, get_unit};
use crate::utils::points::calculate_points;
use crate::utils::streak;
use crate::{Context, Error};
use chrono::{DateTime, NaiveDate};

/// Media type choices for the command
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum MediaType {
    #[name = "Visual Novel (characters)"]
    VisualNovel,
    #[name = "Manga (pages)"]
    Manga,
    #[name = "Anime (episodes)"]
    Anime,
    #[name = "Book (pages)"]
    Book,
    #[name = "Reading Time (minutes)"]
    ReadingTime,
    #[name = "Listening (minutes)"]
    Listening,
    #[name = "Reading (characters)"]
    Reading,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::VisualNovel => "visual_novel",
            MediaType::Manga => "manga",
            MediaType::Anime => "anime",
            MediaType::Book => "book",
            MediaType::ReadingTime => "reading_time",
            MediaType::Listening => "listening",
            MediaType::Reading => "reading",
        }
    }
}

/// Log your Japanese immersion activity
#[poise::command(slash_command, prefix_command)]
pub async fn immersion(
    ctx: Context<'_>,
    #[description = "Type of media"] media_type: MediaType,
    #[description = "Amount (episodes, pages, minutes, characters)"]
    #[min = 1]
    #[max = 100000]
    amount: f64,
    #[description = "Title of the media"]
    #[autocomplete = "autocomplete_title"]
    title: Option<String>,
    #[description = "Optional comment"] comment: Option<String>,
    #[description = "Custom date (YYYY-MM-DD)"] date: Option<String>,
    #[description = "YouTube URL (for listening)"] url: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;

    // Check channel restriction
    if let Some(guild_id) = ctx.guild_id() {
        let data = ctx.data();
        let gid = guild_id.to_string();

        let config = if let Some(cached) = data.guild_configs.get(&gid) {
            Some(cached.clone())
        } else {
            // Try fetch if not in cache (though cache should be populated on startup or first activity)
            // For now, simple cache check or fetch
            match data.firebase.get_document("guilds", &gid).await {
                Ok(Some(doc)) => {
                    let cfg = serde_json::from_value::<crate::models::guild::GuildConfig>(doc)
                        .unwrap_or_default();
                    data.guild_configs.insert(gid.clone(), cfg.clone());
                    Some(cfg)
                }
                _ => None,
            }
        };

        if let Some(cfg) = config {
            if let Some(allowed_channel_id) = cfg.immersion_channel_id {
                let current_channel = ctx.channel_id().to_string();
                if current_channel != allowed_channel_id {
                    ctx.send(
                        poise::CreateReply::default()
                            .content(format!(
                                "Command ini hanya bisa digunakan di <#{}>.",
                                allowed_channel_id
                            ))
                            .ephemeral(true),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
    }

    let user = ctx.author();
    let data = ctx.data();
    let media_type_str = media_type.as_str();
    let label = get_media_label(media_type_str);
    let unit = get_unit(media_type_str);
    // Initialize variables
    let mut raw_title = title.unwrap_or_else(|| "-".to_string());
    let mut final_amount = amount;
    let mut thumbnail = None;
    let mut log_url = None;
    let mut anilist_url = None;
    let mut vndb_url = None;
    let mut source = "manual";
    let mut vndb_metadata = None;
    let mut warning_msg = None;

    // 1. Handle Listening (YouTube) - Interactive flow
    if let MediaType::Listening = media_type {
        let url_str = if let Some(ref u) = url {
            // URL provided directly via parameter
            Some(u.clone())
        } else {
            // No URL provided - prompt user to paste in chat
            let prompt_embed = serenity::CreateEmbed::new()
                .title("Input YouTube Link")
                .description("Paste your YouTube link below\n\n*Timeout in 60 seconds*")
                .color(colors::IMMERSION);

            let prompt_reply = ctx
                .send(poise::CreateReply::default().embed(prompt_embed))
                .await?;

            // Wait for user's next message in this channel
            let channel_id = ctx.channel_id();
            let author_id = ctx.author().id;
            let http = ctx.serenity_context().http.clone();

            // Use serenity's message collector
            use futures::StreamExt;
            let mut collector =
                serenity::collector::MessageCollector::new(ctx.serenity_context().shard.clone())
                    .channel_id(channel_id)
                    .author_id(author_id)
                    .timeout(std::time::Duration::from_secs(60))
                    .stream();

            let user_url_msg = collector.next().await;

            if let Some(msg) = user_url_msg {
                let url_content = msg.content.clone();

                // Delete user's message - requires Manage Messages permission
                if let Err(e) = msg.delete(&http).await {
                    error!("Failed to delete user YouTube link message: {:?}", e);
                    // If it's a permission error, we can't do much but log it.
                    warning_msg = Some("⚠️ Notice: Could not auto-delete link (Missing 'Manage Messages' permission)");
                }

                // Delete prompt embed using poise's handle
                let _ = prompt_reply.delete(ctx).await;

                Some(url_content)
            } else {
                // Timeout - update embed
                let timeout_embed = serenity::CreateEmbed::new()
                    .title("Timeout")
                    .description("No YouTube link received. Please try again.")
                    .color(0xFF0000);

                let _ = prompt_reply
                    .edit(ctx, poise::CreateReply::default().embed(timeout_embed))
                    .await;
                return Ok(());
            }
        };

        if let Some(url_str) = url_str {
            if let Some(video_id) = youtube::extract_video_id(&url_str) {
                let yt_key = std::env::var("YOUTUBE_API_KEY").unwrap_or_default();
                match youtube::get_video_info(&data.http_client, &yt_key, &video_id).await {
                    Ok(Some(info)) => {
                        raw_title = info.title;
                        final_amount = (info.duration_seconds as f64 / 60.0).ceil(); // Convert to minutes
                        thumbnail = info.thumbnail;
                        log_url = Some(youtube::normalize_url(&video_id));
                        source = "youtube";
                    }
                    Ok(None) => debug!("Video not found"),
                    Err(e) => error!("YouTube API error: {:?}", e),
                }
            }
        }
    }

    // 1.5. Handle Reading/ReadingTime with URL (Article/News)
    if matches!(media_type, MediaType::Reading | MediaType::ReadingTime) {
        if let Some(ref url_str) = url {
            // Validate URL
            if url_str.starts_with("http://") || url_str.starts_with("https://") {
                // Fetch title from webpage
                match fetch_page_title(&data.http_client, url_str).await {
                    Ok(Some(page_title)) => {
                        raw_title = page_title;
                        log_url = Some(url_str.clone());
                        source = "web";
                    }
                    Ok(None) => {
                        debug!("Could not extract title from URL");
                        // Still set the URL even if title extraction failed
                        log_url = Some(url_str.clone());
                        source = "web";
                    }
                    Err(e) => {
                        error!("Failed to fetch page title: {:?}", e);
                        // Still set the URL even if fetch failed
                        log_url = Some(url_str.clone());
                        source = "web";
                    }
                }
            }
        }
    }

    // 2. Handle Visual Novel (VNDB)
    if let MediaType::VisualNovel = media_type {
        if raw_title != "-" {
            // Check if title contains ID (from autocomplete: "Title|ID")
            if let Some((_, id_part)) = raw_title.rsplit_once('|') {
                if let Ok(Some(vn)) = vndb::get_vn_by_id(&data.http_client, id_part).await {
                    raw_title = vn.title;
                    thumbnail = vn.image;
                    vndb_url = Some(vn.url);
                    source = "vndb";
                    vndb_metadata = Some(json!({
                        "developer": vn.developer,
                        "released": vn.released,
                        "length": vn.length,
                        "description": vn.description
                    }));
                }
            } else {
                // Fallback search by title
                if let Ok(vns) = vndb::search_vns(&data.http_client, &raw_title, 1).await {
                    if let Some(vn) = vns.first() {
                        raw_title = vn.title.clone();
                        thumbnail = vn.image.clone();
                        vndb_url = Some(vn.url.clone());
                        source = "vndb";
                        vndb_metadata = Some(json!({
                            "developer": vn.developer,
                            "released": vn.released,
                            "length": vn.length,
                            "description": None::<String>
                        }));
                    }
                }
            }
        }
    }

    // 3. Handle Anime/Manga/Book/Reading (AniList)
    if matches!(
        media_type,
        MediaType::Anime | MediaType::Manga | MediaType::Book | MediaType::Reading
    ) {
        if raw_title != "-" {
            let al_type = if matches!(media_type, MediaType::Anime) {
                anilist::MediaType::Anime
            } else {
                anilist::MediaType::Manga
            };

            if let Some((_, id_part)) = raw_title.rsplit_once('|') {
                if let Ok(id) = id_part.parse::<i32>() {
                    match anilist::get_media_by_id(&data.http_client, id, al_type).await {
                        Ok(Some(media)) => {
                            raw_title = media.title;
                            thumbnail = media.image;
                            anilist_url = Some(media.url);
                            source = "anilist";
                        }
                        _ => {}
                    }
                }
            } else {
                // Fallback search
                if let Ok(medias) =
                    anilist::search_media(&data.http_client, &raw_title, al_type, 1).await
                {
                    if let Some(media) = medias.first() {
                        raw_title = media.title.clone();
                        thumbnail = media.image.clone();
                        anilist_url = Some(media.url.clone());
                        source = "anilist";
                    }
                }
            }
        }
    }

    // Validate custom date if provided
    let effective_date = get_effective_date();
    let date_str = if let Some(ref custom_date) = date {
        // Strict validation: YYYY-MM-DD
        match NaiveDate::parse_from_str(custom_date, "%Y-%m-%d") {
            Ok(parsed) => parsed.format("%Y-%m-%d").to_string(),
            Err(_) => {
                ctx.say("Invalid date format. Please use YYYY-MM-DD (e.g. 2026-01-21)")
                    .await?;
                return Ok(());
            }
        }
    } else {
        effective_date.format("%Y-%m-%d").to_string()
    };

    // Calculate points
    let _points = calculate_points(media_type_str, final_amount);

    // Build immersion log data
    let user_id = user.id.to_string();
    let now = chrono::Utc::now();

    let log_data = json!({
        "user": {
            "id": user_id,
            "username": user.name,
            "displayName": user.global_name.as_ref().unwrap_or(&user.name),
            "avatar": user.avatar_url().unwrap_or_default()
        },
        "activity": {
            "type": media_type_str,
            "typeLabel": label,
            "amount": final_amount,
            "unit": unit,
            "title": raw_title,
            "comment": if raw_title != "-" { comment.as_ref() } else { None },
            "url": log_url,
            "anilistUrl": anilist_url,
            "vndbUrl": vndb_url
        },
        "metadata": {
            "thumbnail": thumbnail.clone(),
            "duration": if source == "youtube" { Some(final_amount) } else { None },
            "source": source,
            "vndbInfo": vndb_metadata
        },
        "timestamps": {
            "created": now.to_rfc3339(),
            "date": date_str,
            "month": format!("{}-{:02}", effective_date.year(), effective_date.month()),
            "year": effective_date.year()
        }
    });

    // Save to Firebase
    let firebase = &data.firebase;

    // 1. Add log to user's immersion_logs subcollection
    match firebase
        .add_to_subcollection("users", &user_id, "immersion_logs", &log_data)
        .await
    {
        Ok(log_id) => {
            debug!("Created immersion log: {}", log_id);
        }
        Err(e) => {
            error!("Failed to save immersion log: {:?}", e);
            ctx.say("Failed to save log. Please try again.").await?;
            return Ok(());
        }
    }

    // Get existing user data
    let user_doc = firebase.get_document("users", &user_id).await?;

    let (mut stats, existing_summary, _existing_timestamps) = if let Some(ref doc) = user_doc {
        (
            doc.get("stats").cloned().unwrap_or(json!({})),
            doc.get("summary").cloned().unwrap_or(json!({})),
            doc.get("timestamps").cloned().unwrap_or(json!({})),
        )
    } else {
        (json!({}), json!({}), json!({}))
    };

    // Get current stats for this media type
    let current_total = stats
        .get(media_type_str)
        .and_then(|s| s.get("total"))
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);
    let current_sessions = stats
        .get(media_type_str)
        .and_then(|s| s.get("sessions"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);
    let best_streak = stats
        .get(media_type_str)
        .and_then(|s| s.get("bestStreak"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);
    let current_streak = stats
        .get(media_type_str)
        .and_then(|s| s.get("currentStreak"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);

    // Update stats for this media type (preserve existing fields)
    stats[media_type_str] = json!({
        "total": current_total + amount,
        "sessions": current_sessions + 1,
        "lastActivity": now.to_rfc3339(),
        "bestStreak": best_streak,
        "currentStreak": current_streak,
        "unit": unit,
        "label": label
    });

    // Calculate total sessions across all media types
    let total_sessions: i64 = stats
        .as_object()
        .map(|obj| {
            obj.values()
                .filter_map(|s| s.get("sessions").and_then(|v| v.as_i64()))
                .sum()
        })
        .unwrap_or(0);

    // Get active types
    let active_types: Vec<String> = stats
        .as_object()
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    // Get join date (preserve existing or set new)
    let join_date = existing_summary
        .get("joinDate")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| now.to_rfc3339());

    // Build user update matching Node.js structure
    let user_update = json!({
        "profile": {
            "id": user_id,
            "username": user.name,
            "displayName": user.global_name.as_ref().unwrap_or(&user.name),
            "avatar": user.avatar_url().unwrap_or_default(),
            "lastSeen": now.to_rfc3339()
        },
        "stats": stats,
        "summary": {
            "totalSessions": total_sessions,
            "lastActivity": now.to_rfc3339(),
            "joinDate": join_date,
            "activeTypes": active_types
        },
        "timestamps": {
            "updated": now.to_rfc3339(),
            "lastLog": now.to_rfc3339()
        }
    });

    if let Err(e) = firebase.set_document("users", &user_id, &user_update).await {
        error!("Failed to update user stats: {:?}", e);
        // Don't return error - log was saved successfully
    }

    // Calculate new totals for display
    let updated_total = current_total + amount;

    // Calculate streak from immersion_logs
    // We fetch logs, validte timestamps, and repair history to JST if needed
    let global_streak = match firebase
        .query_subcollection("users", &user_id, "immersion_logs")
        .await
    {
        Ok(logs) => {
            let mut dates: Vec<String> = logs
                .iter()
                .filter_map(|log| {
                    let timestamps = log.get("timestamps")?;

                    // Try to get explicit 'date' field first (YYYY-MM-DD)
                    if let Some(date_str) = timestamps.get("date").and_then(|v| v.as_str()) {
                        return Some(date_str.to_string());
                    }

                    // Fallback to 'created' timestamp for legacy logs
                    // Legacy bot (Node.js) used server local time (WIB/UTC+7) for raw dates
                    if let Some(created_str) = timestamps.get("created").and_then(|v| v.as_str()) {
                        if let Ok(created_utc) = DateTime::parse_from_rfc3339(created_str) {
                            // Convert to UTC+7 (WIB) to match legacy behavior
                            // Legacy toDateStringRaw just dumped local time
                            let wib_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
                            let wib_time = created_utc.with_timezone(&wib_offset);
                            return Some(wib_time.format("%Y-%m-%d").to_string());
                        }
                    }

                    None
                })
                .collect();

            // Inject current date to ensure it's counted even if DB read is stale
            dates.push(date_str.clone());

            streak::calculate_streak(&dates).current
        }
        Err(e) => {
            debug!("Failed to calculate streak: {:?}", e);
            // Even if fetch fails, we know we have at least 1 streak from today's activity
            1
        }
    };

    // Build response embed matching Node.js format
    let mut embed = serenity::CreateEmbed::new()
        .author(serenity::CreateEmbedAuthor::new(format!(
            "{} Logged",
            label
        )))
        .title(if raw_title != "-" {
            raw_title.clone()
        } else {
            String::new()
        })
        .field(
            "Progress",
            format!("+{} {}", format_amount(final_amount), unit),
            true,
        )
        .field(
            "Total",
            format!("{} {}", format_amount(updated_total), unit),
            true,
        )
        .field(
            "Streak",
            format!(
                "{} day{}",
                global_streak,
                if global_streak == 1 { "" } else { "s" }
            ),
            true,
        )
        .color(colors::IMMERSION)
        .footer(serenity::CreateEmbedFooter::new(
            if let Some(warn) = warning_msg {
                format!("{} | {}\n{}", user.name, label, warn)
            } else {
                format!("{} | {}", user.name, label)
            },
        ))
        .thumbnail(thumbnail.unwrap_or_else(|| user.face()));

    // Add clickable URL if available (YouTube, AniList, VNDB)
    if let Some(ref url) = log_url {
        embed = embed.url(url);
    } else if let Some(ref url) = anilist_url {
        embed = embed.url(url);
    } else if let Some(ref url) = vndb_url {
        embed = embed.url(url);
    }

    // Add comment if provided (Discord limit: 1024 characters for field value)
    let embed = if let Some(ref c) = comment {
        const MAX_COMMENT_LENGTH: usize = 1000; // Leave room for truncation message
        let comment_text = if c.len() > MAX_COMMENT_LENGTH {
            format!(
                "{}... (dipotong, terlalu panjang)",
                &c[0..MAX_COMMENT_LENGTH]
            )
        } else {
            c.clone()
        };
        embed.field("Comment", comment_text, false)
    } else {
        embed
    };

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Format amount for display (remove unnecessary decimal places)
fn format_amount(n: f64) -> String {
    if n == n.trunc() {
        (n as i64).to_string()
    } else {
        format!("{:.1}", n)
    }
}

// Local calculate_user_streak removed in favor of utils::streak::calculate_streak

async fn autocomplete_title(ctx: Context<'_>, partial: &str) -> impl Iterator<Item = String> {
    let mut results = Vec::new();

    // Attempt to find media_type in options
    let media_type_val = if let poise::Context::Application(app_ctx) = ctx {
        app_ctx
            .interaction
            .data
            .options
            .iter()
            .find(|o| o.name == "media_type")
            .and_then(|o| match &o.value {
                serenity::model::application::CommandDataOptionValue::String(s) => Some(s.clone()),
                serenity::model::application::CommandDataOptionValue::Integer(i) => match i {
                    0 => Some("visual_novel".to_string()),
                    1 => Some("manga".to_string()),
                    2 => Some("anime".to_string()),
                    _ => None,
                },
                _ => None,
            })
    } else {
        None
    };

    let http = &ctx.data().http_client;

    // Only search if length >= 2
    if let Some(mt) = media_type_val.as_deref() {
        match mt {
            "visual_novel" | "VisualNovel" => {
                if partial.len() >= 2 {
                    if let Ok(vns) = vndb::search_vns(http, partial, 10).await {
                        for vn in vns {
                            let released = vn.released.unwrap_or_default();
                            // Format: "Title (Year)|ID"
                            let mut entry = format!("{} ({})|{}", vn.title, released, vn.id);

                            // Truncate if too long (Discord limit 100)
                            if entry.len() > 100 {
                                let id_len = vn.id.len() + 1; // +1 for pipe
                                let avail = 100 - id_len;
                                if avail > 0 {
                                    entry = format!(
                                        "{}|{}",
                                        &vn.title[0..avail.min(vn.title.len())],
                                        vn.id
                                    );
                                }
                            }

                            results.push(entry);
                        }
                    }
                }
            }
            "anime" | "Anime" | "manga" | "Manga" => {
                if partial.len() >= 2 {
                    let al_type = if mt.eq_ignore_ascii_case("anime") {
                        anilist::MediaType::Anime
                    } else {
                        anilist::MediaType::Manga
                    };
                    if let Ok(medias) = anilist::search_media(http, partial, al_type, 10).await {
                        for media in medias {
                            let mut entry = format!("{}|{}", media.title, media.id);
                            if entry.len() > 100 {
                                let id_len = media.id.to_string().len() + 1;
                                let avail = 100 - id_len;
                                if avail > 0 {
                                    entry = format!(
                                        "{}|{}",
                                        &media.title[0..avail.min(media.title.len())],
                                        media.id
                                    );
                                }
                            }
                            results.push(entry);
                        }
                    }
                }
            }
            _ => {}
        }
    } else {
        results.push("⚠️ Select Media Type First".to_string());
    }

    // If no results, suggest the partial input itself
    if results.is_empty() && !partial.is_empty() {
        results.push(partial.to_string());
    }

    results.into_iter()
}

/// Helper function to fetch page title from URL
async fn fetch_page_title(
    client: &reqwest::Client,
    url: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch the webpage
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let html = response.text().await?;

    // Simple regex to extract <title> tag content
    if let Some(start) = html.find("<title>") {
        if let Some(end) = html[start..].find("</title>") {
            let title_start = start + 7; // Length of "<title>"
            let title_end = start + end;
            let title = html[title_start..title_end].trim();

            // Decode HTML entities if needed (basic decoding)
            let decoded = html_escape::decode_html_entities(title).to_string();

            return Ok(Some(decoded));
        }
    }

    // Fallback: try og:title meta tag
    if let Some(og_title) = extract_meta_property(&html, "og:title") {
        return Ok(Some(og_title));
    }

    Ok(None)
}

/// Helper to extract meta property content
fn extract_meta_property(html: &str, property: &str) -> Option<String> {
    let pattern = format!(r#"<meta property="{}" content=""#, property);
    if let Some(start) = html.find(&pattern) {
        let content_start = start + pattern.len();
        if let Some(end) = html[content_start..].find('"') {
            let content = &html[content_start..content_start + end];
            return Some(html_escape::decode_html_entities(content).to_string());
        }
    }
    None
}
