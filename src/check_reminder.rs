use std::time::Duration;

use poise::serenity_prelude::{
    ChannelId, Context, CreateAllowedMentions, CreateMessage, Mentionable, UserId,
};
use sqlx::{query, PgPool};
use tokio::time::interval;
use tracing::{debug, error};

use crate::Error;

pub(crate) fn check_reminders(ctx: Context, database: PgPool, period: Duration) {
    tokio::spawn(async move {
        let mut interval = interval(period);

        loop {
            interval.tick().await;

            if let Err(why) = send_reminders(&ctx, &database).await {
                error!("Failed sending reminder: {}", why);
            }
        }
    });
}

pub(crate) async fn send_reminders(ctx: &Context, database: &PgPool) -> Result<(), Error> {
    let due = query!(
        "DELETE FROM reminder WHERE time <= now() RETURNING channel_id, msg_id, user_id, content"
    )
    .fetch_all(database)
    .await?;

    debug!("Sending {} reminders...", due.len());
    for reminder in due {
        let channel = ChannelId::new(reminder.channel_id as u64);
        let confirmation = channel.message(ctx, reminder.msg_id as u64).await?;
        let user = UserId::new(reminder.user_id as u64).to_user(ctx).await?;
        let mentions = CreateAllowedMentions::new()
            .everyone(false)
            .all_roles(false)
            .all_users(true);
        let msg = CreateMessage::new()
            .content(format!(
                "Reminder for {} | {}",
                reminder.content,
                user.mention()
            ))
            .allowed_mentions(mentions)
            .reference_message(&confirmation);
        channel.send_message(ctx, msg).await?;
    }
    debug!("Sent all reminders");
    Ok(())
}
