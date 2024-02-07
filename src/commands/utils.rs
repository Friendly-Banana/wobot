use anyhow::Context as _;
use image::DynamicImage;
use poise::serenity_prelude::{Colour, User};
use rand::prelude::SliceRandom;
use rand::thread_rng;

use crate::constants::HTTP_CLIENT;
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

pub(crate) async fn load_avatar(ctx: &Context<'_>, user: &User) -> Result<DynamicImage, Error> {
    let cached = {
        let cache = ctx.data().avatar_cache.read().unwrap();
        cache.get(&user.id).cloned()
    };
    if let Some(img) = cached {
        return Ok(img);
    }

    let avatar_url = get_avatar_url(&ctx, &user).await?;
    let result = HTTP_CLIENT.get(avatar_url).send().await;
    let bytes = result.context("Downloading avatar failed")?.bytes().await?;
    let avatar = image::load_from_memory(&bytes)?;
    {
        let mut cache = ctx.data().avatar_cache.write().unwrap();
        cache.insert(user.id, avatar.clone());
    }
    Ok(avatar)
}

async fn get_avatar_url(ctx: &Context<'_>, user: &User) -> Result<String, Error> {
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
