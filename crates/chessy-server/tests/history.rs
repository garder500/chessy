//! The history of a player's skills: what the Collection screen shows.

mod common;

use axum::http::StatusCode;
use chessy_engine::forge::{Effect, Graded, Rarity, SkillDef};
use chessy_engine::SkillId;
use chessy_server::hub::HubConfig;
use chessy_server::store::Store;
use common::*;
use serde_json::json;

fn fresh() -> Store {
    Store::open(":memory:").unwrap()
}

fn forged(store: &Store, effect: Effect) -> SkillId {
    let graded = Graded {
        rarity: Rarity::Common,
        score: 1.0,
        cost: 1.0,
        tone: 1.0,
        redundant: false,
    };
    SkillId::Forged(
        store
            .insert_forged(&SkillDef::new(effect), &graded)
            .unwrap(),
    )
}

fn lines(store: &Store, player: &str) -> Vec<(String, String, String, Option<String>)> {
    // Oldest first, to read like a story.
    let mut entries = store.skill_history(player).unwrap();
    entries.reverse();
    entries
        .into_iter()
        .map(|e| {
            (
                e.skill.to_string(),
                serde_json::to_value(e.change)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
                e.source,
                e.other,
            )
        })
        .collect()
}

#[test]
fn a_new_player_starts_with_the_starter_deck_in_the_history() {
    let store = fresh();
    let (id, _) = store.create_player().unwrap();
    let history = lines(&store, &id);
    assert_eq!(history.len(), 3);
    assert!(history
        .iter()
        .all(|(_, change, source, other)| change == "gained"
            && source == "starter"
            && other.is_none()));
    let deck: Vec<String> = store
        .deck(&id)
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut got: Vec<String> = history.into_iter().map(|h| h.0).collect();
    got.sort();
    let mut want = deck;
    want.sort();
    assert_eq!(got, want, "the history matches the deck a new player owns");
}

#[test]
fn a_stolen_skill_is_logged_for_both_players() {
    let store = fresh();
    let (alice, _) = store.register("alice", "x", None).unwrap();
    let (bob, _) = store.register("bob", "x", None).unwrap();
    store.set_deck(&alice, &[SkillId::Imune]).unwrap();
    store
        .set_deck(&bob, &[SkillId::Freeze, SkillId::Clone])
        .unwrap();
    store
        .apply_reward(
            &alice,
            &bob,
            Some(SkillId::Freeze),
            Some(SkillId::Freeze),
            None,
        )
        .unwrap();
    let a = lines(&store, &alice);
    assert_eq!(
        a.last().unwrap(),
        &(
            "freeze".into(),
            "gained".into(),
            "stolen".into(),
            Some("bob".into())
        )
    );
    let b = lines(&store, &bob);
    assert_eq!(
        b.last().unwrap(),
        &(
            "freeze".into(),
            "lost".into(),
            "taken".into(),
            Some("alice".into())
        )
    );
}

#[test]
fn a_forged_skill_is_logged_as_forged_and_a_replaced_one_as_replaced() {
    let store = fresh();
    let (alice, _) = store.register("alice", "x", None).unwrap();
    let (bob, _) = store.register("bob", "x", None).unwrap();
    store
        .set_deck(&alice, &[SkillId::Imune, SkillId::Clone])
        .unwrap();
    store.set_deck(&bob, &[SkillId::Freeze]).unwrap();
    let made = forged(&store, Effect::Promote);
    store
        .apply_reward(
            &alice,
            &bob,
            Some(made),
            Some(SkillId::Freeze),
            Some(SkillId::Imune),
        )
        .unwrap();
    let a = lines(&store, &alice);
    let tail: Vec<_> = a.iter().rev().take(2).collect();
    assert!(
        tail.contains(&&(made.to_string(), "gained".into(), "forged".into(), None)),
        "{a:?}"
    );
    assert!(
        tail.contains(&&("imune".into(), "lost".into(), "replaced".into(), None)),
        "{a:?}"
    );
    assert!(lines(&store, &bob)
        .iter()
        .any(|l| l.1 == "lost" && l.2 == "taken"));
}

#[test]
fn a_player_left_with_nothing_is_given_a_skill_and_it_is_logged() {
    let store = fresh();
    let (alice, _) = store.register("alice", "x", None).unwrap();
    store.set_deck(&alice, &[]).unwrap();
    assert_eq!(store.refill_to_minimum(&alice).unwrap(), 3);
    assert_eq!(store.deck(&alice).unwrap().len(), 3);
    let history = lines(&store, &alice);
    assert_eq!(history.last().unwrap().2, "refill");
}

