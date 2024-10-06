use std::collections::HashSet;

use chrono::Utc;
use itertools::Itertools;
use poise::futures_util::StreamExt;
use tracing::{error, warn};

use crate::constants::ONE_DAY;
use crate::{Context, Error};

/// List inactive users
#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn inactive(ctx: Context<'_>, days: Option<u32>) -> Result<(), Error> {
    ctx.defer().await?;
    let cutoff = (Utc::now() - ONE_DAY * days.unwrap_or(30)).timestamp();
    let mut active = HashSet::new();
    let channels = ctx.guild_id().unwrap().channels(ctx).await?;
    for (channel_id, c) in channels {
        println!("Channel {}", c.name);
        let mut messages = channel_id.messages_iter(&ctx).boxed();
        while let Some(message_result) = messages.next().await {
            match message_result {
                Ok(message) => {
                    active.insert(message.author);
                    if message.timestamp.unix_timestamp() < cutoff {
                        break;
                    }
                }
                Err(error) => {
                    warn!("Uh oh! Error: {}", error);
                    break;
                }
            }
        }
    }
    println!("Active user: {}", active.iter().map(|u| &u.name).join(", "));
    let mut inactive = Vec::new();
    let mut members = ctx.guild_id().unwrap().members_iter(&ctx).boxed();
    while let Some(member_result) = members.next().await {
        match member_result {
            Ok(member) => {
                if !active.contains(&member.user) {
                    inactive.push(member.user)
                }
            }
            Err(error) => error!("Error: {}", error),
        }
    }
    println!("Inactive: {}", inactive.iter().map(|u| &u.name).join(", "));
    Ok(())
}
