use poise::serenity_prelude as serenity;
use tracing::error;

use crate::features::role_rank::QUIZZES;
use crate::{Context, Error};

/// Manage Role Rank (Quiz) system
#[poise::command(
    slash_command,
    prefix_command,
    required_permissions = "MANAGE_GUILD",
    subcommands("setup", "delete")
)]
pub async fn role_rank(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Setup the quiz selector in the current channel
#[poise::command(slash_command, prefix_command, required_permissions = "MANAGE_GUILD")]
pub async fn setup(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    // Call helper
    send_quiz_selector(ctx.http(), ctx.channel_id()).await?;

    Ok(())
}

/// Helper function to send/resend the quiz selector
pub async fn send_quiz_selector(
    http: &serenity::Http,
    channel_id: serenity::ChannelId,
) -> Result<(), Error> {
    // Create Dropdown Options from QUIZZES
    // Sort logic: we want levels 0-7 ordered.
    // HashMap iteration order is random, so collect and sort.
    let mut quizzes: Vec<_> = QUIZZES.values().collect();
    quizzes.sort_by_key(|q| q.level);

    let mut options = Vec::new();
    for quiz in quizzes {
        options.push(
            serenity::CreateSelectMenuOption::new(quiz.label, quiz.value)
                .description(quiz.description),
        );
    }

    let select_menu = serenity::CreateSelectMenu::new(
        "quiz_select",
        serenity::CreateSelectMenuKind::String { options },
    )
    .placeholder("Pilih Quiz / Select Quiz")
    .min_values(1)
    .max_values(1);

    let row = serenity::CreateActionRow::SelectMenu(select_menu);

    let embed = serenity::CreateEmbed::new()
        .title("Quiz Selector")
        .description("Pilih quiz di bawah ini untuk memulai tes kenaikan role.\nSelect a quiz below to start the role advancement test.")
        .color(0x00ADEF)
        .image("https://media.discordapp.net/attachments/1176743181803602022/1329665790408261683/role_rank_header.png?ex=6790757d&is=678f23fd&hm=0856017300438183060768407484742790956488390770678125477430045472&"); // Placeholder or use the one from original if available

    channel_id
        .send_message(
            http,
            serenity::CreateMessage::new()
                .embed(embed)
                .components(vec![row]),
        )
        .await?;

    Ok(())
}

/// Manually delete a quiz channel (Admin only)
#[poise::command(slash_command, prefix_command, required_permissions = "MANAGE_GUILD")]
pub async fn delete(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let channel = ctx.guild_channel().await;

    if let Some(gc) = channel {
        let guild_id = gc.guild_id.to_string();
        let data = ctx.data();

        let category_id = if let Some(config) = data.guild_configs.get(&guild_id) {
            config
                .quiz_category_id
                .as_ref()
                .and_then(|id| id.parse::<u64>().ok())
                .map(serenity::ChannelId::new)
        } else {
            None
        };

        if let Some(cat_id) = category_id {
            if gc.parent_id == Some(cat_id) {
                // Check if this is the configured selector channel
                if let Some(config) = data.guild_configs.get(&guild_id) {
                    if let Some(selector_id) = &config.quiz_channel_id {
                        if gc.id.to_string() == *selector_id {
                            ctx.say("Cannot delete main selector channel (Protected via Config).")
                                .await?;
                            return Ok(());
                        }
                    }
                }

                // Clean up session if exists
                {
                    let data = ctx.data();
                    data.role_rank_sessions.retain(|_, v| v.thread_id != gc.id);
                }

                ctx.say("Deleting channel in 3 seconds...").await?;
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                if let Err(e) = gc.delete(&ctx.http()).await {
                    error!("Failed to delete channel: {:?}", e);
                    ctx.say(format!("Failed to delete channel: {}", e)).await?;
                }
            } else {
                ctx.say("This command can only be used in quiz channels.")
                    .await?;
            }
        } // closing category_id
    } else {
        ctx.say("This command must be used in a guild channel.")
            .await?;
    }

    Ok(())
}
