//! One end-to-end test per hand-written skill: each plays the skill through
//! `Game::apply` in a small position and checks its effect, and when the effect
//! is temporary, that it ends when it should. `every_skill_has_a_test` fails
//! when a skill is added to `SkillId::ALL` without a test here.

use crate::common::*;

/// The skills tested below, in `SkillId::ALL` order.
const TESTED: [SkillId; 27] = [
    SkillId::Teleportation,
    SkillId::Imune,
    SkillId::Freeze,
    SkillId::Rollback,
    SkillId::Clone,
    SkillId::DestinySwapper,
    SkillId::Remover,
    SkillId::Wall,
    SkillId::Mirage,
    SkillId::Evolve,
    SkillId::Switch,
    SkillId::Mind,
    SkillId::Control,
    SkillId::Morph,
    SkillId::Canceller,
    SkillId::Tornado,
    SkillId::Invisibility,
    SkillId::Terminator,
    SkillId::Trap,
    SkillId::Bench,
    SkillId::Forcefield,
    SkillId::Transposition,
    SkillId::Queensac,
    SkillId::Temporal,
    SkillId::Geomancy,
    SkillId::Celestial,
    SkillId::Godhelp,
];

#[test]
fn every_skill_has_a_test() {
    assert_eq!(TESTED, SkillId::ALL);
}

/// The piece placement part of the FEN: the whole board, square by square.
fn board(g: &Game) -> String {
    let fen = g.pos.to_fen();
    fen.split(' ').next().unwrap().to_string()
}

/// The skill is spent and cannot be played a second time.
fn spent(g: &Game, color: Color, id: SkillId) {
    let sl = slot(g, color, id);
    assert!(sl.used, "{id:?} is spent");
    assert_eq!(sl.uses, id.max_uses());
}

#[test]
fn teleportation() {
    let mut g = start(&[SkillId::Teleportation], &[]);
    let id = at(&g, "b1").unwrap().id;
    let ev = use_skill(&mut g, SkillId::Teleportation, piece_to("b1", "e5"));
    assert_eq!(board(&g), "rnbqkbnr/pppppppp/8/4N3/8/8/PPPPPPPP/R1BQKBNR");
    assert!(at(&g, "b1").is_none());
    let p = at(&g, "e5").expect("the knight landed on e5");
    assert_eq!((p.id, p.kind), (id, PieceKind::Knight));
    assert!(ev.iter().any(|e| matches!(e, Event::Teleported { .. })));
    assert_eq!(g.side_to_move(), Color::Black);
    spent(&g, Color::White, SkillId::Teleportation);
}

#[test]
fn imune() {
    let mut g = game("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", &[SkillId::Imune], &[]);
    assert!(can_move(&g, "e4", "d5"), "white could take");
    use_skill(&mut g, SkillId::Imune, piece("e4"));
    assert_eq!(board(&g), "4k3/8/8/3p4/4P3/8/8/4K3");
    assert!(!can_move(&g, "d5", "e4"), "the pawn on e4 cannot be taken");
    mv(&mut g, "e8", "d8");
    mv(&mut g, "e1", "d1");
    assert!(can_move(&g, "d5", "e4"), "the protection has worn off");
}

#[test]
fn freeze() {
    let mut g = game("r3k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Freeze], &[]);
    use_skill(&mut g, SkillId::Freeze, piece("a8"));
    assert_eq!(board(&g), "r3k3/8/8/8/8/8/8/4K3");
    assert!(moves_from(&g, "a8").is_empty(), "frozen, black's 1st turn");
    mv(&mut g, "e8", "f8");
    mv(&mut g, "e1", "d1");
    assert!(moves_from(&g, "a8").is_empty(), "frozen, black's 2nd turn");
    mv(&mut g, "f8", "g8");
    mv(&mut g, "d1", "e1");
    assert!(!moves_from(&g, "a8").is_empty(), "free on the 3rd turn");
}

#[test]
fn rollback() {
    let mut g = start(&[SkillId::Rollback], &[]);
    mv(&mut g, "g1", "f3");
    mv(&mut g, "e7", "e5");
    let ev = use_skill(&mut g, SkillId::Rollback, piece("f3"));
    assert_eq!(board(&g), "rnbqkbnr/pppp1ppp/8/4p3/8/8/PPPPPPPP/RNBQKBNR");
    assert!(at(&g, "f3").is_none());
    assert_eq!(kind_at(&g, "g1"), Some((Color::White, PieceKind::Knight)));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::RolledBack { from, to } if *from == s("f3") && *to == s("g1")
    )));
    assert_eq!(g.side_to_move(), Color::Black);
}

