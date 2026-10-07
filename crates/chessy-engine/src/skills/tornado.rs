use std::cmp::Ordering;

use super::{Skill, SkillId, SkillTarget};
use crate::position::Position;
use crate::types::*;

/// Every piece except the kings jumps to the square of the next piece going
/// round the board counterclockwise. A pawn never lands on its promotion rank
/// (and is never promoted): it takes the destination of a piece that can stand
/// there instead, and that piece takes the pawn's. Its own back rank is fine.
pub struct Tornado;

/// A square relative to the centre of the board, doubled so it is integral.
fn offset_from_centre(s: Square) -> (i32, i32) {
    (2 * file_of(s) as i32 - 7, 2 * rank_of(s) as i32 - 7)
}

fn half(p: (i32, i32)) -> u8 {
    if p.1 > 0 || (p.1 == 0 && p.0 > 0) {
        0
    } else {
        1
    }
}

/// Counterclockwise order around the centre, starting east; exact (no floats).
fn by_angle(a: Square, b: Square) -> Ordering {
    let (pa, pb) = (offset_from_centre(a), offset_from_centre(b));
    half(pa)
        .cmp(&half(pb))
        .then_with(|| {
            let cross = pa.0 * pb.1 - pa.1 * pb.0;
            0.cmp(&cross)
        })
        .then(a.cmp(&b))
}

fn rotating_squares(pos: &Position) -> Vec<Square> {
    let mut squares: Vec<Square> = pos
        .board
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_some_and(|p| p.kind != PieceKind::King))
        .map(|(i, _)| i as Square)
        .collect();
    squares.sort_by(|&a, &b| by_angle(a, b));
    squares
}

/// `(from, to)` for every rotating piece, or `None` when a pawn could not be
/// kept off the back ranks.
fn plan(pos: &Position) -> Option<Vec<(Square, Square)>> {
    let squares = rotating_squares(pos);
    let n = squares.len();
    let pieces: Vec<Piece> = squares
        .iter()
        .map(|&s| pos.board[s as usize].expect("piece to rotate"))
        .collect();
    let stands = |i: usize, to: Square| Position::can_stand(pieces[i].color, pieces[i].kind, to);
    let mut dest: Vec<Square> = (0..n).map(|i| squares[(i + 1) % n]).collect();
    for i in 0..n {
        if stands(i, dest[i]) {
            continue;
        }
        let swap = (1..n).map(|d| (i + d) % n).find(|&j| {
            stands(i, dest[j]) && stands(j, dest[i])
        })?;
        dest.swap(i, swap);
    }
    Some(squares.into_iter().zip(dest).collect())
}

impl Skill for Tornado {
    fn id(&self) -> SkillId {
        SkillId::Tornado
    }

    fn targets(&self, pos: &Position, _color: Color) -> Vec<SkillTarget> {
        if rotating_squares(pos).len() >= 2 && plan(pos).is_some() {
            vec![SkillTarget::None]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, pos: &mut Position, _color: Color, _target: SkillTarget, ev: &mut Vec<Event>) {
        let plan = plan(pos).expect("tornado plan");
        let pieces: Vec<Piece> = plan
            .iter()
            .map(|&(from, _)| pos.board[from as usize].take().expect("piece to rotate"))
            .collect();
        let mut moves = Vec::with_capacity(plan.len());
        for (mut piece, (from, to)) in pieces.into_iter().zip(plan) {
            piece.prev = None;
            pos.board[to as usize] = Some(piece);
            moves.push(Shift { from, to });
        }
        ev.push(Event::Rotated { moves });
    }
}
