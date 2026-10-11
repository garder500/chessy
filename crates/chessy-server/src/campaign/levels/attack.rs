use chessy_engine::{PieceKind, SkillId::*};

use super::{imposed, Level};
use crate::campaign::Objective::{KeepPiece, UseAllSkills, UseSkill, WinWithin};

pub(super) const LEVELS: &[Level] = &[
    imposed(
        "Première piste",
        &[Trap],
        &[Remover],
        UseSkill(Trap),
        WinWithin(25),
        "Posez votre Trap Card sur une case que l'adversaire voudra prendre, puis attendez qu'il y marche.",
    ),
    imposed(
        "Appât",
        &[Trap, Terminator],
        &[Trap],
        UseSkill(Terminator),
        WinWithin(30),
        "Terminator retire une pièce adverse : visez celle qui protège son roi.",
    ),
    imposed(
        "Sacrifice",
        &[Queensac, Trap],
        &[Terminator, Remover],
        UseSkill(Queensac),
        KeepPiece(PieceKind::Rook),
        "Queen Sacrifice coûte votre dame : jouez-la quand elle emporte plus que ce qu'elle vaut.",
    ),
    imposed(
        "Tenaille",
        &[Terminator, Queensac],
        &[Trap, Switch],
        UseAllSkills,
        KeepPiece(PieceKind::Rook),
        "Sacrifiez la dame d'abord, puis servez-vous de Terminator pour finir le travail.",
    ),
    imposed(
        "Triple lame",
        &[Queensac, Terminator, Trap],
        &[Remover, Trap, Terminator],
        UseAllSkills,
        WinWithin(28),
        "Chaque compétence ne sert qu'une fois : réservez le piège pour un moment calme.",
    ),
    imposed(
        "Tempête d'acier",
        &[Queensac, Terminator, Trap],
        &[Terminator, Trap, Queensac],
        UseSkill(Terminator),
        KeepPiece(PieceKind::Knight),
        "Sage dispose des mêmes armes que vous : n'offrez pas de pièce qu'il peut prendre gratuitement.",
    ),
    imposed(
        "Le Bélier",
        &[Queensac, Terminator, Trap],
        &[Terminator, Trap, Queensac],
        UseAllSkills,
        WinWithin(35),
        "Le Bélier frappe tôt : tenez vos pièces protégées et gardez Queen Sacrifice pour la fin.",
    ),
];