#[test]
fn a_deck_is_refilled_only_up_to_the_minimum_without_duplicates() {
    let store = fresh();
    let (alice, _) = store.register("alice", "x", None).unwrap();
    let all = chessy_server::store::classic_skills();
    store.set_deck(&alice, &all[..1]).unwrap();
    assert_eq!(store.refill_to_minimum(&alice).unwrap(), 2);
    let deck = store.deck(&alice).unwrap();
    assert_eq!(deck.len(), 3);
    assert_eq!(
        deck.iter().collect::<std::collections::HashSet<_>>().len(),
        3
    );
    // Already at the minimum: nothing changes.
    assert_eq!(store.refill_to_minimum(&alice).unwrap(), 0);
    // A bigger deck is never trimmed.
    store.set_deck(&alice, &all[..5]).unwrap();
    assert_eq!(store.refill_to_minimum(&alice).unwrap(), 0);
    assert_eq!(store.deck(&alice).unwrap().len(), 5);
}

#[test]
fn bots_that_lost_skills_get_them_back_at_startup_once() {
    let db = TempDb::new();
    let (thin, empty, full, human) = {
        let store = Store::open(db.path_str()).unwrap();
        let thin = store.create_bot_account("Thin", 800).unwrap().unwrap();
        let empty = store.create_bot_account("Empty", 900).unwrap().unwrap();
        let full = store.create_bot_account("Full", 1000).unwrap().unwrap();
        let (human, _) = store.register("human", "x", None).unwrap();
        let all = chessy_server::store::classic_skills();
        store.set_deck(&thin.id, &all[..1]).unwrap();
        store.set_deck(&empty.id, &[]).unwrap();
        store.set_deck(&human, &all[..1]).unwrap();
        let full_before = store.deck(&full.id).unwrap();
        assert_eq!(full_before.len(), 3);
        (thin.id, empty.id, (full.id, full_before), human)
    };
    let store = Store::open(db.path_str()).unwrap();
    assert_eq!(store.deck(&thin).unwrap().len(), 3);
    assert_eq!(store.deck(&empty).unwrap().len(), 3);
    // A bot at the minimum keeps exactly its deck; a human is not touched here.
    assert_eq!(store.deck(&full.0).unwrap(), full.1);
    assert_eq!(store.deck(&human).unwrap().len(), 1);
    // Idempotent.
    let before = store.deck(&thin).unwrap();
    assert_eq!(store.top_up_bot_decks().unwrap(), 0);
    assert_eq!(store.deck(&thin).unwrap(), before);
    assert_eq!(lines(&store, &thin).last().unwrap().2, "refill");
}

#[test]
fn skills_owned_before_the_history_existed_are_entered_once() {
    let db = TempDb::new();
    let id = {
        let store = Store::open(db.path_str()).unwrap();
        let (id, _) = store.create_player().unwrap();
        id
    };
    // Back to the schema before the journal.
    {
        let conn = db.raw();
        conn.execute_batch("DROP TABLE skill_history; PRAGMA user_version = 4;")
            .unwrap();
    }
    let store = Store::open(db.path_str()).unwrap();
    let history = lines(&store, &id);
    assert_eq!(history.len(), 3);
    assert!(history.iter().all(|h| h.1 == "gained" && h.2 == "earlier"));
    assert!(store
        .skill_history(&id)
        .unwrap()
        .iter()
        .all(|e| e.at.ends_with('Z')));
    // Opening it again does not duplicate them.
    let again = Store::open(db.path_str()).unwrap();
    assert_eq!(again.skill_history(&id).unwrap().len(), 3);
}

#[tokio::test]
async fn the_rest_api_gives_a_player_their_own_history_newest_first() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let (status, _) = api.get("/api/me/skills", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let token = api.register("alice").await;
    let (status, v) = api.get("/api/me/skills", Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["entries"].as_array().unwrap().len(), 3, "{v}");
    assert_eq!(v["deck"].as_array().unwrap().len(), 3);

    let (alice, _) = (store.player_by_token(&token).unwrap().unwrap(), ());
    // The starter deck is random: make sure it does not already hold Freeze.
    let deck: Vec<SkillId> = store
        .deck(&alice)
        .unwrap()
        .into_iter()
        .filter(|s| *s != SkillId::Freeze)
        .collect();
    store.set_deck(&alice, &deck).unwrap();
    let (bob, _) = store.register("bob", "x", None).unwrap();
    store.set_deck(&bob, &[SkillId::Freeze]).unwrap();
    store
        .apply_reward(
            &alice,
            &bob,
            Some(SkillId::Freeze),
            Some(SkillId::Freeze),
            None,
        )
        .unwrap();
    let (_, v) = api.get("/api/me/skills", Some(&token)).await;
    let first = &v["entries"][0];
    assert_eq!(first["skill"], "freeze");
    assert_eq!(first["change"], "gained");
    assert_eq!(first["source"], "stolen");
    assert_eq!(first["other"], "bob");
    assert!(first["at"].is_string() && first["id"].is_number());
    assert!(v["deck"].as_array().unwrap().contains(&json!("freeze")));
}
