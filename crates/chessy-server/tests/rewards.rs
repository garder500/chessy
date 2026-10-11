//! Ranked rewards: they outlive a restart and a new game, and the loser is
//! told what the winner did with them.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chessy_engine::{Color, SkillId};
use chessy_server::hub::HubConfig;
use chessy_server::protocol::{ClientMsg, RewardChoice};
use chessy_server::store::Store;
use chessy_server::App;
use common::{account, fools_mate, ranked_match, Client, TempDb};
use serde_json::Value;

const WINNER_DECK: &[SkillId] = &[SkillId::Teleportation, SkillId::Imune, SkillId::Freeze];
const LOSER_DECK: &[SkillId] = &[SkillId::Rollback, SkillId::Clone, SkillId::DestinySwapper];

fn skill_of(v: &Value) -> SkillId {
    serde_json::from_value(v.clone()).unwrap()
}

fn open_app(db: &TempDb) -> (Arc<App>, Store) {
    let store = Store::open(db.path_str()).unwrap();
    (App::new(store.clone(), HubConfig::default()), store)
}

/// Two ranked players meet and the one who ends up white loses to a fool's
/// mate. Returns (winner, loser, reward on offer). Decks are set once the
/// colours are known, since the reward reads them when the game ends.
fn ranked_loss_with(
    app: &Arc<App>,
    store: &Store,
    loser_deck: &[SkillId],
) -> (Client, Client, Value) {
    let (a, b) = ranked_match(account(app, store, "alice"), account(app, store, "bob"));
    let (loser, mut winner) = if a.color == Some(Color::White) {
        (a, b)
    } else {
        (b, a)
    };
    store.set_deck(&loser.id, loser_deck).unwrap();
    store.set_deck(&winner.id, WINNER_DECK).unwrap();
    fools_mate(&loser, &winner);
    let offer = winner.next("game_over")["reward"].clone();
    (winner, loser, offer)
}

fn ranked_loss(app: &Arc<App>, store: &Store) -> (Client, Client, Value) {
    ranked_loss_with(app, store, LOSER_DECK)
}

fn skip(winner: &Client) {
    winner.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Skip,
    });
}

fn steal(winner: &Client, offer: &Value) -> SkillId {
    let skill = skill_of(&offer["steal_options"][0]);
    winner.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Steal {
            skill,
            replace: None,
        },
    });
    skill
}

#[tokio::test]
async fn a_pending_reward_is_still_offered_after_a_restart() {
    let db = TempDb::new();
    let (app, store) = open_app(&db);
    let (winner, _loser, offer) = ranked_loss(&app, &store);
    assert!(offer.is_object());
    let token = winner.token.clone();
    drop(winner);

    let (restarted, _) = open_app(&db);
    let mut winner = Client::connect(&restarted, Some(token));
    assert_eq!(winner.welcome["pending_reward"], offer);
    skip(&winner);
    winner.next("deck_update");
}

#[tokio::test]
async fn a_resolved_reward_is_gone_after_a_restart() {
    let db = TempDb::new();
    let (app, store) = open_app(&db);
    let (mut winner, _loser, _) = ranked_loss(&app, &store);
    skip(&winner);
    winner.next("deck_update");
    let token = winner.token.clone();

    let (restarted, _) = open_app(&db);
    let winner = Client::connect(&restarted, Some(token));
    assert!(winner.welcome["pending_reward"].is_null());
}

#[tokio::test]
async fn a_pending_reward_survives_a_new_game() {
    let (app, store) = common::new_app(HubConfig::default());
    let (winner, _loser, _) = ranked_loss(&app, &store);
    let carol = account(&app, &store, "carol");
    let (mut winner, _carol) = ranked_match(winner, carol);
    skip(&winner);
    winner.next("deck_update");
}

#[tokio::test]
async fn skipping_spares_the_loser_who_is_told() {
    let (app, store) = common::new_app(HubConfig::default());
    let (winner, mut loser, _) = ranked_loss(&app, &store);
    skip(&winner);
    let outcome = loser.next("reward_outcome");
    assert_eq!(outcome["kind"], "spared");
    assert!(outcome["skill"].is_null() && outcome["refilled"].is_null());
    assert_eq!(store.deck(&loser.id).unwrap(), LOSER_DECK);
    assert!(store.take_reward_outcomes(&loser.id).unwrap().is_empty());
}

#[tokio::test]
async fn an_expired_reward_spares_the_loser() {
    let config = HubConfig {
        reward_ttl: Duration::from_millis(10),
        ..HubConfig::default()
    };
    let (app, store) = common::new_app(config);
    let (mut winner, mut loser, _) = ranked_loss(&app, &store);
    tokio::time::sleep(Duration::from_millis(30)).await;
    skip(&winner);
    assert_eq!(winner.error_code(), "no_reward");
    assert!(loser.try_next("reward_outcome").is_none());
    assert_eq!(store.deck(&loser.id).unwrap(), LOSER_DECK);
}

#[tokio::test]
async fn stealing_tells_the_loser_what_was_taken() {
    let (app, store) = common::new_app(HubConfig::default());
    let (winner, mut loser, offer) = ranked_loss(&app, &store);
    let stolen = steal(&winner, &offer);
    let outcome = loser.next("reward_outcome");
    assert_eq!(outcome["kind"], "stolen");
    assert_eq!(skill_of(&outcome["skill"]), stolen);
    assert!(outcome["refilled"].is_null());
}

#[tokio::test]
async fn a_forged_reward_tells_the_loser_what_they_lost() {
    let (app, store) = common::new_app(HubConfig::default());
    let (winner, mut loser, _) = ranked_loss(&app, &store);
    winner.send(ClientMsg::RewardChoice {
        choice: RewardChoice::Random { replace: None },
    });
    let outcome = loser.wait_for("reward_outcome").await;
    assert_eq!(outcome["kind"], "forged");
    assert!(LOSER_DECK.contains(&skill_of(&outcome["skill"])));
}

#[tokio::test]
async fn a_loser_left_with_nothing_is_told_about_the_refill() {
    let (app, store) = common::new_app(HubConfig::default());
    let (winner, mut loser, offer) = ranked_loss_with(&app, &store, &[SkillId::Rollback]);
    steal(&winner, &offer);
    let outcome = loser.next("reward_outcome");
    assert_eq!(outcome["kind"], "stolen");
    assert!(!outcome["refilled"].is_null());
    assert_eq!(store.deck(&loser.id).unwrap().len(), 1);
}

#[tokio::test]
async fn an_offline_loser_is_told_once_on_reconnecting() {
    let (app, store) = common::new_app(HubConfig::default());
    let (winner, loser, offer) = ranked_loss(&app, &store);
    let token = loser.token.clone();
    app.disconnect(&loser.id, loser.conn);
    drop(loser);
    steal(&winner, &offer);

    let mut back = Client::connect(&app, Some(token.clone()));
    assert_eq!(back.next("reward_outcome")["kind"], "stolen");
    assert!(back.try_next("reward_outcome").is_none());
    let mut again = Client::connect(&app, Some(token));
    assert!(again.try_next("reward_outcome").is_none());
}
