use chessy_engine::{Color, PieceKind, SkillId, SkillId::*};

use super::{lending, starting, Level};
use crate::campaign::Objective::{KeepPiece, NoSkillUsed, UseAllSkills, UseAnySkill, WinWithin};

const LENT: &[SkillId] = &[Clone, Morph, Godhelp];
const FORGERON_START: &str = "rnbqkbnr/pppppppp/8/8/8/2P2P2/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub(super) const LEVELS: &[Level] = &[
    lending(
        "Première pierre",
        LENT,
        &[Wall],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
        "Clone copie une de vos pièces : doublez celle qui tient la colonne ouverte.",
    ),
    lending(
        "Double",
        LENT,
        &[Mirage],
        UseAnySkill,
        WinWithin(38),
        "Une pièce de plus change le rapport de force : clonez tôt, puis attaquez ensemble.",
    ),
    lending(
        "Mirage",
        LENT,
        &[Evolve, Wall],
        UseAnySkill,
        KeepPiece(PieceKind::Rook),
        "Les murs de Sage coupent vos lignes : contournez-les ou transformez la pièce qui les gêne avec Morph.",
    ),
    lending(
        "Métamorphose",
        LENT,
        &[Mirage, Morph],
        UseAllSkills,
        WinWithin(36),
        "Morph change une pièce en une autre : un pion devenu cavalier peut surprendre une défense.",
    ),
    lending(
        "Main divine",
        LENT,
        &[Clone, Evolve, Wall],
        UseAllSkills,
        KeepPiece(PieceKind::Queen),
        "God Help ramène une pièce perdue : jouez-la après un échange, pas avant.",
    ),
    lending(
        "Chef-d'œuvre",
        LENT,
        &[Godhelp, Mirage, Evolve],
        WinWithin(32),
        NoSkillUsed,
        "Le défi interdit les compétences : gagnez sur l'échiquier seul, sans les prêts.",
    ),
    starting(
        lending(
            "Le Forgeron",
            LENT,
            &[Clone, Godhelp, Morph],
            UseAllSkills,
            KeepPiece(PieceKind::Queen),
            "Le Forgeron recrée ce qu'on lui prend : ne lui offrez pas d'échange si son God Help est encore disponible.",
        ),
        FORGERON_START,
        Color::Black,
    ),
];
