//! Forged skills: ids, definitions, the interpreter, and random games with
//! generated skills in the decks.

use std::sync::atomic::{AtomicU32, Ordering};

use chessy_engine::forge::registry;
use chessy_engine::forge::{Constraint, DefError, Effect, Side, SkillDef, SwapScope};
use chessy_engine::notation::skill_name;

use crate::common::*;
use crate::random_games::{check_invariants, Rng};

/// The registry is shared by every test of the process, so each registration
/// gets an id of its own.
static NEXT: AtomicU32 = AtomicU32::new(1_000_000);

fn forge(def: SkillDef) -> SkillId {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    registry::register(n, def).expect("valid definition");
    SkillId::Forged(n)
}

fn def(effect: Effect) -> SkillDef {
    SkillDef::new(effect)
}

// ---- ids -------------------------------------------------------------------

#[test]
fn forged_ids_serialize_as_forged_underscore_n() {
    let id = SkillId::Forged(12);
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"forged_12\"");
    let back: SkillId = serde_json::from_str("\"forged_12\"").unwrap();
    assert_eq!(back, id);
    assert_eq!(id.to_string(), "forged_12");
    assert_eq!(SkillId::Freeze.to_string(), "freeze");
}

#[test]
fn built_in_ids_keep_their_old_names() {
    for id in SkillId::ALL {
        let json = serde_json::to_string(&id).unwrap();
        let back: SkillId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
        assert_eq!(json, format!("\"{id}\""));
    }
}

#[test]
fn one_spelling_per_forged_id() {
    for bad in [
        "forged_",
        "forged_01",
        "forged_-1",
        "forged_+1",
        "forged_1x",
        "forged_99999999999",
        "Forged_1",
        "frozen",
        "",
    ] {
        assert_eq!(SkillId::parse(bad), None, "{bad:?}");
    }
    assert_eq!(SkillId::parse("forged_0"), Some(SkillId::Forged(0)));
    assert_eq!(
        SkillId::parse("forged_4294967295"),
        Some(SkillId::Forged(u32::MAX))
    );
}

// ---- definitions -----------------------------------------------------------