#[test]
fn clone() {
    let mut g = game("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1", &[SkillId::Clone], &[]);
    let original = at(&g, "d4").unwrap();
    use_skill(&mut g, SkillId::Clone, piece_to("d4", "e5"));
    assert_eq!(board(&g), "4k3/8/8/4N3/3N4/8/8/4K3");
    let copy = at(&g, "e5").expect("a copy on e5");
    assert_eq!((copy.color, copy.kind), (Color::White, PieceKind::Knight));
    assert_ne!(copy.id, original.id, "a new piece");
    assert_eq!(at(&g, "d4"), Some(original), "the original stays");
    assert_eq!(count_pieces(&g, Color::White), 3);
    mv(&mut g, "e8", "d8");
    assert!(can_move(&g, "e5", "f7"), "the copy moves like a knight");
}

#[test]
fn destiny_swapper() {
    let mut g = start(&[SkillId::DestinySwapper], &[]);
    use_skill(&mut g, SkillId::DestinySwapper, pair("b1", "d1"));
    assert_eq!(board(&g), "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RQBNKBNR");
    assert_eq!(kind_at(&g, "b1"), Some((Color::White, PieceKind::Queen)));
    assert_eq!(kind_at(&g, "d1"), Some((Color::White, PieceKind::Knight)));
    assert_eq!(g.side_to_move(), Color::Black);
}

#[test]
fn remover() {
    let mut g = game("4k3/pp6/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Remover], &[]);
    assert_eq!(
        targets_of(&g, SkillId::Remover).len(),
        2,
        "enemy pawns only"
    );
    let ev = use_skill(&mut g, SkillId::Remover, piece("a7"));
    assert_eq!(board(&g), "4k3/1p6/8/8/8/8/8/4K3");
    assert!(at(&g, "a7").is_none());
    assert_eq!(count_pieces(&g, Color::Black), 2);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Removed { square, piece } if *square == s("a7") && piece.kind == PieceKind::Pawn
    )));
}

#[test]
fn wall() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    pos.captured_pawns = [3, 0];
    let mut g = Game::from_position(pos, &[SkillId::Wall], &[]);
    use_skill(&mut g, SkillId::Wall, none());
    assert_eq!(board(&g), "4k3/8/8/8/8/3PPP2/8/4K3");
    for sqr in ["d3", "e3", "f3"] {
        let p = at(&g, sqr).unwrap_or_else(|| panic!("a wall pawn on {sqr}"));
        assert_eq!(
            (p.color, p.kind, p.wall),
            (Color::White, PieceKind::Pawn, true)
        );
    }
    assert_eq!(g.pos.captured_pawns, [0, 0]);
    // Locked during the opponent's turn, free on the owner's next one.
    assert!(g.pos.is_frozen(at(&g, "e3").unwrap().id));
    mv(&mut g, "e8", "d8");
    assert!(can_move(&g, "e3", "e4"));
}

#[test]
fn mirage() {
    let mut g = game("4k3/8/8/8/r7/8/8/4K3 w - - 0 1", &[SkillId::Mirage], &[]);
    use_skill(&mut g, SkillId::Mirage, spawn("e4", PieceKind::Queen));
    assert_eq!(board(&g), "4k3/8/8/8/r3Q3/8/8/4K3");
    let fake = at(&g, "e4").expect("a fake queen on e4");
    assert_eq!(
        (fake.color, fake.kind, fake.mirage),
        (Color::White, PieceKind::Queen, true)
    );
    assert!(!g.pos.in_check(Color::Black), "a mirage attacks nothing");
    let ev = mv(&mut g, "a4", "e4");
    assert_eq!(board(&g), "4k3/8/8/8/4r3/8/8/4K3");
    assert_eq!(kind_at(&g, "e4"), Some((Color::Black, PieceKind::Rook)));
    assert_eq!(count_pieces(&g, Color::White), 1, "the mirage vanished");
    assert!(g.pos.find_piece(fake.id).is_none());
    assert!(!ev.is_empty());
}

