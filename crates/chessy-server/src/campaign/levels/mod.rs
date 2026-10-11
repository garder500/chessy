//! The level tables of the five chapters (see `docs/spec-campagne.md`).

use chessy_engine::{Color, SkillId};

use super::{Chapter, Level, Objective, Start};

mod attack;
mod control;
mod create;
mod defense;
mod mobility;
#[cfg(test)]
mod tests;

/// A level whose hand is imposed on the player.
const fn imposed(
    name: &'static str,
    player_deck: &'static [SkillId],
    bot_deck: &'static [SkillId],
    objective: Objective,
    challenge: Objective,
    hint: &'static str,
) -> Level {
    Level {
        name,
        player_deck,
        bot_deck,
        deck_choice: false,
        start: None,
        objective: Some(objective),
        challenge: Some(challenge),
        hint,
        lent: &[],
    }
}

/// A level where the player brings their own deck and is lent `lent` besides.
const fn lending(
    name: &'static str,
    lent: &'static [SkillId],
    bot_deck: &'static [SkillId],
    objective: Objective,
    challenge: Objective,
    hint: &'static str,
) -> Level {
    Level {
        name,
        player_deck: &[],
        bot_deck,
        deck_choice: true,
        start: None,
        objective: Some(objective),
        challenge: Some(challenge),
        hint,
        lent,
    }
}

const fn starting(level: Level, fen: &'static str, human: Color) -> Level {
    Level {
        start: Some(Start { fen, human }),
        ..level
    }
}

pub const CHAPTERS: [Chapter; 5] = [
    Chapter {
        family: "attack",
        name: "Attaque",
        title: "Tombeur du Bélier",
        levels: attack::LEVELS,
    },
    Chapter {
        family: "defense",
        name: "Défense",
        title: "Briseur de Muraille",
        levels: defense::LEVELS,
    },
    Chapter {
        family: "mobility",
        name: "Mobilité",
        title: "Maître des Routes",
        levels: mobility::LEVELS,
    },
    Chapter {
        family: "control",
        name: "Contrôle",
        title: "Démasqueur",
        levels: control::LEVELS,
    },
    Chapter {
        family: "create",
        name: "Création",
        title: "Maître de forge",
        levels: create::LEVELS,
    },
];
