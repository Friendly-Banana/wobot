use anyhow::Context as _;
use poise::serenity_prelude::{CreateEmbed, CreateEmbedFooter, UserId};
use poise::CreateReply;
use sqlx::query;

use crate::{Context, Error};

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
