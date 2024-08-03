use std::collections::HashMap;
use std::ops::{Add, AddAssign};

use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};
use chrono_tz::Tz;
use itertools::Itertools;
use percent_encoding::utf8_percent_encode;
use poise::serenity_prelude::CreateEmbed;
use poise::CreateReply;
use serde::Deserialize;
use tracing::info;

use crate::commands::utils::random_color;
use crate::constants::{HTTP_CLIENT, TIMEZONE};
use crate::{Context, Error};

const EAT_API_URL: &str = "https://tum-dev.github.io/eat-api";
const GOOGLE_MAPS_SEARCH_URL: &str = "https://www.google.de/maps/place/";

#[derive(Deserialize)]
struct QueueStatus {
    current: u32,
    percent: f32,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Location {
    address: String,
    latitude: f32,
    longitude: f32,
}

#[derive(Deserialize)]
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
pub(crate) async fn mensa(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

const DISCORD_FIELDS_ON_AN_EMBED_LIMIT: usize = 25;

/// list all canteens
#[poise::command(slash_command, prefix_command)]
async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let canteens = get_canteens().await?;
    let mut queue_statuses = HashMap::new();
    for canteen in &canteens {
        if let Some(url) = &canteen.queue_status {
            let queue_status = HTTP_CLIENT
                .get(url)
                .send()
                .await?
                .json::<QueueStatus>()
                .await?;
            queue_statuses.insert(&canteen.canteen_id, queue_status);
        };
    }

    let mut reply = CreateReply::default();
    let colour = random_color();
    for c in canteens.chunks(DISCORD_FIELDS_ON_AN_EMBED_LIMIT) {
        let mut embed = CreateEmbed::default();
        for canteen in c {
            let mut description =
                format!("[{}]({})", canteen.location.address, link_location(canteen));
            if queue_statuses.contains_key(&canteen.canteen_id) {
                let queue_status = queue_statuses.get(&canteen.canteen_id).unwrap();
                description.push_str(&format!(
                    "\nQueue: {} ({:.0}%)",
                    queue_status.current, queue_status.percent
                ));
            }
            embed = embed.field(&canteen.name, description, true);
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
    #[description = "default Mensa Garching"] canteen_name: Option<String>,
) -> Result<(), Error> {
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
        "Today's Menu in [{}]({})",
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
    #[description = "default Mensa Garching"] canteen_name: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let (canteen, menu, _) = get_menu(canteen_name).await?;

    let labels = get_emojis_for_labels().await?;
    let mut reply = CreateReply::default();
    for day in menu.days {
        reply = create_menu_embed(reply, day, &labels);
    }
    ctx.send(reply.content(format!(
        "This week's Menu in [{}]({})",
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
    let (mut side_dishes, mut dishes): (Vec<Dish>, Vec<Dish>) = day
        .dishes
        .into_iter()
        .partition(|d| d.dish_type.eq_ignore_ascii_case("Beilagen"));

    side_dishes.retain(|d| !d.name.contains("Täglich frisch"));

    dishes.push(Dish {
        name: side_dishes.iter().map(|d| &d.name).join(", "),
        dish_type: "Beilagen".to_string(),
        labels: side_dishes
            .into_iter()
            .flat_map(|d| d.labels)
            .unique()
            .collect(),
    });

    let mut embed = CreateEmbed::default();
    for dish in dishes {
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

async fn get_emojis_for_labels() -> Result<HashMap<String, String>, Error> {
    Ok(HTTP_CLIENT
        .get(format!("{}/enums/labels.json", EAT_API_URL))
        .send()
        .await?
        .json::<Vec<LabelCount>>()
        .await?
        .into_iter()
        .map(|l| (l.enum_name, l.abbreviation))
        .collect())
}

async fn get_canteens() -> reqwest::Result<Vec<Canteen>> {
    HTTP_CLIENT
        .get(format!("{}/enums/canteens.json", EAT_API_URL))
        .send()
        .await?
        .json::<Vec<Canteen>>()
        .await
}

async fn get_menu(
    canteen_name: Option<String>,
) -> Result<(Canteen, WeekMenu, DateTime<Tz>), Error> {
    let canteen_id = canteen_name
        .unwrap_or("Mensa Garching".to_string())
        .to_lowercase();
    let canteen = get_canteens()
        .await?
        .into_iter()
        .find(|m| m.name.to_lowercase().contains(&canteen_id))
        .ok_or("Canteen not found")?;

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
            info!("Fetched menu {:?}", menu);
            Ok((canteen, menu, day))
        }
        Err(e) => Err(Error::from(format!("Menu fetching failed: {}", e))),
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
    info!("Looking for menu for {}", now.format("%Y-%m-%d"));
    now
}
