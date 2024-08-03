use itertools::Itertools;
use poise::serenity_prelude::{Message, ReactionType};
use reqwest::Url;

use crate::{done, Context, Error};

/// create embeds and remove tracking parameters from URLs
#[poise::command(slash_command, prefix_command, track_edits)]
pub(crate) async fn embed(ctx: Context<'_>, mut url: Url) -> Result<(), Error> {
    if let Some(mut host) = url.host_str() {
        if let Some(stripped) = host.strip_prefix("www.") {
            host = stripped;
        }
        if let Some(fix) = ctx.data().link_fixes.get(host) {
            if let Some(tracking) = &fix.tracking {
                let query = url
                    .query_pairs()
                    .filter(|(key, _)| key != tracking)
                    .map(|(k, v)| format!("{}={}", k, v))
                    .join("&");
                url.set_query(Some(&query));
            }
            if let Some(host) = &fix.host {
                url.set_host(Some(host))?;
            }
        }
    }
    ctx.reply(url).await?;
    Ok(())
}

/// Say something
#[poise::command(slash_command, prefix_command, track_edits)]
pub(crate) async fn say(
    ctx: Context<'_>,
    text: String,
    message: Option<Message>,
) -> Result<(), Error> {
    if let Some(message) = message {
        message.reply(ctx, text).await?;
        done!(ctx);
    } else {
        ctx.reply(text).await?;
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