#[test]
fn evolve() {
    let mut g = game("4k3/8/8/8/8/8/P7/4K3 w - - 0 1", &[SkillId::Evolve], &[]);
    use_skill(&mut g, SkillId::Evolve, piece("a2"));
    assert_eq!(board(&g), "4k3/8/8/8/8/8/Q7/4K3");
    assert_eq!(kind_at(&g, "a2"), Some((Color::White, PieceKind::Queen)));
    mv(&mut g, "e8", "d8");
    let moves = moves_from(&g, "a2");
    assert!(moves.contains(&"h2".to_string()) && moves.contains(&"a8".to_string()));
    assert!(moves.contains(&"g8".to_string()), "moves diagonally");
    mv(&mut g, "a2", "a3");
    mv(&mut g, "d8", "e8");
    assert_eq!(
        kind_at(&g, "a3"),
        Some((Color::White, PieceKind::Queen)),
        "for good"
    );
}

#[test]
fn switch() {
    let mut g = game("4k3/7p/8/8/8/8/8/n3K3 w - - 0 1", &[SkillId::Switch], &[]);
    let ev = use_skill(&mut g, SkillId::Switch, piece("a1"));
    assert_eq!(board(&g), "4k3/7p/8/8/8/8/8/N3K3");
    assert_eq!(kind_at(&g, "a1"), Some((Color::White, PieceKind::Knight)));
    assert!(ev.iter().any(|e| matches!(e, Event::Switched { .. })));
    mv(&mut g, "e8", "d8");
    mv(&mut g, "a1", "b3");
    mv(&mut g, "d8", "e8");
    assert_eq!(
        kind_at(&g, "b3"),
        Some((Color::White, PieceKind::Knight)),
        "for good"
    );
}

#[test]
fn mind() {
    // Back-rank mate in one: Ra8#.
    let mut g = game("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1", &[SkillId::Mind], &[]);
    let ev = use_skill(&mut g, SkillId::Mind, none());
    assert_eq!(board(&g), "6k1/5ppp/8/8/8/8/8/R5K1");
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::BestMove { from, to, .. } if *from == s("a1") && *to == s("a8")
    )));
    assert_eq!(g.side_to_move(), Color::White, "the turn is not spent");
    assert_eq!(g.pos.ply, 0);
    // Three uses.
    use_skill(&mut g, SkillId::Mind, none());
    use_skill(&mut g, SkillId::Mind, none());
    spent(&g, Color::White, SkillId::Mind);
    assert!(skill_fails(&mut g, SkillId::Mind, none()));
}

#[test]
fn control() {
    let mut g = game("4k3/7p/8/8/8/8/1b6/4K3 w - - 0 1", &[SkillId::Control], &[]);
    use_skill(&mut g, SkillId::Control, piece("b2"));
    assert_eq!(board(&g), "4k3/7p/8/8/8/8/1B6/4K3");
    assert_eq!(g.side_to_move(), Color::White, "the turn is not spent");
    assert_eq!(kind_at(&g, "b2"), Some((Color::White, PieceKind::Bishop)));
    let ev = mv(&mut g, "b2", "a3");
    assert_eq!(board(&g), "4k3/7p/8/8/8/b7/8/4K3");
    assert_eq!(
        kind_at(&g, "a3"),
        Some((Color::Black, PieceKind::Bishop)),
        "goes back"
    );
    assert!(ev.iter().any(|e| matches!(e, Event::LoanEnded { .. })));
    assert_eq!(g.side_to_move(), Color::Black);
}

#[test]
fn morph() {
    let mut g = game("4k3/8/8/3n4/8/8/8/4K3 w - - 0 1", &[SkillId::Morph], &[]);
    use_skill(&mut g, SkillId::Morph, spawn("d5", PieceKind::Pawn));
    assert_eq!(board(&g), "4k3/8/8/3p4/8/8/8/4K3");
    assert_eq!(kind_at(&g, "d5"), Some((Color::Black, PieceKind::Pawn)));
    assert_eq!(moves_from(&g, "d5"), ["d4"], "it moves like a pawn");
    let ev = mv(&mut g, "d5", "d4");
    assert_eq!(board(&g), "4k3/8/8/8/3n4/8/8/4K3");
    assert_eq!(
        kind_at(&g, "d4"),
        Some((Color::Black, PieceKind::Knight)),
        "worn off"
    );
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Transformed {
            kind: PieceKind::Knight,
            ..
        }
    )));
}

