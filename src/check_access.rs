use std::collections::{HashMap, HashSet};
use std::time::Duration;

use poise::futures_util::StreamExt;
use poise::serenity_prelude::{Context, CreateMessage, GuildId};
use sqlx::{query, PgPool};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::{Access, Error};

#[allow(dead_code)]
pub(crate) fn check_access(
    ctx: Context,
    period: Duration,
    database: PgPool,
    config: HashMap<GuildId, Access>,
) {
    tokio::spawn(async move {
        let mut interval = interval(period);

        loop {
            interval.tick().await;

            if let Err(why) = access(&ctx, &database, &config).await {
                error!("Failed checking access: {}", why);
            }
        }
    });
}

#[allow(dead_code)]
async fn access(
    ctx: &Context,
    database: &PgPool,
    config: &HashMap<GuildId, Access>,
) -> Result<(), Error> {
    debug!("Checking {} guilds...", config.len());

    for (guild, access) in config {
        if access.descending_roles.len() < 2 {
            warn!("Need at least 2 roles to change access");
            continue;
        }
        let users = query!(
            "SELECT user_id FROM activity WHERE guild_id = $1 AND now() - last_active <= interval '1 day' * $2",
            guild.get() as i64,
            access.active_days as i32
        )
            .fetch_all(database)
            .await?;
        let active_users = users
            .into_iter()
            .map(|row| row.user_id as u64)
            .collect::<HashSet<_>>();

        let mut count = 0;

        let mut members = guild.members_iter(&ctx).boxed();
        while let Some(member_result) = members.next().await {
            match member_result {
                Ok(member) => {
                    if active_users.contains(&member.user.id.get()) {
                        continue;
                    }
                    // skip lowest role
                    for (i, role) in access
                        .descending_roles
                        .iter()
                        .take(access.descending_roles.len() - 1)
                        .enumerate()
                    {
                        if member.roles.contains(role) {
                            info!(
                                "Demoting {} to {}",
                                member.user.name,
                                access.descending_roles[i + 1]
                            );
                            query!("INSERT INTO activity (user_id, guild_id) VALUES ($1, $2) ON CONFLICT (user_id, guild_id) DO UPDATE SET last_active = now()", member.user.id.get() as i64, guild.get() as i64)
                                .execute(database)
                                .await?;
                            member.add_role(ctx, access.descending_roles[i + 1]).await?;
                            member.remove_role(ctx, role).await?;
                            count += 1;
                            break;
                        }
                    }
                }
                Err(error) => {
                    warn!("Member checking failed: {}", error);
                    break;
                }
            }
        }
        if count != 0 {
            let message = CreateMessage::new()
                .content(format!("{} users were demoted for being inactive", count));
            access.log_channel.send_message(ctx, message).await?;
        }
    }
    info!("Checked all guilds");
    Ok(())
}
