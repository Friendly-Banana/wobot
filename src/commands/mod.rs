use poise::serenity_prelude::{ChannelId, GuildId, MessageId};

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

pub(crate) fn link_message(guild: Option<GuildId>, channel_id: i64, msg_id: i64) -> String {
    MessageId::new(msg_id as u64).link(ChannelId::new(channel_id as u64), guild)
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
