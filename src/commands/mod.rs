use poise::serenity_prelude::GuildId;

pub(crate) use self::{
    events::*, images::*, mensa::*, mensaplan::*, misc::*, modules::*, owner::*, reaction_role::*,
    utility::*, bets::*,
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
mod bets;

#[macro_export]
macro_rules! done {
    ($ctx:expr) => {
        use poise::CreateReply;
        $ctx.send(
            CreateReply::default()
                .content("Done")
                .ephemeral(true)
                .reply(true),
        )
        .await?;
        return Ok(());
    };
}
