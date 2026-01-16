use std::time::Duration;

use poise::CreateReply;
use poise::serenity_prelude::{
    ButtonStyle, ComponentInteractionCollector, CreateActionRow, CreateButton,
    CreateInteractionResponse, EditMessage,
};

use crate::Context;

const BOOP_TIMEOUT: Duration = Duration::from_secs(10);

// Adopted from https://github.com/serenity-rs/poise/blob/ec19915d817cc6ad8f02ec0cab260d29d2704cce/examples/feature_showcase/collector.rs
/// Boop the bot!
#[poise::command(slash_command, prefix_command, track_edits)]
pub(crate) async fn boop(ctx: Context<'_>) -> anyhow::Result<()> {
    let uuid_boop = ctx.id();

    let reply = {
        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(uuid_boop.to_string())
                .style(ButtonStyle::Primary)
                .label("Boop me!"),
        ])];

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
    // clear button
    let message = CreateReply::default()
        .content(format!("Total boops: {}", boop_count))
        .components(vec![]);
    reply_handle.edit(ctx, message).await?;

    Ok(())
}
