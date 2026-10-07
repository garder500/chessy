use super::{Skill, SkillId, SkillKind, SkillTarget};
use crate::position::Position;
use crate::types::*;

/// Unique skill: an enemy piece (not the king) fights for you until the end of
/// this turn. Using it does not use up the turn.
pub struct Control;

impl Skill for Control {
    fn id(&self) -> SkillId {
        SkillId::Control
    }

    fn kind(&self) -> SkillKind {
        SkillKind::Unique
    }

    fn ends_turn(&self) -> bool {
        false
    }

    fn targets(&self, pos: &Position, color: Color) -> Vec<SkillTarget> {
        pos.pieces(color.opposite())
            .filter(|(_, p)| p.kind != PieceKind::King)
            // A pawn must not end up on its new promotion rank.
            .filter(|&(square, p)| Position::can_stand(color, p.kind, square))
            .map(|(square, _)| SkillTarget::Piece { square })
            .collect()
    }

    fn apply(&self, pos: &mut Position, color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        let SkillTarget::Piece { square } = target else {
            return;
        };
        let piece = pos.board[square as usize].as_mut().expect("control target");
        let id = piece.id;
        let orig = piece.color;
        piece.color = color;
        let mut loan = ActiveEffect::new(EffectKind::ColorLoan, id, pos.ply + 1);
        loan.orig_color = Some(orig);
        ev.push(pos.push_effect(loan));
    }
}
