use crate::commands::utils;
use crate::{Context, Data};
use poise::serenity_prelude::{GuildId, Reaction};
use sqlx::{query, query_as};
use std::collections::VecDeque;
use tracing::warn;

/// List emoji usage statistics for a guild
#[poise::command(slash_command, prefix_command, guild_only, aliases("emojis"))]
pub(crate) async fn emoji_usage(ctx: Context<'_>, guild_id: Option<GuildId>) -> anyhow::Result<()> {
    ctx.defer().await?;
    let guild_id = guild_id.unwrap_or(ctx.guild_id().expect("guild_only"));
    let emoji_stats = query_as!(
        EmojiUsage,
        "SELECT * FROM emoji_usage WHERE guild_id = $1 ORDER BY count DESC",
        guild_id.get() as i64
    )
    .fetch_all(&ctx.data().database)
    .await?;
    if emoji_stats.is_empty() {
        ctx.reply("No emoji usage recorded in this guild.").await?;
        return Ok(());
    }
    let mut lines = VecDeque::from(["**Emoji | Usage Count**".to_string()]);
    for stat in emoji_stats {
        lines.push_back(format!("{} {}", stat.emoji, stat.count));
    }
    utils::paginate_text(ctx, &mut lines).await?;
    Ok(())
}

pub(crate) async fn track_emoji_usage(
    data: &Data,
    reaction: &Reaction,
    added: bool,
) -> anyhow::Result<()> {
    let guild_id = match reaction.guild_id {
        Some(g) => g,
        None => return Ok(()),
    };
    let emoji = utils::get_emoji_text(&reaction.emoji, data);
    let result = if !added {
        query!(
            "UPDATE emoji_usage SET count = count - 1 WHERE guild_id = $1 AND emoji = $2",
            guild_id.get() as i64,
            emoji
        )
        .execute(&data.database)
        .await
    } else {
        query!(
            "INSERT INTO emoji_usage (guild_id, emoji, count) VALUES ($1, $2, 1)
         ON CONFLICT (guild_id, emoji) DO UPDATE SET count = emoji_usage.count + 1",
            guild_id.get() as i64,
            emoji
        )
        .execute(&data.database)
        .await
    };
    if let Err(e) = result {
        warn!("Failed to track emoji usage: {}", e);
    }
    Ok(())
}

#[allow(dead_code)]
struct EmojiUsage {
    guild_id: i64,
    emoji: String,
    count: i64,
}
