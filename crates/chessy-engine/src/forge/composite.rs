//! The interpreter: one [`Skill`] that does whatever its [`SkillDef`] says.

use super::def::{Constraint, Effect, Side, SkillDef, SwapScope};
use crate::position::Position;
use crate::skills::{neighbors, Skill, SkillId, SkillKind, SkillTarget};
use crate::types::*;

pub struct Composite {
    pub(crate) id: SkillId,
    pub(crate) def: SkillDef,
    pub(crate) name: &'static str,
}

/// Squares a temporary piece may appear on: ranks 3 to 6.
const SPAWN_SQUARES: std::ops::Range<u8> = 16..48;

fn side_color(caster: Color, side: Side) -> Color {
    match side {
        Side::Own => caster,
        Side::Enemy => caster.opposite(),
    }
}

fn piece_targets(
    pos: &Position,
    color: Color,
    keep: impl Fn(Square, &Piece) -> bool,
) -> Vec<SkillTarget> {
    pos.pieces(color)
        .filter(|(square, piece)| keep(*square, piece))
        .map(|(square, _)| SkillTarget::Piece { square })
        .collect()
}

fn not_king(_: Square, p: &Piece) -> bool {
    p.kind != PieceKind::King
}

/// Where a revived piece goes: its starting square, or the closest free one.
fn revive_square(pos: &Position, color: Color, grave: &Piece) -> Option<Square> {
    if pos.can_place(color, grave.kind, grave.home) {
        Some(grave.home)
    } else {
        pos.nearest_free(grave.home, color, grave.kind)
    }
}

fn first_grave(pos: &Position, color: Color, kind: PieceKind) -> Option<usize> {
    pos.graveyard
        .iter()
        .position(|p| p.color == color && p.kind == kind)
}

impl Composite {
    fn spawn_targets(pos: &Position, color: Color, kinds: &[PieceKind]) -> Vec<SkillTarget> {
        let mut out = Vec::new();
        for square in SPAWN_SQUARES {
            for &kind in kinds {
                if pos.can_place(color, kind, square) {
                    out.push(SkillTarget::Spawn { square, kind });
                }
            }
        }
        out
    }

    fn place_new(pos: &mut Position, piece: Piece, ev: &mut Vec<Event>) {
        pos.board[piece.home as usize] = Some(piece);
        ev.push(Event::Spawned {
            square: piece.home,
            piece,
        });
    }
}

impl Skill for Composite {
    fn id(&self) -> SkillId {
        self.id
    }

    fn kind(&self) -> SkillKind {
        if self.def.unique {
            SkillKind::Unique
        } else {
            SkillKind::Classic
        }
    }

    fn forbids_mate(&self) -> bool {
        self.def.constraints.contains(&Constraint::ForbidMate)
    }

    fn forbids_check(&self) -> bool {
        self.def.constraints.contains(&Constraint::ForbidCheck)
            || matches!(
                self.def.effect,
                Effect::Swap {
                    scope: SwapScope::Any
                }
            )
    }

    fn ends_turn(&self) -> bool {
        !self.def.free_action
    }

    fn max_uses(&self) -> u8 {
        self.def.max_uses
    }