#[test]
fn definitions_round_trip_through_json() {
    let mut d = def(Effect::Morph {
        side: Side::Enemy,
        into: PieceKind::Pawn,
        plies: 4,
    });
    d.constraints = vec![Constraint::ForbidMate];
    d.max_uses = 2;
    let json = serde_json::to_string(&d).unwrap();
    assert_eq!(serde_json::from_str::<SkillDef>(&json).unwrap(), d);
    // Optional fields may be left out.
    let short: SkillDef =
        serde_json::from_str(r#"{"version":1,"effect":{"op":"promote"}}"#).unwrap();
    assert_eq!(short, def(Effect::Promote));
}

#[test]
fn invalid_definitions_are_refused() {
    let bad = |d: SkillDef| matches!(d.validate(), Err(DefError::Invalid(_)));
    assert!(bad(def(Effect::Freeze { plies: 1 })));
    assert!(bad(def(Effect::Freeze { plies: 9 })));
    assert!(bad(def(Effect::Remove { kinds: vec![] })));
    assert!(bad(def(Effect::Revive {
        kinds: vec![PieceKind::King]
    })));
    assert!(bad(def(Effect::Morph {
        side: Side::Own,
        into: PieceKind::King,
        plies: 4
    })));
    let mut d = def(Effect::Promote);
    d.version = 2;
    assert!(bad(d));
    let mut d = def(Effect::Promote);
    d.max_uses = 0;
    assert!(bad(d));
    let mut d = def(Effect::Promote);
    d.max_uses = 4;
    assert!(bad(d));
    assert!(def(Effect::Freeze { plies: 4 }).validate().is_ok());
}

#[test]
fn the_fingerprint_ignores_the_order_of_lists() {
    let a = def(Effect::Remove {
        kinds: vec![PieceKind::Rook, PieceKind::Pawn, PieceKind::Pawn],
    });
    let b = def(Effect::Remove {
        kinds: vec![PieceKind::Pawn, PieceKind::Rook],
    });
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_ne!(
        a.fingerprint(),
        def(Effect::Remove {
            kinds: vec![PieceKind::Pawn]
        })
        .fingerprint()
    );
}

#[test]
fn registering_twice_is_fine_but_a_different_definition_conflicts() {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let d = def(Effect::Promote);
    registry::register(n, d.clone()).unwrap();
    registry::register(n, d).unwrap();
    assert_eq!(
        registry::register(n, def(Effect::Convert)),
        Err(DefError::Conflict(n))
    );
    assert!(registry::is_registered(n));
}

#[test]
fn an_unregistered_forged_skill_can_never_be_played() {
    let id = SkillId::Forged(u32::MAX - 7);
    let g = game(KINGS, &[id], &[]);
    assert!(!has_skill(&g, id));
    assert_eq!(skill_name(id), "Unknown skill");
}

#[test]
fn a_forged_skill_has_a_name_for_the_notation() {
    let id = forge(def(Effect::Promote));
    assert!(!skill_name(id).is_empty());
}

// ---- behaviour -------------------------------------------------------------

#[test]
fn freeze_stops_an_enemy_piece_for_two_of_its_turns() {
    let id = forge(def(Effect::Freeze { plies: 4 }));
    let mut g = game("4k3/8/8/8/3r4/8/8/4K3 w - - 0 1", &[id], &[]);
    assert_eq!(
        targets_of(&g, id),
        vec![piece("d4")],
        "the enemy king is not a target, and neither are your own pieces"
    );
    use_skill(&mut g, id, piece("d4"));
    assert!(effects_on(&g, "d4").contains(&EffectKind::Frozen));
    assert!(moves_from(&g, "d4").is_empty(), "frozen on its first turn");
    mv(&mut g, "e8", "d8");
    mv(&mut g, "e1", "e2");
    assert!(moves_from(&g, "d4").is_empty(), "frozen on its second turn");
    mv(&mut g, "d8", "e8");
    assert!(!effects_on(&g, "d4").contains(&EffectKind::Frozen));
    assert!(!moves_from(&g, "d4").is_empty() || g.side_to_move() == Color::White);
    assert!(slot(&g, Color::White, id).used);
}

#[test]
fn shield_and_cloak_target_your_own_pieces_only() {
    let shield = forge(def(Effect::Shield { plies: 2 }));
    let cloak = forge(def(Effect::Cloak { plies: 4 }));
    let mut g = game("4k3/8/8/8/3r4/8/3N4/4K3 w - - 0 1", &[shield, cloak], &[]);
    assert_eq!(targets_of(&g, shield), vec![piece("d2")]);
    assert_eq!(targets_of(&g, cloak), vec![piece("d2")]);
    use_skill(&mut g, shield, piece("d2"));
    assert!(effects_on(&g, "d2").contains(&EffectKind::Immune));
    assert!(g.pos.is_immune(at(&g, "d2").unwrap().id));
}

#[test]
fn promote_makes_a_queen_for_good() {
    let id = forge(def(Effect::Promote));
    let mut g = game("4k3/8/8/8/8/8/3N1Q2/4K3 w - - 0 1", &[id], &[]);
    assert_eq!(
        targets_of(&g, id),
        vec![piece("d2")],
        "not the king or the queen"
    );
    use_skill(&mut g, id, piece("d2"));
    assert_eq!(kind_at(&g, "d2"), Some((Color::White, PieceKind::Queen)));
}

#[test]
fn remove_only_takes_the_listed_kinds() {
    let id = forge(def(Effect::Remove {
        kinds: vec![PieceKind::Pawn, PieceKind::Rook],
    }));
    let mut g = game("4k3/3p4/8/2n5/8/8/8/R3K3 w - - 0 1", &[id], &[]);
    assert_eq!(targets_of(&g, id), vec![piece("d7")]);
    use_skill(&mut g, id, piece("d7"));
    assert!(at(&g, "d7").is_none());
    assert!(
        g.pos.graveyard.iter().all(|p| p.color != Color::Black),
        "a removed piece is gone, not dead"
    );
}

#[test]
fn forbid_mate_refuses_a_removal_that_checkmates() {
    let mut d = def(Effect::Remove {
        kinds: vec![PieceKind::Pawn],
    });
    d.constraints = vec![Constraint::ForbidMate];
    let strict = forge(d);
    let loose = forge(def(Effect::Remove {
        kinds: vec![PieceKind::Pawn],
    }));
    // The pawn on e7 is all that shields the boxed-in black king from Re1.
    let fen = "3rkr2/3ppp2/8/8/8/8/8/K3R3 w - - 0 1";
    let g = game(fen, &[strict, loose], &[]);
    assert!(!can_skill(&g, strict, piece("e7")), "that would be mate");
    assert!(can_skill(&g, strict, piece("d7")));
    assert!(
        can_skill(&g, loose, piece("e7")),
        "allowed without the constraint"
    );
}

#[test]
fn convert_takes_a_harmless_enemy_piece_for_good() {
    let id = forge(def(Effect::Convert));
    let mut g = game("4k3/8/8/8/8/2n5/8/R3K3 w - - 0 1", &[id], &[]);
    assert_eq!(targets_of(&g, id), vec![piece("c3")]);
    use_skill(&mut g, id, piece("c3"));
    assert_eq!(kind_at(&g, "c3"), Some((Color::White, PieceKind::Knight)));
}

#[test]
fn convert_skips_pieces_that_attack_yours() {
    let id = forge(def(Effect::Convert));
    // The knight on b3 attacks the rook on a1.
    let g = game("4k3/8/8/8/8/1n6/8/R3K3 w - - 0 1", &[id], &[]);
    assert!(!has_skill(&g, id));
}

#[test]
fn teleport_and_duplicate_never_move_the_king() {
    let tp = forge(def(Effect::Teleport));
    let dup = forge(def(Effect::Duplicate));
    let g = game("4k3/8/8/8/8/8/8/R3K3 w - - 0 1", &[tp, dup], &[]);
    for id in [tp, dup] {
        assert!(!targets_of(&g, id)
            .iter()
            .any(|t| matches!(t, SkillTarget::PieceTo { from, .. } if *from == s("e1"))));
    }
    assert!(can_skill(&g, tp, piece_to("a1", "h8")));
    assert!(can_skill(&g, dup, piece_to("a1", "a2")));
    assert!(!can_skill(&g, dup, piece_to("a1", "h8")));
}

#[test]
fn swap_any_can_trade_pieces_between_the_sides() {
    let own = forge(def(Effect::Swap {
        scope: SwapScope::Own,
    }));
    let any = forge(def(Effect::Swap {
        scope: SwapScope::Any,
    }));
    let mut g = game("4k3/8/8/2r5/8/8/3N4/R3K3 w - - 0 1", &[own, any], &[]);
    assert!(can_skill(&g, own, pair("a1", "d2")));
    assert!(!can_skill(&g, own, pair("d2", "c5")), "own pieces only");
    assert!(can_skill(&g, any, pair("d2", "c5")));
    // The black rook would land on a1 and check the white king.
    assert!(!can_skill(&g, any, pair("a1", "c5")));
    use_skill(&mut g, any, pair("d2", "c5"));
    assert_eq!(kind_at(&g, "c5"), Some((Color::White, PieceKind::Knight)));
    assert_eq!(kind_at(&g, "d2"), Some((Color::Black, PieceKind::Rook)));
}

#[test]
fn spawn_makes_a_temporary_piece_that_vanishes() {
    let id = forge(def(Effect::Spawn {
        kinds: vec![PieceKind::Knight],
        plies: 4,
    }));
    let mut g = game("4k3/p7/8/8/8/8/P7/4K3 w - - 0 1", &[id], &[]);
    use_skill(&mut g, id, spawn("d4", PieceKind::Knight));
    assert!(at(&g, "d4").is_some_and(|p| p.temp));
    mv(&mut g, "e8", "d8");
    mv(&mut g, "e1", "e2");
    assert!(at(&g, "d4").is_some(), "still there after a turn of each");
    mv(&mut g, "d8", "e8");
    assert!(at(&g, "d4").is_none(), "gone four plies after it came");
}

#[test]
fn only_in_check_is_respected() {
    let mut d = def(Effect::Freeze { plies: 4 });
    d.constraints = vec![Constraint::OnlyInCheck];
    let id = forge(d);
    // The rook on a8 is not giving check: the skill is not available.
    let calm = game("r3k3/8/8/8/8/8/8/4K3 w - - 0 1", &[id], &[]);
    assert!(!has_skill(&calm, id));
    // The rook on a1 gives check; freezing it answers the check.
    let checked = game("4k3/8/8/8/8/8/8/r3K3 w - - 0 1", &[id], &[]);
    assert_eq!(targets_of(&checked, id), vec![piece("a1")]);
}

#[test]
fn a_free_action_keeps_the_turn() {
    let mut d = def(Effect::Shield { plies: 2 });
    d.free_action = true;
    let id = forge(d);
    let mut g = game("4k3/8/8/8/8/8/3N4/4K3 w - - 0 1", &[id], &[]);
    use_skill(&mut g, id, piece("d2"));
    assert_eq!(g.side_to_move(), Color::White);
}

#[test]
fn max_uses_is_honoured() {
    let mut d = def(Effect::Shield { plies: 2 });
    d.max_uses = 2;
    d.free_action = true;
    let id = forge(d);
    let mut g = game("4k3/8/8/8/8/8/3N4/4K3 w - - 0 1", &[id], &[]);
    use_skill(&mut g, id, piece("d2"));
    assert!(!slot(&g, Color::White, id).used);
    use_skill(&mut g, id, piece("d2"));
    assert!(slot(&g, Color::White, id).used);
    assert!(!has_skill(&g, id));
}

#[test]
fn a_unique_definition_is_a_unique_skill() {
    let mut d = def(Effect::Promote);
    d.unique = true;
    assert_eq!(forge(d).kind(), SkillKind::Unique);
    assert_eq!(forge(def(Effect::Promote)).kind(), SkillKind::Classic);
}

// ---- graveyard and revive ---------------------------------------------------

#[test]
fn captured_pieces_go_to_the_graveyard() {
    let mut g = game("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", &[], &[]);
    mv(&mut g, "e4", "d5");
    assert_eq!(g.pos.graveyard.len(), 1);
    let dead = g.pos.graveyard[0];
    assert_eq!((dead.color, dead.kind), (Color::Black, PieceKind::Pawn));
}

#[test]
fn a_temporary_piece_leaves_no_grave() {
    let id = forge(def(Effect::Spawn {
        kinds: vec![PieceKind::Knight],
        plies: 8,
    }));
    let mut g = game("4k3/8/8/8/8/2n5/8/4K3 w - - 0 1", &[id], &[]);
    use_skill(&mut g, id, spawn("e4", PieceKind::Knight));
    mv(&mut g, "c3", "e4");
    assert!(g.pos.graveyard.is_empty());
}

#[test]
fn revive_brings_a_piece_back_on_its_home_square() {
    let id = forge(def(Effect::Revive {
        kinds: vec![PieceKind::Knight, PieceKind::Rook],
    }));
    // White's knight from b1 was lost on d4.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let id_n = pos.alloc_id();
    pos.graveyard
        .push(Piece::new(id_n, PieceKind::Knight, Color::White, s("b1")));
    let mut g = Game::from_position(pos, &[id], &[]);
    assert_eq!(targets_of(&g, id), vec![spawn("b1", PieceKind::Knight)]);
    use_skill(&mut g, id, spawn("b1", PieceKind::Knight));
    assert_eq!(kind_at(&g, "b1"), Some((Color::White, PieceKind::Knight)));
    assert!(g.pos.graveyard.is_empty());
}

#[test]
fn revive_uses_the_nearest_free_square_when_home_is_taken() {
    let id = forge(def(Effect::Revive {
        kinds: vec![PieceKind::Bishop],
    }));
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/2B1K3 w - - 0 1").unwrap();
    let dead = pos.alloc_id();
    pos.graveyard
        .push(Piece::new(dead, PieceKind::Bishop, Color::White, s("c1")));
    let g = Game::from_position(pos, &[id], &[]);
    let t = targets_of(&g, id);
    assert_eq!(t.len(), 1);
    let SkillTarget::Spawn { square, kind } = t[0] else {
        panic!("{t:?}")
    };
    assert_eq!(kind, PieceKind::Bishop);
    assert_ne!(square, s("c1"));
    assert!(chessy_engine::position::king_distance(square, s("c1")) == 1);
}

#[test]
fn revive_only_offers_listed_kinds_of_your_own_color() {
    let id = forge(def(Effect::Revive {
        kinds: vec![PieceKind::Queen],
    }));
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let a = pos.alloc_id();
    let b = pos.alloc_id();
    pos.graveyard
        .push(Piece::new(a, PieceKind::Rook, Color::White, s("a1")));
    pos.graveyard
        .push(Piece::new(b, PieceKind::Queen, Color::Black, s("d8")));
    let g = Game::from_position(pos, &[id], &[]);
    assert!(!has_skill(&g, id), "no white queen is dead");
}

#[test]
fn a_revived_pawn_is_no_longer_owed_by_wall() {
    let id = forge(def(Effect::Revive {
        kinds: vec![PieceKind::Pawn],
    }));
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let dead = pos.alloc_id();
    pos.graveyard
        .push(Piece::new(dead, PieceKind::Pawn, Color::White, s("e2")));
    pos.captured_pawns = [1, 0];
    let mut g = Game::from_position(pos, &[id, SkillId::Wall], &[]);
    use_skill(&mut g, id, spawn("e2", PieceKind::Pawn));
    assert_eq!(g.pos.captured_pawns, [0, 0]);
    assert!(g.pos.graveyard.is_empty());
}

#[test]
fn wall_pawns_leave_the_graveyard() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/PPPP1PPP/4K3 w - - 0 1").unwrap();
    let dead = pos.alloc_id();
    pos.graveyard
        .push(Piece::new(dead, PieceKind::Pawn, Color::White, s("e2")));
    pos.captured_pawns = [1, 0];
    let mut g = Game::from_position(pos, &[SkillId::Wall], &[]);
    use_skill(&mut g, SkillId::Wall, none());
    assert!(g.pos.graveyard.is_empty());
}

