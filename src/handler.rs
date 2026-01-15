#[cfg(feature = "activity")]
use crate::CacheEntry;
use crate::commands::{change_reaction_role, track_song};
use crate::constants::HTTP_CLIENT;
use crate::{Data, Error};
use itertools::Itertools;
use poise::FrameworkContext;
use poise::serenity_prelude::json::json;
use poise::serenity_prelude::*;
use rand::random_bool;
use regex::Regex;
use songbird::input::File;
use sqlx::query;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::Ordering;
#[cfg(feature = "activity")]
use tracing::warn;

static WORD_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\w+\b").unwrap());

pub(crate) async fn event_handler(
    ctx: &Context,
    event: &FullEvent,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        FullEvent::VoiceStateUpdate { new, old } => {
            if let Some(guild) = new.guild_id {
                let switched_channel = old.as_ref().is_some_and(|old| old.channel_id.is_some());
                let user_has_entry_sound = data.entry_sounds.contains_key(&new.user_id);
                if new.channel_id.is_some() && !switched_channel && user_has_entry_sound {
                    let channel = new.channel_id.unwrap();
                    let sound = data.entry_sounds.get(&new.user_id).unwrap().clone();
                    let file = File::new(PathBuf::from(sound));
                    let manager = songbird::get(ctx).await.expect("Songbird initialized");
                    let handler_lock = manager.join(guild, channel).await?;
                    let mut handler = handler_lock.lock().await;
                    let song = handler.play_input(file.into());
                    track_song(manager, guild, song)?;
                }
                #[cfg(feature = "activity")]
                update_activity(data, guild, new.user_id).await;
            }
            Ok(())
        }
        FullEvent::ReactionAdd { add_reaction } => {
            #[cfg(feature = "activity")]
            if let Some(guild) = add_reaction.guild_id {
                if let Some(user) = add_reaction.user_id {
                    update_activity(data, guild, user).await;
                }
            }
            change_reaction_role(ctx, data, add_reaction, true).await
        }
        FullEvent::ReactionRemove { removed_reaction } => {
            change_reaction_role(ctx, data, removed_reaction, false).await
        }
        FullEvent::Message { new_message } => {
            if new_message.author.bot {
                return Ok(());
            }
            let content = new_message.content.to_lowercase();
            let result = tokio::join!(
                auto_react(ctx, data, new_message, &content),
                auto_reply(ctx, data, new_message, &content),
                celery_fact(ctx, data, new_message.channel_id),
                async {
                    #[cfg(feature = "activity")]
                    if let Some(guild) = new_message.guild_id {
                        update_activity(data, guild, new_message.author.id).await;
                    }
                },
                async {
                    #[cfg(feature = "activity")]
                    if let Some(guild) = new_message.guild_id {
                        update_message_count(data, guild, new_message.author.id).await;
                    }
                }
            );
            result.0.and(result.1).and(result.2)
        }
        _ => Ok(()),
    }
}

#[cfg(feature = "activity")]
async fn update_message_count(data: &Data, guild: GuildId, user: UserId) {
    if !data.activity_per_guild.contains_key(&guild) {
        return;
    }
    let result = query!("UPDATE activity SET message_count = message_count + 1 WHERE user_id = $1 AND guild_id = $2", user.get() as i64, guild.get() as i64)
        .execute(&data.database)
        .await;
    if let Err(e) = result {
        warn!("Failed to update message count for {}: {}", user.get(), e);
    }
}

#[cfg(feature = "activity")]
async fn update_activity(data: &Data, guild: GuildId, user: UserId) {
    if let Some(guild_activity) = data.activity_per_guild.get(&guild) {
        if guild_activity.get(&user).is_none() {
            let result = query!("INSERT INTO activity (user_id, guild_id) VALUES ($1, $2) ON CONFLICT (user_id, guild_id) DO UPDATE SET last_active = now()", user.get() as i64, guild.get() as i64)
                .execute(&data.database)
                .await;
            if let Err(e) = result {
                warn!("Failed to update activity for {}: {}", user.get(), e);
            } else {
                guild_activity.insert(user, CacheEntry {});
            }
        }
    }
}

