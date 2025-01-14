use crate::{done, Context, Error};
use itertools::Itertools;
use poise::serenity_prelude::{Emoji, GuildId, Message, ReactionType};
use reqwest::Url;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tracing::info;

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

static EMOJI_CACHE: LazyLock<RwLock<Vec<Emoji>>> = LazyLock::new(|| RwLock::new(Vec::new()));

pub(crate) async fn load_bot_emojis(
    ctx: &poise::serenity_prelude::Context,
    guilds: Vec<GuildId>,
) -> Result<(), Error> {
    let mut emojis = ctx.get_application_emojis().await?;
    for guild in guilds {
        let mut guild_emojis = guild.emojis(ctx).await?;
        emojis.append(&mut guild_emojis);
    }
    let mut cache = EMOJI_CACHE.write().await;
    *cache = emojis;
    info!("Loaded {} emojis", cache.len());
    Ok(())
}

async fn autocomplete_emoji_in_text(ctx: Context<'_>, partial: &str) -> Vec<String> {
    // autocomplete can be max 100 chars
    if partial.len() < 2 || partial.len() > 90 {
        return vec![];
    }
    // emoji names are max 32 characters long
    let len = partial.len();
    let start = if len > 33 { len - 33 } else { 0 };
    if let Some(index) = partial[start..].rfind(':') {
        return autocomplete_emoji(ctx, &partial[start + index + 1..])
            .await
            .iter()
            .map(|e| format!("{}{}", &partial[..start + index], e))
            .collect();
    }
    vec![]
}

async fn autocomplete_emoji(_ctx: Context<'_>, partial: &str) -> Vec<String> {
    let emojis = EMOJI_CACHE.read().await;
    emojis
        .iter()
        .filter(move |e| e.available && e.name.contains(partial))
        .take(25) // max 25 suggestions
        .map(|e| e.to_string())
        .collect_vec()
}

/// Say something
#[poise::command(slash_command, prefix_command, track_edits)]
pub(crate) async fn say(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_emoji_in_text"] text: String,
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
    #[autocomplete = "autocomplete_emoji"] emoji: ReactionType,
    message: Message,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    message.react(ctx.http(), emoji).await?;
    done!(ctx);
}
