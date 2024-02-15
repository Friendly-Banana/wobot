use std::sync::OnceLock;

use anyhow::Context as _;
use image::codecs::png::PngEncoder;
use image::DynamicImage;
use imageproc::drawing::draw_text_mut;
use poise::serenity_prelude::{CreateAttachment, CreateMessage, GetMessages, Message};
use rusttype::Scale;
use tracing::{debug, info};

use crate::constants::{FONT, WHITE};
use crate::{done, Context, Error};

const SCALE: Scale = Scale { x: 50f32, y: 50f32 };
const OBAMA_PATH: &str = "assets/obama_medal.jpg";
static OBAMA_IMAGE: OnceLock<DynamicImage> = OnceLock::new();

/// Creates an obama medal meme
/// the last 30 messages are checked for self-reactions and self replies
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn obama(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    debug!("Loading messages");
    let msgs = ctx
        .channel_id()
        .messages(ctx.http(), GetMessages::new().limit(30))
        .await?;
    for msg in msgs {
        if msg
            .referenced_message
            .clone()
            .is_some_and(|re| re.author.id == msg.author.id)
            || has_self_reacted(ctx, &msg).await?
        {
            debug!("Found message {}, creating meme", msg.link());
            let text = msg.author.name.as_str();

            OBAMA_IMAGE.get_or_init(|| {
                info!("Loading obama image");
                image::open(OBAMA_PATH).expect("Failed to load obama image")
            });

            let mut img = OBAMA_IMAGE.get().context("OBAMA_IMAGE loaded")?.clone();
            draw_text_mut(&mut img, WHITE, 150, 160, SCALE, &FONT, text); // gets medal
            draw_text_mut(&mut img, WHITE, 540, 20, SCALE, &FONT, text); // puts medal

            let mut output_bytes: Vec<u8> = Vec::new();
            img.write_with_encoder(PngEncoder::new(&mut output_bytes))?;

            debug!("Sending obama");
            ctx.channel_id()
                .send_message(
                    ctx.http(),
                    CreateMessage::new()
                        .reference_message(&msg)
                        .add_file(CreateAttachment::bytes(output_bytes, OBAMA_PATH)),
                )
                .await?;
            done!(ctx);
        }
    }
    debug!("Nothing found");
    ctx.say("Nothing found").await?;
    Ok(())
}

async fn has_self_reacted(ctx: Context<'_>, msg: &Message) -> Result<bool, Error> {
    for reaction in &msg.reactions {
        if msg
            .reaction_users(ctx.http(), reaction.reaction_type.clone(), None, None)
            .await?
            .iter()
            .any(|user| user.id == msg.author.id)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
