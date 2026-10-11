use chessy_engine::{Color, PieceKind, SkillId, SkillId::*};

use super::{lending, starting, Level};
use crate::campaign::Objective::{
    KeepPiece, NoPieceLostBefore, NoSkillUsed, UseAllSkills, UseAnySkill, WinWithin,
};

const LENT: &[SkillId] = &[Freeze, Tornado, Canceller];
const ILLUSIONNISTE_START: &str = "rnbqkbnr/pppppppp/5n2/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub(super) const LEVELS: &[Level] = &[
    lending(
        "Premier signe",
        LENT,
        &[Canceller],
        UseAnySkill,
        KeepPiece(PieceKind::Queen),
        "Canceller annule la dernière compétence adverse : attendez que Sage en joue une avant de répondre.",
    ),
    lending(
        "Vent contraire",
        LENT,
        &[Tornado],
        UseAnySkill,
        WinWithin(38),
        "Tornado dérange le plan de Sage : lancez-le quand il aligne ses pièces.",
    ),
    lending(
        "Givre",
        LENT,
        &[Freeze, Geomancy],
        UseAnySkill,
        KeepPiece(PieceKind::Rook),
        "Freeze fige une pièce adverse : gelez celle qui menace votre roi.",
    ),
    lending(
        "Fracture",
        LENT,
        &[Tornado, Canceller],
        UseAllSkills,
        WinWithin(36),
        "Alternez les compétences : une seule ne suffit pas, il faut les employer toutes pendant la partie.",
    ),
    lending(
        "Œil du cyclone",
        LENT,
        &[Freeze, Geomancy, Tornado],
        UseAllSkills,
        KeepPiece(PieceKind::Queen),
        "Sage joue trois compétences de terrain : Canceller peut en annuler une, gardez-le pour la plus dangereuse.",
    ),
    lending(
        "Maîtrise du terrain",
        LENT,
        &[Canceller, Tornado, Freeze],
        WinWithin(32),
        NoSkillUsed,
        "Le défi demande de gagner sans compétence : misez sur le jeu simple et les coups précis.",
    ),
    starting(
        lending(
            "L'Illusionniste",
            LENT,
            &[Invisibility, Geomancy, Tornado],
            UseAllSkills,
            NoPieceLostBefore(20),
            "L'Illusionniste cache ses pièces : comptez les coups qu'il joue pour deviner où elles se trouvent.",
        ),
        ILLUSIONNISTE_START,
        Color::White,
    ),
];
