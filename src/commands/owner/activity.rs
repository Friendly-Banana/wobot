use poise::serenity_prelude::ActivityData;

use crate::{Context, Error, done};

#[derive(poise::ChoiceParameter, PartialEq)]
pub(crate) enum ActivityChoice {
    Unset,
    Playing,
    Listening,
    Watching,
    Streaming,
    Competing,
    Custom,
}

/// Set the bot's activity
#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn activity(
    ctx: Context<'_>,
    activity: ActivityChoice,
    action: String,
    #[description = "stream url if Streaming or details if Custom"] details: Option<String>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let activity = match activity {
        ActivityChoice::Unset => None,
        ActivityChoice::Playing => Some(ActivityData::playing(action)),
        ActivityChoice::Listening => Some(ActivityData::listening(action)),
        ActivityChoice::Watching => Some(ActivityData::watching(action)),
        ActivityChoice::Competing => Some(ActivityData::competing(action)),
        ActivityChoice::Streaming => Some(ActivityData::streaming(
            action,
            details.unwrap_or("https://www.twitch.tv/".to_string()),
        )?),
        ActivityChoice::Custom => Some(ActivityData::custom(details.unwrap_or("".to_string()))),
    };
    ctx.serenity_context().set_activity(activity);
    done!(ctx);
}
