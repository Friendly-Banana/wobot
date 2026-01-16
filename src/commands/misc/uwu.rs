use poise::serenity_prelude::Message;
use uwuifier::uwuify_str_sse;

use crate::Context;
use crate::done;

/// Uwuify a message
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn uwu(ctx: Context<'_>, message: Message) -> anyhow::Result<()> {
    message.reply(ctx, uwuify_str_sse(&message.content)).await?;
    done!(ctx);
}

/// Uwuify text
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn uwu_text(ctx: Context<'_>, text: String) -> anyhow::Result<()> {
    ctx.say(uwuify_str_sse(&text)).await?;
    Ok(())
}
