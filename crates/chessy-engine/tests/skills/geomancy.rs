use crate::common::*;

fn terrain_squares(g: &Game) -> Vec<String> {
    let mut out: Vec<String> = g
        .pos
        .effects
        .iter()
        .filter(|e| e.kind == EffectKind::Terrain)
        .map(|e| name(e.square.unwrap()))
        .collect();
    out.sort();
    out
}

#[test]
fn an_empty_square_and_its_empty_neighbours_on_the_rank_become_terrain() {
    let mut g = game(KINGS, &[SkillId::Geomancy], &[]);
    let ev = use_skill(&mut g, SkillId::Geomancy, square("e4"));
    assert_eq!(terrain_squares(&g), ["d4", "e4", "f4"]);
    let effect = g.pos.effects[0];
    assert_eq!(effect.owner, Some(Color::White));
    assert_eq!(effect.expires_at, 6);
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::Terrain { squares } if squares.len() == 3)));

    // Edge of the board, and occupied neighbours are skipped.
    let mut g = game("4k3/8/8/8/1p6/8/8/4K3 w - - 0 1", &[SkillId::Geomancy], &[]);
    use_skill(&mut g, SkillId::Geomancy, square("a4"));
    assert_eq!(terrain_squares(&g), ["a4"], "b4 holds a pawn");
}

#[test]
fn only_empty_squares_not_already_terrain() {
    let mut g = game(KINGS, &[SkillId::Geomancy, SkillId::Geomancy], &[]);
    assert!(skill_fails(&mut g, SkillId::Geomancy, square("e1")));
    use_skill(&mut g, SkillId::Geomancy, square("e4"));
    mv(&mut g, "e8", "d8");
    assert!(!can_skill(&g, SkillId::Geomancy, square("d4")));
    assert!(can_skill(&g, SkillId::Geomancy, square("c4")));
}

#[test]
fn the_opponents_sliders_cannot_stop_on_or_cross_it() {
    let mut g = game("r3k3/8/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Geomancy], &[]);
    use_skill(&mut g, SkillId::Geomancy, square("a4"));
    let moves = moves_from(&g, "a8");
    for ok in ["a7", "a6", "a5", "b8", "c8", "d8"] {
        assert!(moves.contains(&ok.to_string()), "{ok} in {moves:?}");
    }
    for blocked in ["a4", "a3", "a2", "a1"] {
        assert!(!moves.contains(&blocked.to_string()), "{blocked}");
    }
}

#[test]
fn the_owners_pieces_move_freely() {
    let mut g = game("4k3/8/8/8/8/8/8/R3K3 w - - 0 1", &[SkillId::Geomancy], &[]);
    use_skill(&mut g, SkillId::Geomancy, square("a4"));
    mv(&mut g, "e8", "d8");
    let moves = moves_from(&g, "a1");
    assert!(moves.contains(&"a4".to_string()));
    assert!(moves.contains(&"a8".to_string()));
}

#[test]
fn knights_jump_over_terrain_but_cannot_land_on_it() {
    let mut g = game(
        "1n2k3/8/8/8/8/8/8/4K3 w - - 0 1",
        &[SkillId::Geomancy],
        &[SkillId::Freeze],
    );
    use_skill(&mut g, SkillId::Geomancy, square("c6"));
    let moves = moves_from(&g, "b8");
    assert!(!moves.contains(&"c6".to_string()));
    assert!(moves.contains(&"a6".to_string()));
    assert!(moves.contains(&"d7".to_string()));
}

#[test]
fn kings_and_pawns_respect_it_too() {
    let mut g = game(
        "8/8/8/4k3/8/8/8/4K3 w - - 0 1",
        &[SkillId::Geomancy],
        &[SkillId::Freeze],
    );
    use_skill(&mut g, SkillId::Geomancy, square("e4"));
    let moves = moves_from(&g, "e5");
    assert_eq!(moves, ["d5", "d6", "e6", "f5", "f6"]);

    let mut g = game("4k3/3p4/8/8/8/8/8/4K3 w - - 0 1", &[SkillId::Geomancy], &[]);
    use_skill(&mut g, SkillId::Geomancy, square("d6"));
    assert!(
        moves_from(&g, "d7").is_empty(),
        "no step, no double step across it"
    );
}

#[test]
fn wears_off_after_six_plies() {
    // (Black keeps an unused skill, otherwise two bare kings are a draw.)
    let mut g = game(KINGS, &[SkillId::Geomancy], &[SkillId::Freeze]);
    use_skill(&mut g, SkillId::Geomancy, square("e4"));
    let steps = [("e8", "d8"), ("e1", "d1"), ("d8", "e8"), ("d1", "e1")];
    for (from, to) in steps {
        mv(&mut g, from, to);
        assert!(!terrain_squares(&g).is_empty(), "ply {}", g.pos.ply);
    }
    mv(&mut g, "e8", "d8");
    assert_eq!(g.pos.ply, 6);
    assert!(terrain_squares(&g).is_empty());
}

#[test]
fn terrain_cuts_an_attack_line_so_it_can_answer_a_check() {
    let g = game("4k3/8/8/8/8/8/8/4R1K1 b - - 0 1", &[], &[SkillId::Geomancy]);
    assert!(g.pos.in_check(Color::Black));
    assert!(can_skill(&g, SkillId::Geomancy, square("e4")));
    assert!(!can_skill(&g, SkillId::Geomancy, square("a4")));
}

#[test]
fn skills_cannot_drop_enemy_pieces_on_the_terrain() {
    let mut g = game(
        "r3k3/8/8/8/8/8/8/4K3 w - - 0 1",
        &[SkillId::Geomancy],
        &[SkillId::Teleportation],
    );
    use_skill(&mut g, SkillId::Geomancy, square("e4"));
    let targets = targets_of(&g, SkillId::Teleportation);
    assert!(!targets.is_empty());
    for t in targets {
        let SkillTarget::PieceTo { to, .. } = t else {
            panic!("unexpected target {t:?}")
        };
        assert!(
            !["d4", "e4", "f4"].contains(&name(to).as_str()),
            "{}",
            name(to)
        );
    }
}

#[test]
fn terrain_effects_serialize_with_their_square_and_owner() {
    let mut g = game(KINGS, &[SkillId::Geomancy], &[]);
    use_skill(&mut g, SkillId::Geomancy, square("h4"));
    let json = serde_json::to_value(g.pos.effects[0]).unwrap();
    assert_eq!(json["kind"], "terrain");
    assert_eq!(json["owner"], "white");
    assert_eq!(json["expires_at"], 6);
    assert_eq!(json["square"], s("h4"));
}
