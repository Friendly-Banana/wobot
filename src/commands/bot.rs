use std::string::ToString;

use poise::serenity_prelude::{Activity, ActivityType, Message, ReactionType, ShardId};

use crate::{done, Context, Error};

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
    let manager = ctx.framework().shard_manager.lock().await;
    let runners = manager.runners.lock().await;
    let id = ShardId(ctx.serenity_context().shard_id);

    match runners.get(&id).and_then(|r| r.latency) {
        None => ctx.say("No shard or latency found").await?,
        Some(latency) => {
            ctx.say(format!("This shard's latency is {}ms", latency.as_millis()))
                .await?
        }
    };
    Ok(())
}

/// Say something
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn say(
    ctx: Context<'_>,
    text: String,
    message: Option<Message>,
) -> Result<(), Error> {
    if let Some(message) = message {
        message.reply(ctx, text).await?;
        done!(ctx);
    } else {
        ctx.say(text).await?;
        Ok(())
    }
}

/// React to a message
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn react(
    ctx: Context<'_>,
    emoji: ReactionType,
    message: Message,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    message.react(ctx.http(), emoji).await?;
    done!(ctx);
}

#[derive(poise::ChoiceParameter)]
pub(crate) enum ActivityChoice {
    Playing,
    Listening,
    Watching,
    Streaming,
    Competing,
    Custom,
}

/// Set the bot's activity
#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn activity(
    ctx: Context<'_>,
    activity: ActivityChoice,
    action: String,
    #[description = "stream url if Streaming or details if Custom"] details: Option<String>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let activity = match activity {
        ActivityChoice::Playing => Activity::playing(action),
        ActivityChoice::Listening => Activity::listening(action),
        ActivityChoice::Watching => Activity::watching(action),
        ActivityChoice::Competing => Activity::competing(action),
        ActivityChoice::Streaming => Activity::streaming(
            action,
            details.unwrap_or("https://www.twitch.tv/".to_string()),
        ),
        ActivityChoice::Custom => {
            let mut activity = Activity::from(Activity::playing(action));
            activity.kind = ActivityType::Custom;
            activity.details = details;
            activity
        }
    };
    ctx.serenity_context().set_activity(activity).await;
    done!(ctx);
}
