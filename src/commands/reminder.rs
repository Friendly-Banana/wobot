use std::convert::identity;
use std::time::Duration;

use poise::serenity_prelude::model::timestamp;
use poise::serenity_prelude::{CreateEmbed, FormattedTimestamp, Mentionable, User, UserId};
use poise::CreateReply;
use sqlx::{query, query_as};
use timestamp::Timestamp;

use crate::constants::{ONE_HOUR, ONE_YEAR};
use crate::{Context, Error};

const MAX_REMINDER_TIME: Duration = ONE_YEAR;
const DEFAULT_REMINDER_TIME: Duration = ONE_HOUR;

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
    #[description = "when to notify\nsupports human-readable format like 3m\ndefault 1 hour, max 1 year"]
    time: Option<String>,
    #[description = "The reminder message"] message: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let duration = match time {
        Some(x) => parse_duration::parse(x.as_str())?.min(MAX_REMINDER_TIME),
        None => DEFAULT_REMINDER_TIME,
    };

    let reminder_time = chrono::Utc::now() + chrono::Duration::from_std(duration)?;
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

    let (title, due) = if let Some(user) = user {
        (
            format!("Reminders for {}", user.name),
            query_as!(
                Reminder,
                "SELECT * FROM reminder WHERE user_id = $1",
                user.id.get() as i64
            )
            .fetch_all(&ctx.data().database)
            .await?,
        )
    } else {
        (
            "Reminders".to_string(),
            query_as!(Reminder, "SELECT * FROM reminder")
                .fetch_all(&ctx.data().database)
                .await?,
        )
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

/// delete your scheduled reminders by the start of its content
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
