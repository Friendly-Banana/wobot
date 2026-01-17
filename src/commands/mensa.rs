use anyhow::{Context as _, bail};
use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};
use chrono_tz::Tz;
use deunicode::deunicode;
use itertools::Itertools;
use percent_encoding::utf8_percent_encode;
use poise::CreateReply;
use poise::serenity_prelude::CreateEmbed;
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::ops::{Add, AddAssign};
use tokio::sync::OnceCell;
use tracing::{debug, info};

use crate::commands::utils::random_color;
use crate::constants::{HTTP_CLIENT, TIMEZONE};
use crate::{Context, UserError};

const EAT_API_URL: &str = "https://tum-dev.github.io/eat-api";
const GOOGLE_MAPS_SEARCH_URL: &str = "https://www.google.de/maps/place/";

#[allow(dead_code)]
#[derive(Deserialize, Clone)]
struct Location {
    address: String,
    latitude: f32,
    longitude: f32,
}

#[derive(Deserialize, Clone)]
struct Canteen {
    name: String,
    canteen_id: String,
    queue_status: Option<String>,
    location: Location,
}

#[derive(Deserialize)]
struct LabelCount {
    enum_name: String,
    abbreviation: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Dish {
    name: String,
    dish_type: String,
    labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DayMenu {
    date: String,
    dishes: Vec<Dish>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WeekMenu {
    year: u32,
    days: Vec<DayMenu>,
}

#[poise::command(slash_command, prefix_command, subcommands("next", "week", "list"))]
pub(crate) async fn mensa(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

const DISCORD_FIELDS_ON_AN_EMBED_LIMIT: usize = 25;

/// list all canteens
#[poise::command(slash_command, prefix_command)]
async fn list(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    let canteens = get_canteens().await?;

    let mut reply = CreateReply::default();
    let colour = random_color();
    for c in canteens
        .into_iter()
        .chunks(DISCORD_FIELDS_ON_AN_EMBED_LIMIT)
        .into_iter()
    {
        let mut embed = CreateEmbed::default();
        for canteen in c {
            let description = format!(
                "[{}]({})\n{}",
                canteen.location.address,
                link_location(&canteen),
                canteen.queue_status.unwrap_or("".to_string())
            );
            embed = embed.field(canteen.name, description, true);
        }
        reply = reply.embed(embed.title("List of all canteens").color(colour));
    }
    ctx.send(reply).await?;
    Ok(())
}

/// show the next weekday's menu (might be today), switches to next day after 20:00
#[poise::command(slash_command, prefix_command)]
async fn next(
    ctx: Context<'_>,
    #[description = "default Mensa Garching"]
    #[autocomplete = "autocomplete_canteen"]
    canteen_name: Option<String>,
) -> anyhow::Result<()> {
    ctx.defer().await?;
    let (canteen, mut menu, now) = get_menu(canteen_name).await?;

    let index = menu
        .days
        .iter()
        .position(|day| day.date == now.format("%Y-%m-%d").to_string());
    if index.is_none() {
        ctx.say("No menu available for today.").await?;
        return Ok(());
    }
    let day = menu.days.remove(index.unwrap());

    let labels = get_emojis_for_labels().await?;
    let reply = create_menu_embed(CreateReply::default(), day, &labels);
    ctx.send(reply.content(format!(
        "The menu in [{}]({})",
        canteen.name,
        link_location(&canteen)
    )))
    .await?;
    Ok(())
}

/// show this (or next) week's menu
#[poise::command(slash_command, prefix_command)]
async fn week(
    ctx: Context<'_>,
    #[description = "default Mensa Garching"]
    #[autocomplete = "autocomplete_canteen"]
    canteen_name: Option<String>,
) -> anyhow::Result<()> {
    ctx.defer().await?;
    let (canteen, menu, _) = get_menu(canteen_name).await?;

    let labels = get_emojis_for_labels().await?;
    let mut reply = CreateReply::default();
    for day in menu.days {
        reply = create_menu_embed(reply, day, &labels);
    }
    ctx.send(reply.content(format!(
        "This week's menu in [{}]({})",
        canteen.name,
        link_location(&canteen)
    )))
    .await?;
    Ok(())
}

fn create_menu_embed(
    msg: CreateReply,
    day: DayMenu,
    emojis_for_labels: &HashMap<String, String>,
) -> CreateReply {
    let mut embed = CreateEmbed::default();
    for dish in day.dishes {
        let emojis = dish
            .labels
            .iter()
            .filter_map(|l| emojis_for_labels.get(l))
            .join(" ");
        embed = embed.field(dish.dish_type, format!("{}\n{}", dish.name, emojis), true);
    }
    msg.embed(embed.title(day.date).color(random_color()))
}

fn link_location(canteen: &Canteen) -> String {
    format!(
        "{}{}",
        GOOGLE_MAPS_SEARCH_URL,
        utf8_percent_encode(
            &canteen.location.address,
            percent_encoding::NON_ALPHANUMERIC,
        )
    )
}

static LABELS: OnceCell<HashMap<String, String>> = OnceCell::const_new();
async fn get_emojis_for_labels() -> anyhow::Result<HashMap<String, String>> {
    LABELS
        .get_or_try_init(async || {
            Ok(HTTP_CLIENT
                .get(format!("{}/enums/labels.json", EAT_API_URL))
                .send()
                .await?
                .json::<Vec<LabelCount>>()
                .await?
                .into_iter()
                .map(|l| (l.enum_name, l.abbreviation))
                .collect())
        })
        .await
        .cloned()
}

static CANTEENS: OnceCell<Vec<Canteen>> = OnceCell::const_new();
async fn get_canteens() -> reqwest::Result<Vec<Canteen>> {
    CANTEENS
        .get_or_try_init(async || {
            HTTP_CLIENT
                .get(format!("{}/enums/canteens.json", EAT_API_URL))
                .send()
                .await?
                .json::<Vec<Canteen>>()
                .await
        })
        .await
        .cloned()
}

fn normalize(s: &str) -> String {
    deunicode(s).to_lowercase().replace([' ', '-'], "")
}

async fn autocomplete_canteen(_ctx: Context<'_>, partial: &str) -> Vec<String> {
    let partial = normalize(partial);
    if let Ok(canteens) = get_canteens().await {
        canteens
            .into_iter()
            .map(|c| c.name)
            .filter(|name| normalize(name).contains(&partial))
            .collect_vec()
    } else {
        vec![]
    }
}

async fn get_menu(
    canteen_name: Option<String>,
) -> anyhow::Result<(Canteen, WeekMenu, DateTime<Tz>)> {
    let canteen_id = canteen_name
        .as_deref()
        .map_or("Mensa Garching".to_string(), normalize);
    let canteen = get_canteens()
        .await?
        .into_iter()
        .find(|c| normalize(&c.name).contains(&canteen_id))
        .ok_or_else(|| UserError::err("Canteen not found"))?;

    let day = next_week_day();
    let week = day.iso_week().week();

    let menu_url = format!(
        "{}/{}/{}/{:02}.json",
        EAT_API_URL,
        canteen.canteen_id,
        day.year(),
        week
    );
    info!("Fetching menu from {}", menu_url);

    let response = HTTP_CLIENT.get(menu_url).send().await?;
    match response.error_for_status() {
        Ok(response) => {
            let menu = response.json::<WeekMenu>().await?;
            debug!("Fetched menu {:?}", menu);
            Ok((canteen, menu, day))
        }
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) {
                bail!("No menu found, maybe the mensa is closed?");
            }
            Err(e).context("Menu fetching failed")
        }
    }
}

fn next_week_day() -> DateTime<Tz> {
    let mut now = Local::now().with_timezone(&TIMEZONE);
    if now.hour() >= 20 {
        now = now.add(Duration::days(1));
    }
    while now.weekday() == Weekday::Sat || now.weekday() == Weekday::Sun {
        now.add_assign(Duration::days(1));
    }
    now
}
