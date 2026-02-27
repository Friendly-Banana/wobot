use std::collections::{HashSet, VecDeque};

use crate::Context;
use crate::commands::utils;
use itertools::Itertools;
use poise::futures_util::StreamExt;
use poise::serenity_prelude::{Mentionable, RoleId};
use sqlx::query;
use tracing::warn;

/// List inactive users, default 60 days, no bots
#[poise::command(slash_command, prefix_command, guild_only, owners_only)]
pub(crate) async fn inactive(
    ctx: Context<'_>,
    days: Option<u32>,
    include_bots: Option<bool>,
    exclude_role: Option<RoleId>,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let days = days.unwrap_or(60);
    let include_bots = include_bots.unwrap_or(false);
    let guild = ctx.guild_id().unwrap();
    let active = query!(
            "SELECT user_id FROM activity WHERE guild_id = $1 AND now() - last_active <= interval '1 day' * $2",
            guild.get() as i64,
            days as i32
        )
        .fetch_all(&ctx.data().database)
        .await?
        .into_iter()
        .map(|row| row.user_id as u64)
        .collect::<HashSet<_>>();

    let mut inactive = Vec::new();

    let mut members = guild.members_iter(&ctx).boxed();
    while let Some(member_result) = members.next().await {
        match member_result {
            Ok(member) => {
                if !active.contains(&member.user.id.get())
                    && (!member.user.bot || include_bots)
                    && exclude_role.is_none_or(|role| !member.roles.contains(&role))
                {
                    inactive.push(member.user.id);
                }
            }
            Err(error) => warn!("Member checking failed: {}", error),
        }
    }

    let mut lines: VecDeque<_> = inactive
        .into_iter()
        .map(|u| u.mention().to_string())
        .chunks(10)
        .into_iter()
        .map(|mut chunk| chunk.join(", "))
        .collect();
    lines.push_front(format!("Inactive for {} days", days));

    utils::paginate_text(ctx, &mut lines).await?;
    Ok(())
}
