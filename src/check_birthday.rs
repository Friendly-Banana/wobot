use crate::constants::{ONE_DAY, TIMEZONE};
use chrono::{Duration, Utc};
use itertools::Itertools;
use poise::serenity_prelude::{ChannelId, Context, GuildId, Mentionable, UserId};
use sqlx::{PgPool, query};
use std::collections::HashMap;
use tokio::time::{Instant, interval_at};
use tracing::{Level, debug, error, info, span, trace, warn};

pub(crate) fn check_birthdays(
    ctx: Context,
    database: PgPool,
    event_channel: HashMap<GuildId, ChannelId>,
) {
    tokio::spawn(async move {
        if let Err(err) = send_birthdays(&ctx, &database, &event_channel).await {
            error!(error = ?err, "Failed checking birthdays");
        }

        // run at local midnight
        let local_now = Utc::now().with_timezone(&TIMEZONE);
        let tomorrow = local_now + Duration::days(1);
        let midnight = tomorrow.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let until = midnight.signed_duration_since(local_now.naive_local());
        let instant = Instant::now() + until.to_std().unwrap();

        let mut interval = interval_at(instant, ONE_DAY);

        loop {
            interval.tick().await;

            if let Err(err) = send_birthdays(&ctx, &database, &event_channel).await {
                error!(error = ?err, "Failed checking birthdays");
            }
        }
    });
    info!("Started birthday thread");
}

async fn send_birthdays(
    ctx: &Context,
    database: &PgPool,
    event_channel: &HashMap<GuildId, ChannelId>,
) -> anyhow::Result<()> {
    let _ = span!(Level::DEBUG, "Sending birthday wishes").enter();
    let local_now = Utc::now().with_timezone(&TIMEZONE);
    let due = query!(
        "UPDATE birthdays SET last_congratulated = $1::date
         WHERE (last_congratulated IS NULL OR last_congratulated < $1::date)
         AND ((EXTRACT(MONTH FROM birthday) = EXTRACT(MONTH FROM $1::timestamptz)
             AND EXTRACT(DAY FROM birthday) = EXTRACT(DAY FROM $1::timestamptz))
           OR -- handle February 29
            (EXTRACT(MONTH FROM birthday) = 2
            AND EXTRACT(DAY FROM birthday) = 29
            AND EXTRACT(MONTH FROM $1::timestamptz) = 3 -- on March 1
            AND EXTRACT(DAY FROM $1::timestamptz) = 1
            AND EXTRACT(DAY FROM ($1::timestamptz - INTERVAL '1 day')) = 28) -- in non leap years
         )
         RETURNING guild_id, user_id",
        local_now.date_naive()
    )
    .fetch_all(database)
    .await?;
    debug!(?due, "Fetched due wishes");

    let mut guild_users: HashMap<GuildId, Vec<UserId>> = HashMap::new();
    for congrats in due {
        let guild_id = GuildId::new(congrats.guild_id as u64);
        let user_id = UserId::new(congrats.user_id as u64);
        guild_users.entry(guild_id).or_default().push(user_id);
    }

    for (guild_id, users) in guild_users {
        let mentions = users
            .iter()
            .map(|user| user.mention().to_string())
            .join(", ");
        let message = format!(
            "Happy birthday to {}! <:blobCuddleFerris:1313272092879360040>🎉",
            mentions
        );

        if let Some(channel) = event_channel.get(&guild_id) {
            if let Err(e) = channel.say(ctx, message).await {
                error!(error = ?e, guild = ?guild_id, users = ?users, "Failed to send birthday message");
            } else {
                trace!(users = ?users, guild = ?guild_id, "Sent congrats");
            }
        } else {
            warn!(guild = ?guild_id, "No event channel configured for guild");
        }
    }
    Ok(())
}
