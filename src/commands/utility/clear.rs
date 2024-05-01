use poise::serenity_prelude::{GetMessages, Message};
use tracing::error;

use crate::{Context, Error};

/// Delete messages in bulk
/// Only works for messages less than 14 days old
#[poise::command(
    slash_command,
    prefix_command,
    ephemeral,
    required_permissions = "MANAGE_MESSAGES",
    required_bot_permissions = "MANAGE_MESSAGES"
)]
pub(crate) async fn clear(
    ctx: Context<'_>,
    #[description = "between 2 and 100, default 2"] amount: Option<u8>,
    #[description = "Message to delete before"] message: Option<Message>,
) -> Result<(), Error> {
    let amount = amount.unwrap_or(2);
    if amount < 2 || amount > 100 {
        ctx.say("Amount must be between 2 and 100").await?;
        return Ok(());
    }
    let mut b = GetMessages::new();
    if let Some(msg) = message {
        b = b.before(msg.id);
    }
    let msgs = ctx
        .channel_id()
        .messages(ctx.http(), b.limit(amount))
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
