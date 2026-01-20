// Help command - show usage guide

use poise::serenity_prelude as serenity;
use crate::{Context, Error};
use crate::utils::config::colors;

/// Show help and usage guide
#[poise::command(slash_command, prefix_command)]
pub async fn help(ctx: Context<'_>) -> Result<(), Error> {
    let embed = serenity::CreateEmbed::new()
        .title("📚 Yuyuko Bot - Help")
        .description("A lightweight Japanese immersion tracker")
        .color(colors::PRIMARY)
        .field(
            "📝 Immersion Logging",
            "`/immersion` - Log your immersion activities\n\
            Supported: Anime, Manga, Visual Novel, Book, Reading, Listening",
            false,
        )
        .field(
            "📊 Statistics",
            "`/stat` - View your stats\n\
            `/stat visual_type:heatmap` - Activity heatmap\n\
            `/stat visual_type:barchart` - Bar chart",
            false,
        )
        .field(
            "🏆 Community",
            "`/leaderboard` - View rankings\n\
            `/log time` - View recent logs",
            false,
        )
        .field(
            "📖 Content",
            "`/novel` - Search & download light novels\n\
            `/afk` - Set your AFK status",
            false,
        )
        .field(
            "⚙️ Configuration",
            "`/config set` - Configure bot channels\n\
            `/config get` - View current configuration",
            false,
        )
        .field(
            "💯 Points System",
            "• Anime: 13 pts/episode\n\
            • Manga: 0.25 pts/page\n\
            • VN/Reading: 1 pt/~350 chars\n\
            • Listening/Reading Time: 0.67 pts/min",
            false,
        )
        .footer(serenity::CreateEmbedFooter::new(
            "Rust Edition • Built with Serenity & Poise",
        ));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}
