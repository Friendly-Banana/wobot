use crate::{Context, UserError};
use chrono::{DateTime, Utc};
use poise::CreateReply;
use poise::serenity_prelude::{ChannelId, Mentionable, MessageId, UserId};
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
    #[description = "Your pick/reasoning for this bet (max 100 chars)"] comment: String,
    #[description = "How long until it expires? (datetime or duration)"] duration: String,
    #[description = "Users to ping (optional)"] pings: Option<String>,
) -> Result<(), anyhow::Error> {
    if description.len() > 2000 {
        return Err(UserError::err(
            "Description is too long (max 2000 characters).",
        ));
    }

    if comment.len() > 100 {
        return Err(UserError::err(
            "Pick/reasoning is too long (max 100 characters).",
        ));
    }

    let expiry = crate::commands::utils::parse_duration_or_date(Utc::now(), &duration).await?;
    if expiry <= Utc::now() {
        return Err(UserError::err("Expiry cannot be in the past."));
    }
    if expiry > Utc::now() + chrono::Duration::days(365) {
        return Err(UserError::err(
            "Expiry cannot be more than 1 year in the future.",
        ));
    }

    let author_id = ctx.author().id.get() as i64;
    let channel_id = ctx.channel_id().get() as i64;
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    if guild_id == 0 {
        return Err(UserError::err("Bets can only be created in servers."));
    }

    let mut attempts = 0;
    let max_attempts = 3;
    let mut bet_short_id = 0;
    let mut bet_id = 0;

    while attempts < max_attempts {
        let mut tx = ctx.data().database.begin().await?;

        let next_id_row = sqlx::query!(
            "SELECT COALESCE(MAX(bet_short_id), 0) + 1 as next_id FROM bets WHERE guild_id = $1",
            guild_id
        )
        .fetch_one(&mut *tx)
        .await?;

        let current_short_id = next_id_row.next_id.unwrap_or(1);

        // Try insert bet
        let bet_row = sqlx::query!(
            "INSERT INTO bets (guild_id, bet_short_id, channel_id, message_id, author_id, description, expiry) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
            guild_id,
            current_short_id,
            channel_id,
            0,
            author_id,
            description,
            expiry
        )
        .fetch_one(&mut *tx)
        .await;

        match bet_row {
            Ok(row) => {
                bet_id = row.id;
                bet_short_id = current_short_id;

                sqlx::query!(
                    "INSERT INTO bet_participants (bet_id, user_id, status, comment) VALUES ($1, $2, 'accepted', $3)",
                    bet_id,
                    author_id,
                    &comment
                )
                .execute(&mut *tx)
                .await?;

                tx.commit().await?;
                break;
            }
            Err(e) => {
                // Check if it's a unique constraint violation (code 23505 in Postgres)
                if let Some(db_err) = e.as_database_error()
                    && let Some(code) = db_err.code()
                    && code == "23505"
                {
                    attempts += 1;
                    continue;
                }
                return Err(e.into());
            }
        }
    }

    if bet_id == 0 {
        return Err(UserError::err(
            "Failed to create bet due to high contention. Please try again.",
        ));
    }

    let mut embed = build_bet_embed(
        bet_short_id,
        &description,
        expiry,
        &[Participant {
            user_id: author_id,
            status: "accepted".to_string(),
            comment,
        }],
    );

    if let Some(ref p) = pings {
        embed = embed.field("Attention", p, false);
    }

    let allowed_mentions = poise::serenity_prelude::CreateAllowedMentions::new()
        .everyone(false)
        .all_roles(false)
        .all_users(true);

    let handle = ctx
        .send(
            CreateReply::default()
                .embed(embed)
                .allowed_mentions(allowed_mentions)
                .reply(true),
        )
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
    #[description = "Your pick/reasoning for this bet (max 100 chars)"] comment: String,
    #[description = "Bet ID (optional, defaults to last bet in channel)"] bet_id: Option<i32>,
) -> Result<(), anyhow::Error> {
    if comment.len() > 100 {
        return Err(UserError::err(
            "Pick/reasoning is too long (max 100 characters).",
        ));
    }

    let bet = find_bet(ctx, bet_id).await?;
    let user_id_i64 = ctx.author().id.get() as i64;

    let status_row = query!(
        "SELECT status FROM bet_participants WHERE bet_id = $1 AND user_id = $2",
        bet.id,
        user_id_i64
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    if let Some(row) = status_row
        && row.status == "accepted"
    {
        ctx.send(
            CreateReply::default()
                .content("You have already joined this bet!")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    query!(
        "INSERT INTO bet_participants (bet_id, user_id, status, comment) VALUES ($1, $2, 'accepted', $3) ON CONFLICT (bet_id, user_id) DO UPDATE SET status = 'accepted', comment = $3",
        bet.id,
        user_id_i64,
        comment
    )
    .execute(&ctx.data().database)
    .await?;

    if let Err(e) = update_bet_message(ctx, bet.id).await {
        tracing::error!("Failed to update bet message: {:?}", e);
        ctx.send(
            CreateReply::default()
                .content("You were added to the bet, but I couldn't update the original message.")
                .ephemeral(true),
        )
        .await?;
    }

    let link = format!(
        "https://discord.com/channels/{}/{}/{}",
        bet.guild_id, bet.channel_id, bet.message_id
    );

    ctx.send(
        CreateReply::default().embed(
            poise::serenity_prelude::CreateEmbed::new()
                .title("Joined Bet")
                .description(format!(
                    "{} joined the bet #{}: **{}**\n\n[Jump to Bet]({})",
                    ctx.author().mention(),
                    bet.bet_short_id,
                    bet.description,
                    link
                ))
                .color(poise::serenity_prelude::Color::DARK_GREEN),
        ),
    )
    .await?;
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
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        } else if row.status == "accepted" {
            ctx.send(
                CreateReply::default()
                    .content(
                        "You are already a participant in this bet and cannot switch to watching!",
                    )
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    }

    query!(
        "INSERT INTO bet_participants (bet_id, user_id, status, comment) VALUES ($1, $2, 'watching', '') ON CONFLICT (bet_id, user_id) DO UPDATE SET status = 'watching'",
        bet.id,
        user_id_i64
    )
    .execute(&ctx.data().database)
    .await?;

    if let Err(e) = update_bet_message(ctx, bet.id).await {
        tracing::error!("Failed to update bet message: {:?}", e);
        ctx.send(
            CreateReply::default()
                .content(
                    "You are now watching the bet, but I couldn't update the original message.",
                )
                .ephemeral(true),
        )
        .await?;
    }

    ctx.send(
        CreateReply::default()
            .embed(
                poise::serenity_prelude::CreateEmbed::new()
                    .title("Watching Bet")
                    .description(format!(
                        "You are now watching the bet #{}: **{}**",
                        bet.bet_short_id, bet.description
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
        "SELECT user_id, status, comment FROM bet_participants WHERE bet_id = $1 ORDER BY status, user_id",
        bet_data.id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    let mut embed = build_bet_embed(
        bet.bet_short_id,
        &bet.description,
        bet.expiry,
        &participants,
    );

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
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);
    if guild_id == 0 {
        return Err(UserError::err("Bets only work in servers"));
    }

    let bets = query!(
        "SELECT bet_short_id, description, expiry, channel_id, message_id FROM bets WHERE guild_id = $1 ORDER BY expiry ASC LIMIT 25",
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

    let mut embed = poise::serenity_prelude::CreateEmbed::new()
        .title("Active Bets")
        .color(crate::commands::utils::random_color());

    for bet in bets {
        let link = format!(
            "https://discord.com/channels/{}/{}/{}",
            guild_id, bet.channel_id, bet.message_id
        );
        let desc = format!(
            "[{}]({}) (Ends <t:{}:R>)",
            bet.description,
            link,
            bet.expiry.timestamp()
        );

        embed = embed.field(format!("ID: {}", bet.bet_short_id), desc, false);
    }

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

struct BetData {
    id: i32,
    bet_short_id: i32,
    description: String,
    guild_id: i64,
    channel_id: i64,
    message_id: i64,
}

struct Participant {
    user_id: i64,
    status: String,
    comment: String,
}

async fn find_bet(
    ctx: Context<'_>,
    bet_short_id_opt: Option<i32>,
) -> Result<BetData, anyhow::Error> {
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);
    if guild_id == 0 {
        return Err(UserError::err("Bets only work in servers"));
    }

    if let Some(short_id) = bet_short_id_opt {
        let record = query_as!(
            BetData,
            "SELECT id, bet_short_id, description, guild_id, channel_id, message_id FROM bets WHERE guild_id = $1 AND bet_short_id = $2",
            guild_id,
            short_id
        )
        .fetch_optional(&ctx.data().database)
        .await?;

        return record.ok_or_else(|| UserError::err("Bet not found"));
    }

    let channel_id = ctx.channel_id().get() as i64;
    let record = query_as!(
        BetData,
        "SELECT id, bet_short_id, description, guild_id, channel_id, message_id FROM bets WHERE channel_id = $1 ORDER BY created_at DESC LIMIT 1",
        channel_id
    )
    .fetch_optional(&ctx.data().database)
    .await?;

    record.ok_or_else(|| UserError::err("No active bets found in this channel"))
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
        let entry = if p.comment.is_empty() {
            mention
        } else {
            format!("{} ({})", mention, p.comment)
        };

        if p.status == "accepted" {
            accepted.push(entry);
        } else if p.status == "watching" {
            watching.push(entry);
        }
    }

    let mut embed = poise::serenity_prelude::CreateEmbed::new()
        .title(format!("Bet #{}", short_id))
        .description(description)
        .color(crate::commands::utils::random_color())
        .footer(poise::serenity_prelude::CreateEmbedFooter::new(format!(
            "Join this bet with /bet join <reasoning> {}",
            short_id
        )));

    if !accepted.is_empty() {
        let count = accepted.len();
        let max_display = 20;
        if count > max_display {
            let displayed = accepted
                .iter()
                .take(max_display)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            embed = embed.field(
                "Participants",
                format!("{} and {} others", displayed, count - max_display),
                false,
            );
        } else {
            embed = embed.field("Participants", accepted.join("\n"), false);
        }
    } else {
        embed = embed.field("Participants", "No one yet", false);
    }

    if !watching.is_empty() {
        let count = watching.len();
        let max_display = 20;
        if count > max_display {
            let displayed = watching
                .iter()
                .take(max_display)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            embed = embed.field(
                "Watching",
                format!("{} and {} others", displayed, count - max_display),
                false,
            );
        } else {
            embed = embed.field("Watching", watching.join("\n"), false);
        }
    }

    embed = embed.field("Expires", format!("<t:{}:R>", expiry.timestamp()), true);

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
        "SELECT user_id, status, comment FROM bet_participants WHERE bet_id = $1 ORDER BY status, user_id",
        bet_id
    )
    .fetch_all(&ctx.data().database)
    .await?;

    let embed = build_bet_embed(
        bet.bet_short_id,
        &bet.description,
        bet.expiry,
        &participants,
    );

    channel_id
        .edit_message(
            ctx,
            message_id,
            poise::serenity_prelude::EditMessage::new().embed(embed),
        )
        .await?;

    Ok(())
}