#[test]
fn a_morphed_piece_is_buried_as_what_it_really_was() {
    // Black's queen is morphed into a pawn, then taken: the grave holds a queen.
    let mut g = game("4k3/8/8/3q4/4P3/8/8/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    use_skill(&mut g, SkillId::Morph, spawn("d5", PieceKind::Pawn));
    mv(&mut g, "e8", "e7");
    mv(&mut g, "e4", "d5");
    assert_eq!(g.pos.graveyard.len(), 1);
    assert_eq!(g.pos.graveyard[0].kind, PieceKind::Queen);
}

// ---- tone atoms ------------------------------------------------------------

#[test]
fn a_domain_strikes_the_piece_that_checks_the_caster() {
    let id = forge(def(Effect::Ambush { plies: 4 }));
    // Black's rook is not giving check yet; it will once it reaches e-file.
    let mut g = game("r3k3/8/8/8/8/8/8/4K2R w - - 0 1", &[id], &[]);
    use_skill(&mut g, id, none());
    assert!(g.pos.has_domain(Color::White));
    let ev = mv(&mut g, "a8", "a1");
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::Ambushed { square, .. } if *square == s("a1"))));
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::Captured { square, .. } if *square == s("a1"))));
    assert_eq!(kind_at(&g, "a1"), None, "the checking rook is gone");
    assert!(!g.pos.in_check(Color::White));
    assert!(!g.pos.has_domain(Color::White), "the domain is spent");
    assert_eq!(g.pos.captured_pawns, [0, 0]);
}

