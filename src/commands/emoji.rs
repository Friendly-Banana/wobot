use std::time::Duration;

use anyhow::Context as _;
use base64::Engine;
use once_cell::sync::Lazy;
use poise::serenity_prelude::json::json;
use poise::serenity_prelude::{
    Attachment, ButtonStyle, CollectComponentInteraction, EmojiIdentifier, Message, ReactionType,
    SerenityError,
};
use regex::Regex;
use tracing::error;

use crate::commands::remove_components_but_keep_embeds;
use crate::constants::HTTP_CLIENT;
use crate::{done, Context, Error};

const ADD_EMOJIS_TIMEOUT: Duration = Duration::from_secs(30);
const EMOJI_URL: &str = "https://cdn.discordapp.com/emojis/";
const EMOJI_FORMAT: &str = "png";
static EMOJI_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new("<:([a-zA-Z0-9_]+):([0-9]+)>").expect("EMOJI_REGEX"));
const ANIMATED_EMOJI_FORMAT: &str = "gif";
static ANIMATED_EMOJI_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new("<a:([a-zA-Z0-9_]+):([0-9]+)>").expect("ANIMATED_EMOJI_REGEX"));

struct NewEmoji {
    name: String,
    url: String,
    content_type: String,
}

impl NewEmoji {
    fn new(name: &str, id: &str, animated: bool) -> NewEmoji {
        let format = if animated {
            ANIMATED_EMOJI_FORMAT
        } else {
            EMOJI_FORMAT
        };
        NewEmoji {
            name: name.to_string(),
            url: format!("{}{}.{}", EMOJI_URL, id.to_string(), format),
            content_type: "image/".to_string() + format,
        }
    }
}

async fn add_emoji(
    ctx: Context<'_>,
    name: String,
    data: poise::serenity_prelude::Result<Vec<u8>>,
    content_type: &String,
) -> Result<(), Error> {
    let partial_guild = match ctx.partial_guild().await {
        None => {
            error!("Can't fetch guild {}", ctx.author());
            ctx.send(|m| m.content("Can't fetch your guild, try again later."))
                .await?;
            return Ok(());
        }
        Some(guild) => guild,
    };
    if partial_guild.emojis.iter().any(|(_, e)| e.name == name) {
        ctx.send(|m| m.content("There already is an emoji with the same name"))
            .await?;
        return Ok(());
    }
    let content = match data {
        Ok(content) => content,
        Err(why) => {
            error!("Error downloading image: {:?}", why);
            ctx.send(|m| m.content("Error downloading image")).await?;
            return Ok(());
        }
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&content);
    let data = format!("data:{};base64,{}", content_type, b64);
    let guild = ctx.guild_id().context("guild_only")?;
    // ugly, but has audit reason
    let map = json!({
        "name": &name,
        "image": &data,
    });
    ctx.http()
        .as_ref()
        .create_emoji(
            guild.0,
            &map,
            Some(&format!("{} used a command", &ctx.author().name)),
        )
        .await?;

    let emojis = guild.emojis(ctx.http()).await?;
    let emoji = emojis
        .iter()
        .find(|e| e.name == name)
        .expect("we just added it");
    ctx.send(|m| {
        let animated = if content_type.contains(ANIMATED_EMOJI_FORMAT) {
            "a"
        } else {
            ""
        };
        m.content(format!(
            "Added new emoji <{}:{}:{}>",
            animated, emoji.name, emoji.id
        ))
    })
    .await?;
    Ok(())
}

