use chrono::{Duration, Utc};
use poise::CreateReply;
use poise::serenity_prelude::model::timestamp;
use poise::serenity_prelude::{CreateEmbed, FormattedTimestamp, Mentionable, User, UserId};
use sqlx::{query, query_as};
use std::convert::identity;
use timestamp::Timestamp;

use crate::commands::utils::parse_duration_or_date;
use crate::{Context, Error};

const DEFAULT_REMINDER_TIME: Duration = Duration::hours(1);

#[allow(dead_code)]
pub(crate) struct Reminder {
    pub(crate) channel_id: i64,
    pub(crate) msg_id: i64,
    pub(crate) user_id: i64,
    pub(crate) content: String,
    time: Timestamp,
}

#[poise::command(slash_command, prefix_command, subcommands("add", "list", "delete"))]
pub(crate) async fn reminder(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Add a reminder
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn add(
    ctx: Context<'_>,
    #[description = "date or duration, default 1 hour"] when: Option<String>,
    #[description = "The reminder message"] message: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let reminder_time = match when {
        None => Utc::now() + DEFAULT_REMINDER_TIME,
        Some(x) => parse_duration_or_date(Utc::now(), &x).await?,
    };

    let msg_id = ctx
        .say(format!(
            "Reminder set for {}",
            FormattedTimestamp::from(Timestamp::from(reminder_time))
        ))
        .await?
        .message()
        .await?
        .id;

    query!("INSERT INTO reminder (channel_id, msg_id, user_id, content, time) VALUES ($1, $2, $3, $4, $5)", ctx.channel_id().get() as i64, msg_id.get() as i64, ctx.author().id.get() as i64, message, reminder_time)
        .execute(&ctx.data().database)
        .await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn list(ctx: Context<'_>, user: Option<User>) -> Result<(), Error> {
    ctx.defer().await?;

    let title;
    let due = if let Some(user) = user {
        title = format!("Reminders for {}", user.name);
        query_as!(
            Reminder,
            "SELECT * FROM reminder WHERE user_id = $1 ORDER BY time",
            user.id.get() as i64
        )
        .fetch_all(&ctx.data().database)
        .await?
    } else {
        title = "Reminders".to_string();
        query_as!(Reminder, "SELECT * FROM reminder ORDER BY time")
            .fetch_all(&ctx.data().database)
            .await?
    };
    const MAX_FIELD_LENGTH: usize = 1024;
    let mut e = CreateEmbed::default().title(title);
    for mut reminder in due {
        let author = format!(" ~ {}", UserId::new(reminder.user_id as u64).mention());
        reminder.content.truncate(MAX_FIELD_LENGTH - author.len());
        reminder.content.push_str(&author);
        e = e.field(
            FormattedTimestamp::from(reminder.time).to_string(),
            reminder.content,
            false,
        );
    }
    ctx.send(CreateReply::default().embed(e)).await?;
    Ok(())
}

/// Delete your scheduled reminders by the start of its content
/// bot owner can delete all reminders
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn delete(
    ctx: Context<'_>,
    message: String,
    all: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let del = if all.is_some_and(identity)
        && ctx.framework().options.owners.contains(&ctx.author().id)
    {
        query!("WITH deleted AS (DELETE FROM reminder WHERE content ILIKE $1 || '%' RETURNING *) SELECT count(*) FROM deleted", message)
            .fetch_one(&ctx.data().database)
            .await?.count
    } else {
        query!(
                "WITH deleted AS (DELETE FROM reminder WHERE content ILIKE $1 || '%' AND user_id = $2 RETURNING *) SELECT count(*) FROM deleted",
                message,
                ctx.author().id.get() as i64
            )
            .fetch_one(&ctx.data().database)
            .await?.count
    };

    ctx.reply(format!("Deleted {} reminder(s)", del.unwrap_or_default()))
        .await?;

    Ok(())
}
