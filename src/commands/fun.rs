use std::time::Duration;

use anyhow::Context as _;
use poise::serenity_prelude::{
    ButtonStyle, CollectComponentInteraction, InteractionResponseType, Message, UserId,
};
use sqlx::query;
use tracing::debug;
use uwuifier::uwuify_str_sse;

use crate::{done, link_msg, Context, Error};

/// Uwuify a message
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn uwu(ctx: Context<'_>, message: Message) -> Result<(), Error> {
    debug!(
        "{} uwuified {} here {}",
        ctx.author().name,
        message.content,
        link_msg!(ctx.guild_id(), message.channel_id, message.id)
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
    let similar_words = format!("%{}%", keyword);
    let stats = query!("SELECT user_id, SUM(count)::int AS count FROM auto_replies WHERE keyword LIKE $1 GROUP BY user_id", similar_words)
        .fetch_all(&ctx.data().database)
        .await?;
    let mut data: Vec<_> = Vec::new();
    let mut total = 0;
    for stat in stats {
        let name = match UserId(stat.user_id as u64).to_user(ctx.http()).await {
            Ok(user) => user.name,
            Err(_) => stat.user_id.to_string(),
        };
        let count = stat.count.context("count is null")?;
        data.push(format!("{}: {}", name, count));
        total += count;
    }

    ctx.send(|m| {
        m.embed(|e| {
            e.title(format!("{keyword} statistics"))
                .description(data.join("\n"))
                .footer(|f| f.text(format!("Total: {}", total)))
        })
    })
    .await?;
    Ok(())
}

const BOOP_TIMEOUT: Duration = Duration::from_secs(10);

/// Boop the bot!
#[poise::command(slash_command, prefix_command, track_edits)]
pub(crate) async fn boop(ctx: Context<'_>) -> Result<(), Error> {
    let uuid_boop = ctx.id();

    let reply = ctx
        .send(|m| {
            m.content("I want some boops!").components(|c| {
                c.create_action_row(|ar| {
                    ar.create_button(|b| {
                        b.style(ButtonStyle::Primary)
                            .label("Boop me!")
                            .custom_id(uuid_boop)
                    })
                })
            })
        })
        .await?;

    let mut boop_count = 0;
    while let Some(mci) = CollectComponentInteraction::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(BOOP_TIMEOUT)
        .filter(move |mci| mci.data.custom_id == uuid_boop.to_string())
        .await
    {
        boop_count += 1;

        let mut msg = mci.message.clone();
        msg.edit(ctx, |m| m.content(format!("Boop count: {}", boop_count)))
            .await?;

        mci.create_interaction_response(ctx, |ir| {
            ir.kind(InteractionResponseType::DeferredUpdateMessage)
        })
        .await?;
    }

    reply
        .edit(ctx, |m| {
            m.content(format!("Total boops: {}", boop_count))
                .components(|c| c)
        })
        .await?;

    Ok(())
}
