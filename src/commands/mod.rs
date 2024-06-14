use poise::serenity_prelude::{ChannelId, GuildId, MessageId};

pub(crate) use self::{
    boop::*, events::*, images::*, keyword_statistics::*, mensa::*, mensaplan::*, misc::*,
    modules::*, owner::*, reaction_role::*, utility::*, uwu::*,
};

mod events;
mod images;
mod mensa;
mod mensaplan;
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
