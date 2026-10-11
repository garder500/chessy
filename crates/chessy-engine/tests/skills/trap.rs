use crate::common::*;

fn trap_at(g: &Game, sqr: &str) -> Option<Trap> {
    g.pos.traps.iter().copied().find(|t| t.square == s(sqr))
}

#[test]
fn lays_a_trap_on_an_empty_square() {
    let mut g = game(KINGS, &[SkillId::Trap], &[]);
    let ev = use_skill(&mut g, SkillId::Trap, square("e4"));
    assert_eq!(
        trap_at(&g, "e4"),
        Some(Trap {
            square: s("e4"),
            owner: Color::White
        })
    );
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::TrapSet { square } if *square == s("e4"))));
    // Nothing visible on the board.
    assert!(at(&g, "e4").is_none());
}

#[test]
fn only_empty_free_squares_and_two_traps_at_most() {
    let mut g = game(KINGS, &[SkillId::Trap, SkillId::Trap, SkillId::Trap], &[]);
    assert!(skill_fails(&mut g, SkillId::Trap, square("e1")), "occupied");
    use_skill(&mut g, SkillId::Trap, square("a4"));
    mv(&mut g, "e8", "d8");
    assert!(
        !can_skill(&g, SkillId::Trap, square("a4")),
        "already trapped"
    );
    use_skill(&mut g, SkillId::Trap, square("b4"));
    mv(&mut g, "d8", "e8");
    assert!(!has_skill(&g, SkillId::Trap), "a third trap is not allowed");
}

#[test]
fn a_slider_stops_on_the_trap_and_never_captures_beyond_it() {
    let mut g = game("4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1", &[SkillId::Trap], &[]);
    use_skill(&mut g, SkillId::Trap, square("e4"));
    assert!(
        can_move(&g, "e8", "e2"),
        "the plan is legal when it is made"
    );
    let ev = mv(&mut g, "e8", "e2");
    assert_eq!(kind_at(&g, "e4"), Some((Color::Black, PieceKind::Rook)));
    assert_eq!(
        kind_at(&g, "e2"),
        Some((Color::White, PieceKind::Knight)),
        "no capture"
    );
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::TrapSprung { square, .. } if *square == s("e4"))));
    assert!(!ev.iter().any(|e| matches!(e, Event::Captured { .. })));
    assert!(g.pos.traps.is_empty(), "the trap is used up");
    assert!(effects_on(&g, "e4").contains(&EffectKind::Frozen));
}

#[test]
fn the_trapped_piece_loses_two_of_its_turns() {
    let mut g = game("4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1", &[SkillId::Trap], &[]);
    use_skill(&mut g, SkillId::Trap, square("e4"));
    mv(&mut g, "e8", "e4"); // ply 1 -> 2
    let white_shuffle = [("e1", "d1"), ("d1", "e1")];
    let black_shuffle = [("h8", "g8"), ("g8", "h8")];
    // Black's turns at plies 3 and 5: the rook is frozen.
    for i in 0..2 {
        mv(&mut g, white_shuffle[i].0, white_shuffle[i].1);
        assert!(
            moves_from(&g, "e4").is_empty(),
            "frozen at ply {}",
            g.pos.ply
        );
        mv(&mut g, black_shuffle[i].0, black_shuffle[i].1);
    }
    mv(&mut g, "e1", "d1");
    assert!(g.pos.ply == 7);
    assert!(!moves_from(&g, "e4").is_empty(), "free on the third turn");
}

#[test]
fn a_trap_does_not_hurt_its_owner() {
    let mut g = game("7k/8/8/8/R7/8/8/4K3 w - - 0 1", &[SkillId::Trap], &[]);
    use_skill(&mut g, SkillId::Trap, square("e4"));
    mv(&mut g, "h8", "g8");
    let ev = mv(&mut g, "a4", "h4");
    assert_eq!(kind_at(&g, "h4"), Some((Color::White, PieceKind::Rook)));
    assert!(!ev.iter().any(|e| matches!(e, Event::TrapSprung { .. })));
    assert!(trap_at(&g, "e4").is_some(), "still armed");
}

