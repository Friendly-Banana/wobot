use poise::serenity_prelude::{ChannelId, GuildId, MessageId};

pub(crate) use self::{
    boop::*, cruisine::*, events::*, images::*, keyword_statistics::*, mensa::*, misc::*,
    modules::*, owner::*, reaction_role::*, utility::*, uwu::*,
};

mod cruisine;
mod events;
mod images;
mod mensa;
mod misc;
mod modules;
mod owner;
mod reaction_role;
mod utility;
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
