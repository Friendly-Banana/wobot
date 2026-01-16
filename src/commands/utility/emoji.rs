use anyhow::Context as _;
use base64::Engine;
use poise::CreateReply;
use poise::serenity_prelude::{
    Attachment, ButtonStyle, ComponentInteractionCollector, CreateActionRow, CreateButton,
    CreateEmbed, EmojiIdentifier, Message, ReactionType,
};
use regex::Regex;
use std::sync::LazyLock;
use std::time::Duration;
use tracing::error;

use crate::commands::utils::remove_components_but_keep_embeds;
use crate::constants::HTTP_CLIENT;
use crate::{Context, UserError, done};

const ADD_EMOJIS_TIMEOUT: Duration = Duration::from_secs(30);
const EMOJI_URL: &str = "https://cdn.discordapp.com/emojis/";
const EMOJI_FORMAT: &str = "png";
static EMOJI_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("<:([a-zA-Z0-9_]+):([0-9]+)>").expect("EMOJI_REGEX"));
const ANIMATED_EMOJI_FORMAT: &str = "gif";
static ANIMATED_EMOJI_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("<a:([a-zA-Z0-9_]+):([0-9]+)>").expect("ANIMATED_EMOJI_REGEX"));

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
            url: format!("{}{}.{}", EMOJI_URL, id, format),
            content_type: "image/".to_string() + format,
        }
    }
}

async fn add_emoji(
    ctx: Context<'_>,
    name: String,
    data: Vec<u8>,
    content_type: &String,
) -> anyhow::Result<()> {
    let partial_guild = ctx
        .partial_guild()
        .await
        .context("Can't fetch your guild, try again later.")?;
    if partial_guild.emojis.iter().any(|(_, e)| e.name == name) {
        return Err(UserError::err(format!(
            "There already is an emoji with the same name {name}"
        )));
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    let data = format!("data:{};base64,{}", content_type, b64);
    let emoji = partial_guild.create_emoji(ctx.http(), &name, &data).await?;

    ctx.reply(format!("Added new emoji {emoji}")).await?;
    Ok(())
}

async fn extract_and_upload_emojis(ctx: Context<'_>, emojis: Vec<NewEmoji>) -> anyhow::Result<()> {
    ctx.defer().await?;

    let add_uuid = format!("{}add", ctx.id());
    let cancel_uuid = format!("{}cancel", ctx.id());
    let mut amount_emojis = emojis.len();

    let mut reply = {
        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&add_uuid)
                .style(ButtonStyle::Success)
                .label("Add"),
            CreateButton::new(&cancel_uuid)
                .style(ButtonStyle::Danger)
                .label("Cancel"),
        ])];

        CreateReply::default()
            .content(format!("Create {} emojis?", amount_emojis))
            .components(components)
    };
    for emoji in &emojis {
        reply = reply.embed(CreateEmbed::new().title(&emoji.name).thumbnail(&emoji.url));
    }

    let reply_handle = ctx.send(reply).await?;

    let string = ctx.id().to_string();
    let answer = match ComponentInteractionCollector::new(ctx)
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
                    // TODO refactor once async closures become available
                    let response = HTTP_CLIENT.get(emoji.url).send().await;
                    if let Ok(body) = response {
                        let result = body.bytes().await;
                        if let Ok(bytes) = result {
                            if add_emoji(ctx, emoji.name, bytes.to_vec(), &emoji.content_type)
                                .await
                                .is_err()
                            {
                                amount_emojis -= 1;
                            }
                        } else {
                            error!("Error decoding emoji: {:?}", result);
                            ctx.reply("Error decoding emoji").await?;
                            amount_emojis -= 1;
                        }
                    } else {
                        error!("Error downloading emoji: {:?}", response);
                        ctx.reply("Error downloading emoji").await?;
                        amount_emojis -= 1;
                    }
                }
                format!("Added {} emojis.", amount_emojis)
            } else {
                "Cancelled".to_string()
            }
        }
    };
    remove_components_but_keep_embeds(ctx, CreateReply::default().content(answer), reply_handle)
        .await
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
    required_permissions = "MANAGE_GUILD_EXPRESSIONS",
    required_bot_permissions = "MANAGE_GUILD_EXPRESSIONS",
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
pub(crate) async fn emoji(_ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn remove(ctx: Context<'_>, emoji: EmojiIdentifier) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().expect("guild_only");
    guild.delete_emoji(ctx.http(), emoji.id).await?;
    done!(ctx);
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn rename(
    ctx: Context<'_>,
    emoji: EmojiIdentifier,
    new_name: String,
) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().expect("guild_only");
    guild.edit_emoji(ctx.http(), emoji.id, &new_name).await?;
    done!(ctx);
}

/// Add emojis from a message
#[poise::command(slash_command, prefix_command)]
#[inline]
pub(crate) async fn copy_msg(ctx: Context<'_>, message: Message) -> anyhow::Result<()> {
    let emojis = extract_emojis(message.content);
    extract_and_upload_emojis(ctx, emojis).await
}

/// Add emojis from reactions to a message
#[poise::command(slash_command, prefix_command)]
#[inline]
pub(crate) async fn copy_reactions(ctx: Context<'_>, message: Message) -> anyhow::Result<()> {
    let mut emojis = Vec::new();
    for r in message.reactions {
        match r.reaction_type {
            ReactionType::Custom { id, name, animated } => emojis.push(NewEmoji::new(
                &name.expect("Emoji has a name"),
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
pub(crate) async fn copy_text(ctx: Context<'_>, text: String) -> anyhow::Result<()> {
    let emojis = extract_emojis(text);
    extract_and_upload_emojis(ctx, emojis).await
}

/// Uploads image as new emoji
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn upload(
    ctx: Context<'_>,
    name: String,
    image: Attachment,
) -> anyhow::Result<()> {
    match &image.content_type {
        None => {
            ctx.reply("Not an image").await?;
            Ok(())
        }
        Some(content_type) => {
            if !content_type.starts_with("image/") {
                ctx.reply("Not an image").await?;
                return Ok(());
            }
            ctx.defer().await?;
            add_emoji(ctx, name, image.download().await?, content_type).await
        }
    }
}