#[test]
fn a_domain_does_nothing_without_a_check_and_runs_out() {
    let id = forge(def(Effect::Ambush { plies: 2 }));
    let mut g = game("r3k3/8/8/8/8/8/8/4K2R w - - 0 1", &[id], &[]);
    use_skill(&mut g, id, none());
    mv(&mut g, "a8", "a7");
    assert_eq!(kind_at(&g, "a7"), Some((Color::Black, PieceKind::Rook)));
    mv(&mut g, "h1", "h2");
    mv(&mut g, "a7", "a1");
    assert_eq!(kind_at(&g, "a1"), Some((Color::Black, PieceKind::Rook)));
    assert!(g.pos.in_check(Color::White), "too late, the domain ended");
}

#[test]
fn a_domain_spares_an_immune_checker_and_cannot_be_stacked() {
    let id = forge(def(Effect::Ambush { plies: 6 }));
    let mut g = game("r3k3/8/8/8/8/8/8/4K2R w - - 0 1", &[id, id], &[]);
    use_skill(&mut g, id, none());
    assert!(!has_skill(&g, id), "already under a domain");
    let rook = at(&g, "a8").unwrap().id;
    g.pos
        .effects
        .push(ActiveEffect::new(EffectKind::Immune, rook, 100));
    mv(&mut g, "a8", "a1");
    assert_eq!(kind_at(&g, "a1"), Some((Color::Black, PieceKind::Rook)));
}

