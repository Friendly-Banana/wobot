use crate::{Context, UserError};
use chrono::{DateTime, Utc};
use poise::CreateReply;
use poise::serenity_prelude::{ChannelId, MessageId, UserId, Mentionable};
use sqlx::{query, query_as};

#[poise::command(
    slash_command,
    subcommands("create", "join", "watch", "status", "list"),
    category = "Betting"
)]
pub async fn bet(_ctx: Context<'_>) -> Result<(), anyhow::Error> {
    Ok(())
}

/// Create a new bet
#[poise::command(slash_command)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "What is the bet about?"] description: String,
    #[description = "How long until it expires? (datetime or duration)"] duration: String,
    #[description = "Users to ping (optional)"] pings: Option<String>,
) -> Result<(), anyhow::Error> {
    let expiry = crate::commands::utils::parse_duration_or_date(Utc::now(), &duration).await?;

    let author_id = ctx.author().id.get() as i64;
    let channel_id = ctx.channel_id().get() as i64;
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    if guild_id == 0 {
        return Err(UserError::err("Bets can only be created in servers."));
    }

    let mut tx = ctx.data().database.begin().await?;

    let next_id_row = sqlx::query!(
        "SELECT COALESCE(MAX(bet_short_id), 0) + 1 as next_id FROM bets WHERE guild_id = $1",
        guild_id
    )
    .fetch_one(&mut *tx)
    .await?;

    let bet_short_id = next_id_row.next_id.unwrap_or(1);

    let bet_row = sqlx::query!(
        "INSERT INTO bets (guild_id, bet_short_id, channel_id, message_id, author_id, description, expiry) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        guild_id,
        bet_short_id,
        channel_id,
        0,
        author_id,
        description,
        expiry
    )
    .fetch_one(&mut *tx)
    .await?;

    let bet_id = bet_row.id;

    sqlx::query!(
        "INSERT INTO bet_participants (bet_id, user_id, status) VALUES ($1, $2, 'accepted')",
        bet_id,
        author_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut embed = poise::serenity_prelude::CreateEmbed::new()
        .title(format!("Bet #{}", bet_short_id))
        .description(&description)
        .field("Expires", format!("<t:{}:R>", expiry.timestamp()), true)
        .field("Bet ID", bet_short_id.to_string(), true)
        .field("Participants", ctx.author().mention().to_string(), false)
        .color(crate::commands::utils::random_color())
        .footer(poise::serenity_prelude::CreateEmbedFooter::new(format!(
            "Use /bet join {} (or just /bet join)",
            bet_short_id
        )));

    if let Some(ref p) = pings {
         embed = embed.field("Attention", p, false);
    }

    let handle = ctx.send(CreateReply::default().embed(embed).reply(true)).await?;
    let message = handle.message().await?;
    let message_id = message.id.get() as i64;

    query!(
        "UPDATE bets SET message_id = $1 WHERE id = $2",
        message_id,
        bet_id
    )
    .execute(&ctx.data().database)
    .await?;

    Ok(())
}

/// Join a bet
#[poise::command(slash_command)]
pub async fn join(
    ctx: Context<'_>,
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i32>,
) -> Result<(), anyhow::Error> {
    let bet = find_bet(ctx, bet_id).await?;
    let user_id_i64 = ctx.author().id.get() as i64;

    let status_row = query!(
        "SELECT status FROM bet_participants WHERE bet_id = $1 AND user_id = $2",
        bet.id,
        user_id_i64
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    if let Some(row) = status_row {
        if row.status == "accepted" {
            ctx.send(
                CreateReply::default()
                    .content("You have already joined this bet!")
                    .ephemeral(true)
            ).await?;
            return Ok(());
        }
    }

    query!(
        "INSERT INTO bet_participants (bet_id, user_id, status) VALUES ($1, $2, 'accepted') ON CONFLICT (bet_id, user_id) DO UPDATE SET status = 'accepted'",
        bet.id,
        user_id_i64
    )
    .execute(&ctx.data().database)
    .await?;

    if let Err(e) = update_bet_message(ctx, bet.id).await {
        tracing::error!("Failed to update bet message: {:?}", e);
    }

    ctx.send(
        CreateReply::default()
            .embed(
                poise::serenity_prelude::CreateEmbed::new()
                    .title("Joined Bet")
                    .description(format!("You joined the bet #{}: **{}**", bet.bet_short_id, bet.description))
                    .color(poise::serenity_prelude::Color::DARK_GREEN)
            )
            .ephemeral(true)
    ).await?;
    Ok(())
}

/// Watch a bet (get notified but not a participant)
#[poise::command(slash_command)]
pub async fn watch(
    ctx: Context<'_>,
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i32>,
) -> Result<(), anyhow::Error> {
    let bet = find_bet(ctx, bet_id).await?;
    let user_id_i64 = ctx.author().id.get() as i64;

    let status_row = query!(
        "SELECT status FROM bet_participants WHERE bet_id = $1 AND user_id = $2",
        bet.id,
        user_id_i64
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    if let Some(row) = status_row {
        if row.status == "watching" {
            ctx.send(
                CreateReply::default()
                    .content("You are already watching this bet!")
                    .ephemeral(true)
            ).await?;
            return Ok(());
        } else if row.status == "accepted" {
             ctx.send(
                CreateReply::default()
                    .content("You are already a participant in this bet and cannot switch to watching!")
                    .ephemeral(true)
            ).await?;
            return Ok(());
        }
    }

    query!(
        "INSERT INTO bet_participants (bet_id, user_id, status) VALUES ($1, $2, 'watching') ON CONFLICT (bet_id, user_id) DO UPDATE SET status = 'watching'",
        bet.id,
        user_id_i64
    )
    .execute(&ctx.data().database)
    .await?;

    if let Err(e) = update_bet_message(ctx, bet.id).await {
        tracing::error!("Failed to update bet message: {:?}", e);
    }

    ctx.send(
        CreateReply::default()
            .embed(
                poise::serenity_prelude::CreateEmbed::new()
                    .title("Watching Bet")
                    .description(format!("You are now watching the bet #{}: **{}**", bet.bet_short_id, bet.description))
                    .color(poise::serenity_prelude::Color::BLUE)
            )
            .ephemeral(true)
    ).await?;
    Ok(())
}

/// Show status of a bet and link to original message
#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i32>,
) -> Result<(), anyhow::Error> {
    let bet_data = find_bet(ctx, bet_id).await?;

    let bet = query!(
        "SELECT guild_id, channel_id, message_id, description, expiry, bet_short_id FROM bets WHERE id = $1",
        bet_data.id
    )
    .fetch_one(&ctx.data().database)
    .await?;

    let participants = query_as!(
        Participant,
        "SELECT user_id, status FROM bet_participants WHERE bet_id = $1 ORDER BY status, user_id",
        bet_data.id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    let mut embed = build_bet_embed(
        bet.bet_short_id,
        &bet.description,
        bet.expiry,
        &participants
    );

    let link = format!(
        "https://discord.com/channels/{}/{}/{}",
        bet.guild_id, bet.channel_id, bet.message_id
    );
    embed = embed.field("Original Message", format!("[Jump to Bet]({})", link), false);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// List all active bets in this server
#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), anyhow::Error> {
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);
    if guild_id == 0 {
         return Err(UserError::err("Bets only work in servers"));
    }

    let bets = query!(
        "SELECT bet_short_id, description, expiry FROM bets WHERE guild_id = $1 ORDER BY expiry ASC LIMIT 25",
        guild_id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    if bets.is_empty() {
        ctx.send(CreateReply::default().content("No active bets found on this server.").ephemeral(true)).await?;
        return Ok(());
    }

    let mut embed = poise::serenity_prelude::CreateEmbed::new()
        .title("Active Bets")
        .color(crate::commands::utils::random_color());

    for bet in bets {
        embed = embed.field(
            format!("ID: {}", bet.bet_short_id),
            format!("{} (Ends <t:{}:R>)", bet.description, bet.expiry.timestamp()),
            false
        );
    }

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

struct BetData {
    id: i32,
    bet_short_id: i32,
    description: String,
}

struct Participant {
    user_id: i64,
    status: String,
}

async fn find_bet(ctx: Context<'_>, bet_short_id_opt: Option<i32>) -> Result<BetData, anyhow::Error> {
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);
    if guild_id == 0 {
         return Err(UserError::err("Bets only work in servers"));
    }

    if let Some(short_id) = bet_short_id_opt {
        let record = query_as!(
            BetData,
            "SELECT id, bet_short_id, description FROM bets WHERE guild_id = $1 AND bet_short_id = $2",
            guild_id,
            short_id
        )
        .fetch_optional(&ctx.data().database)
        .await?;

        return record.ok_or_else(|| UserError::err("Bet not found").into());
    }

    let channel_id = ctx.channel_id().get() as i64;
    let record = query_as!(
        BetData,
        "SELECT id, bet_short_id, description FROM bets WHERE channel_id = $1 ORDER BY created_at DESC LIMIT 1",
        channel_id
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    record.ok_or_else(|| UserError::err("No active bets found in this channel").into())
}

fn build_bet_embed(
    short_id: i32,
    description: &str,
    expiry: DateTime<Utc>,
    participants: &[Participant],
) -> poise::serenity_prelude::CreateEmbed {
     let mut accepted = Vec::new();
    let mut watching = Vec::new();

    for p in participants {
        let mention = UserId::new(p.user_id as u64).mention().to_string();
        if p.status == "accepted" {
            accepted.push(mention);
        } else if p.status == "watching" {
            watching.push(mention);
        }
    }

    let mut embed = poise::serenity_prelude::CreateEmbed::new()
        .title(format!("Bet #{}", short_id))
        .description(description)
        .field("Expires", format!("<t:{}:R>", expiry.timestamp()), true)
        .field("Bet ID", short_id.to_string(), true)
        .color(crate::commands::utils::random_color())
        .footer(poise::serenity_prelude::CreateEmbedFooter::new(format!(
            "Use /bet join {} (or just /bet join)",
            short_id
        )));

    if !accepted.is_empty() {
        embed = embed.field("Participants", accepted.join(", "), false);
    } else {
         embed = embed.field("Participants", "No one yet", false);
    }

    if !watching.is_empty() {
        embed = embed.field("Watching", watching.join(", "), false);
    }

    embed
}

async fn update_bet_message(ctx: Context<'_>, bet_id: i32) -> anyhow::Result<()> {
    let bet = query!(
        "SELECT channel_id, message_id, description, expiry, bet_short_id FROM bets WHERE id = $1",
        bet_id
    )
    .fetch_one(&ctx.data().database)
    .await?;

    let channel_id = ChannelId::new(bet.channel_id as u64);
    let message_id = MessageId::new(bet.message_id as u64);

    let participants = query_as!(
        Participant,
        "SELECT user_id, status FROM bet_participants WHERE bet_id = $1 ORDER BY status, user_id",
        bet_id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    let embed = build_bet_embed(
        bet.bet_short_id,
        &bet.description,
        bet.expiry,
        &participants
    );

    channel_id.edit_message(ctx, message_id, poise::serenity_prelude::EditMessage::new().embed(embed)).await?;

    Ok(())
}
