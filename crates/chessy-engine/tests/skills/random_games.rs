//! Property tests: random games with random decks must never panic, keep the
//! board consistent, and `legal_actions` must agree with `apply`.

use std::collections::HashSet;

use crate::common::*;

/// xorshift64*: tiny, deterministic, no dependency.
pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub(crate) fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

fn random_decks(rng: &mut Rng) -> ([Vec<SkillId>; 2], Vec<SkillId>) {
    let mut classic: Vec<SkillId> = SkillId::ALL
        .into_iter()
        .filter(|s| s.kind() == SkillKind::Classic)
        .collect();
    let mut unique: Vec<SkillId> = SkillId::ALL
        .into_iter()
        .filter(|s| s.kind() == SkillKind::Unique)
        .collect();
    rng.shuffle(&mut classic);
    rng.shuffle(&mut unique);
    let mut decks = [classic[..3].to_vec(), classic[3..6].to_vec()];
    // Unique skills live in one deck only.
    for (i, u) in unique.iter().take(rng.below(4)).enumerate() {
        decks[i % 2].push(*u);
    }
    let all = decks[0].iter().chain(decks[1].iter()).copied().collect();
    (decks, all)
}

pub(crate) fn check_invariants(g: &Game, context: &str) {
    let pos = &g.pos;
    for color in Color::BOTH {
        let kings = pos
            .pieces(color)
            .filter(|(_, p)| p.kind == PieceKind::King)
            .count();
        assert_eq!(kings, 1, "{color:?} kings: {context}\n{}", pos.to_fen());
    }
    assert!(
        !pos.in_check(pos.side.opposite()),
        "the side that is not to move is in check (its king could be taken): {context}\n{}",
        pos.to_fen()
    );
    let mut ids = HashSet::new();
    for (i, p) in pos.board.iter().enumerate() {
        let Some(p) = p else { continue };
        assert!(ids.insert(p.id), "duplicate id {}: {context}", p.id);
        if p.kind == PieceKind::Pawn {
            assert!(
                rank_of(i as Square) != p.color.promotion_rank(),
                "pawn on its promotion rank ({}): {context}\n{}",
                square_name(i as Square),
                pos.to_fen()
            );
        }
        assert!(!p.mirage || p.kind != PieceKind::King);
    }
    for b in &pos.benched {
        assert!(
            ids.insert(b.piece.id),
            "benched piece also on board: {context}"
        );
        assert_ne!(b.piece.kind, PieceKind::King);
    }
    for e in &pos.effects {
        if e.kind == EffectKind::Terrain {
            assert!(e.square.is_some() && e.owner.is_some(), "{context}");
            continue;
        }
        if e.kind.is_global() {
            assert_eq!(e.piece, NO_PIECE, "{context}");
            assert!(
                e.expires_at > pos.ply,
                "stale effect {:?}: {context}",
                e.kind
            );
            continue;
        }
        assert!(
            ids.contains(&e.piece),
            "{:?} effect on a missing piece {}: {context}\n{}",
            e.kind,
            e.piece,
            pos.to_fen()
        );
        assert!(
            e.expires_at > pos.ply,
            "stale effect {:?}: {context}",
            e.kind
        );
    }
    assert!(pos.traps.len() <= 4);
    for slot in g.loadouts.iter().flat_map(|l| &l.slots) {
        assert!(slot.uses <= slot.skill.max_uses());
        assert_eq!(slot.used, slot.uses >= slot.skill.max_uses());
    }
}