#[test]
fn truce_forbids_captures_and_check_until_it_ends() {
    let id = forge(def(Effect::Truce { plies: 4 }));
    // White's rook could take black's on a8 and black's rook is giving check.
    let mut g = game("r3k3/8/8/8/8/8/8/R3K3 w - - 0 1", &[id], &[]);
    assert!(can_move(&g, "a1", "a8"));
    let ev = use_skill(&mut g, id, none());
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::GlobalEffect {
            effect: EffectKind::Truce,
            ..
        }
    )));
    assert!(g.pos.truce());
    assert!(!can_move(&g, "a8", "a1"), "no captures for black");
    assert!(!g.pos.in_check(Color::White));
    mv(&mut g, "a8", "a7");
    assert!(!can_move(&g, "a1", "a7"), "no captures for white either");
    mv(&mut g, "a1", "a2");
    mv(&mut g, "a7", "a6");
    assert!(!g.pos.truce(), "over after four plies");
}

#[test]
fn truce_can_answer_a_check() {
    let id = forge(def(Effect::Truce { plies: 2 }));
    let g = game("4k3/8/8/8/8/8/8/r3K3 w - - 0 1", &[id], &[]);
    assert!(g.pos.in_check(Color::White));
    assert!(can_skill(&g, id, none()));
}

#[test]
fn truce_does_not_stack() {
    let id = forge(def(Effect::Truce { plies: 4 }));
    let mut g = game("r3k3/8/8/8/8/8/8/R3K3 w - - 0 1", &[id, id], &[id]);
    use_skill(&mut g, id, none());
    assert!(!has_skill(&g, id), "black cannot start a second armistice");
}

#[test]
fn mirror_swaps_the_armies() {
    let id = forge(def(Effect::Mirror));
    let mut g = game(
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        &[],
        &[id],
    );
    let ev = use_skill(&mut g, id, none());
    assert!(ev.iter().any(|e| matches!(e, Event::Rotated { .. })));
    // White's e4 pawn is now a black pawn on e5; black's army is white and at the bottom.
    assert_eq!(kind_at(&g, "e5"), Some((Color::Black, PieceKind::Pawn)));
    assert_eq!(kind_at(&g, "e1"), Some((Color::White, PieceKind::King)));
    assert_eq!(kind_at(&g, "d1"), Some((Color::White, PieceKind::Queen)));
    assert_eq!(kind_at(&g, "e8"), Some((Color::Black, PieceKind::King)));
    assert_eq!(kind_at(&g, "e2"), Some((Color::White, PieceKind::Pawn)));
    assert_eq!(g.pos.en_passant, None);
    assert_eq!(g.side_to_move(), Color::White);
    // Both sides keep their castling rights: the armies kept theirs.
    assert_eq!(g.pos.castling, 0xF);
}

#[test]
fn mirror_keeps_one_king_per_side_and_the_pieces_ids() {
    let id = forge(def(Effect::Mirror));
    let mut g = game("4k3/8/8/3p4/4P3/8/8/R3K3 w - - 0 1", &[id], &[]);
    let ids: Vec<PieceId> = g.pos.board.iter().flatten().map(|p| p.id).collect();
    use_skill(&mut g, id, none());
    let mut after: Vec<PieceId> = g.pos.board.iter().flatten().map(|p| p.id).collect();
    let mut before = ids;
    before.sort();
    after.sort();
    assert_eq!(before, after);
    for c in Color::BOTH {
        assert!(g.pos.king_square(c).is_some());
    }
}

#[test]
fn mirror_is_refused_when_it_leaves_you_in_check() {
    let id = forge(def(Effect::Mirror));
    // The white rook on e4 becomes a black rook on e5, and the black king on e8
    // becomes the white king on e1: the rook would give check down the e-file.
    let g = game("4k3/8/8/8/4R3/8/8/4K3 w - - 0 1", &[id], &[]);
    assert!(!can_skill(&g, id, none()));
}

#[test]
fn silence_stops_every_skill_of_the_opponent() {
    let id = forge(def(Effect::Silence { plies: 4 }));
    let mut g = game(
        "4k3/8/8/8/8/8/3N4/4K3 w - - 0 1",
        &[id],
        &[SkillId::Freeze, SkillId::Imune],
    );
    use_skill(&mut g, id, none());
    assert!(g.pos.is_silenced(Color::Black));
    assert!(!g.pos.is_silenced(Color::White));
    assert!(g
        .legal_actions()
        .iter()
        .all(|a| matches!(a, Action::Move { .. })));
    assert!(skill_fails(&mut g, SkillId::Imune, piece("e8")));
    mv(&mut g, "e8", "d8");
    mv(&mut g, "e1", "e2");
    mv(&mut g, "d8", "e8");
    assert!(!g.pos.is_silenced(Color::Black));
    assert!(has_skill(&g, SkillId::Freeze) || g.side_to_move() == Color::White);
}

#[test]
fn fog_is_a_global_effect_and_does_not_stack() {
    let id = forge(def(Effect::Fog { plies: 6 }));
    let mut g = game("4k3/8/8/8/8/8/3N4/4K3 w - - 0 1", &[id, id], &[id]);
    use_skill(&mut g, id, none());
    assert!(g.pos.fog());
    assert!(!has_skill(&g, id));
    let fog = g
        .pos
        .effects
        .iter()
        .find(|e| e.kind == EffectKind::Fog)
        .expect("fog effect");
    assert_eq!(fog.piece, NO_PIECE);
}