#[test]
fn a_knight_only_springs_a_trap_where_it_lands() {
    let mut g = game(
        "7k/8/8/6n1/8/8/8/4K3 w - - 0 1",
        &[SkillId::Trap, SkillId::Trap],
        &[SkillId::Freeze],
    );
    use_skill(&mut g, SkillId::Trap, square("g4")); // beside its path
    mv(&mut g, "h8", "g8");
    use_skill(&mut g, SkillId::Trap, square("e4")); // where it lands
    let ev = mv(&mut g, "g5", "e4");
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::TrapSprung { square, .. } if *square == s("e4"))));
    assert!(trap_at(&g, "g4").is_some());
    assert!(trap_at(&g, "e4").is_none());
}

#[test]
fn a_double_pawn_step_is_cut_short() {
    let mut g = game("4k3/3p4/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Trap], &[]);
    use_skill(&mut g, SkillId::Trap, square("d6"));
    mv(&mut g, "d7", "d5");
    assert_eq!(kind_at(&g, "d6"), Some((Color::Black, PieceKind::Pawn)));
    assert!(at(&g, "d5").is_none());
    assert_eq!(g.pos.en_passant, None);
    assert!(effects_on(&g, "d6").contains(&EffectKind::Frozen));
}

#[test]
fn a_stopped_move_still_has_to_be_legal() {
    // Black is in check from Re1; Rxe1 would answer it, but the trap on c1 stops the rook.
    let fen = "4k3/8/8/8/8/8/7K/r3R3 b - - 0 1";
    let mut pos = Position::from_fen(fen).unwrap();
    let g = Game::from_position(pos.clone(), &[], &[]);
    assert!(can_move(&g, "a1", "e1"));

    pos.traps.push(Trap {
        square: s("c1"),
        owner: Color::White,
    });
    let g = Game::from_position(pos, &[], &[]);
    assert!(!can_move(&g, "a1", "e1"));
    assert!(!can_move(&g, "a1", "c1"));
}

#[test]
fn nobody_can_be_dropped_on_a_trapped_square() {
    let mut pos = Position::from_fen("r3k3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
    pos.traps.push(Trap {
        square: s("e4"),
        owner: Color::White,
    });
    let g = Game::from_position(pos, &[], &[SkillId::Teleportation]);
    let targets = targets_of(&g, SkillId::Teleportation);
    assert!(!targets.is_empty());
    assert!(targets
        .iter()
        .all(|t| !matches!(t, SkillTarget::PieceTo { to, .. } if *to == s("e4"))));
}

#[test]
fn an_enemy_trap_does_not_rule_a_square_out_and_both_traps_stand() {
    // The opponent's traps are secret, so they cannot change the targets.
    let mut pos = Position::from_fen(KINGS).unwrap();
    pos.traps.push(Trap {
        square: s("e4"),
        owner: Color::Black,
    });
    pos.traps.push(Trap {
        square: s("a4"),
        owner: Color::White,
    });
    let mut g = Game::from_position(pos, &[SkillId::Trap, SkillId::Trap], &[]);
    assert!(can_skill(&g, SkillId::Trap, square("e4")), "enemy trap");
    assert!(!can_skill(&g, SkillId::Trap, square("a4")), "own trap");
    use_skill(&mut g, SkillId::Trap, square("e4"));
    let on_e4: Vec<Color> = g
        .pos
        .traps
        .iter()
        .filter(|t| t.square == s("e4"))
        .map(|t| t.owner)
        .collect();
    assert_eq!(on_e4.len(), 2);
    // Each springs for the other side only.
    mv(&mut g, "e8", "d8");
    assert_eq!(g.pos.traps.len(), 3);
}

#[test]
fn offered_actions_can_be_listed_on_another_position() {
    // The server lists what a player may do on the board they see.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/7K/r3R3 b - - 0 1").unwrap();
    pos.traps.push(Trap {
        square: s("c1"),
        owner: Color::White,
    });
    let g = Game::from_position(pos.clone(), &[], &[]);
    let take = Action::Move {
        from: s("a1"),
        to: s("e1"),
        promo: None,
    };
    assert!(!g.legal_actions().contains(&take));
    pos.traps.clear();
    assert!(g.legal_actions_on(&pos).contains(&take));
    assert!(g.is_legal_on(&pos, take));
    // The real position still refuses it.
    let mut real = g.clone();
    assert!(real.apply(take).is_err());
}

#[test]
fn a_trap_is_not_a_piece() {
    let mut g = game(KINGS, &[SkillId::Trap], &[]);
    use_skill(&mut g, SkillId::Trap, square("e4"));
    assert_eq!(g.pos.traps.len(), 1);
    assert!(g.pos.board.iter().flatten().count() == 2);
}
