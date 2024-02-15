use anyhow::format_err;
use image::Rgba;
use imageproc::drawing::draw_text_mut;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;
use rusttype::Scale;
use tracing::debug;

use crate::commands::utils::send_image;
use crate::constants::HTTP_CLIENT;
use crate::constants::{FONT, WHITE};
use crate::{Context, Error};

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn meme(
    ctx: Context<'_>,
    image_url: Url,
    text: String,
    x: i32,
    y: i32,
    text2: Option<String>,
    x2: Option<i32>,
    y2: Option<i32>,
    #[description = "text color as R G B, default white"] color: Option<String>,
    #[description = "Font height in pixels, default 50"] font_height: Option<f32>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let bytes = HTTP_CLIENT.get(image_url).send().await?.bytes().await?;
    let mut image =
        image::load_from_memory(&bytes).map_err(|e| format_err!("Loading image failed: {}", e))?;
    let scale = Scale::uniform(font_height.unwrap_or(50f32));
    let color = color.map_or(Ok(WHITE), parse_color)?;
    draw_text_mut(&mut image, color, x, y, scale, &FONT, &text);
    if text2.is_some() && x2.is_some() && y2.is_some() {
        draw_text_mut(
            &mut image,
            color,
            x2.unwrap(),
            y2.unwrap(),
            scale,
            &FONT,
            text2.as_ref().unwrap(),
        );
    }

    debug!("Sending meme {} for {}", text, ctx.author().name);
    send_image(ctx, image, format!("{}.png", text)).await
}

static COLOR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+ \d+ \d+").expect("COLOR_REGEX"));

fn parse_color(color: String) -> Result<Rgba<u8>, Error> {
    if !COLOR_REGEX.is_match(&color) {
        return Err(Error::from("Invalid color format: need 3 colors"));
    }
    let colors = color.splitn(3, " ").map(|d| d.parse::<u8>());
    let mut vec = Vec::with_capacity(4);
    for x in colors {
        vec.push(x.map_err(|e| format_err!("Invalid color [0-255]: {}", e))?);
    }
    // no transparency
    vec.push(255);
    return Ok(Rgba(<[u8; 4]>::try_from(vec).unwrap()));
}