    fn targets(&self, pos: &Position, color: Color) -> Vec<SkillTarget> {
        if self.def.constraints.contains(&Constraint::OnlyInCheck) && !pos.in_check(color) {
            return Vec::new();
        }
        let enemy = color.opposite();
        match &self.def.effect {
            Effect::Freeze { .. } => piece_targets(pos, enemy, not_king),
            Effect::Shield { .. } | Effect::Cloak { .. } => piece_targets(pos, color, not_king),
            Effect::Morph { side, into, .. } => {
                let who = side_color(color, *side);
                pos.pieces(who)
                    .filter(|(square, p)| {
                        p.kind != PieceKind::King
                            && p.kind != *into
                            && Position::can_stand(who, *into, *square)
                    })
                    .map(|(square, _)| SkillTarget::Spawn {
                        square,
                        kind: *into,
                    })
                    .collect()
            }
            Effect::Promote => piece_targets(pos, color, |_, p| {
                !matches!(p.kind, PieceKind::King | PieceKind::Queen)
            }),
            Effect::Remove { kinds } => {
                piece_targets(pos, enemy, |_, p| kinds.contains(&p.kind) && not_king(0, p))
            }
            Effect::Convert => pos
                .pieces(enemy)
                .filter(|(_, p)| p.kind != PieceKind::King)
                .filter(|&(from, _)| {
                    pos.attacked_squares(from)
                        .into_iter()
                        .all(|s| !matches!(pos.board[s as usize], Some(p) if p.color == color))
                })
                .map(|(square, _)| SkillTarget::Piece { square })
                .collect(),
            Effect::Teleport => {
                let mut out = Vec::new();
                for (from, piece) in pos.pieces(color) {
                    if piece.kind == PieceKind::King || pos.is_frozen(piece.id) {
                        continue;
                    }
                    for to in 0..64u8 {
                        if pos.can_place(color, piece.kind, to) {
                            out.push(SkillTarget::PieceTo { from, to });
                        }
                    }
                }
                out
            }
            Effect::Duplicate => {
                let mut out = Vec::new();
                for (from, piece) in pos.pieces(color) {
                    if piece.kind == PieceKind::King {
                        continue;
                    }
                    for to in neighbors(from) {
                        if pos.can_place(color, piece.kind, to) {
                            out.push(SkillTarget::PieceTo { from, to });
                        }
                    }
                }
                out
            }
            Effect::Swap { scope } => {
                let mut pool: Vec<(Square, Piece)> = match scope {
                    SwapScope::Own => pos
                        .pieces(color)
                        .filter(|(_, p)| !pos.is_frozen(p.id))
                        .collect(),
                    SwapScope::Any => Color::BOTH
                        .into_iter()
                        .flat_map(|c| pos.pieces(c))
                        .filter(|(_, p)| p.kind != PieceKind::King)
                        .collect(),
                };
                // A pair is listed with its lower square first.
                pool.sort_by_key(|&(square, _)| square);
                let mut out = Vec::new();
                for (i, &(a, pa)) in pool.iter().enumerate() {
                    for &(b, pb) in &pool[i + 1..] {
                        if Position::can_stand(pa.color, pa.kind, b)
                            && Position::can_stand(pb.color, pb.kind, a)
                        {
                            out.push(SkillTarget::Pair { a, b });
                        }
                    }
                }
                out
            }
            Effect::Truce { .. } => {
                if pos.truce() {
                    Vec::new()
                } else {
                    vec![SkillTarget::None]
                }
            }
            Effect::Fog { .. } => {
                if pos.fog() {
                    Vec::new()
                } else {
                    vec![SkillTarget::None]
                }
            }
            Effect::Silence { .. } => {
                if pos.is_silenced(enemy) {
                    Vec::new()
                } else {
                    vec![SkillTarget::None]
                }
            }
            Effect::Ambush { .. } => {
                if pos.has_domain(color) {
                    Vec::new()
                } else {
                    vec![SkillTarget::None]
                }
            }
            Effect::Mirror => vec![SkillTarget::None],
            Effect::Spawn { kinds, .. } => Self::spawn_targets(pos, color, kinds),
            Effect::Revive { kinds } => {
                let mut out: Vec<SkillTarget> = Vec::new();
                for &kind in kinds {
                    let Some(i) = first_grave(pos, color, kind) else {
                        continue;
                    };
                    if let Some(square) = revive_square(pos, color, &pos.graveyard[i]) {
                        out.push(SkillTarget::Spawn { square, kind });
                    }
                }
                out
            }
        }
    }

