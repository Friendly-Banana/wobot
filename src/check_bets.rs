use poise::serenity_prelude::{
    ChannelId, Context, CreateAllowedMentions, CreateMessage, Mentionable, UserId,
};
use sqlx::{PgPool, query};
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

struct ExpiredBet {
    id: i32,
    bet_short_id: i32,
    channel_id: i64,
    message_id: i64,
    author_id: i64,
    description: String,
    expiry: chrono::DateTime<chrono::Utc>,
}

struct Participant {
    user_id: i64,
    status: String,
}

async fn process_expired_bets(ctx: &Context, database: &PgPool) -> anyhow::Result<()> {
    let _ = span!(Level::DEBUG, "Processing expired bets").enter();

    let expired_bets = sqlx::query_as!(
        ExpiredBet,
        r#"
        SELECT id, bet_short_id, channel_id, message_id, author_id, description, expiry as "expiry!"
        FROM bets
        WHERE expiry <= now()
        "#
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
        let original_msg_id = poise::serenity_prelude::MessageId::new(bet.message_id as u64);

        let participants = sqlx::query_as!(
            Participant,
            "SELECT user_id, status FROM bet_participants WHERE bet_id = $1",
            bet_id
        )
        .fetch_all(database)
        .await?;

        let user_mentions: Vec<String> = participants
            .iter()
            .map(|p| UserId::new(p.user_id as u64).mention().to_string())
            .collect();

        let participants_text = if user_mentions.is_empty() {
            "No participants".to_string()
        } else {
            user_mentions.join(", ")
        };



        let embed = poise::serenity_prelude::CreateEmbed::new()
            .title(format!("Bet #{} Expired!", bet.bet_short_id))
            .description(&bet.description)
            .field("Participants", participants_text, false)
            .color(poise::serenity_prelude::Color::RED);

        let allowed_mentions = CreateAllowedMentions::new()
            .everyone(false)
            .all_roles(false)
            .all_users(true);

        let mut msg = CreateMessage::new()
            .embed(embed)
            .allowed_mentions(allowed_mentions);

        match channel.message(ctx, original_msg_id).await {
            Ok(original_msg) => {
                msg = msg.reference_message(&original_msg);
            }
            Err(_) => {
                debug!("Original bet message not found, sending regular message");
            }
        }

        if let Err(e) = channel.send_message(ctx, msg).await {
             error!(error = ?e, "Failed to send bet expiration message");
        } else {
            trace!(?bet.id, "Sent bet expiration");
        }

        // Delete the bet after attempting to notify
        if let Err(e) = sqlx::query!("DELETE FROM bets WHERE id = $1", bet_id)
            .execute(database)
            .await
        {
             error!(error = ?e, "Failed to delete expired bet {}", bet_id);
        }
    }

    Ok(())
}
