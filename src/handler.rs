use itertools::Itertools;
use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    CacheHttp, Context, CreateEmbed, CreateEmbedAuthor, CreateMessage, FullEvent, GuildId,
    Mentionable, UserId,
};
use poise::FrameworkContext;
use regex::Regex;
use sqlx::query;
use tracing::warn;

use crate::commands::change_reaction_role;
use crate::{CacheEntry, Data, Error};

static WORD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\w+\b").unwrap());

pub(crate) async fn event_handler(
    ctx: &Context,
    event: &FullEvent,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        FullEvent::VoiceStateUpdate { new, .. } => {
            if let Some(guild) = new.guild_id {
                update_activity(data, guild, new.user_id).await;
            }
            Ok(())
        }
        FullEvent::ReactionAdd { add_reaction } => {
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
            if let Some(guild) = new_message.guild_id {
                update_activity(data, guild, new_message.author.id).await;
            }

            let content = new_message.content.to_lowercase();
            let words = WORD_REGEX
                .find_iter(&content)
                .map(|mat| mat.as_str().to_string())
                .collect_vec();
            for (keyword, reaction) in &data.auto_reactions {
                if words.contains(keyword) {
                    new_message.react(&ctx.http, reaction.clone()).await?;
                }
            }

            let matches = data
                .auto_replies
                .iter()
                .filter(|r| r.keywords.iter().any(|s| content.contains(s)));

            for reply in matches {
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
                new_message
                    .channel_id
                    .send_message(
                        &ctx.http,
                        m.reference_message(new_message).embed(
                            CreateEmbed::new()
                                .title(&reply.title)
                                .description(desc)
                                .colour(reply.colour)
                                .author(CreateEmbedAuthor::new(&user.name).icon_url(
                                    user.avatar_url().unwrap_or(user.default_avatar_url()),
                                )),
                        ),
                    )
                    .await?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

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
