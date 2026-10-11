//! Reward announcements kept for an offline loser: stored once, taken once.

mod common;

use chessy_engine::SkillId;
use chessy_server::protocol::RewardOutcomeKind;
use chessy_server::reward_outcome_store::RewardOutcomeRow;
use chessy_server::store::Store;
use common::TempDb;

fn outcome(kind: RewardOutcomeKind, skill: Option<SkillId>) -> RewardOutcomeRow {
    RewardOutcomeRow {
        by: "winner".to_owned(),
        kind,
        skill,
        refilled: skill.map(|_| SkillId::ALL[1]),
    }
}

#[test]
fn reopening_the_database_keeps_the_pending_outcomes() {
    let db = TempDb::new();
    let first = Store::open(db.path_str()).unwrap();
    let (player, _) = first.create_player().unwrap();
    let kept = outcome(RewardOutcomeKind::Stolen, Some(SkillId::ALL[0]));
    first.push_reward_outcome(&player, &kept).unwrap();
    drop(first);

    let second = Store::open(db.path_str()).unwrap();
    assert_eq!(second.take_reward_outcomes(&player).unwrap(), vec![kept]);
}

#[test]
fn outcomes_come_out_in_insertion_order_then_nothing() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let (player, _) = store.create_player().unwrap();
    let first = outcome(RewardOutcomeKind::Forged, Some(SkillId::ALL[2]));
    let second = outcome(RewardOutcomeKind::Spared, None);
    store.push_reward_outcome(&player, &first).unwrap();
    store.push_reward_outcome(&player, &second).unwrap();
    assert_eq!(
        store.take_reward_outcomes(&player).unwrap(),
        vec![first, second]
    );
    assert!(store.take_reward_outcomes(&player).unwrap().is_empty());
}

#[test]
fn outcomes_of_a_player_are_not_given_to_another() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let (owner, _) = store.create_player().unwrap();
    let (other, _) = store.create_player().unwrap();
    let mine = outcome(RewardOutcomeKind::Stolen, Some(SkillId::ALL[0]));
    store.push_reward_outcome(&owner, &mine).unwrap();
    assert!(store.take_reward_outcomes(&other).unwrap().is_empty());
    assert_eq!(store.take_reward_outcomes(&owner).unwrap(), vec![mine]);
}
