use std::sync::OnceLock;

use anyhow::Context as _;
use image::{DynamicImage, GenericImage};
use poise::serenity_prelude::User;
use tracing::{debug, info};

use crate::commands::utils;
use crate::{Context, Error};

const CUTIE_PIE_PATH: &str = "assets/cutie_pie.png";
static CUTIE_PIE_IMAGE: OnceLock<DynamicImage> = OnceLock::new();

/// Create a cutie pie meme with someone's avatar
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn cutie_pie(ctx: Context<'_>, user: User) -> Result<(), Error> {
    ctx.defer().await?;
    let avatar = utils::load_avatar(&ctx, &user).await?;

    CUTIE_PIE_IMAGE.get_or_init(|| {
        info!("Loading cutie pie image");
        image::open(CUTIE_PIE_PATH).expect("Failed to load cutie pie image")
    });

    let mut img = CUTIE_PIE_IMAGE.get().context("OBAMA_IMAGE loaded")?.clone();
    img.copy_from(&avatar, 354, 336)
        .context("Failed to copy avatar")?;

    debug!("Sending cutie pie");
    utils::send_image(ctx, img, format!("cutie_pie_{}.png", user.name)).await
}