async fn extract_and_upload_emojis(ctx: Context<'_>, emojis: Vec<NewEmoji>) -> Result<(), Error> {
    ctx.defer().await?;

    let add_uuid = format!("{}add", ctx.id());
    let cancel_uuid = format!("{}cancel", ctx.id());
    let amount_emojis = emojis.len();

    let reply = ctx
        .send(|m| {
            m.content(format!("Create {} emojis?", amount_emojis));
            for emoji in &emojis {
                m.embed(|e| e.title(&emoji.name).thumbnail(&emoji.url));
            }
            m.components(|c| {
                c.create_action_row(|a| {
                    a.create_button(|confirm| {
                        confirm
                            .style(ButtonStyle::Success)
                            .label("Add")
                            .custom_id(&add_uuid)
                    });
                    a.create_button(|cancel| {
                        cancel
                            .style(ButtonStyle::Danger)
                            .label("Cancel")
                            .custom_id(&cancel_uuid)
                    })
                })
            })
        })
        .await?;

    let string = ctx.id().to_string();
    let answer = match CollectComponentInteraction::new(ctx)
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .timeout(ADD_EMOJIS_TIMEOUT)
        .filter(move |mci| mci.data.custom_id.starts_with(&string))
        .await
    {
        None => "No reaction, timeout :(".to_string(),
        Some(mci) => {
            mci.defer(ctx.http()).await?;
            if mci.data.custom_id == add_uuid {
                for emoji in emojis {
                    let bytes = match HTTP_CLIENT.get(emoji.url).send().await {
                        Ok(content) => content.bytes().await,
                        Err(why) => {
                            error!("Error downloading emoji: {:?}", why);
                            ctx.send(|m| m.content("Error downloading emoji")).await?;
                            return Ok(());
                        }
                    };
                    add_emoji(
                        ctx,
                        emoji.name,
                        bytes
                            .map(|b| b.to_vec())
                            .map_err(|e| SerenityError::from(e)),
                        &emoji.content_type,
                    )
                    .await?;
                }
                format!("Added {} emojis.", amount_emojis)
            } else {
                "Cancelled".to_string()
            }
        }
    };
    reply.edit(ctx, |b| b.content(answer)).await?;
    remove_components_but_keep_embeds(ctx, reply).await
}

fn extract_emojis(content: String) -> Vec<NewEmoji> {
    let emojis: Vec<NewEmoji> = EMOJI_REGEX
        .captures_iter(&content)
        .map(|c| {
            let (_, [name, id]) = c.extract::<2>();
            NewEmoji::new(name, id, false)
        })
        .chain(ANIMATED_EMOJI_REGEX.captures_iter(&content).map(|c| {
            let (_, [name, id]) = c.extract::<2>();
            NewEmoji::new(name, id, true)
        }))
        .collect();
    emojis
}

/// Manage emojis
#[poise::command(
    slash_command,
    prefix_command,
    required_permissions = "MANAGE_EMOJIS_AND_STICKERS",
    required_bot_permissions = "MANAGE_EMOJIS_AND_STICKERS",
    guild_only,
    subcommands(
        "upload",
        "rename",
        "remove",
        "copy_msg",
        "copy_text",
        "copy_reactions"
    )
)]
pub(crate) async fn emoji(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn remove(ctx: Context<'_>, emoji: EmojiIdentifier) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().context("guild_only")?;
    guild.delete_emoji(ctx.http(), emoji.id).await?;
    done!(ctx);
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn rename(
    ctx: Context<'_>,
    emoji: EmojiIdentifier,
    new_name: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().context("guild_only")?;
    // ugly, but has audit reason
    let map = json!({
        "name": &new_name,
    });
    ctx.http()
        .as_ref()
        .edit_emoji(
            guild.0,
            emoji.id.0,
            &map,
            Some(&format!("{} used a command", &ctx.author().name)),
        )
        .await?;
    done!(ctx);
}

/// Add emojis from a message
#[poise::command(slash_command, prefix_command)]
#[inline]
pub(crate) async fn copy_msg(ctx: Context<'_>, message: Message) -> Result<(), Error> {
    let emojis = extract_emojis(message.content);
    extract_and_upload_emojis(ctx, emojis).await
}

/// Add emojis from reactions to a message
#[poise::command(slash_command, prefix_command)]
#[inline]
pub(crate) async fn copy_reactions(ctx: Context<'_>, message: Message) -> Result<(), Error> {
    let mut emojis = Vec::new();
    for r in message.reactions {
        match r.reaction_type {
            ReactionType::Custom { id, name, animated } => emojis.push(NewEmoji::new(
                &name.expect("Emoji has name"),
                &id.to_string(),
                animated,
            )),
            ReactionType::Unicode(e) => {
                ctx.say(e + " is a builtin emoji").await?;
            }
            _ => {}
        }
    }
    extract_and_upload_emojis(ctx, emojis).await
}

/// Add emojis from text
#[poise::command(slash_command, prefix_command)]
#[inline]
pub(crate) async fn copy_text(ctx: Context<'_>, text: String) -> Result<(), Error> {
    let emojis = extract_emojis(text);
    extract_and_upload_emojis(ctx, emojis).await
}

/// Uploads image as new emoji
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn upload(ctx: Context<'_>, name: String, image: Attachment) -> Result<(), Error> {
    match &image.content_type {
        None => {
            ctx.send(|m| m.content("Not an image")).await?;
            return Ok(());
        }
        Some(content_type) => {
            if !content_type.starts_with("image/") {
                ctx.send(|m| m.content("Not an image")).await?;
                return Ok(());
            }
            ctx.defer().await?;
            add_emoji(ctx, name, image.download().await, content_type).await
        }
    }
}
