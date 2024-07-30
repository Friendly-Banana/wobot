use poise::serenity_prelude::{GetMessages, Message};
use tracing::error;

use crate::{Context, Error};

/// Delete messages in bulk
/// Only works for messages less than 14 days old
#[poise::command(
    slash_command,
    prefix_command,
    ephemeral,
    required_bot_permissions = "MANAGE_MESSAGES"
)]
pub(crate) async fn clear(
    ctx: Context<'_>,
    #[description = "between 2 and 100, default 2"] amount: Option<u8>,
    before: Option<Message>,
) -> Result<(), Error> {
    let is_mod = ctx
        .author_member()
        .await
        .is_some_and(|member| member.permissions.unwrap().manage_messages());
    if !(is_mod || ctx.framework().options.owners.contains(&ctx.author().id)) {
        ctx.say("You do not have permission to use this command")
            .await?;
        return Ok(());
    }

    let amount = amount.unwrap_or(2);
    if !(2..=100).contains(&amount) {
        ctx.say("Amount must be between 2 and 100").await?;
        return Ok(());
    }
    let mut get_messages = GetMessages::new();
    if let Some(msg) = before {
        get_messages = get_messages.before(msg.id);
    }
    let msgs = ctx
        .channel_id()
        .messages(ctx.http(), get_messages.limit(amount))
        .await?;
    let actual_amount = msgs.len();
    if actual_amount < 2 {
        ctx.say("Amount must be between 2 and 100").await?;
        return Ok(());
    }
    match ctx.channel_id().delete_messages(ctx.http(), msgs).await {
        Ok(_) => {
            ctx.say(format!("Deleted {} messages.", actual_amount))
                .await?;
        }
        Err(e) => {
            error!("Error deleting messages: {:?}", e);
            ctx.say("Error deleting messages").await?;
        }
    }

    Ok(())
}
