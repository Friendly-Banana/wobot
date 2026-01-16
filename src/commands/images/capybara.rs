use poise::serenity_prelude::CreateEmbed;
use poise::{CreateReply, command};
use serde::Deserialize;

use crate::Context;
use crate::constants::HTTP_CLIENT;

const CAPI: &str = "https://api.capy.lol/v1";

#[derive(Deserialize)]
struct Fact {
    fact: String,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

/// random capybara together with a fact
#[command(slash_command, prefix_command)]
pub async fn capy(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;

    let response = HTTP_CLIENT.get(format!("{CAPI}/fact")).send().await?;
    let fact = response.json::<Data<Fact>>().await?.data.fact;

    let response = HTTP_CLIENT
        .get(format!("{CAPI}/capybara?json=true"))
        .send()
        .await?;
    let image = response.json::<Data<Image>>().await?.data.url;

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::default()
                .title("Capybara")
                .description(fact)
                .image(image),
        ),
    )
    .await?;
    Ok(())
}
