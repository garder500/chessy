use crate::common::*;

#[test]
fn a_captured_piece_goes_back_to_its_starting_square() {
    let mut g = start(&[SkillId::Celestial], &[]);
    mv(&mut g, "b1", "c3");
    mv(&mut g, "e7", "e5");
    let id = at(&g, "c3").unwrap().id;
    use_skill(&mut g, SkillId::Celestial, piece("c3"));
    mv(&mut g, "f8", "b4");
    mv(&mut g, "a2", "a3");
    let ev = mv(&mut g, "b4", "c3");
    assert_eq!(
        kind_at(&g, "c3"),
        Some((Color::Black, PieceKind::Bishop)),
        "the capturer takes the square"
    );
    let home = at(&g, "b1").expect("the knight is home");
    assert_eq!((home.id, home.kind), (id, PieceKind::Knight));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Saved { piece, from, to } if *piece == id && *from == s("c3") && *to == s("b1")
    )));
    assert!(
        !ev.iter().any(|e| matches!(e, Event::Captured { .. })),
        "it was not captured"
    );
    assert!(g.pos.effects.is_empty(), "the intervention is used up");
}

#[test]
fn uses_the_nearest_free_square_when_home_is_occupied() {
    let mut g = game(
        "3rk3/8/8/8/3N4/8/8/4K3 w - - 0 1",
        &[SkillId::Celestial],
        &[],
    );
    use_skill(&mut g, SkillId::Celestial, piece("d4"));
    mv(&mut g, "e8", "f8");
    mv(&mut g, "e1", "e2");
    let ev = mv(&mut g, "d8", "d4");
    assert_eq!(kind_at(&g, "d4"), Some((Color::Black, PieceKind::Rook)));
    assert_eq!(kind_at(&g, "c3"), Some((Color::White, PieceKind::Knight)));
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::Saved { to, .. } if *to == s("c3"))));
}

#[test]
fn only_works_once() {
    let mut g = game(
        "3qk3/8/8/8/3N4/8/8/4K3 w - - 0 1",
        &[SkillId::Celestial],
        &[],
    );
    use_skill(&mut g, SkillId::Celestial, piece("d4"));
    mv(&mut g, "e8", "f8");
    mv(&mut g, "e1", "e2");
    mv(&mut g, "d8", "d4"); // saved, now on c3
    mv(&mut g, "e2", "e1");
    let ev = mv(&mut g, "d4", "c3"); // taken for real this time
    assert!(ev.iter().any(|e| matches!(e, Event::Captured { .. })));
    assert_eq!(count_pieces(&g, Color::White), 1);
}

#[test]
fn a_saved_pawn_is_not_a_dead_pawn() {
    let mut g = game(
        "4k3/8/8/3r4/8/8/3P4/4K3 w - - 0 1",
        &[SkillId::Celestial],
        &[],
    );
    use_skill(&mut g, SkillId::Celestial, piece("d2"));
    mv(&mut g, "e8", "f8");
    mv(&mut g, "e1", "f1");
    mv(&mut g, "d5", "d2");
    assert_eq!(g.pos.captured_pawns, [0, 0]);
    // Home is taken: c1 (a pawn may rest on its own back rank) is the first of the
    // closest free squares in square order.
    assert_eq!(kind_at(&g, "c1"), Some((Color::White, PieceKind::Pawn)));
}

#[test]
fn never_the_king() {
    let mut g = start(&[SkillId::Celestial], &[]);
    assert!(skill_fails(&mut g, SkillId::Celestial, piece("e1")));
    assert!(skill_fails(&mut g, SkillId::Celestial, piece("e7")));
}

#[test]
fn a_capture_is_illegal_if_the_saved_piece_would_check_the_capturers_king() {
    // The knight started on b1; after it is saved it attacks c3, where the king would stand.
    let fen = "8/8/8/8/8/3k4/8/1N2K3 w - - 0 1";
    let play_until_the_king_may_capture = |celestial: bool| {
        let skills: &[SkillId] = if celestial {
            &[SkillId::Celestial]
        } else {
            &[]
        };
        // (An unused skill on the other side keeps K+N vs K from being a draw.)
        let mut g = game(fen, skills, &[SkillId::Freeze]);
        mv(&mut g, "b1", "c3");
        mv(&mut g, "d3", "e3");
        if celestial {
            use_skill(&mut g, SkillId::Celestial, piece("c3"));
            mv(&mut g, "e3", "d3");
            mv(&mut g, "e1", "f1");
        } else {
            mv(&mut g, "e1", "f1");
            mv(&mut g, "e3", "d3");
            mv(&mut g, "f1", "e1");
        }
        g
    };
    // Without it the unprotected knight is simply taken ...
    let g = play_until_the_king_may_capture(false);
    assert!(can_move(&g, "d3", "c3"));
    // ... with it, taking would send the knight home to give check.
    let g = play_until_the_king_may_capture(true);
    assert!(!can_move(&g, "d3", "c3"));
    assert!(can_move(&g, "d3", "c4"));
}
