use super::{Skill, SkillId, SkillTarget};
use crate::position::{offset, Position};
use crate::types::*;

/// One of your pieces (not the king) repeats its last move: the same step
/// again from where it stands, played like a real move of that piece. This
/// replaces the move of the turn.
pub struct Temporal;

/// Where `piece` on `from` would land by repeating its last move, or `None`
/// when the piece could not make that move: a slider (or a pawn's double
/// step) cannot jump over a piece or cross enemy terrain, a pawn only takes
/// diagonally and only walks straight onto an empty square.
fn replay(pos: &Position, from: Square, piece: &Piece) -> Option<Square> {
    let prev = piece.prev?;
    let df = file_of(from) as i8 - file_of(prev) as i8;
    let dr = rank_of(from) as i8 - rank_of(prev) as i8;
    let to = offset(from, df, dr)?;
    if !Position::can_stand(piece.kind, to) {
        return None;
    }
    let blocked = pos.blocked_mask(piece.color);
    let crosses = match piece.kind {
        PieceKind::Bishop | PieceKind::Rook | PieceKind::Queen => true,
        PieceKind::Pawn => df == 0,
        _ => false,
    };
    if crosses {
        let (sf, sr) = (df.signum(), dr.signum());
        let mut cur = offset(from, sf, sr)?;
        while cur != to {
            if pos.board[cur as usize].is_some() || blocked & (1u64 << cur) != 0 {
                return None;
            }
            cur = offset(cur, sf, sr)?;
        }
    }
    if blocked & (1u64 << to) != 0 {
        return None;
    }
    match pos.board[to as usize] {
        // A pawn's diagonal step is a capture: it needs a victim.
        None => (piece.kind != PieceKind::Pawn || df == 0).then_some(to),
        Some(victim) => {
            let truce = !pos.effects.is_empty() && pos.truce();
            (!piece.mirage
                && !truce
                && !(piece.kind == PieceKind::Pawn && df == 0)
                && victim.color != piece.color
                && victim.kind != PieceKind::King
                && !pos.is_immune(victim.id))
            .then_some(to)
        }
    }
}

impl Skill for Temporal {
    fn id(&self) -> SkillId {
        SkillId::Temporal
    }

    fn targets(&self, pos: &Position, color: Color) -> Vec<SkillTarget> {
        pos.pieces(color)
            .filter(|(_, p)| p.kind != PieceKind::King && !pos.is_frozen(p.id))
            .filter(|(from, p)| replay(pos, *from, p).is_some())
            .map(|(square, _)| SkillTarget::Piece { square })
            .collect()
    }

    fn apply(&self, pos: &mut Position, _color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        let SkillTarget::Piece { square: from } = target else {
            return;
        };
        let piece = pos.board[from as usize].expect("temporal target");
        let to = replay(pos, from, &piece).expect("temporal destination");
        // The same path as a move: captures (Force Field, Celestial), enemy
        // traps on the way, castling rights and `prev`.
        pos.play_move(
            Move {
                from,
                to,
                promo: None,
            },
            ev,
        );
    }
}