fn play_one(seed: u64) {
    let mut rng = Rng::new(seed);
    let (decks, _) = random_decks(&mut rng);
    let mut g = Game::new(&decks[0], &decks[1]);
    let mut log: Vec<String> = Vec::new();
    let context = |log: &Vec<String>| format!("seed {seed}, decks {decks:?}, played {log:?}");

    for _ in 0..90 {
        check_invariants(&g, &context(&log));
        let actions = g.legal_actions();
        if g.outcome().is_over() {
            assert!(actions.is_empty(), "{}", context(&log));
            return;
        }
        assert!(
            !actions.is_empty(),
            "ongoing game without actions: {}\n{}",
            context(&log),
            g.pos.to_fen()
        );

        // Every sampled legal action must apply, and every one must serialize.
        let mut sample: Vec<Action> = Vec::new();
        for _ in 0..6 {
            sample.push(actions[rng.below(actions.len())]);
        }
        for a in &sample {
            let mut copy = g.clone();
            copy.apply(*a)
                .unwrap_or_else(|e| panic!("legal action {a:?} refused ({e}): {}", context(&log)));
            let back: Action = serde_json::from_str(&serde_json::to_string(a).unwrap()).unwrap();
            assert_eq!(&back, a);
        }

        // Actions outside the list must be refused and change nothing.
        for _ in 0..4 {
            let id = SkillId::ALL[rng.below(SkillId::ALL.len())];
            let target = match rng.below(6) {
                0 => SkillTarget::None,
                1 => SkillTarget::Piece {
                    square: rng.below(64) as Square,
                },
                2 => SkillTarget::Square {
                    square: rng.below(64) as Square,
                },
                3 => SkillTarget::PieceTo {
                    from: rng.below(64) as Square,
                    to: rng.below(64) as Square,
                },
                4 => SkillTarget::Pair {
                    a: rng.below(64) as Square,
                    b: rng.below(64) as Square,
                },
                _ => SkillTarget::Spawn {
                    square: rng.below(64) as Square,
                    kind: PieceKind::PROMOTIONS[rng.below(4)],
                },
            };
            let action = Action::Skill { skill: id, target };
            let mut normalized = action;
            if let Action::Skill {
                target: SkillTarget::Pair { a, b },
                ..
            } = &mut normalized
            {
                let (lo, hi) = ((*a).min(*b), (*a).max(*b));
                *a = lo;
                *b = hi;
            }
            if actions.contains(&normalized) {
                continue;
            }
            let mut copy = g.clone();
            let fen = copy.pos.to_fen();
            assert!(
                copy.apply(action).is_err(),
                "{action:?} is not listed but was accepted: {}",
                context(&log)
            );
            assert_eq!(copy.pos.to_fen(), fen);
            assert_eq!(copy.pos.ply, g.pos.ply);
        }

        // Prefer skills now and then so that they all get exercised; Mind
        // Reading searches, so keep it rare.
        let skills: Vec<Action> = actions
            .iter()
            .copied()
            .filter(|a| {
                !matches!(
                    a,
                    Action::Skill {
                        skill: SkillId::Mind,
                        ..
                    }
                )
            })
            .filter(|a| matches!(a, Action::Skill { .. }))
            .collect();
        let chosen = if !skills.is_empty() && rng.chance(35) {
            skills[rng.below(skills.len())]
        } else {
            actions[rng.below(actions.len())]
        };
        let mover = g.side_to_move();
        let before_ply = g.pos.ply;
        g.apply(chosen)
            .unwrap_or_else(|e| panic!("{chosen:?}: {e}: {}", context(&log)));
        log.push(format!("{chosen:?}"));
        if g.pos.ply != before_ply {
            // The player who just acted never leaves their own king in check.
            assert!(
                !g.pos.in_check(mover) || g.outcome().is_over(),
                "{mover:?} left in check by {chosen:?}: {}\n{}",
                context(&log),
                g.pos.to_fen()
            );
        }
    }
    check_invariants(&g, &context(&log));
}

fn games() -> u64 {
    std::env::var("CHESSY_RANDOM_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150)
}

#[test]
fn random_games_with_random_decks_stay_consistent() {
    for seed in 0..games() {
        play_one(seed);
    }
}

#[test]
fn random_games_with_every_skill_in_both_decks() {
    // Both sides hold all the skills: stresses combinations (shared unique
    // skills are fine for the engine).
    let mut used: HashSet<SkillId> = HashSet::new();
    for seed in 1000..1000 + games() / 3 {
        let mut rng = Rng::new(seed);
        let all: Vec<SkillId> = SkillId::ALL
            .into_iter()
            .filter(|s| *s != SkillId::Mind)
            .collect();
        let mut g = Game::new(&all, &all);
        for _ in 0..80 {
            let actions = g.legal_actions();
            if g.outcome().is_over() {
                break;
            }
            assert!(!actions.is_empty(), "seed {seed}: {}", g.pos.to_fen());
            let skills: Vec<Action> = actions
                .iter()
                .copied()
                .filter(|a| matches!(a, Action::Skill { .. }))
                .collect();
            let chosen = if !skills.is_empty() && rng.chance(60) {
                skills[rng.below(skills.len())]
            } else {
                actions[rng.below(actions.len())]
            };
            if let Action::Skill { skill, .. } = chosen {
                used.insert(skill);
            }
            g.apply(chosen)
                .unwrap_or_else(|e| panic!("seed {seed}: {chosen:?}: {e}\n{}", g.pos.to_fen()));
            check_invariants(&g, &format!("seed {seed}"));
        }
    }
    // Rarely available skills (Queen Sacrifice needs a check and a queen) may be missing in a
    // short run; everything else must have been played.
    let missing: Vec<SkillId> = SkillId::ALL
        .into_iter()
        .filter(|s| !matches!(s, SkillId::Mind | SkillId::Queensac) && !used.contains(s))
        .collect();
    assert!(missing.is_empty(), "never played: {missing:?}");
}
