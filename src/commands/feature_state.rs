use std::fmt::{Display, Formatter};

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
    pub(crate) fn menu(state: FeatureState) -> CreateSelectMenuOption {
        CreateSelectMenuOption::new(state.to_string(), (state as i64).to_string())
            .emoji(ReactionType::from(state))
    }
}

impl Display for FeatureState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            All => write!(f, "All"),
            ToDo => write!(f, "To Do"),
            Implemented => write!(f, "Implemented"),
            Rejected => write!(f, "Rejected"),
            Postponed => write!(f, "Postponed"),
        }
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
