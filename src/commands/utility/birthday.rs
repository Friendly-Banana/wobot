use crate::commands::utils::parse_date;
use crate::{Context, Error};
use poise::serenity_prelude::FormattedTimestamp;
use poise::serenity_prelude::model::timestamp;
use sqlx::query;
use timestamp::Timestamp;

#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    subcommands("add", "delete")
)]
pub(crate) async fn birthday(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Sign up for birthday wishes
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn add(ctx: Context<'_>, date: String) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let birthday = parse_date(&date).await?;
    ctx.reply(format!(
        "Added your birthday at {}",
        FormattedTimestamp::from(Timestamp::from_unix_timestamp(birthday.timestamp())?)
    ))
    .await?;

    query!(
        "INSERT INTO birthdays (guild_id, user_id, birthday) VALUES ($1, $2, $3) ON CONFLICT (user_id) DO UPDATE SET guild_id = $1, birthday = $3",
        ctx.guild_id().expect("guild_only").get() as i64,
        ctx.author().id.get() as i64,
        birthday.date_naive()
    )
        .execute(&ctx.data().database)
        .await?;

    Ok(())
}

/// Delete your birthday
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn delete(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    query!(
        "DELETE FROM birthdays WHERE user_id = $1",
        ctx.author().id.get() as i64
    )
    .execute(&ctx.data().database)
    .await?;

    ctx.reply("No more congratulations :(").await?;
    Ok(())
}
