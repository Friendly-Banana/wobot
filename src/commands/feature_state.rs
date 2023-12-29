use poise::serenity_prelude::{Colour, CreateSelectMenuOption, ReactionType};

use self::FeatureState::*;

#[repr(i64)]
#[derive(Copy, Clone, PartialEq, poise::ChoiceParameter)]
pub(crate) enum FeatureState {
    All = -1,
    ToDo,
    Implemented,
    Rejected,
    Postponed,
}

impl FeatureState {
    pub(crate) fn menu(
        state: FeatureState,
        m: &mut CreateSelectMenuOption,
    ) -> &mut CreateSelectMenuOption {
        m.label(state.to_string()).value(state as u32).emoji(state)
    }
}

impl From<FeatureState> for Colour {
    fn from(state: FeatureState) -> Self {
        match state {
            ToDo => Colour::DARK_BLUE,
            Implemented => Colour::DARK_GREEN,
            Rejected => Colour::DARK_RED,
            Postponed => Colour::FADED_PURPLE,
            _ => Colour::GOLD,
        }
    }
}

impl From<FeatureState> for ReactionType {
    fn from(state: FeatureState) -> Self {
        ReactionType::from(match state {
            ToDo => '🔵',
            Implemented => '🟢',
            Rejected => '🔴',
            Postponed => '🟣',
            _ => '🟡',
        })
    }
}

impl From<i64> for FeatureState {
    fn from(value: i64) -> Self {
        match value {
            0 => ToDo,
            1 => Implemented,
            2 => Rejected,
            3 => Postponed,
            _ => All,
        }
    }
}
