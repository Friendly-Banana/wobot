use ab_glyph::Font;
use image::DynamicImage;
use image::codecs::png::PngEncoder;
use imageproc::drawing::draw_text_mut;
use poise::serenity_prelude::{CreateAttachment, CreateMessage, GetMessages, Message};
use std::sync::OnceLock;
use tracing::{debug, info};

use crate::constants::{FONT, WHITE};
use crate::{Context, done};

const FONT_SIZE: f32 = 50.0;
const OBAMA_PATH: &str = "assets/obama_medal.jpg";
static OBAMA_IMAGE: OnceLock<DynamicImage> = OnceLock::new();

/// Creates an obama medal meme
/// the last 30 messages are checked for self-reactions and self replies
#[poise::command(slash_command, prefix_command)]
pub(crate) async fn obama(ctx: Context<'_>) -> anyhow::Result<()> {
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

            let mut img = OBAMA_IMAGE.get().unwrap().clone();
            let scale = FONT.pt_to_px_scale(FONT_SIZE).unwrap();
            draw_text_mut(&mut img, WHITE, 150, 160, scale, &*FONT, text); // gets medal
            draw_text_mut(&mut img, WHITE, 540, 20, scale, &*FONT, text); // puts medal

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

async fn has_self_reacted(ctx: Context<'_>, msg: &Message) -> anyhow::Result<bool> {
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
