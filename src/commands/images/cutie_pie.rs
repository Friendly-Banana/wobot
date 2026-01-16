use std::sync::OnceLock;

use anyhow::Context as _;
use image::{DynamicImage, GenericImage};
use poise::serenity_prelude::User;
use tracing::{debug, info};

use crate::Context;
use crate::commands::utils::{get_avatar_url, load_avatar, send_image};

const CUTIE_PIE_PATH: &str = "assets/cutie_pie.png";
static CUTIE_PIE_IMAGE: OnceLock<DynamicImage> = OnceLock::new();

/// Create a cutie pie meme with someone's avatar
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn cutie_pie(ctx: Context<'_>, user: User) -> anyhow::Result<()> {
    ctx.defer().await?;
    let avatar = load_avatar(get_avatar_url(&ctx, &user).await?).await?;

    CUTIE_PIE_IMAGE.get_or_init(|| {
        info!("Loading cutie pie image");
        image::open(CUTIE_PIE_PATH).expect("Failed to load cutie pie image")
    });

    let mut img = CUTIE_PIE_IMAGE.get().unwrap().clone();
    img.copy_from(&avatar, 354, 336)
        .context("Failed to copy avatar")?;

    debug!("Sending cutie pie");
    send_image(ctx, img, format!("cutie_pie_{}.png", user.name)).await
}
