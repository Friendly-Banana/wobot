use crate::commands::utils;
use crate::{Context, Data};
use poise::serenity_prelude::Reaction;
use sqlx::{query, query_as};
use std::collections::VecDeque;
use tracing::warn;

/// List emoji usage statistics for this guild
#[poise::command(slash_command, prefix_command, guild_only, aliases("emojis"))]
pub(crate) async fn emoji_usage(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    let guild_id = ctx.guild_id().expect("guild_only");
    let emoji_stats = query_as!(
        EmojiUsage,
        "SELECT * FROM emoji_usage WHERE guild_id = $1 ORDER BY count DESC",
        guild_id.get() as i64
    )
    .fetch_all(&ctx.data().database)
    .await?;
    if emoji_stats.is_empty() {
        ctx.reply("No emoji usage recorded yet.").await?;
        return Ok(());
    }
    let mut lines = VecDeque::from(["**Emoji | Usage Count**".to_string()]);
    for stat in emoji_stats {
        let emoji = utils::get_emoji_from_id(ctx, guild_id.get() as i64, stat.emoji_id).await?;
        lines.push_back(format!("{} {}", emoji, stat.count));
    }
    utils::paginate_text(ctx, &mut lines).await?;
    Ok(())
}

pub(crate) async fn track_emoji_usage(data: &Data, reaction: &Reaction) -> anyhow::Result<()> {
    let guild_id = match reaction.guild_id {
        Some(g) => g,
        None => return Ok(()),
    };
    let emoji_id = utils::get_emoji_id(&reaction.emoji, data).await?;
    let result = query!(
        "INSERT INTO emoji_usage (guild_id, emoji_id, count) VALUES ($1, $2, 1)
         ON CONFLICT (guild_id, emoji_id) DO UPDATE SET count = emoji_usage.count + 1",
        guild_id.get() as i64,
        emoji_id
    )
    .execute(&data.database)
    .await;
    if let Err(e) = result {
        warn!("Failed to track emoji usage: {}", e);
    }
    Ok(())
}

#[allow(dead_code)]
struct EmojiUsage {
    guild_id: i64,
    emoji_id: i64,
    count: i64,
}
