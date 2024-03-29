use std::time::Duration;

use anyhow::Context as _;
use poise::serenity_prelude::{
    ButtonStyle, ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponse, EditMessage, Message, UserId,
};
use poise::CreateReply;
use sqlx::query;
use tracing::debug;
use uwuifier::uwuify_str_sse;

use crate::{done, Context, Error};

/// Uwuify a message
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn uwu(ctx: Context<'_>, message: Message) -> Result<(), Error> {
    debug!(
        "{} uwuified {} here {}",
        ctx.author().name,
        message.content,
        message.link()
    );
    message.reply(ctx, uwuify_str_sse(&message.content)).await?;
    done!(ctx);
}

/// Uwuify text
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn uwu_text(ctx: Context<'_>, text: String) -> Result<(), Error> {
    debug!("{} uwuified {}", ctx.author().name, text);
    ctx.say(uwuify_str_sse(&text)).await?;
    Ok(())
}

/// Show the amount of keyword usages
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn keyword_statistics(ctx: Context<'_>, keyword: String) -> Result<(), Error> {
    ctx.defer().await?;
    let stats = query!("SELECT user_id, SUM(count)::int AS count FROM auto_replies WHERE keyword ILIKE '%' || $1 || '%' GROUP BY user_id", keyword)
        .fetch_all(&ctx.data().database)
        .await?;
    let mut data: Vec<_> = Vec::new();
    let mut total = 0;
    for stat in stats {
        let name = match UserId::new(stat.user_id as u64).to_user(ctx.http()).await {
            Ok(user) => user.name,
            Err(_) => stat.user_id.to_string(),
        };
        let count = stat.count.context("count is null")?;
        data.push(format!("{}: {}", name, count));
        total += count;
    }

    let reply = CreateReply::default().embed(
        CreateEmbed::default()
            .title(format!("{keyword} statistics"))
            .description(data.join("\n"))
            .footer(CreateEmbedFooter::new(format!("Total: {}", total))),
    );
    ctx.send(reply).await?;
    Ok(())
}

const BOOP_TIMEOUT: Duration = Duration::from_secs(10);

// Adopted from https://github.com/serenity-rs/poise/blob/ec19915d817cc6ad8f02ec0cab260d29d2704cce/examples/feature_showcase/collector.rs
/// Boop the bot!
#[poise::command(slash_command, prefix_command, track_edits)]
pub(crate) async fn boop(ctx: Context<'_>) -> Result<(), Error> {
    let uuid_boop = ctx.id();

    let reply = {
        let components = vec![CreateActionRow::Buttons(vec![CreateButton::new(
            uuid_boop.to_string(),
        )
        .style(ButtonStyle::Primary)
        .label("Boop me!")])];

        CreateReply::default()
            .content("I want some boops!")
            .components(components)
    };

    let reply_handle = ctx.send(reply).await?;

    let mut boop_count = 0;
    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(BOOP_TIMEOUT)
        .filter(move |mci| mci.data.custom_id == uuid_boop.to_string())
        .await
    {
        boop_count += 1;

        let mut msg = mci.message.clone();
        msg.edit(
            ctx,
            EditMessage::new().content(format!("Boop count: {}", boop_count)),
        )
        .await?;

        mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
            .await?;
    }

    let message = CreateReply::default().content(format!("Total boops: {}", boop_count));
    reply_handle.edit(ctx, message).await?;

    Ok(())
}