#[test]
fn tone_atoms_are_the_ones_that_rewrite_a_game() {
    assert!(Effect::Mirror.is_tone());
    assert!(Effect::Truce { plies: 2 }.is_tone());
    assert!(Effect::Revive {
        kinds: vec![PieceKind::Pawn]
    }
    .is_tone());
    assert!(!Effect::Freeze { plies: 2 }.is_tone());
    assert!(!Effect::Teleport.is_tone());
}

// ---- identity ---------------------------------------------------------------

#[test]
fn every_effect_has_a_description_a_glyph_and_a_family() {
    use chessy_engine::forge::generate::random_def;
    use chessy_engine::forge::identity::{identity, GLYPHS};
    let mut rng = chessy_engine::ai::Rng::new(11);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..2000 {
        let d = random_def(&mut rng);
        let id = identity(&d);
        seen.insert(std::mem::discriminant(&d.effect));
        assert!(id.description.ends_with('.'), "{}", id.description);
        assert!(!id.description.contains('{'), "{}", id.description);
        assert!(!id.name.is_empty());
        assert!(!id.name.contains("  "), "{}", id.name);
        assert!(GLYPHS.contains(&id.icon.glyph.as_str()));
        assert!(id.sound.degree < 7 && id.sound.timbre < 4 && id.sound.length < 3);
    }
    assert_eq!(seen.len(), 17, "the generator draws every effect");
}

#[test]
fn a_description_says_what_the_skill_does() {
    use chessy_engine::forge::identity::identity;
    let mut d = def(Effect::Freeze { plies: 4 });
    d.constraints = vec![Constraint::OnlyInCheck, Constraint::ForbidMate];
    d.free_action = true;
    d.max_uses = 2;
    let text = identity(&d).description;
    assert!(text.contains("Immobilise"), "{text}");
    assert!(text.contains("4 coups (2 de chaque camp)"), "{text}");
    assert!(text.contains("en échec"), "{text}");
    assert!(text.contains("échec et mat"), "{text}");
    assert!(text.contains("Ne consomme pas ton tour"), "{text}");
    assert!(text.contains("2 fois"), "{text}");
    let revive = identity(&def(Effect::Revive {
        kinds: vec![PieceKind::Knight, PieceKind::Queen],
    }))
    .description;
    assert!(revive.contains("cavaliers ou dames"), "{revive}");
}

#[test]
fn names_are_stable_and_nearly_all_different() {
    use chessy_engine::forge::generate::random_def;
    use chessy_engine::forge::identity::identity;
    let mut rng = chessy_engine::ai::Rng::new(5);
    let mut defs = std::collections::HashMap::new();
    for _ in 0..5000 {
        let d = random_def(&mut rng);
        defs.insert(d.fingerprint(), d);
    }
    let mut names = std::collections::HashSet::new();
    for d in defs.values() {
        assert_eq!(identity(d).name, identity(d).name);
        names.insert(identity(d).name);
    }
    let ratio = names.len() as f64 / defs.len() as f64;
    assert!(
        ratio > 0.99,
        "{} names for {} skills",
        names.len(),
        defs.len()
    );
}

// ---- rarity ------------------------------------------------------------------

#[test]
fn the_calibration_is_sane() {
    use chessy_engine::forge::generate::calibration;
    let t = calibration().thresholds;
    assert!(0.0 < t.uncommon && t.uncommon < t.rare && t.rare < t.epic && t.epic < t.legendary);
    assert!(t.legendary < 100.0);
}

#[test]
fn redundant_combinations_fall_to_common() {
    use chessy_engine::forge::generate::calibration;
    use chessy_engine::forge::measure::measure;
    use chessy_engine::forge::rarity::grade;
    let t = calibration().thresholds;
    let d = def(Effect::Mirror);
    let m = measure(&d, 8);
    let fresh = grade(&d, &m, &t, false);
    let copy = grade(&d, &m, &t, true);
    assert_eq!(copy.rarity, chessy_engine::forge::Rarity::Common);
    assert!(copy.redundant && !fresh.redundant);
    assert_eq!(
        copy.score, fresh.score,
        "redundancy changes the tier, not the score"
    );
}

#[test]
fn only_tone_atoms_can_be_legendary() {
    use chessy_engine::forge::measure::measure;
    use chessy_engine::forge::rarity::{grade, Thresholds};
    // Thresholds so low that anything would be Legendary.
    let low = Thresholds {
        uncommon: 0.0,
        rare: 0.0,
        epic: 0.0,
        legendary: 0.0,
    };
    let plain = def(Effect::Teleport);
    let tone = def(Effect::Mirror);
    let g_plain = grade(&plain, &measure(&plain, 4), &low, false);
    let g_tone = grade(&tone, &measure(&tone, 4), &low, false);
    assert_eq!(g_plain.rarity, chessy_engine::forge::Rarity::Epic);
    assert_eq!(g_tone.rarity, chessy_engine::forge::Rarity::Legendary);
}

#[test]
fn the_signature_ignores_details_that_do_not_change_how_it_plays() {
    let a = def(Effect::Freeze { plies: 4 });
    let b = def(Effect::Freeze { plies: 5 });
    let c = def(Effect::Freeze { plies: 8 });
    assert_eq!(a.signature(), b.signature(), "same duration bucket");
    assert_ne!(a.signature(), c.signature());
    let pawns = def(Effect::Remove {
        kinds: vec![PieceKind::Pawn],
    });
    let more = def(Effect::Remove {
        kinds: vec![PieceKind::Pawn, PieceKind::Pawn],
    });
    assert_eq!(pawns.signature(), more.signature());
    let mut free = def(Effect::Freeze { plies: 4 });
    free.free_action = true;
    assert_ne!(a.signature(), free.signature());
}

