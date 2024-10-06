use std::collections::HashSet;

use itertools::Itertools;
use poise::futures_util::StreamExt;
use poise::serenity_prelude::Mentionable;
use sqlx::query;
use tracing::warn;

use crate::{Context, Error};

/// List inactive users, default 60 days
#[poise::command(slash_command, prefix_command, guild_only, owners_only)]
pub(crate) async fn inactive(ctx: Context<'_>, days: Option<u32>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().unwrap();
    let active = query!(
            "SELECT user_id FROM activity WHERE guild_id = $1 AND now() - last_active <= interval '1 day' * $2",
            guild.get() as i64,
            days.unwrap_or(60) as i32
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
                if !active.contains(&member.user.id.get()) {
                    inactive.push(member.user.id);
                }
            }
            Err(error) => warn!("Member checking failed: {}", error),
        }
    }

    ctx.reply(format!(
        "Inactive: {}",
        inactive.into_iter().map(|u| u.mention()).join(", ")
    ))
    .await?;
    Ok(())
}
