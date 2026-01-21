use poise::serenity_prelude as serenity;
use tracing::{error, info};

use crate::features::custom_prompt;
use crate::{Context, Error};
use crate::utils::config::colors;

/// Action to perform on custom prompt
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum PromptAction {
    #[name = "Set Prompt"]
    Set,
    #[name = "View Prompt"]
    View,
    #[name = "Delete Prompt"]
    Delete,
}

/// Manage your custom Ayumi personality prompt
#[poise::command(slash_command, prefix_command)]
pub async fn prompt(
    ctx: Context<'_>,
    #[description = "Action to perform"] action: PromptAction,
    #[description = "Rentry URL (required for Set action)"] url: Option<String>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let user_id = ctx.author().id.get();

    match action {
        PromptAction::Set => {
            let url = match url {
                Some(u) => u,
                None => {
                    ctx.say("Please provide a Rentry URL to set your prompt.").await?;
                    return Ok(());
                }
            };

            // Rate limit check
            if let Err(time_left) = custom_prompt::is_rate_limited(user_id) {
                ctx.say(format!(
                    "Please wait {} seconds before updating your prompt again.",
                    time_left
                ))
                .await?;
                return Ok(());
            }

            // Validate URL
            if !custom_prompt::is_valid_rentry_url(&url) {
                ctx.say("Invalid URL. Please provide a valid Rentry.co URL (e.g., https://rentry.co/xxxxx).")
                    .await?;
                return Ok(());
            }

            // Fetch content
            let content = match custom_prompt::fetch_prompt_from_rentry(&ctx.data().http_client, &url).await {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to fetch Rentry prompt: {:?}", e);
                    ctx.say(format!("Failed to fetch prompt from Rentry: {}", e)).await?;
                    return Ok(());
                }
            };

            // Validate content
            if let Err(e) = custom_prompt::validate_prompt_content(&content) {
                ctx.say(format!("Invalid prompt content: {}", e)).await?;
                return Ok(());
            }

            // Save prompt
            if custom_prompt::save_user_custom_prompt(user_id, &content) {
                info!("Updated custom prompt for user {}", user_id);
                
                let embed = serenity::CreateEmbed::new()
                    .title("Custom Prompt Updated")
                    .description("Your custom Ayumi personality has been successfully updated!")
                    .field("Source", &url, false)
                    .field("Length", format!("{} characters", content.len()), true)
                    .color(colors::SUCCESS);
                    
                ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true)).await?;
            } else {
                ctx.say("Failed to save custom prompt. Please try again later.").await?;
            }
        }
        PromptAction::View => {
            if let Some(prompt) = custom_prompt::get_user_custom_prompt(user_id) {
                let display_prompt = if prompt.len() > 1900 {
                    format!("{}...", &prompt[..1900])
                } else {
                    prompt.clone()
                };

                let embed = serenity::CreateEmbed::new()
                    .title("Your Custom Prompt")
                    .description(format!("```\n{}\n```", display_prompt))
                    .field("Full Length", format!("{} characters", prompt.len()), true)
                    .color(colors::INFO);

                ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true)).await?;
            } else {
                let embed = serenity::CreateEmbed::new()
                    .title("No Custom Prompt")
                    .description("You don't have a custom prompt set. Ayumi is using her default personality.")
                    .color(colors::WARNING);

                ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true)).await?;
            }
        }
        PromptAction::Delete => {
            // Check if exists first
            if custom_prompt::get_user_custom_prompt(user_id).is_none() {
                 ctx.say("You don't have a custom prompt set.").await?;
                 return Ok(());
            }

            if custom_prompt::delete_user_custom_prompt(user_id) {
                info!("Deleted custom prompt for user {}", user_id);
                
                let embed = serenity::CreateEmbed::new()
                    .title("Custom Prompt Deleted")
                    .description("Your custom prompt has been removed. Ayumi has reverted to her default personality.")
                    .color(colors::SUCCESS);

                ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true)).await?;
            } else {
                ctx.say("Failed to delete custom prompt. Please try again later.").await?;
            }
        }
    }

    Ok(())
}
