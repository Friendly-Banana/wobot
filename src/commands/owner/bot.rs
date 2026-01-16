use crate::commands::load_bot_emojis;
use crate::{Context, done};

/// Test bot function, should respond with "pong!"
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn ping(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.say("pong!").await?;
    Ok(())
}

/// List servers where the bot is a member
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn servers(ctx: Context<'_>) -> anyhow::Result<()> {
    poise::builtins::servers(ctx).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn register_commands(ctx: Context<'_>) -> anyhow::Result<()> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn latency(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
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

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn refresh_emojis(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    let guilds = ctx.http().get_guilds(None, None).await?;
    let ids = guilds.iter().map(|g| g.id).collect();
    load_bot_emojis(ctx.serenity_context(), ids).await?;
    done!(ctx);
}
