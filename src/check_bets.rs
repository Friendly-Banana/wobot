use chrono::{DateTime, Utc};
use itertools::Itertools;
use poise::serenity_prelude::*;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;
use tracing::{Level, debug, error, info, span, trace};

pub(crate) fn check_bets(ctx: Context, database: PgPool) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60));

        loop {
            interval.tick().await;

            if let Err(err) = process_expired_bets(&ctx, &database).await {
                error!(error = ?err, "Failed processing expired bets");
            }
        }
    });
    info!("Started bet checker thread");
}

async fn process_expired_bets(ctx: &Context, database: &PgPool) -> anyhow::Result<()> {
    let _ = span!(Level::DEBUG, "Sending expired bets").enter();

    let expired_bets = sqlx::query_as!(
        Bet,
        "SELECT id, channel_id, message_id, description, created_at FROM bets WHERE expiry <= now()"
    )
    .fetch_all(database)
    .await?;

    if expired_bets.is_empty() {
        return Ok(());
    }

    debug!(count = expired_bets.len(), "Found expired bets");

    for bet in expired_bets {
        let bet_id = bet.id;
        let channel = ChannelId::new(bet.channel_id as u64);
        let original_msg_id = MessageId::new(bet.message_id as u64);

        let participants = sqlx::query!(
            "SELECT user_id, watching FROM bet_participants WHERE bet_id = $1",
            bet_id
        )
        .fetch_all(database)
        .await?;

        let user_mentions: Vec<String> = participants
            .iter()
            .filter(|p| !p.watching)
            .map(|p| UserId::new(p.user_id as u64).mention().to_string())
            .collect();

        let participants_text = if user_mentions.is_empty() {
            "No participants".to_string()
        } else {
            user_mentions.join(", ")
        };

        let embed = CreateEmbed::new()
            .title(format!("Bet #{} is over!", bet.id))
            .description(&bet.description)
            .field(
                "Created at",
                format!("<t:{}:R>", bet.created_at.timestamp()),
                true,
            )
            .field("Participants", participants_text, false)
            .color(Color::GOLD);

        // ping participants and watchers
        let user_mentions = participants
            .iter()
            .map(|p| UserId::new(p.user_id as u64).mention().to_string())
            .join(", ");

        let msg = CreateMessage::new()
            .embed(embed)
            .reference_message(
                MessageReference::new(MessageReferenceKind::Default, channel)
                    .message_id(original_msg_id)
                    .fail_if_not_exists(false),
            )
            .content(user_mentions);

        channel.send_message(ctx, msg).await?;

        sqlx::query!("DELETE FROM bets WHERE id = $1", bet_id)
            .execute(database)
            .await?;
        trace!(?bet, "Sent bet expiration");
    }

    Ok(())
}

#[derive(Debug)]
struct Bet {
    pub id: i64,
    pub channel_id: i64,
    pub message_id: i64,
    pub description: String,
    pub created_at: DateTime<Utc>,
}
