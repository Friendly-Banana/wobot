use std::collections::HashMap;

use poise::serenity_prelude::{
    CacheHttp, CreateEmbed, CreateEmbedAuthor, CreateMessage, FullEvent, Mentionable,
};
use poise::FrameworkContext;
use sqlx::query;

use crate::commands::change_reaction_role;
use crate::{Data, Error};

pub(crate) async fn event_handler(
    ctx: &poise::serenity_prelude::Context,
    event: &FullEvent,
    _framework: FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        FullEvent::ReactionAdd { add_reaction } => {
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
            for (reaction, keyword) in &data.auto_reactions {
                if keyword.is_match(&content) {
                    new_message.react(&ctx.http, reaction.clone()).await?;
                }
            }

            let nsfw_channel = new_message.channel(&ctx).await?.is_nsfw();
            let mut map = HashMap::new();
            for reply in &data.auto_replies {
                if !reply.nsfw || nsfw_channel {
                    if let Some(keyword) = reply.keywords.iter().find(|&s| content.contains(s)) {
                        map.insert(keyword, reply);
                    }
                }
            }
            for (keyword, reply) in map {
                query!("INSERT INTO auto_replies(user_id, keyword, count) VALUES ($1, $2, 1) ON CONFLICT (keyword, user_id) DO UPDATE SET count = auto_replies.count + 1", new_message.author.id.get() as i64, keyword).execute(&data.database).await?;
                let bot_id = data.bot_id.read().expect("bot_id").get() as i64;
                let amount_replied = query!("INSERT INTO auto_replies(user_id, keyword, count) VALUES ($1, $2, 1) ON CONFLICT (keyword, user_id) DO UPDATE SET count = auto_replies.count + 1 RETURNING count", bot_id, keyword)
                .fetch_one(&data.database)
                .await?
                .count;

                let user = reply.user.to_user(ctx.http()).await?;
                let desc = reply
                    .description
                    .replace("{user}", &user.to_string())
                    .replace("{replies}", &amount_replied.to_string());
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
