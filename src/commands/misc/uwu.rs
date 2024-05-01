use poise::serenity_prelude::Message;
use tracing::debug;
use uwuifier::uwuify_str_sse;

use crate::done;
use crate::{Context, Error};

/// Uwuify a message
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn uwu(ctx: Context<'_>, message: Message) -> Result<(), Error> {
    debug!(
        "{} uwuified {} here {}",
        ctx.author().name,
        message.content,
        message.link()
    );
    message.reply(ctx, uwuify_str_sse(&message.content)).await?;
    done!(ctx);
}

/// Uwuify text
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn uwu_text(ctx: Context<'_>, text: String) -> Result<(), Error> {
    debug!("{} uwuified {}", ctx.author().name, text);
    ctx.say(uwuify_str_sse(&text)).await?;
    Ok(())
}
