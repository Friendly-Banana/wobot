use crate::constants::HTTP_CLIENT;
use crate::{Context, Error};
use poise::async_trait;
use poise::serenity_prelude::GuildId;
use songbird::input::YoutubeDl;
use songbird::tracks::TrackHandle;
use songbird::{Event, EventContext, EventHandler as VoiceEventHandler, Songbird, TrackEvent};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tracing::error;

#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    subcommands("volume", "play", "stop", "skip")
)]
pub(crate) async fn music(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

static VOLUME: AtomicU8 = AtomicU8::new(50);

#[poise::command(slash_command, prefix_command)]
async fn volume(ctx: Context<'_>, #[description = "percent"] volume: u8) -> Result<(), Error> {
    if volume == 0 || volume > 200 {
        return Err("Volume must be between 1 and 200".into());
    }
    ctx.defer().await?;
    VOLUME.store(volume, Ordering::Relaxed);
    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird initialized")
        .clone();
    let handler_lock = match manager.get(ctx.guild_id().expect("guild_only")) {
        Some(handler) => handler,
        None => {
            return Err("Not in a voice channel".into());
        }
    };
    let handler = handler_lock.lock().await;
    if let Some(track) = handler.queue().current() {
        track.set_volume(volume as f32 / 100.0)?;
    }
    ctx.reply(format!("Volume set to {volume}%")).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn play(ctx: Context<'_>, url: String) -> Result<(), Error> {
    ctx.defer().await?;
    let channel = ctx
        .guild()
        .expect("guild in cache")
        .voice_states
        .get(&ctx.author().id)
        .and_then(|vs| vs.channel_id);
    if channel.is_none() {
        return Err("Please join a voice channel first".into());
    }

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird initialized")
        .clone();

    let handler_lock = match manager
        .join(ctx.guild_id().expect("guild_only"), channel.unwrap())
        .await
    {
        Ok(handler) => handler,
        Err(e) => {
            error!("Failed to join voice channel: {:?}", e);
            return Err("Failed to join voice channel".into());
        }
    };

    let mut handler = handler_lock.lock().await;
    let src = YoutubeDl::new(HTTP_CLIENT.clone(), url);
    let song = handler.enqueue_input(src.into()).await;
    track_song(manager, ctx.guild_id().unwrap(), song)?;

    ctx.reply("Added to the queue!").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird initialized")
        .clone();
    manager.remove(ctx.guild_id().expect("guild_only")).await?;
    ctx.reply("See you next time").await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird initialized")
        .clone();
    let handler_lock = match manager.get(ctx.guild_id().expect("guild_only")) {
        Some(handler) => handler,
        None => {
            return Err("Not in a voice channel".into());
        }
    };
    let handler = handler_lock.lock().await;
    handler.queue().skip()?;
    ctx.reply("Skipped!").await?;
    Ok(())
}

struct SongEndNotifier {
    guild: GuildId,
    manager: Arc<Songbird>,
}

#[async_trait]
impl VoiceEventHandler for SongEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            // this track is not yet removed
            if track_list.len() <= 1 {
                if let Err(e) = self.manager.remove(self.guild).await {
                    error!("Failed to leave channel: {:?}", e);
                }
            }
        }
        None
    }
}

pub(crate) fn track_song(
    manager: Arc<Songbird>,
    guild: GuildId,
    song: TrackHandle,
) -> Result<(), Error> {
    song.set_volume(VOLUME.load(Ordering::Relaxed) as f32 / 100.0)?;
    song.add_event(
        Event::Track(TrackEvent::End),
        SongEndNotifier { guild, manager },
    )?;
    Ok(())
}
