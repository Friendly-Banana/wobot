use chrono::{Duration, Utc};
use itertools::Itertools;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage, GuildId, Mentionable, UserId};
use sqlx::{query, PgPool};
use std::collections::HashMap;
use tokio::time::{interval_at, Instant};
use tracing::{debug, error, info};

use crate::constants::{ONE_DAY, TIMEZONE};
use crate::Error;

pub(crate) fn check_birthdays(
    ctx: Context,
    database: PgPool,
    event_channel: HashMap<GuildId, ChannelId>,
) {
    tokio::spawn(async move {
        if let Err(why) = birthday(&ctx, &database, &event_channel).await {
            error!("Failed checking birthdays: {}", why);
        }

        // run shortly after midnight
        let now_utc = Utc::now().with_timezone(&TIMEZONE);
        let tomorrow = now_utc + Duration::days(1);
        let after_midnight = tomorrow.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let until = after_midnight.signed_duration_since(now_utc.naive_local());
        let instant = Instant::now() + until.to_std().unwrap();

        let mut interval = interval_at(instant, ONE_DAY);

        loop {
            interval.tick().await;

            if let Err(why) = birthday(&ctx, &database, &event_channel).await {
                error!("Failed checking birthdays: {}", why);
            }
        }
    });
    info!("Started birthday thread");
}

async fn birthday(
    ctx: &Context,
    database: &PgPool,
    event_channel: &HashMap<GuildId, ChannelId>,
) -> Result<(), Error> {
    let due = query!(
        "UPDATE birthdays SET last_congratulated = now() WHERE birthday = now()::date AND (last_congratulated IS NULL OR last_congratulated < now()::date)  RETURNING guild_id, user_id"
    )
        .fetch_all(database)
        .await?;

    debug!("Sending {} congratulations...", due.len());
    let mut guild_users: HashMap<GuildId, Vec<UserId>> = HashMap::new();
    for congrat in due {
        let guild_id = GuildId::new(congrat.guild_id as u64);
        let user_id = UserId::new(congrat.user_id as u64);
        guild_users.entry(guild_id).or_default().push(user_id);
    }

    for (guild_id, users) in guild_users {
        let mentions = users
            .iter()
            .map(|user| user.mention().to_string())
            .join(", ");
        let content = format!(
            "Happy birthday to {}! <:blobCuddleFerris:1313272092879360040>🎉",
            mentions
        );
        let message = CreateMessage::new().content(content);

        if let Some(channel) = event_channel.get(&guild_id) {
            channel.send_message(ctx, message).await?;
        }
    }
    debug!("Sent all congratulations");
    Ok(())
}
