use anyhow::Context as _;
use image::codecs::png::PngEncoder;
use image::DynamicImage;
use mini_moka::sync::Cache;
use poise::serenity_prelude::{Colour, CreateAttachment, CreateEmbed, User};
use poise::{CreateReply, ReplyHandle};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use std::sync::LazyLock;

use crate::constants::{HTTP_CLIENT, ONE_DAY};
use crate::{Context, Error};

const COLORS: [Colour; 19] = [
    Colour::BLURPLE,
    Colour::DARK_GOLD,
    Colour::DARK_GREEN,
    Colour::BLITZ_BLUE,
    Colour::DARK_PURPLE,
    Colour::DARK_RED,
    Colour::DARK_TEAL,
    Colour::GOLD,
    Colour::MAGENTA,
    Colour::BLUE,
    Colour::ORANGE,
    Colour::PURPLE,
    Colour::RED,
    Colour::ROSEWATER,
    Colour::TEAL,
    Colour::BLITZ_BLUE,
    Colour::MEIBE_PINK,
    Colour::MAGENTA,
    Colour::FOOYOO,
];

pub(crate) fn random_color() -> Colour {
    *COLORS.choose(&mut thread_rng()).unwrap()
}

static AVATAR_CACHE: LazyLock<Cache<String, DynamicImage>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(50 * 1024 * 1024) // 50 MB
        .time_to_idle(10 * ONE_DAY)
        .weigher(|_, v: &DynamicImage| v.as_bytes().len() as u32)
        .build()
});

pub(crate) async fn load_avatar(avatar_url: String) -> Result<DynamicImage, Error> {
    if let Some(avatar) = AVATAR_CACHE.get(&avatar_url) {
        return Ok(avatar);
    }

    let result = HTTP_CLIENT.get(&avatar_url).send().await;
    let bytes = result.context("Downloading avatar failed")?.bytes().await?;
    let avatar = image::load_from_memory(&bytes)?;
    AVATAR_CACHE.insert(avatar_url, avatar.clone());

    Ok(avatar)
}

pub(crate) async fn get_avatar_url(ctx: &Context<'_>, user: &User) -> Result<String, Error> {
    let partial_guild = ctx.partial_guild().await;
    if let Some(guild) = partial_guild {
        let member = guild.member(&ctx, user.id).await?;
        if let Some(hash) = member.avatar {
            return Ok(format!(
                "https://cdn.discordapp.com/guilds/{}/users/{}/avatars/{}.png?size=256",
                guild.id, user.id, hash
            ));
        }
    }

    if let Some(hash) = &user.avatar {
        return Ok(format!(
            "https://cdn.discordapp.com/avatars/{}/{}.png?size=256",
            user.id, hash
        ));
    }

    let mut url = user.default_avatar_url();
    url.push_str("?size=256");
    Ok(url)
}

pub(crate) async fn remove_components_but_keep_embeds(
    ctx: Context<'_>,
    mut m: CreateReply,
    reply: ReplyHandle<'_>,
) -> Result<(), Error> {
    let original = reply.message().await?;
    m = m.components(Vec::new());
    m.embeds = original
        .embeds
        .iter()
        .cloned()
        .map(CreateEmbed::from)
        .collect();
    reply.edit(ctx, m).await?;
    Ok(())
}

pub async fn send_image(
    ctx: Context<'_>,
    img: DynamicImage,
    filename: String,
) -> Result<(), Error> {
    let mut output_bytes: Vec<u8> = Vec::new();
    img.write_with_encoder(PngEncoder::new(&mut output_bytes))?;

    ctx.send(CreateReply::default().attachment(CreateAttachment::bytes(output_bytes, filename)))
        .await?;
    Ok(())
}
