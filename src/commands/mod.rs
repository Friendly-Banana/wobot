use poise::serenity_prelude::GuildId;

pub(crate) use self::{
    bets::*, events::*, images::*, mensa::*, mensaplan::*, misc::*, modules::*, owner::*,
    reaction_role::*, utility::*,
};

mod bets;
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
