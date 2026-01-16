use anyhow::Context as _;
use poise::serenity_prelude::{GetMessages, Message};

use crate::{Context, UserError};

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
) -> anyhow::Result<()> {
    let is_mod = ctx
        .author_member()
        .await
        .is_some_and(|member| member.permissions.unwrap().manage_messages());
    if !(is_mod || ctx.framework().options.owners.contains(&ctx.author().id)) {
        return Err(UserError::err(
            "You do not have permission to use this command",
        ));
    }

    let amount = amount.unwrap_or(2);
    if !(2..=100).contains(&amount) {
        return Err(UserError::err("Amount must be between 2 and 100"));
    }
    let mut get_messages = GetMessages::new();
    if let Some(msg) = before {
        get_messages = get_messages.before(msg.id);
    }
    let messages = ctx
        .channel_id()
        .messages(ctx.http(), get_messages.limit(amount))
        .await?;
    let actual_amount = messages.len();
    match ctx.channel_id().delete_messages(ctx.http(), messages).await {
        Ok(_) => {
            ctx.say(format!("Deleted {} messages.", actual_amount))
                .await?;
            Ok(())
        }
        err => err.context("Error deleting messages"),
    }
}