#[test]
fn measuring_is_deterministic() {
    use chessy_engine::forge::measure::measure;
    let d = def(Effect::Convert);
    assert_eq!(measure(&d, 6), measure(&d, 6));
}

#[test]
fn forging_is_deterministic_and_lands_on_the_target_often_enough() {
    use chessy_engine::forge::generate::{forge, Budget};
    use chessy_engine::forge::Rarity;
    let budget = Budget {
        attempts: 25,
        positions: 6,
    };
    let known = std::collections::HashSet::new();
    let run = |seed| {
        let mut rng = chessy_engine::ai::Rng::new(seed);
        forge(&mut rng, Rarity::Rare, &known, budget)
    };
    assert_eq!(run(3), run(3));
    let hits = (0..12)
        .filter(|&s| run(100 + s).graded.rarity == Rarity::Rare)
        .count();
    assert!(hits >= 6, "only {hits} of 12 forges were Rare");
}

#[test]
fn forging_avoids_signatures_already_in_the_world() {
    use chessy_engine::forge::generate::{forge, Budget};
    use chessy_engine::forge::Rarity;
    let budget = Budget {
        attempts: 25,
        positions: 6,
    };
    let mut known = std::collections::HashSet::new();
    let mut rng = chessy_engine::ai::Rng::new(9);
    for _ in 0..8 {
        let f = forge(&mut rng, Rarity::Uncommon, &known, budget);
        assert!(!f.graded.redundant || f.graded.rarity == Rarity::Common);
        assert_eq!(f.def.unique, f.graded.rarity == Rarity::Legendary);
        known.insert(f.def.signature());
    }
}

// ---- random games ----------------------------------------------------------

fn random_kinds(rng: &mut Rng) -> Vec<PieceKind> {
    let all = [
        PieceKind::Pawn,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Rook,
        PieceKind::Queen,
    ];
    let mut out: Vec<PieceKind> = all.into_iter().filter(|_| rng.chance(50)).collect();
    if out.is_empty() {
        out.push(all[rng.below(all.len())]);
    }
    out
}

fn random_def(rng: &mut Rng) -> SkillDef {
    let plies = 2 + rng.below(7) as u8;
    let side = if rng.chance(50) {
        Side::Own
    } else {
        Side::Enemy
    };
    let effect = match rng.below(16) {
        0 => Effect::Freeze { plies },
        1 => Effect::Shield { plies },
        2 => Effect::Cloak { plies },
        3 => Effect::Morph {
            side,
            into: PieceKind::PROMOTIONS[rng.below(4)],
            plies,
        },
        4 => Effect::Promote,
        5 => Effect::Remove {
            kinds: random_kinds(rng),
        },
        6 => Effect::Convert,
        7 => Effect::Teleport,
        8 => Effect::Duplicate,
        9 => Effect::Swap {
            scope: if rng.chance(50) {
                SwapScope::Own
            } else {
                SwapScope::Any
            },
        },
        10 => Effect::Spawn {
            kinds: random_kinds(rng),
            plies,
        },
        11 => Effect::Revive {
            kinds: random_kinds(rng),
        },
        12 => Effect::Truce { plies },
        13 => Effect::Mirror,
        14 => Effect::Fog { plies },
        15 => Effect::Silence { plies },
        _ => Effect::Ambush { plies },
    };
    let mut d = SkillDef::new(effect);
    if rng.chance(25) {
        d.constraints.push(Constraint::ForbidMate);
    }
    if rng.chance(15) {
        d.constraints.push(Constraint::ForbidCheck);
    }
    if rng.chance(15) {
        d.max_uses = 1 + rng.below(3) as u8;
    }
    if rng.chance(15) {
        d.free_action = true;
    }
    d
}

