use poise::serenity_prelude::{Message, ReactionType};

use crate::{done, Context, Error};

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
