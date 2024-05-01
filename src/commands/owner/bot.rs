use std::string::ToString;

use crate::{Context, Error};

/// Test bot function, should respond with "pong!"
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong!").await?;
    Ok(())
}

/// List servers where the bot is a member
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn servers(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::servers(ctx).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn register_commands(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn latency(ctx: Context<'_>) -> Result<(), Error> {
    let manager = ctx.framework().shard_manager.clone();
    let runners = manager.runners.lock().await;
    let id = ctx.serenity_context().shard_id;

    match runners.get(&id).and_then(|r| r.latency) {
        None => ctx.say("No shard or latency found").await?,
        Some(latency) => {
            ctx.say(format!("This shard's latency is {}ms", latency.as_millis()))
                .await?
        }
    };
    Ok(())
}
