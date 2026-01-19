use crate::{Context, Error};

/// Registers application commands in this guild or globally
/// 
/// Run with no arguments to register in guild, or with argument "global" to register globally.
#[poise::command(prefix_command, hide_in_help, owners_only = true)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
