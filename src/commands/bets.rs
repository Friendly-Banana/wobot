use crate::commands::utils::{parse_duration_or_date, random_color};

use crate::{Context, UserError};
use chrono::{DateTime, Utc};
use poise::CreateReply;
use poise::serenity_prelude::{
    ChannelId, CreateEmbed, CreateEmbedFooter, FormattedTimestamp, Mentionable, MessageId,
    Timestamp, UserId,
};
use sqlx::{query, query_as};
use tracing::error;

#[poise::command(
    slash_command,
    subcommands("create", "join", "watch", "status", "list")
)]
pub async fn bet(_ctx: Context<'_>) -> Result<(), anyhow::Error> {
    Ok(())
}

/// Create a new bet
#[poise::command(slash_command, guild_only)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "What is the bet about?"]
    #[max_length = 4096]
    description: String,
    #[description = "When is it over? datetime or duration"] end: String,
) -> Result<(), anyhow::Error> {
    ctx.defer().await?;
    let expiry = parse_duration_or_date(Utc::now(), &end).await?;

    let author_id = ctx.author().id.get() as i64;
    let channel_id = ctx.channel_id().get() as i64;
    let guild_id = ctx.guild_id().expect("guild_only").get() as i64;

    let mut tx = ctx.data().database.begin().await?;

    let bet_row = sqlx::query!(
        "INSERT INTO bets (guild_id, channel_id, message_id, author_id, description, expiry) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        guild_id,
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
        "INSERT INTO bet_participants (bet_id, user_id, watching) VALUES ($1, $2, false)",
        bet_id,
        author_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let embed = CreateEmbed::new()
        .title(format!("Bet #{}", bet_id))
        .description(&description)
        .field(
            "Expires",
            FormattedTimestamp::from(Timestamp::from(expiry)).to_string(),
            true,
        )
        .field("Participants", ctx.author().mention().to_string(), false)
        .color(random_color())
        .footer(CreateEmbedFooter::new(format!(
            "Use /bet join {} to participate",
            bet_id
        )));

    let handle = ctx
        .send(CreateReply::default().embed(embed).reply(true))
        .await?;
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
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i64>,
) -> Result<(), anyhow::Error> {
    ctx.defer().await?;
    let bet = get_bet(ctx, bet_id).await?;
    let user_id_i64 = ctx.author().id.get() as i64;

    let status_row = query!(
        "SELECT watching FROM bet_participants WHERE bet_id = $1 AND user_id = $2",
        bet.id,
        user_id_i64
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    if let Some(row) = status_row
        && !row.watching
    {
        return Err(UserError::err("You have already joined this bet!"));
    }

    query!(
        "INSERT INTO bet_participants (bet_id, user_id, watching) VALUES ($1, $2, false) ON CONFLICT (user_id, bet_id) DO UPDATE SET watching = false",
        bet.id,
        user_id_i64
    )
    .execute(&ctx.data().database)
    .await?;

    if let Err(e) = update_bet_message(ctx, bet.id).await {
        error!("Failed to update bet message: {:?}", e);
    }

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title("Joined Bet")
                .description(format!(
                    "You joined the bet #{}: **{}**",
                    bet.id, bet.description
                ))
                .color(poise::serenity_prelude::Color::DARK_GREEN),
        ),
    )
    .await?;
    Ok(())
}

