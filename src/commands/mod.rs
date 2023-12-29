use image::codecs::png::PngEncoder;
use image::DynamicImage;
use poise::serenity_prelude::{AttachmentType, CreateEmbed};
use poise::ReplyHandle;

use crate::{Context, Error};

pub(crate) use self::{
    bot::*, cruisine::*, cutie_pie::*, emoji::*, events::*, features::*, fun::*, meme::*, mensa::*,
    moderation::*, obama::*, owner::*, reaction_role::*,
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
        link_message($guild_id.context("guild_only")?.0, $channel_id.0, $msg_id.0)
    }};
}

#[macro_export]
macro_rules! done {
    ($ctx:expr) => {
        $ctx.send(|m| m.content("Done✅").ephemeral(true).reply(true))
            .await?;
        return Ok(());
    };
}

pub(crate) async fn remove_components_but_keep_embeds(
    ctx: Context<'_>,
    reply: ReplyHandle<'_>,
) -> Result<(), Error> {
    let emoji_embeds = reply.message().await?;
    reply
        .edit(ctx, |m| {
            m.embeds = emoji_embeds
                .embeds
                .iter()
                .map(|e| CreateEmbed::from(e.clone()))
                .collect();
            m.components(|c| c)
        })
        .await?;
    Ok(())
}

async fn send_image(ctx: Context<'_>, img: DynamicImage, filename: String) -> Result<(), Error> {
    let mut output_bytes: Vec<u8> = Vec::new();
    img.write_with_encoder(PngEncoder::new(&mut output_bytes))?;

    ctx.send(|m| {
        m.attachment(AttachmentType::Bytes {
            data: output_bytes.into(),
            filename,
        })
    })
    .await?;
    Ok(())
}