#[test]
fn canceller() {
    let mut g = start(&[SkillId::Canceller], &[SkillId::Evolve]);
    mv(&mut g, "e2", "e4");
    let before = g.pos.board;
    use_skill(&mut g, SkillId::Evolve, piece("a7"));
    assert_eq!(kind_at(&g, "a7"), Some((Color::Black, PieceKind::Queen)));
    let ev = use_skill(&mut g, SkillId::Canceller, none());
    assert_eq!(board(&g), "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR");
    assert_eq!(g.pos.board, before);
    assert_eq!(kind_at(&g, "a7"), Some((Color::Black, PieceKind::Pawn)));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Cancelled {
            skill: SkillId::Evolve
        }
    )));
    spent(&g, Color::Black, SkillId::Evolve);
    assert_eq!(g.side_to_move(), Color::Black);
}

#[test]
fn tornado() {
    // Non-king pieces in counterclockwise order around the centre, from the
    // east: Rh5, Nd8, Pc7, ba1. Each takes the square of the next one.
    let mut g = game(
        "3N4/2P4k/8/7R/8/8/8/b3K3 w - - 0 1",
        &[SkillId::Tornado],
        &[],
    );
    let ev = use_skill(&mut g, SkillId::Tornado, none());
    // h5 -> d8, d8 -> c7, c7 -> a1 (the pawn reaches a back rank: queen),
    // a1 -> h5; the kings stay.
    assert_eq!(board(&g), "3R4/2N4k/8/7b/8/8/8/Q3K3");
    let Some(Event::Rotated { moves }) = ev.iter().find(|e| matches!(e, Event::Rotated { .. }))
    else {
        panic!("a rotation event");
    };
    let moves: Vec<(String, String)> = moves.iter().map(|m| (name(m.from), name(m.to))).collect();
    let expected = [("h5", "d8"), ("d8", "c7"), ("c7", "a1"), ("a1", "h5")];
    assert_eq!(moves, expected.map(|(a, b)| (a.to_string(), b.to_string())));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Promoted { square, to: PieceKind::Queen } if *square == s("a1")
    )));
}

#[test]
fn invisibility() {
    let mut g = game(
        "4k3/7p/8/8/8/8/N7/4K3 w - - 0 1",
        &[SkillId::Invisibility],
        &[],
    );
    use_skill(&mut g, SkillId::Invisibility, piece("a2"));
    assert_eq!(board(&g), "4k3/7p/8/8/8/8/N7/4K3");
    let id = at(&g, "a2").unwrap().id;
    // The server hides pieces carrying this effect from the opponent
    // (covered by chessy-server's `hidden` tests); here, its duration.
    assert!(g.pos.has_effect(id, EffectKind::Invisible));
    mv(&mut g, "e8", "d8");
    mv(&mut g, "a2", "c3");
    assert!(
        g.pos.has_effect(id, EffectKind::Invisible),
        "still hidden for black's 2nd turn"
    );
    mv(&mut g, "d8", "e8");
    assert!(
        !g.pos.has_effect(id, EffectKind::Invisible),
        "visible again"
    );
}

#[test]
fn terminator() {
    let mut g = game(
        "4k3/8/2r5/8/8/8/8/4K3 w - - 0 1",
        &[SkillId::Terminator],
        &[],
    );
    use_skill(&mut g, SkillId::Terminator, piece("c6"));
    assert_eq!(board(&g), "4k3/8/2r5/8/8/2R5/8/4K3");
    let copy = at(&g, "c3").expect("a copy on the mirror square");
    assert_eq!(
        (copy.color, copy.kind, copy.temp),
        (Color::White, PieceKind::Rook, true)
    );
    assert_eq!(
        kind_at(&g, "c6"),
        Some((Color::Black, PieceKind::Rook)),
        "the original stays"
    );
    mv(&mut g, "e8", "d8");
    // The copy fights for white this turn, then disappears.
    let ev = mv(&mut g, "c3", "c6");
    assert!(
        ev.iter().any(|e| matches!(e, Event::Captured { .. })),
        "it took the rook"
    );
    assert!(g.pos.find_piece(copy.id).is_none(), "and vanished");
    assert_eq!(board(&g), "3k4/8/8/8/8/8/8/4K3");
    assert_eq!(count_pieces(&g, Color::White), 1);
    assert_eq!(count_pieces(&g, Color::Black), 1);
}

