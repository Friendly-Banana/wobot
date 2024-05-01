use crate::{Context, Error};

/// Exclude channel
#[poise::command(slash_command, prefix_command, owners_only, ephemeral, guild_only)]
pub(crate) async fn exclude(ctx: Context<'_>) -> Result<(), Error> {
    let channel_name = ctx
        .channel_id()
        .name(ctx.http())
        .await
        .unwrap_or("Channel".to_string());

    let inserted = {
        let mut excluded = ctx
            .data()
            .excluded_channels
            .write()
            .expect("excluded_channels");
        excluded.insert(ctx.channel_id())
    };
    if inserted {
        ctx.reply(format!("Excluded {}", channel_name)).await?;
    } else {
        {
            let mut lock = ctx
                .data()
                .excluded_channels
                .write()
                .expect("excluded_channels");
            lock.remove(&ctx.channel_id());
        }
        ctx.reply(format!("{} is no longer excluded", channel_name))
            .await?;
    }
    Ok(())
}
