use std::collections::HashMap;
use std::ops::AddAssign;

use chrono::{Datelike, Duration, Local, Timelike, Utc, Weekday};
use itertools::Itertools;
use percent_encoding::utf8_percent_encode;
use poise::serenity_prelude::Colour;
use poise::CreateReply;
use serde::Deserialize;

use crate::constants::HTTP_CLIENT;
use crate::constants::TIMEZONE;
use crate::{Context, Error};

/// https://tum-dev.github.io/eat-api/docs/
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
#[derive(Deserialize)]
struct Dish {
    name: String,
    dish_type: String,
    labels: Vec<String>,
}

#[derive(Deserialize)]
struct DayMenu {
    date: String,
    dishes: Vec<Dish>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct WeekMenu {
    year: u32,
    days: Vec<DayMenu>,
}

#[poise::command(slash_command, prefix_command, subcommands("next", "week", "list"))]
pub(crate) async fn canteen(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, prefix_command, subcommands("next", "week", "list"))]
pub(crate) async fn mensa(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

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

    ctx.send(|m| {
        m.embed(|e| {
            for canteen in &canteens {
                let mut description = format!(
                    "[{}]({})",
                    canteen.location.address,
                    link_location(&canteen)
                );
                if queue_statuses.contains_key(&canteen.canteen_id) {
                    let queue_status = queue_statuses.get(&canteen.canteen_id).unwrap();
                    description.push_str(&format!(
                        "\nQueue: {} ({:.0}%)",
                        queue_status.current, queue_status.percent
                    ));
                }
                e.field(&canteen.name, description, true);
            }
            e.title("List of all canteens")
        })
    })
    .await?;
    Ok(())
}

/// show the next weekday's menu (might be today)
#[poise::command(slash_command, prefix_command)]
async fn next(
    ctx: Context<'_>,
    #[description = "default Mensa Garching"] canteen_name: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let (canteen, mut menu) = get_menu(canteen_name).await?;
    let mut now = Local::now().with_timezone(&TIMEZONE);
    while now.weekday() == Weekday::Sat || now.weekday() == Weekday::Sun {
        now.add_assign(Duration::days(1));
    }
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
    ctx.send(|m| {
        create_menu_embed(m, day, &labels);
        m.content(format!(
            "Today's Menu in [{}]({})",
            canteen.name,
            link_location(&canteen)
        ))
    })
    .await?;
    Ok(())
}

/// show this week's menu
#[poise::command(slash_command, prefix_command)]
async fn week(
    ctx: Context<'_>,
    #[description = "default Mensa Garching"] canteen_name: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let (canteen, menu) = get_menu(canteen_name).await?;

    let labels = get_emojis_for_labels().await?;
    ctx.send(|m| {
        for day in menu.days {
            create_menu_embed(m, day, &labels);
        }
        m.content(format!(
            "This week's Menu in [{}]({})",
            canteen.name,
            link_location(&canteen)
        ))
    })
    .await?;
    Ok(())
}

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

fn create_menu_embed(
    msg: &mut CreateReply,
    day: DayMenu,
    emojis_for_labels: &HashMap<String, String>,
) {
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

    msg.embed(|embed| {
        for dish in dishes {
            let emojis = dish
                .labels
                .iter()
                .filter_map(|l| emojis_for_labels.get(l))
                .join(" ");
            embed.field(dish.dish_type, format!("{}\n{}", dish.name, emojis), true);
        }

        embed.title(day.date);
        embed.color(COLORS[Utc::now().second() as usize % COLORS.len()])
    });
}

fn link_location(canteen: &Canteen) -> String {
    format!(
        "{}{}",
        GOOGLE_MAPS_SEARCH_URL,
        utf8_percent_encode(
            &canteen.location.address,
            percent_encoding::NON_ALPHANUMERIC
        )
    )
}

async fn get_emojis_for_labels() -> Result<HashMap<String, String>, Error> {
    return Ok(HTTP_CLIENT
        .get(format!("{}/enums/labels.json", EAT_API_URL))
        .send()
        .await?
        .json::<Vec<LabelCount>>()
        .await?
        .into_iter()
        .map(|l| (l.enum_name, l.abbreviation))
        .collect());
}

async fn get_canteens() -> reqwest::Result<Vec<Canteen>> {
    return HTTP_CLIENT
        .get(format!("{}/enums/canteens.json", EAT_API_URL))
        .send()
        .await?
        .json::<Vec<Canteen>>()
        .await;
}

async fn get_menu(canteen_name: Option<String>) -> Result<(Canteen, WeekMenu), Error> {
    let canteen_id = canteen_name
        .unwrap_or("Mensa Garching".to_string())
        .to_lowercase();
    let mut canteens = get_canteens().await?;
    let canteen = match canteens
        .iter()
        .position(|m| m.name.to_lowercase().contains(&canteen_id))
    {
        None => {
            return Err(Error::from("Canteen not found"));
        }
        Some(c) => canteens.remove(c),
    };
    let now = Local::now().with_timezone(&TIMEZONE);
    let week = now.iso_week().week();

    let menu = HTTP_CLIENT
        .get(format!(
            "{}/{}/{}/{:02}.json",
            EAT_API_URL,
            canteen.canteen_id,
            now.year(),
            week
        ))
        .send()
        .await?
        .json::<WeekMenu>()
        .await?;

    Ok((canteen, menu))
}