    fn apply(&self, pos: &mut Position, color: Color, target: SkillTarget, ev: &mut Vec<Event>) {
        match (&self.def.effect, target) {
            (Effect::Freeze { plies }, SkillTarget::Piece { square }) => {
                let id = pos.board[square as usize].expect("freeze target").id;
                ev.push(pos.add_effect(EffectKind::Frozen, id, u32::from(*plies)));
            }
            (Effect::Shield { plies }, SkillTarget::Piece { square }) => {
                let id = pos.board[square as usize].expect("shield target").id;
                ev.push(pos.add_effect(EffectKind::Immune, id, u32::from(*plies)));
            }
            (Effect::Cloak { plies }, SkillTarget::Piece { square }) => {
                let id = pos.board[square as usize].expect("cloak target").id;
                ev.push(pos.add_effect(EffectKind::Invisible, id, u32::from(*plies)));
            }
            (Effect::Morph { plies, .. }, SkillTarget::Spawn { square, kind }) => {
                let piece = pos.board[square as usize].as_mut().expect("morph target");
                let id = piece.id;
                let mut orig = piece.kind;
                piece.kind = kind;
                // Morphing a morphed piece keeps the type it finally goes back to.
                if let Some(old) = pos
                    .effects
                    .iter()
                    .find(|e| e.piece == id && e.kind == EffectKind::Morphed)
                {
                    orig = old.orig_kind.unwrap_or(orig);
                }
                pos.effects
                    .retain(|e| !(e.piece == id && e.kind == EffectKind::Morphed));
                if kind != orig {
                    let mut morphed =
                        ActiveEffect::new(EffectKind::Morphed, id, pos.ply + u32::from(*plies));
                    morphed.orig_kind = Some(orig);
                    pos.effects.push(morphed);
                }
                ev.push(Event::Transformed { square, kind });
            }
            (Effect::Promote, SkillTarget::Piece { square }) => {
                let piece = pos.board[square as usize].as_mut().expect("promote target");
                piece.kind = PieceKind::Queen;
                let id = piece.id;
                pos.effects
                    .retain(|e| !(e.piece == id && e.kind == EffectKind::Morphed));
                ev.push(Event::Transformed {
                    square,
                    kind: PieceKind::Queen,
                });
            }
            (Effect::Remove { .. }, SkillTarget::Piece { square }) => {
                let piece = pos.board[square as usize].take().expect("remove target");
                pos.effects.retain(|e| e.piece != piece.id);
                ev.push(Event::Removed { square, piece });
            }
            (Effect::Convert, SkillTarget::Piece { square }) => {
                let piece = pos.board[square as usize].as_mut().expect("convert target");
                piece.color = color;
                piece.prev = None;
                let piece = *piece;
                pos.effects
                    .retain(|e| e.piece != piece.id || e.kind == EffectKind::Vanish);
                ev.push(Event::Switched { square, piece });
            }
            (Effect::Teleport, SkillTarget::PieceTo { from, to }) => {
                let mut piece = pos.board[from as usize].take().expect("teleport source");
                piece.prev = None;
                pos.board[to as usize] = Some(piece);
                ev.push(Event::Teleported { from, to });
            }
            (Effect::Duplicate, SkillTarget::PieceTo { from, to }) => {
                let original = pos.board[from as usize].expect("duplicate source");
                let copy = Piece {
                    id: pos.alloc_id(),
                    prev: None,
                    ..original
                };
                pos.board[to as usize] = Some(copy);
                ev.push(Event::Cloned {
                    from,
                    to,
                    piece: copy,
                });
            }
            (Effect::Swap { .. }, SkillTarget::Pair { a, b }) => {
                pos.board.swap(a as usize, b as usize);
                for s in [a, b] {
                    if let Some(p) = pos.board[s as usize].as_mut() {
                        p.prev = None;
                    }
                }
                ev.push(Event::Swapped { a, b });
            }
            (Effect::Spawn { plies, .. }, SkillTarget::Spawn { square, kind }) => {
                let id = pos.alloc_id();
                let mut piece = Piece::new(id, kind, color, square);
                piece.temp = true;
                Self::place_new(pos, piece, ev);
                ev.push(pos.add_effect(EffectKind::Vanish, id, u32::from(*plies)));
            }
            (Effect::Revive { .. }, SkillTarget::Spawn { square, kind }) => {
                let Some(i) = first_grave(pos, color, kind) else {
                    return;
                };
                let grave = pos.graveyard.remove(i);
                if kind == PieceKind::Pawn {
                    let n = &mut pos.captured_pawns[color.index()];
                    *n = n.saturating_sub(1);
                }
                let id = pos.alloc_id();
                let mut piece = Piece::new(id, kind, color, grave.home);
                piece.home = grave.home;
                piece.prev = None;
                pos.board[square as usize] = Some(piece);
                ev.push(Event::Spawned { square, piece });
            }
            (Effect::Truce { plies }, SkillTarget::None) => {
                ev.push(pos.add_global_effect(EffectKind::Truce, None, u32::from(*plies)));
            }
            (Effect::Fog { plies }, SkillTarget::None) => {
                ev.push(pos.add_global_effect(EffectKind::Fog, None, u32::from(*plies)));
            }
            (Effect::Silence { plies }, SkillTarget::None) => {
                ev.push(pos.add_global_effect(
                    EffectKind::Silenced,
                    Some(color.opposite()),
                    u32::from(*plies),
                ));
            }
            (Effect::Ambush { plies }, SkillTarget::None) => {
                ev.push(pos.add_global_effect(EffectKind::Domain, Some(color), u32::from(*plies)));
            }
            (Effect::Mirror, SkillTarget::None) => {
                let moves = pos.mirror_armies();
                ev.push(Event::Rotated { moves });
            }
            // A target of the wrong shape is ignored, as in the built-in skills.
            _ => {}
        }
    }
}
