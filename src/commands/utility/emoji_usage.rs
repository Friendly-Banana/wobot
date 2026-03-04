use crate::commands::utils;
use crate::{Context, Data};
use itertools::Itertools;
use poise::serenity_prelude::{GuildId, Reaction};
use sqlx::{query, query_as};
use std::collections::{HashSet, VecDeque};
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

/// List unused emojis in a guild
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn emoji_unused(
    ctx: Context<'_>,
    guild_id: Option<GuildId>,
) -> anyhow::Result<()> {
    ctx.defer().await?;
    let guild_id = guild_id.unwrap_or(ctx.guild_id().expect("guild_only"));

    let used_custom_emoji_ids = query!(
        r#"SELECT substring(emoji FROM '<a?:[A-Za-z0-9_]+:(\d+)>')::bigint AS "emoji_id!"
           FROM emoji_usage
           WHERE guild_id = $1 AND count > 0 AND position(':' in emoji) > 0"#,
        guild_id.get() as i64
    )
    .fetch_all(&ctx.data().database)
    .await?
    .into_iter()
    .map(|row| row.emoji_id as u64)
    .collect::<HashSet<_>>();

    let guild_emojis = guild_id.emojis(ctx).await?;
    let mut lines: VecDeque<_> = guild_emojis
        .into_iter()
        .filter(|e| e.available && !used_custom_emoji_ids.contains(&e.id.get()))
        .sorted_by(|a, b| Ord::cmp(&b.animated, &a.animated))
        .map(|e| e.to_string())
        .chunks(15)
        .into_iter()
        .map(|mut chunk| chunk.join(" "))
        .collect();
    lines.push_front("Unused emojis".to_string());

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
