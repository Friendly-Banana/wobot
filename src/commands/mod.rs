use image::codecs::png::PngEncoder;
use image::DynamicImage;
use poise::serenity_prelude::{CreateAttachment, CreateEmbed};
use poise::{CreateReply, ReplyHandle};

use crate::{Context, Error};

pub(crate) use self::{
    bot::*, cruisine::*, cutie_pie::*, emoji::*, events::*, features::*, fun::*, meme::*, mensa::*,
    moderation::*, obama::*, owner::*, reaction_role::*, reminder::*,
};

mod bot;
mod cruisine;
mod cutie_pie;
mod emoji;
mod events;
mod feature_state;
mod features;
mod fun;
mod meme;
mod mensa;
mod moderation;
mod obama;
mod owner;
mod reaction_role;
mod reminder;
mod utils;

pub(crate) fn link_message(guild_id: u64, channel_id: u64, msg_id: u64) -> String {
    format!(
        "https://discord.com/channels/{}/{}/{}",
        guild_id, channel_id, msg_id
    )
}

#[macro_export]
macro_rules! link_msg {
    ($guild_id:expr, $channel_id:expr, $msg_id:expr) => {{
        use crate::commands::link_message;
        link_message(
            $guild_id.context("guild_only")?.get(),
            $channel_id.get(),
            $msg_id.get(),
        )
    }};
}

#[macro_export]
macro_rules! done {
    ($ctx:expr) => {
        use poise::CreateReply;
        $ctx.send(
            CreateReply::default()
                .content("Done✅")
                .ephemeral(true)
                .reply(true),
        )
        .await?;
        return Ok(());
    };
}

pub(crate) async fn remove_components_but_keep_embeds(
    ctx: Context<'_>,
    reply: ReplyHandle<'_>,
) -> Result<(), Error> {
    let emoji_embeds = reply.message().await?;
    let mut m = CreateReply::default();
    m.embeds = emoji_embeds
        .embeds
        .to_owned()
        .into_iter()
        .map(|e| CreateEmbed::from(e))
        .collect();
    reply.edit(ctx, m).await?;
    Ok(())
}

async fn send_image(ctx: Context<'_>, img: DynamicImage, filename: String) -> Result<(), Error> {
    let mut output_bytes: Vec<u8> = Vec::new();
    img.write_with_encoder(PngEncoder::new(&mut output_bytes))?;

    ctx.send(CreateReply::default().attachment(CreateAttachment::bytes(output_bytes, filename)))
        .await?;
    Ok(())
}