#[test]
fn trap() {
    let mut g = game("4k3/4r3/8/8/8/8/8/K7 w - - 0 1", &[SkillId::Trap], &[]);
    use_skill(&mut g, SkillId::Trap, square("e5"));
    assert!(at(&g, "e5").is_none(), "nothing visible on the board");
    let id = at(&g, "e7").unwrap().id;
    // The rook means to go to e2 but crosses e5.
    let ev = mv(&mut g, "e7", "e2");
    assert_eq!(board(&g), "4k3/8/8/4r3/8/8/8/K7");
    assert_eq!(
        kind_at(&g, "e5"),
        Some((Color::Black, PieceKind::Rook)),
        "stopped on the trap"
    );
    assert!(at(&g, "e2").is_none());
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::TrapSprung { square, .. } if *square == s("e5"))));
    assert!(g.pos.is_frozen(id));
    assert!(g.pos.traps.is_empty(), "a trap springs once");
}

#[test]
fn bench() {
    let mut g = start(&[SkillId::Bench], &[]);
    let id = at(&g, "g1").unwrap().id;
    use_skill(&mut g, SkillId::Bench, piece("g1"));
    assert_eq!(board(&g), "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKB1R");
    assert!(at(&g, "g1").is_none(), "off the board");
    assert_eq!(count_pieces(&g, Color::White), 15);
    mv(&mut g, "e7", "e5");
    assert_eq!(
        at(&g, "g1").map(|p| p.id),
        Some(id),
        "back for white's turn"
    );
    assert_eq!(board(&g), "rnbqkbnr/pppp1ppp/8/4p3/8/8/PPPPPPPP/RNBQKBNR");
    assert!(can_move(&g, "g1", "f3"));
}

#[test]
fn forcefield() {
    let mut g = game(
        "4k3/8/8/8/1b6/8/3N4/4K3 w - - 0 1",
        &[SkillId::Forcefield],
        &[],
    );
    use_skill(&mut g, SkillId::Forcefield, piece("d2"));
    let ev = mv(&mut g, "b4", "d2");
    assert_eq!(board(&g), "4k3/8/8/8/1b6/8/8/4K3");
    assert!(at(&g, "d2").is_none(), "the knight is still captured");
    assert_eq!(
        kind_at(&g, "b4"),
        Some((Color::Black, PieceKind::Bishop)),
        "pushed back two squares"
    );
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Pushed { from, to, .. } if *from == s("d2") && *to == s("b4")
    )));
}

#[test]
fn transposition() {
    let mut g = game(
        "4k3/8/7n/8/8/8/8/R3K3 w - - 0 1",
        &[SkillId::Transposition],
        &[],
    );
    use_skill(&mut g, SkillId::Transposition, pair("a1", "h6"));
    assert_eq!(board(&g), "4k3/8/7R/8/8/8/8/n3K3");
    assert_eq!(kind_at(&g, "h6"), Some((Color::White, PieceKind::Rook)));
    assert_eq!(kind_at(&g, "a1"), Some((Color::Black, PieceKind::Knight)));
    // Refused when it would put a king in check.
    let g = game(
        "4k3/8/7n/8/8/8/8/R3K3 w - - 0 1",
        &[SkillId::Transposition],
        &[],
    );
    assert!(!can_skill(&g, SkillId::Transposition, pair("a1", "e1")));
}

