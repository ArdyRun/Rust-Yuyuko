// Ayumu JLPT exam commands for Discord bot

use crate::utils::config::colors;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

/// JLPT level choice
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum JlptLevel {
    #[name = "N5"]
    N5,
    #[name = "N4"]
    N4,
    #[name = "N3"]
    N3,
    #[name = "N2"]
    N2,
    #[name = "N1"]
    N1,
}

/// Time period for leaderboard
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum LeaderboardPeriod {
    #[name = "Weekly"]
    Weekly,
    #[name = "Monthly"]
    Monthly,
    #[name = "All-time"]
    AllTime,
}

impl LeaderboardPeriod {
    fn as_str(&self) -> &'static str {
        match self {
            LeaderboardPeriod::Weekly => "weekly",
            LeaderboardPeriod::Monthly => "monthly",
            LeaderboardPeriod::AllTime => "alltime",
        }
    }
}

/// Start a JLPT exam session
#[poise::command(slash_command, prefix_command)]
pub async fn exam(
    ctx: Context<'_>,
    #[description = "JLPT level"] level: JlptLevel,
) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let discord_id = ctx.author().id.to_string();
    let level_str = format!("{:?}", level);

    let session = match data
        .ayumu
        .create_session(&discord_id, &level_str, "balanced_75")
        .await
    {
        Ok(s) => s,
        Err(e) => {
            ctx.say(format!("Failed to create exam: {}", e)).await?;
            return Ok(());
        }
    };

    let embed = serenity::CreateEmbed::new()
        .title(format!("{} Exam Ready!", level_str))
        .description(format!("[Start Exam]({})", session.url))
        .field("Questions", session.question_count.to_string(), true)
        .field("Session", session.session_code.clone(), true)
        .field("Expires", "24 hours", true)
        .color(colors::PRIMARY)
        .footer(serenity::CreateEmbedFooter::new("Powered by Ayumu"));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// View your Ayumu profile
#[poise::command(slash_command, prefix_command)]
pub async fn profile(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let discord_id = ctx.author().id.to_string();

    let prof = match data.ayumu.get_profile(&discord_id).await {
        Ok(p) => p,
        Err(e) => {
            ctx.say(format!("Failed to load profile: {}", e)).await?;
            return Ok(());
        }
    };

    let mut description = format!(
        "**Rank:** {} ({})\n**XP:** {}\n**Streak:** {} days\n**Exams:** {}\n",
        prof.rank, prof.display_name, prof.total_xp, prof.current_streak, prof.total_exams
    );

    if !prof.achievements.is_empty() {
        description.push_str("\n**Recent Achievements:**\n");
        for ach in prof.achievements.iter().take(5) {
            description.push_str(&format!("• {}\n", ach.name));
        }
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("{}'s Profile", prof.display_name))
        .description(description)
        .color(colors::PRIMARY);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// View the JLPT leaderboard
#[poise::command(slash_command, prefix_command)]
pub async fn jlpt_leaderboard(
    ctx: Context<'_>,
    #[description = "JLPT level (optional)"] level: Option<JlptLevel>,
    #[description = "Time period"] period: Option<LeaderboardPeriod>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let level_str = level.map(|l| format!("{:?}", l));
    let period_str = period.map(|p| p.as_str());

    let entries = match data
        .ayumu
        .get_leaderboard(level_str.as_deref(), period_str)
        .await
    {
        Ok(e) => e,
        Err(e) => {
            ctx.say(format!("Failed to load leaderboard: {}", e))
                .await?;
            return Ok(());
        }
    };

    if entries.is_empty() {
        ctx.say("No leaderboard data yet.").await?;
        return Ok(());
    }

    let mut description = String::new();
    for entry in entries.iter().take(10) {
        description.push_str(&format!(
            "**#{}. {}** — {} pts ({} exams)\n",
            entry.rank, entry.username, entry.total_score, entry.total_exams
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title("JLPT Leaderboard")
        .description(description)
        .color(colors::PRIMARY);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}
