//! Placement games (docs/spec-v5.md): five games against bots of a hidden
//! level replace the default rating with an estimate.

mod common;

use std::time::Duration;

use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use common::*;
use serde_json::{json, Value};

fn cfg() -> HubConfig {
    HubConfig {
        bot_delay_min: Duration::from_millis(5),
        bot_delay_max: Duration::from_millis(10),
        bot_think_max: Duration::from_millis(100),
        msg_rate: 10_000.0,
        msg_burst: 10_000,
        ..HubConfig::default()
    }
}

/// Starts a placement game, gives up at once and returns the `game_over`.
fn lose_one(c: &mut Client) -> Value {
    c.say(json!({"type": "placement_start", "color": "white"}));
    let ds = c.next("deck_select");
    assert_eq!(ds["opponent"]["elo"], Value::Null, "the level stays hidden");
    assert_eq!(ds["rated"], false);
    c.color = serde_json::from_value(ds["you"].clone()).ok();
    c.pick_nothing();
    c.send(ClientMsg::Resign);
    c.next("game_over")
}

#[tokio::test]
async fn five_games_replace_the_rating_with_an_estimate() {
    let (app, store) = new_app(cfg());
    let mut c = account(&app, &store, "Newcomer");
    let id = c.welcome["account"]["player_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(c.welcome["account"]["placement"]["placed"], false);
    assert_eq!(c.welcome["account"]["placement"]["total"], 5);

    for n in 1..=4 {
        let over = lose_one(&mut c);
        assert_eq!(over["placement"]["done"], n);
        assert_eq!(over["placement"]["elo"], Value::Null);
        assert_eq!(
            store.me(&id).unwrap().unwrap().elo,
            1200,
            "no change before the end"
        );
    }
    let over = lose_one(&mut c);
    assert_eq!(over["placement"]["done"], 5);
    assert_eq!(over["placement"]["before"], 1200);
    assert_eq!(
        over["placement"]["elo"], 200,
        "five losses: the floor of the estimate"
    );
    // No game was counted as a rated one.
    let me = store.me(&id).unwrap().unwrap();
    assert_eq!((me.elo, me.games, me.losses), (200, 0, 0));
    assert!(me.placement.placed);

    // The five bots were five different levels.
    let mut levels = store.placement_levels_played(&id).unwrap();
    levels.sort_unstable();
    assert_eq!(levels, vec![400, 800, 1200, 1600, 2000]);

    // Once placed, no more.
    c.say(json!({"type": "placement_start"}));
    assert_eq!(c.next("error")["code"], "already_placed");
}

#[tokio::test]
async fn guests_cannot_be_placed() {
    let (app, _) = new_app(cfg());
    let mut g = guest(&app);
    g.say(json!({"type": "placement_start"}));
    assert_eq!(g.next("error")["code"], "account_required");
}