#[test]
fn queensac() {
    let mut g = game(
        "6k1/8/8/8/8/7Q/5PPP/r5K1 w - - 0 1",
        &[SkillId::Queensac],
        &[],
    );
    assert!(g.pos.in_check(Color::White));
    use_skill(&mut g, SkillId::Queensac, none());
    assert_eq!(board(&g), "6k1/8/8/8/8/7K/5PPP/r7");
    assert_eq!(kind_at(&g, "h3"), Some((Color::White, PieceKind::King)));
    assert!(at(&g, "g1").is_none(), "the queen died");
    assert!(!g.pos.in_check(Color::White));
    assert_eq!(g.outcome(), Outcome::Ongoing);
}

#[test]
fn temporal() {
    let mut g = game("4k3/8/8/8/8/8/8/B3K3 w - - 0 1", &[SkillId::Temporal], &[]);
    mv(&mut g, "a1", "c3");
    mv(&mut g, "e8", "d8");
    use_skill(&mut g, SkillId::Temporal, piece("c3"));
    assert_eq!(board(&g), "3k4/8/8/4B3/8/8/8/4K3");
    assert!(at(&g, "c3").is_none());
    assert_eq!(kind_at(&g, "e5"), Some((Color::White, PieceKind::Bishop)));
    assert_eq!(g.side_to_move(), Color::Black, "it replaces the move");
}

#[test]
fn geomancy() {
    let mut g = game("4k3/8/8/8/r7/8/8/4K3 w - - 0 1", &[SkillId::Geomancy], &[]);
    use_skill(&mut g, SkillId::Geomancy, square("c4"));
    assert_eq!(board(&g), "4k3/8/8/8/r7/8/8/4K3");
    let moves = moves_from(&g, "a4");
    for blocked in ["b4", "c4", "d4", "h4"] {
        assert!(
            !moves.contains(&blocked.to_string()),
            "{blocked} is cut off"
        );
    }
    assert!(
        moves.contains(&"a1".to_string()),
        "other directions are free"
    );
    // Six actions later the terrain is gone.
    for (from, to) in [
        ("e8", "d8"),
        ("e1", "d1"),
        ("d8", "e8"),
        ("d1", "e1"),
        ("e8", "d8"),
        ("e1", "d1"),
    ] {
        mv(&mut g, from, to);
    }
    assert!(g.pos.effects.is_empty());
    assert!(can_move(&g, "a4", "h4"));
}

#[test]
fn celestial() {
    let mut g = game(
        "4k3/8/8/8/8/1b6/8/2R1K3 w - - 0 1",
        &[SkillId::Celestial],
        &[],
    );
    mv(&mut g, "c1", "c4");
    mv(&mut g, "e8", "d8");
    let id = at(&g, "c4").unwrap().id;
    use_skill(&mut g, SkillId::Celestial, piece("c4"));
    let ev = mv(&mut g, "b3", "c4");
    assert_eq!(board(&g), "3k4/8/8/8/2b5/8/8/2R1K3");
    assert_eq!(kind_at(&g, "c4"), Some((Color::Black, PieceKind::Bishop)));
    assert_eq!(
        at(&g, "c1").map(|p| p.id),
        Some(id),
        "sent home instead of dying"
    );
    assert!(ev.iter().any(|e| matches!(e, Event::Saved { .. })));
    assert!(g.pos.effects.is_empty(), "works once");
}

#[test]
fn godhelp() {
    let mut g = game("4k3/7p/8/8/8/8/P7/4K3 w - - 0 1", &[SkillId::Godhelp], &[]);
    let ev = use_skill(&mut g, SkillId::Godhelp, none());
    let (square, piece) = ev
        .iter()
        .find_map(|e| match e {
            Event::Spawned { square, piece } => Some((*square, *piece)),
            _ => None,
        })
        .expect("a piece appeared");
    assert!((2..=5).contains(&rank_of(square)));
    assert_eq!(count_pieces(&g, Color::White), 3, "exactly one new piece");
    assert_eq!(at(&g, &name(square)), Some(piece));
    assert_eq!((piece.color, piece.temp), (Color::White, true));
    // It serves on white's next three turns (plies 2, 4 and 6), then is gone.
    for _ in 0..5 {
        assert!(g.pos.find_piece(piece.id).is_some());
        filler(&mut g);
    }
    assert!(
        g.pos.find_piece(piece.id).is_some(),
        "still there for white's 3rd turn"
    );
    filler(&mut g);
    assert!(g.pos.find_piece(piece.id).is_none(), "gone after it");
}
