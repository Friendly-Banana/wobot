use crate::{Context, Data, Error, done};
use anyhow::{Context as _, anyhow};
use poise::serenity_prelude::{
    CacheHttp, ChannelId, EmojiId, GuildId, MESSAGE_CODE_LIMIT, Mentionable, Message, MessageId,
    Reaction, ReactionCollector, ReactionType, RoleId,
};
use poise::{CreateReply, serenity_prelude};
use sqlx::{query, query_as};
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const REACTION_ROLE_TIMEOUT: Duration = Duration::from_secs(60);

#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    subcommands("list", "add_easy", "add", "remove")
)]
pub(crate) async fn reaction_role(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Choose the role, then react to the message with the emoji you want to use
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn add_easy(ctx: Context<'_>, role: RoleId) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    ctx.say("React to the message with the emoji").await?;

    let guild_id = ctx.guild_id().expect("guild_only");
    let reaction = ReactionCollector::new(ctx)
        .guild_id(guild_id)
        .author_id(ctx.author().id)
        .timeout(REACTION_ROLE_TIMEOUT)
        .await;
    let reaction = match reaction {
        None => {
            info!("Timeout :(, try again");
            ctx.reply("Timeout :(, try again").await?;
            return Ok(());
        }
        Some(r) => r,
    };
    let msg = reaction
        .channel_id
        .message(ctx.http(), reaction.message_id)
        .await?;
    reaction.delete(ctx.http()).await?;
    add_reaction_role(ctx, role, msg, reaction.emoji.clone()).await
}

/// Choose the role, message and emoji for a new reaction role
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn add(
    ctx: Context<'_>,
    role: RoleId,
    #[description = "Existing Message to react to"] message: Message,
    emoji: ReactionType,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    add_reaction_role(ctx, role, message, emoji).await
}

async fn add_reaction_role(
    ctx: Context<'_>,
    role_id: RoleId,
    message: Message,
    reaction: ReactionType,
) -> Result<(), Error> {
    let mut roles = ctx
        .guild_id()
        .expect("guild_only")
        .roles(ctx.http())
        .await?;
    if !roles.contains_key(&role_id) {
        debug!("Couldn't find role, make sure it exists");
        ctx.say("Couldn't find role, make sure it exists").await?;
        return Ok(());
    }

    if let ReactionType::Custom { id, .. } = reaction
        && ctx
            .guild_id()
            .expect("guild_only")
            .emoji(ctx.http(), id)
            .await
            .is_err()
    {
        debug!("Couldn't find emoji, make sure it's from this guild");
        ctx.say("Couldn't find emoji, make sure it's from this guild")
            .await?;
        return Ok(());
    }

    let role = roles.remove(&role_id).expect("role exists");
    info!(
        "Adding reaction role '{}' here {} with emoji {}...",
        role.name,
        message.link(),
        reaction
    );

    let emoji_id = get_emoji_id(reaction.clone(), ctx.data()).await?;
    let guild_id = ctx.guild_id().expect("guild_only");
    if let Err(e) = query!("INSERT INTO reaction_roles (message_id, channel_id, guild_id, role_id, emoji_id) VALUES ($1, $2, $3, $4, $5)",
        message.id.get() as i64, message.channel_id.get() as i64, guild_id.get() as i64, role_id.get() as i64, emoji_id,
    ).execute(&ctx.data().database).await {
        info!("Adding failed, possible duplicate: {e}");
        ctx.say("Assigning failed, is the role/emoji already assigned to this message?")
            .await?;
        return Ok(());
    }
    {
        let mut reaction_roles = ctx.data().reaction_msgs.write().expect("reaction_msgs");
        reaction_roles.insert(message.id.into())
    };

    message.react(ctx.http(), reaction).await?;
    done!(ctx);
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn remove(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    ctx.say("React to the message").await?;

    let guild_id = ctx.guild_id().expect("guild_only");
    let reaction = ReactionCollector::new(ctx)
        .guild_id(guild_id)
        .author_id(ctx.author().id)
        .timeout(REACTION_ROLE_TIMEOUT)
        .await;
    let reaction = match reaction {
        None => {
            info!("Timeout :(, try again");
            ctx.say("Timeout :(, try again").await?;
            return Ok(());
        }
        Some(r) => r,
    };

    reaction.delete_all(ctx.http()).await?;
    remove_reaction_role(ctx, reaction).await
}

async fn remove_reaction_role(ctx: Context<'_>, reaction: Reaction) -> Result<(), Error> {
    info!(
        "Removing reaction role here {} with emoji {}...",
        reaction
            .message_id
            .link(reaction.channel_id, reaction.guild_id),
        reaction.emoji
    );
    let emoji_id = get_emoji_id(reaction.emoji, ctx.data()).await?;
    query!(
        "DELETE FROM reaction_roles WHERE message_id = $1 AND emoji_id = $2",
        reaction.message_id.get() as i64,
        emoji_id
    )
    .execute(&ctx.data().database)
    .await?;
    {
        let mut reaction_roles = ctx.data().reaction_msgs.write().expect("reaction_msgs");
        reaction_roles.remove(&reaction.message_id.get());
    }
    done!(ctx);
}

#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let show_all_roles = ctx.framework().options.owners.contains(&ctx.author().id);
    let reaction_roles = if show_all_roles {
        ctx.defer_ephemeral().await?;
        query_as!(ReactionRole, "SELECT * FROM reaction_roles")
            .fetch_all(&ctx.data().database)
            .await?
    } else {
        ctx.defer().await?;
        let guild = ctx.guild_id().expect("guild_only");
        query_as!(
            ReactionRole,
            "SELECT * FROM reaction_roles WHERE guild_id = $1",
            guild.get() as i64
        )
        .fetch_all(&ctx.data().database)
        .await?
    };
    let mut roles = VecDeque::from(["**Message | Emoji | Role**".to_string()]);
    for reaction_role in reaction_roles {
        let emoji = get_emoji_from_id(ctx, reaction_role.guild_id, reaction_role.emoji_id).await?;
        let guild = Some(GuildId::new(reaction_role.guild_id as u64));
        let channel_id = reaction_role.channel_id as u64;
        let msg_id = reaction_role.message_id as u64;
        roles.push_back(format!(
            "{} {} {}",
            MessageId::new(msg_id).link(ChannelId::new(channel_id), guild),
            emoji,
            RoleId::new(reaction_role.role_id as u64).mention()
        ));
    }
    // split message into in chunks
    let mut s = roles.pop_front().unwrap();
    loop {
        match roles.pop_front() {
            Some(line) => {
                // we'll add a newline
                if line.len() + s.len() + 1 > MESSAGE_CODE_LIMIT {
                    ctx.reply(s).await?;
                    s = String::new();
                }
                s.push('\n');
                s.push_str(&line);
            }
            None => {
                ctx.send(CreateReply::default().content(s).ephemeral(show_all_roles))
                    .await?;
                break;
            }
        }
    }
    Ok(())
}

async fn get_emoji_id(reaction: ReactionType, data: &Data) -> Result<i64, Error> {
    match reaction {
        ReactionType::Custom { id, .. } => Ok(id.get() as i64),
        ReactionType::Unicode(unicode) => {
            query!(
                "INSERT INTO unicode_to_emoji (unicode) VALUES ($1) ON CONFLICT DO NOTHING",
                unicode
            )
            .execute(&data.database)
            .await?;
            let id = query!(
                "SELECT id FROM unicode_to_emoji WHERE unicode = $1",
                unicode
            )
            .fetch_one(&data.database)
            .await
            .context(format!("Emoji {unicode} should have an id now"))?;
            Ok(id.id)
        }
        _ => unimplemented!(),
    }
}
async fn get_emoji_from_id(
    ctx: Context<'_>,
    guild_id: i64,
    emoji_id: i64,
) -> Result<String, Error> {
    if emoji_id >> 32 == 0 {
        let emoji = query!(
            "SELECT unicode FROM unicode_to_emoji WHERE id = $1",
            emoji_id
        )
        .fetch_one(&ctx.data().database)
        .await
        .context("Emoji should have gotten an id")?;
        return Ok(emoji.unicode);
    }
    Ok(GuildId::new(guild_id as u64)
        .emoji(ctx.http(), EmojiId::new(emoji_id as u64))
        .await
        .map(|e| e.to_string())
        .unwrap_or(emoji_id.to_string()))
}

pub(crate) async fn change_reaction_role(
    ctx: &serenity_prelude::Context,
    data: &Data,
    reaction: &Reaction,
    add: bool,
) -> Result<(), Error> {
    let has_reaction_role = data
        .reaction_msgs
        .read()
        .unwrap()
        .contains(&reaction.message_id.get());
    if !has_reaction_role || reaction.user(ctx.http()).await?.bot {
        return Ok(());
    }

    let emoji = get_emoji_id(reaction.emoji.clone(), data).await?;
    let reaction_role = query!(
        "SELECT * FROM reaction_roles WHERE message_id = $1 AND emoji_id = $2",
        reaction.message_id.get() as i64,
        emoji
    )
    .fetch_optional(&data.database)
    .await?;
    if reaction_role.is_none() {
        warn!(
            "Expected reaction role here {} with reaction {}, might be unrelated reaction",
            reaction
                .message_id
                .link(reaction.channel_id, reaction.guild_id),
            reaction.emoji
        );
        return Ok(());
    };
    let record = reaction_role.unwrap();
    let user_id = match reaction.user_id {
        None => {
            error!(
                "Couldn't get user from reaction {} here {}",
                reaction.emoji,
                reaction
                    .message_id
                    .link(reaction.channel_id, reaction.guild_id)
            );
            return Err(anyhow!("Couldn't get user from reaction"));
        }
        Some(user_id) => user_id,
    };
    let member = GuildId::new(record.guild_id as u64)
        .member(ctx.http(), user_id)
        .await;
    if let Err(e) = member {
        error!(
            "Couldn't get member {} from reaction {} here {}: {}",
            user_id.mention(),
            reaction.emoji,
            reaction
                .message_id
                .link(reaction.channel_id, reaction.guild_id),
            e
        );
        return Err(anyhow!("Couldn't get user from reaction"));
    }

    let role_id = RoleId::new(record.role_id as u64);
    let member = member.unwrap();
    let change = if add {
        member.add_role(ctx.http(), role_id).await
    } else {
        member.remove_role(ctx.http(), role_id).await
    };
    if let Err(e) = change {
        let typ = if add { "add" } else { "remove" };
        error!("Couldn't {} role {}: {}", typ, record.role_id, e);
        Err(anyhow!("Couldn't {} role {} role", typ, role_id))
    } else {
        Ok(())
    }
}

#[allow(dead_code)]
struct ReactionRole {
    message_id: i64,
    channel_id: i64,
    guild_id: i64,
    role_id: i64,
    emoji_id: i64,
}
