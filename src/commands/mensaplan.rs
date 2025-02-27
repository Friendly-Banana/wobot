use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Context as _};
use image::{DynamicImage, GenericImage};
use itertools::Itertools;
use mini_moka::sync::Cache;
use poise::futures_util::future::try_join_all;
use poise::serenity_prelude::json::json;
use poise::serenity_prelude::{GuildId, UserId};
use reqwest::{RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use stitchy_core::Stitch;
use tracing::{debug, info};

use crate::commands::utils::send_image;
use crate::commands::utils::{get_avatar_url, load_avatar};
use crate::constants::{HTTP_CLIENT, ONE_DAY, ONE_HOUR};
use crate::{Context, Error};

const MENSA_PLAN_API: &str = "https://mensa.gabriels.cloud/api";

const MENSA_PLAN_PATH: &str = "assets/mensa_plan.png";
static MENSA_PLAN_IMAGE: OnceLock<DynamicImage> = OnceLock::new();

const DEFAULT_DISAPPEAR_TIME: Duration = ONE_HOUR;
const MAX_DISAPPEAR_TIME: Duration = ONE_DAY;

const MIN_X: char = 'A';
const MAX_X: char = 'J';
const MIN_Y: u8 = 1;
const MAX_Y: u8 = 10;
const X_SCALE: f32 = (MAX_X as u8 - MIN_X as u8) as f32;
const Y_SCALE: f32 = (MAX_Y - MIN_Y) as f32;
const X_OFFSET: u32 = 40;
const Y_OFFSET: u32 = 12;
const SCALING: u32 = 53;

#[derive(Serialize, Deserialize, Clone)]
struct MPUser {
    id: u32,
    auth_id: String,
    name: String,
    avatar: String,
    default_public: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct MPGroup {
    id: u32,
    name: String,
    avatar: String,
    server_id: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct MPPosition {
    id: u32,
    name: String,
    avatar: String,
    x: f32,
    y: f32,
}

static API_USER_CACHE: LazyLock<Cache<UserId, MPUser>> = LazyLock::new(|| Cache::new(100));
static API_GROUP_CACHE: LazyLock<Cache<GuildId, MPGroup>> = LazyLock::new(|| Cache::new(10));

async fn send_with_auth(ctx: Context<'_>, rb: RequestBuilder) -> Result<Response, reqwest::Error> {
    rb.bearer_auth(&ctx.data().mp_api_token).send().await
}

#[poise::command(slash_command, prefix_command, subcommands("add", "delete", "show"))]
pub(crate) async fn mp(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// mark your position in the mensa
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn add(
    ctx: Context<'_>,
    #[description = "a Letter and a Number"] position: String,
    #[description = "Time until your position disappears, default 1 hour"] expires: Option<String>,
    #[description = "Visible to all users, not only this server, default false. Always on in DMs!"]
    public: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let (letter, number) = parse_cruisine_letters(&position)?;
    let duration = match expires {
        Some(x) => parse_duration::parse(x.as_str())?.min(MAX_DISAPPEAR_TIME),
        None => DEFAULT_DISAPPEAR_TIME,
    };

    // get or create user
    let user = if let Some(user) = API_USER_CACHE.get(&ctx.author().id) {
        user
    } else {
        let new_user = MPUser {
            id: 0,
            auth_id: format!("oauth2|discord|{}", ctx.author().id),
            name: ctx.author().name.to_string(),
            avatar: get_avatar_url(&ctx, ctx.author()).await?,
            default_public: false,
        };
        let user = send_with_auth(
            ctx,
            HTTP_CLIENT
                .post(format!(
                    "{}/users/auth/{}",
                    MENSA_PLAN_API, new_user.auth_id
                ))
                .json(&json!({"user": new_user})),
        )
        .await?
        .json::<MPUser>()
        .await?;
        API_USER_CACHE.insert(ctx.author().id, user.clone());
        user
    };

    if let Some(guild_id) = ctx.guild_id() {
        // get or create the group for this server
        let group = if let Some(group) = API_GROUP_CACHE.get(&guild_id) {
            group
        } else {
            let new_group = MPGroup {
                id: 0,
                name: guild_id.name(ctx).unwrap_or(format!("Server {guild_id}")),
                avatar: guild_id.to_partial_guild(ctx).await?.icon_url().unwrap(),
                server_id: guild_id.get(),
            };
            let group = send_with_auth(
                ctx,
                HTTP_CLIENT
                    .post(format!(
                        "{}/groups/server/{}",
                        MENSA_PLAN_API, new_group.server_id
                    ))
                    .json(&json!({"group": new_group})),
            )
            .await?
            .json::<MPGroup>()
            .await?;
            API_GROUP_CACHE.insert(guild_id, group.clone());
            group
        };
        // add user to group
        send_with_auth(
            ctx,
            HTTP_CLIENT
                .post(format!("{}/groups/join", MENSA_PLAN_API))
                .json(&json!({
                    "group_id": group.id,
                    "user_id": user.id,
                })),
        )
        .await?;
    }

    // set user position
    send_with_auth(
        ctx,
        HTTP_CLIENT
            .post(format!("{}/positions", MENSA_PLAN_API))
            .json(&json!({"position": {
                "x": (letter as u8 - MIN_X as u8) as f32 * 100. / X_SCALE,
                "y": (number - MIN_Y) as f32 * 100. / Y_SCALE,
                "owner_id": user.id,
                "public": public.unwrap_or(false),
                "expires_in": duration.as_secs() / 60
            }})),
    )
    .await?;

    show_plan(ctx).await
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn delete(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    send_with_auth(
        ctx,
        HTTP_CLIENT.delete(format!(
            "{}/positions/user/oauth2|discord|{}",
            MENSA_PLAN_API,
            ctx.author().id
        )),
    )
    .await?;

    ctx.reply("Position deleted").await?;
    Ok(())
}

/// see the plan
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn show(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    show_plan(ctx).await
}

async fn show_plan(ctx: Context<'_>) -> Result<(), Error> {
    MENSA_PLAN_IMAGE.get_or_init(|| {
        info!("Loading mensa plan image");
        image::open(MENSA_PLAN_PATH).expect("Failed to load mensa plan image")
    });
    let mut image = MENSA_PLAN_IMAGE
        .get()
        .context("MENSA_PLAN_IMAGE loaded")?
        .clone();

    let positions = send_with_auth(
        ctx,
        HTTP_CLIENT.get(format!(
            "{}/positions/server/{}",
            MENSA_PLAN_API,
            ctx.guild_id().unwrap().get()
        )),
    )
    .await?
    .json::<Vec<MPPosition>>()
    .await?;

    let mut avatars = try_join_all(
        positions
            .iter()
            .map(|pos| load_avatar(pos.avatar.to_string())),
    )
    .await?
    .into_iter()
    .enumerate()
    .map(|(i, img)| (positions[i].id, img))
    .collect::<HashMap<u32, DynamicImage>>();
    // group the positions from 0..100 range into x * y tiles
    let grouped_avatars = positions
        .into_iter()
        .map(|pos| {
            let x = pos.x * X_SCALE / 100.0;
            let y = pos.y * Y_SCALE / 100.0;
            ((x as u8, y as u8), avatars.remove(&pos.id).unwrap())
        })
        .into_group_map();

    for (tile, imgs) in grouped_avatars {
        let stitch = Stitch::builder()
            .images(imgs)
            .height_limit(SCALING)
            .width_limit(SCALING)
            .stitch()?;
        let x = X_OFFSET + tile.0 as u32 * SCALING;
        let y = Y_OFFSET + tile.1 as u32 * SCALING;
        image.copy_from(&stitch, x, y)?;
    }

    debug!("Sending updated mensa plan");
    send_image(ctx, image, "mensa_plan.png".to_string()).await
}

fn parse_cruisine_letters(position: &str) -> Result<(char, u8), Error> {
    if position.len() < 2 || position.len() > 3 {
        return Err(anyhow!("Bad position format, 2-3 characters").into());
    }

    let position = position.to_ascii_uppercase();
    let mut chars: Vec<char> = position.chars().collect();

    let letter = if chars[0].is_ascii_alphabetic() {
        chars.remove(0)
    } else if chars.last().unwrap().is_ascii_alphabetic() {
        chars.pop().unwrap()
    } else {
        return Err(anyhow!("Bad position format, no letter").into());
    };

    let number = str::parse::<u8>(&chars.into_iter().collect::<String>())?;

    if (MIN_X..=MAX_X).contains(&letter) && (MIN_Y..=MAX_Y).contains(&number) {
        Ok((letter, number))
    } else {
        Err(anyhow!("Bad position format, out of bounds: {MIN_X}-{MAX_X}, {MIN_Y}-{MAX_Y}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cruisine_letters;

    #[test]
    fn test_parse_cruisine_letters() {
        for x in 'A'..='J' {
            for y in 1..=10 {
                let pos = format!("{}{}", x, y);
                assert_eq!(parse_cruisine_letters(&pos).unwrap(), (x, y));

                let pos_reverse = format!("{}{}", y, x);
                assert_eq!(parse_cruisine_letters(&pos_reverse).unwrap(), (x, y));
            }
        }
    }

    #[test]
    fn test_reject_invalid_cruisine_letters() {
        assert!(parse_cruisine_letters("A11").is_err());
        assert!(parse_cruisine_letters("K5").is_err());
        assert!(parse_cruisine_letters("A").is_err());
        assert!(parse_cruisine_letters("10").is_err());
    }
}