/// Watch a bet (get notified when it ends without participating)
#[poise::command(slash_command)]
pub async fn watch(
    ctx: Context<'_>,
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i64>,
) -> Result<(), anyhow::Error> {
    ctx.defer().await?;
    let bet = get_bet(ctx, bet_id).await?;
    let user_id_i64 = ctx.author().id.get() as i64;

    let result = query!(
        "INSERT INTO bet_participants (bet_id, user_id, watching) VALUES ($1, $2, true)",
        bet.id,
        user_id_i64
    )
    .execute(&ctx.data().database)
    .await;
    if let Err(sqlx::Error::Database(db_err)) = result
        && db_err.is_unique_violation()
    {
        return Err(UserError::err("You are already participating in this bet!"));
    };

    if let Err(e) = update_bet_message(ctx, bet.id).await {
        error!("Failed to update bet message: {:?}", e);
    }

    ctx.send(
        CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .title("Watching Bet")
                    .description(format!(
                        "You are now watching the bet #{}: **{}**",
                        bet.id, bet.description
                    ))
                    .color(poise::serenity_prelude::Color::BLUE),
            )
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

/// Show status of a bet and link to original message
#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i64>,
) -> Result<(), anyhow::Error> {
    ctx.defer().await?;
    let bet_data = get_bet(ctx, bet_id).await?;

    let bet = query!(
        "SELECT guild_id, channel_id, message_id, description, expiry FROM bets WHERE id = $1",
        bet_data.id
    )
    .fetch_one(&ctx.data().database)
    .await?;

    let participants = query_as!(
        BetParticipant,
        "SELECT user_id, watching FROM bet_participants WHERE bet_id = $1 ORDER BY watching, user_id",
        bet_data.id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    let mut embed = build_bet_embed(bet_data.id, &bet.description, bet.expiry, &participants);

    let link = format!(
        "https://discord.com/channels/{}/{}/{}",
        bet.guild_id, bet.channel_id, bet.message_id
    );
    embed = embed.field(
        "Original Message",
        format!("[Jump to Bet]({})", link),
        false,
    );

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// List all active bets in this server
#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), anyhow::Error> {
    ctx.defer().await?;
    let guild_id = ctx.guild_id().expect("guild_only").get() as i64;

    let bets = query!(
        "SELECT id, description, expiry FROM bets WHERE guild_id = $1 ORDER BY expiry ASC LIMIT 25",
        guild_id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    if bets.is_empty() {
        ctx.send(
            CreateReply::default()
                .content("No active bets found on this server.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let mut embed = CreateEmbed::new()
        .title("Active Bets")
        .color(random_color());

    for bet in bets {
        embed = embed.field(
            format!("ID: {}", bet.id),
            format!(
                "{} (Ends <t:{}:R>)",
                bet.description,
                bet.expiry.timestamp()
            ),
            false,
        );
    }

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

struct Bet {
    id: i64,
    description: String,
}

struct BetParticipant {
    pub user_id: i64,
    pub watching: bool,
}

async fn get_bet(ctx: Context<'_>, bet_id_opt: Option<i64>) -> Result<Bet, anyhow::Error> {
    if let Some(id) = bet_id_opt {
        let record = query_as!(Bet, "SELECT id, description FROM bets WHERE id = $1", id)
            .fetch_optional(&ctx.data().database)
            .await?;

        return record.ok_or_else(|| UserError::err("Bet not found"));
    }

    let channel_id = ctx.channel_id().get() as i64;
    let record = query_as!(
        Bet,
        "SELECT id, description FROM bets WHERE channel_id = $1 ORDER BY created_at DESC LIMIT 1",
        channel_id
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    record.ok_or_else(|| UserError::err("No active bets found in this channel"))
}

fn build_bet_embed(
    short_id: i64,
    description: &str,
    expiry: DateTime<Utc>,
    participants: &[BetParticipant],
) -> CreateEmbed {
    let mut accepted = Vec::new();
    let mut watching = Vec::new();

    for p in participants {
        let mention = UserId::new(p.user_id as u64).mention().to_string();
        if p.watching {
            watching.push(mention);
        } else {
            accepted.push(mention);
        }
    }

    let mut embed = CreateEmbed::new()
        .title(format!("Bet #{}", short_id))
        .description(description)
        .field("Expires", format!("<t:{}:R>", expiry.timestamp()), true)
        .color(random_color())
        .footer(CreateEmbedFooter::new(format!(
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

async fn update_bet_message(ctx: Context<'_>, bet_id: i64) -> anyhow::Result<()> {
    let bet = query!(
        "SELECT channel_id, message_id, description, expiry FROM bets WHERE id = $1",
        bet_id
    )
    .fetch_one(&ctx.data().database)
    .await?;

    let channel_id = ChannelId::new(bet.channel_id as u64);
    let message_id = MessageId::new(bet.message_id as u64);

    let participants = query_as!(
        BetParticipant,
        "SELECT user_id, watching FROM bet_participants WHERE bet_id = $1 ORDER BY watching, user_id",
        bet_id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    let embed = build_bet_embed(bet_id, &bet.description, bet.expiry, &participants);

    channel_id
        .edit_message(
            ctx,
            message_id,
            poise::serenity_prelude::EditMessage::new().embed(embed),
        )
        .await?;

    Ok(())
}
