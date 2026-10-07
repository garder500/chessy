use super::{Skill, SkillId, SkillKind, SkillTarget};
use crate::position::Position;
use crate::types::*;

/// Unique skill: an enemy piece (not the king) that attacks none of your pieces
/// changes sides for good. Refused if that checkmates the opponent.
pub struct Switch;

impl Skill for Switch {
    fn id(&self) -> SkillId {
        SkillId::Switch
    }

    fn kind(&self) -> SkillKind {
        SkillKind::Unique
    }

    fn forbids_mate(&self) -> bool {
        true
    }

    fn targets(&self, pos: &Position, color: Color) -> Vec<SkillTarget> {
        pos.pieces(color.opposite())
            .filter(|(_, p)| p.kind != PieceKind::King)
            // A pawn must not end up on its new promotion rank.
            .filter(|&(square, p)| Position::can_stand(color, p.kind, square))
            .filter(|&(from, _)| {
                pos.attacked_squares(from)
                    .into_iter()
                    .all(|s| !matches!(pos.board[s as usize], Some(p) if p.color == color))
            })
            .map(|(square, _)| SkillTarget::Piece { square })
            .collect()
    }

    fn apply(&self, pos: &mut Position, color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        let SkillTarget::Piece { square } = target else {
            return;
        };
        let piece = pos.board[square as usize].as_mut().expect("switch target");
        piece.color = color;
        piece.prev = None;
        let piece = *piece;
        // Its effects go with its old allegiance, except a countdown to vanishing.
        pos.effects
            .retain(|e| e.piece != piece.id || e.kind == EffectKind::Vanish);
        ev.push(Event::Switched { square, piece });
    }
}