async fn auto_reply(
    ctx: &Context,
    data: &Data,
    new_message: &Message,
    content: &str,
) -> Result<(), Error> {
    let matches = data
        .auto_replies
        .iter()
        .filter(|r| r.keywords.iter().any(|s| content.contains(s)));

    for reply in matches {
        if reply.chance.is_some_and(|chance| random_bool(chance)) {
            continue;
        }

        let keyword = reply.keywords.first().unwrap();
        query!("INSERT INTO auto_replies(user_id, keyword, count) VALUES ($1, $2, 1) ON CONFLICT (keyword, user_id) DO UPDATE SET count = auto_replies.count + 1", new_message.author.id.get() as i64, keyword).execute(&data.database).await?;
        let stats = query!(
                        "SELECT SUM(count)::int AS count FROM auto_replies WHERE keyword ILIKE '%' || $1 || '%'",
                        keyword
                    )
            .fetch_one(&data.database)
            .await?;
        let amount_replied = stats.count.unwrap_or_default().to_string();

        let user = reply.user.to_user(ctx.http()).await?;
        let desc = reply
            .description
            .replace("{user}", &user.to_string())
            .replace("{replies}", &amount_replied);
        let mut m = CreateMessage::new();
        // embeds can't ping
        if reply.ping {
            m = m.content(user.mention().to_string());
        }
        let message = m.reference_message(new_message).embed(
            CreateEmbed::new()
                .title(&reply.title)
                .description(desc)
                .colour(reply.colour)
                .author(
                    CreateEmbedAuthor::new(&user.name)
                        .icon_url(user.avatar_url().unwrap_or(user.default_avatar_url())),
                ),
        );
        new_message
            .channel_id
            .send_message(&ctx.http, message)
            .await?;
    }
    Ok(())
}

async fn auto_react(
    ctx: &Context,
    data: &Data,
    new_message: &Message,
    content: &str,
) -> Result<(), Error> {
    let words = WORD_REGEX
        .find_iter(content)
        .map(|mat| mat.as_str())
        .collect_vec();
    for (keyword, reaction) in &data.auto_reactions {
        if words.contains(&keyword.as_str()) {
            new_message.react(&ctx.http, reaction.clone()).await?;
        }
    }
    Ok(())
}

async fn celery_fact(ctx: &Context, data: &Data, channel: ChannelId) -> Result<(), Error> {
    if let Some(config) = data.celery.get(&channel) {
        // saturating subtraction
        let previous_value =
            config
                .counter
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |val| {
                    if val == 0 { Some(0) } else { Some(val - 1) }
                });
        // not 0 now, still in cooldown
        if previous_value.unwrap() > 1 {
            return Ok(());
        }
        // unlucky
        if !random_bool(config.chance) {
            return Ok(());
        }
        // other thread was faster
        let lock = config.mutex.try_lock();
        if lock.is_err() {
            return Ok(());
        }
        // reset cooldown and check if we waited so long another thread is done
        if config
            .counter
            .compare_exchange(0, config.cooldown, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        let payload = json!({
            "model": "deepseek-v3.1:671b",
            "messages": [{"role": "user", "content": config.prompt}],
            "stream": false
        });

        let response = HTTP_CLIENT
            .post("https://ollama.com/api/chat")
            .bearer_auth(&data.ollama_token)
            .json(&payload)
            .send()
            .await?;

        let json: json::Value = response.json().await?;

        let fact = json["message"]["content"]
            .as_str()
            .unwrap_or("Celery contains negative calories, meaning you burn more energy chewing it than you gain from eating it!")
            .to_string();
        channel.say(&ctx.http, fact).await?;
        drop(lock);
    }

    Ok(())
}
