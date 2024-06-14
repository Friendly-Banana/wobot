use poise::serenity_prelude::{
    CacheHttp, Context, CreateEmbed, CreateEmbedAuthor, CreateMessage, FullEvent, Mentionable,
};
use poise::FrameworkContext;
use sqlx::query;

use crate::commands::change_reaction_role;
use crate::{Data, Error};

pub(crate) async fn event_handler(
    ctx: &Context,
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
            for (keyword, reaction) in &data.auto_reactions {
                if keyword.is_match(&content) {
                    new_message.react(&ctx.http, reaction.clone()).await?;
                }
            }

            let matches = data
                .auto_replies
                .iter()
                .filter(|r| r.keywords.iter().any(|s| s.is_match(&content)));

            for reply in matches {
                let keyword = reply.keywords.first().unwrap().to_string();
                query!("INSERT INTO auto_replies(user_id, keyword, count) VALUES ($1, $2, 1) ON CONFLICT (keyword, user_id) DO UPDATE SET count = auto_replies.count + 1", new_message.author.id.get() as i64, keyword).execute(&data.database).await?;
                let stats = query!(
                    "SELECT count(*) as count FROM auto_replies WHERE keyword = $1",
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