#[test]
fn random_games_with_forged_skills_stay_consistent() {
    let games: u64 = std::env::var("CHESSY_RANDOM_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150);
    for seed in 5000..5000 + games {
        let mut rng = Rng::new(seed);
        let mut decks: [Vec<SkillId>; 2] = [Vec::new(), Vec::new()];
        for deck in &mut decks {
            for _ in 0..3 {
                deck.push(forge(random_def(&mut rng)));
            }
            // A built-in skill or two keeps the mix realistic.
            deck.push(SkillId::ALL[rng.below(SkillId::ALL.len())]);
        }
        let mut g = Game::new(&decks[0], &decks[1]);
        for _ in 0..90 {
            check_invariants(&g, &format!("seed {seed}, decks {decks:?}"));
            let actions = g.legal_actions();
            if g.outcome().is_over() {
                break;
            }
            assert!(!actions.is_empty(), "seed {seed}: {}", g.pos.to_fen());
            let skills: Vec<Action> = actions
                .iter()
                .copied()
                .filter(|a| matches!(a, Action::Skill { skill, .. } if *skill != SkillId::Mind))
                .collect();
            let chosen = if !skills.is_empty() && rng.chance(50) {
                skills[rng.below(skills.len())]
            } else {
                actions[rng.below(actions.len())]
            };
            g.apply(chosen).unwrap_or_else(|e| {
                panic!("seed {seed}: {chosen:?}: {e}\n{}", g.pos.to_fen());
            });
        }
        check_invariants(&g, &format!("seed {seed} end"));
    }
}

// --- Icons: one layer per brick of the definition -------------------------

/// Every definition of a small but varied corner of the space: all effects, durations,
/// piece lists, constraint sets, uses and free/not free.
fn icon_space() -> Vec<SkillDef> {
    use chessy_engine::PieceKind::*;
    let mut effects = vec![
        Effect::Promote,
        Effect::Convert,
        Effect::Teleport,
        Effect::Duplicate,
        Effect::Mirror,
        Effect::Swap {
            scope: chessy_engine::forge::def::SwapScope::Own,
        },
        Effect::Swap {
            scope: chessy_engine::forge::def::SwapScope::Any,
        },
        Effect::Remove { kinds: vec![Pawn] },
        Effect::Remove {
            kinds: vec![Pawn, Knight],
        },
        Effect::Remove {
            kinds: vec![Knight, Bishop],
        },
        Effect::Revive {
            kinds: vec![Rook, Queen],
        },
    ];
    for plies in 2..=8 {
        effects.extend([
            Effect::Freeze { plies },
            Effect::Shield { plies },
            Effect::Cloak { plies },
            Effect::Truce { plies },
            Effect::Fog { plies },
            Effect::Silence { plies },
            Effect::Ambush { plies },
            Effect::Spawn {
                kinds: vec![Pawn, Queen],
                plies,
            },
        ]);
        for side in [
            chessy_engine::forge::def::Side::Own,
            chessy_engine::forge::def::Side::Enemy,
        ] {
            effects.push(Effect::Morph {
                side,
                into: Knight,
                plies,
            });
        }
    }
    let constraint_sets: [&[Constraint]; 5] = [
        &[],
        &[Constraint::OnlyInCheck],
        &[Constraint::ForbidMate],
        &[Constraint::ForbidCheck],
        &[Constraint::OnlyInCheck, Constraint::ForbidMate],
    ];
    let mut out = Vec::new();
    for effect in effects {
        for cs in constraint_sets {
            for max_uses in 1..=3 {
                for free_action in [false, true] {
                    let mut d = def(effect.clone());
                    d.constraints = cs.to_vec();
                    d.max_uses = max_uses;
                    d.free_action = free_action;
                    out.push(d);
                }
            }
        }
    }
    out
}

#[test]
fn two_skills_that_differ_in_any_brick_get_two_icons() {
    use chessy_engine::forge::identity::identity;
    let space = icon_space();
    let mut seen = std::collections::HashMap::new();
    for d in &space {
        let spec = identity(d).icon;
        let key = (d.clone().canonical().fingerprint(), spec.clone());
        if let Some(other) = seen.insert(serde_json::to_string(&spec).unwrap(), key.0) {
            assert_eq!(other, key.0, "two different skills share the icon {spec:?}");
        }
    }
    assert!(seen.len() > 1000, "{}", seen.len());
}

#[test]
fn two_freezes_no_longer_share_a_glyph_alone() {
    use chessy_engine::forge::identity::identity;
    let short = identity(&def(Effect::Freeze { plies: 2 })).icon;
    let long = identity(&def(Effect::Freeze { plies: 8 })).icon;
    assert_eq!(short.glyph, long.glyph);
    assert_ne!(short, long);
    assert_eq!((short.plies, long.plies), (Some(2), Some(8)));
    let mut gated = def(Effect::Freeze { plies: 2 });
    gated.constraints = vec![Constraint::OnlyInCheck];
    gated.free_action = true;
    gated.max_uses = 3;
    let icon = identity(&gated).icon;
    assert_eq!(icon.marks, ["in_check", "free"]);
    assert_eq!(icon.uses, 3);
    assert_eq!(icon.target.as_deref(), Some("enemy"));
}

#[test]
fn an_icon_stays_legible_and_follows_the_definition() {
    use chessy_engine::forge::identity::{identity, GLYPHS};
    for d in icon_space() {
        let icon = identity(&d).icon;
        assert!(GLYPHS.contains(&icon.glyph.as_str()));
        // Few enough marks to stay readable at 40px.
        assert!(icon.marks.len() <= 4 && (1..=3).contains(&icon.uses));
        assert!(matches!(
            icon.target.as_deref(),
            Some("own" | "enemy" | "any")
        ));
        if let Some(p) = icon.plies {
            assert!((2..=8).contains(&p));
            assert!(icon.badge.is_some());
        }
        // A permanent effect shows the infinity badge and no gauge.
        if icon.badge.as_deref() == Some("forever") {
            assert!(icon.plies.is_none());
        }
        // The silhouette is the strongest of the pieces named, and they are sorted weakest first.
        assert_eq!(icon.piece.as_deref(), icon.kinds.last().map(String::as_str));
        // Permutations and duplicates do not change the icon.
        assert_eq!(icon, identity(&d.clone().canonical()).icon);
    }
    let a = identity(&def(Effect::Remove {
        kinds: vec![PieceKind::Rook, PieceKind::Pawn, PieceKind::Rook],
    }))
    .icon;
    let b = identity(&def(Effect::Remove {
        kinds: vec![PieceKind::Pawn, PieceKind::Rook],
    }))
    .icon;
    assert_eq!(a, b);
    assert_eq!(a.kinds, ["pawn", "rook"]);
}
