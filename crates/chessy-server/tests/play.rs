//! In-game features: clocks, draw offers, chat, rematches and the enriched
//! `state` / `game_over` messages.

mod common;

use std::time::Duration;

use chessy_engine::SkillId;
use chessy_server::hub::HubConfig;
use chessy_server::protocol::ClientMsg;
use common::*;
use serde_json::json;

fn friendly_pair(app: &std::sync::Arc<chessy_server::App>) -> (Client, Client) {
    let a = guest(app);
    let b = guest(app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    into_game(a, b)
}

fn clock_config(initial_ms: u64, increment_ms: u64) -> HubConfig {
    HubConfig {
        clock_initial: Duration::from_millis(initial_ms),
        clock_increment: Duration::from_millis(increment_ms),
        ..HubConfig::default()
    }
}

// ---- enriched messages -------------------------------------------------

#[tokio::test]
async fn state_carries_clock_opponent_and_draw_offer() {
    let (app, store) = new_app(HubConfig::default());
    let mut a = account(&app, &store, "alice");
    let b = account(&app, &store, "bob");
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.color = serde_json::from_value(a.next("deck_select")["you"].clone()).ok();
    a.pick_nothing();
    b.pick_nothing();
    let (mut white, mut black) = if a.color == Some(chessy_engine::Color::White) {
        (a, b)
    } else {
        (b, a)
    };
    let s = white.last("state");
    assert_eq!(s["ply_count"], 0);
    assert_eq!(s["rated"], true);
    assert_eq!(s["draw_offer"], "none");
    assert_eq!(s["clock"]["running"], "white");
    assert_eq!(s["clock"]["black_ms"], 600_000);
    let white_ms = s["clock"]["white_ms"].as_u64().unwrap();
    assert!((599_000..=600_000).contains(&white_ms), "{white_ms}");
    let opp = &s["opponent"];
    assert_eq!(opp["guest"], false);
    assert_eq!(opp["elo"], 1200);
    assert!(["alice", "bob"].contains(&opp["username"].as_str().unwrap()));
    assert_ne!(
        s["opponent"]["username"],
        black.last("state")["opponent"]["username"],
        "each side sees the other"
    );
}

#[tokio::test]
async fn guests_see_each_other_as_guests_and_unrated() {
    let (app, _) = new_app(HubConfig::default());
    let (mut white, _black) = friendly_pair(&app);
    white.mv("e2", "e4");
    let s = white.last("state");
    assert_eq!(s["rated"], false);
    assert_eq!(
        s["opponent"],
        json!({"username": null, "elo": null, "guest": true})
    );
    assert_eq!(s["ply_count"], 1);
    assert_eq!(s["ply"], 1);
}

#[tokio::test]
async fn game_over_reports_the_reason() {
    let (app, _) = new_app(HubConfig::default());
    let (white, mut black) = friendly_pair(&app);
    fools_mate(&white, &black);
    let over = black.next("game_over");
    assert_eq!(over["reason"], "checkmate");
    assert_eq!(over["rated"], false);
    assert!(over["elo"].is_null());
}

#[tokio::test]
async fn a_forfeit_by_disconnecting_has_its_own_reason() {
    let (app, _) = new_app(HubConfig {
        reconnect_grace: Duration::from_millis(80),
        ..HubConfig::default()
    });
    let (white, mut black) = friendly_pair(&app);
    app.disconnect(&white.id, white.conn);
    let over = black.wait_for("game_over").await;
    assert_eq!(over["reason"], "disconnect");
    assert_eq!(
        over["outcome"],
        json!({"type": "resignation", "winner": "black"})
    );
}

// ---- clocks ------------------------------------------------------------

#[tokio::test]
async fn the_clock_runs_for_the_side_to_move_and_adds_an_increment() {
    let (app, _) = new_app(clock_config(10_000, 2_000));
    let (mut white, mut black) = friendly_pair(&app);
    white.mv("e2", "e4");
    let s = black.last("state");
    assert_eq!(s["clock"]["running"], "black");
    let w = s["clock"]["white_ms"].as_u64().unwrap();
    assert!(
        (11_900..=12_000).contains(&w),
        "10 s + 2 s increment, got {w}"
    );
    let b = s["clock"]["black_ms"].as_u64().unwrap();
    assert!((9_900..=10_000).contains(&b), "black's own clock, got {b}");
    let _ = white.last("state");

    // Reconnecting shows the time left right now.
    tokio::time::sleep(Duration::from_millis(120)).await;
    let token = black.token.clone();
    let mut back = Client::connect(&app, Some(token));
    let s = back.next("state");
    let b = s["clock"]["black_ms"].as_u64().unwrap();
    assert!(
        (9_000..9_900).contains(&b),
        "black has been running, got {b}"
    );
    assert_eq!(s["clock"]["running"], "black");
}

#[tokio::test]
async fn running_out_of_time_loses_the_game() {
    let (app, _) = new_app(clock_config(150, 0));
    let (mut white, mut black) = friendly_pair(&app);
    // White never moves.
    let over = black.wait_for("game_over").await;
    assert_eq!(
        over["outcome"],
        json!({"type": "timeout", "winner": "black"})
    );
    assert_eq!(over["reason"], "timeout");
    assert!(over["reward"].is_null(), "friendly games pay no reward");
    assert!(white.next("game_over")["reward"].is_null());
    let s = white.last("state");
    assert_eq!(s["outcome"], json!({"type": "timeout", "winner": "black"}));
    assert_eq!(s["clock"]["running"], json!(null));
    assert_eq!(s["clock"]["white_ms"], 0);
    // The game is over: further moves are refused.
    white.mv("e2", "e4");
    assert_eq!(white.error_code(), "not_in_game");
}

#[tokio::test]
async fn black_can_flag_after_white_moved() {
    let (app, _) = new_app(clock_config(200, 0));
    let (white, mut black) = friendly_pair(&app);
    white.mv("e2", "e4");
    let over = black.wait_for("game_over").await;
    assert_eq!(
        over["outcome"],
        json!({"type": "timeout", "winner": "white"})
    );
}

#[tokio::test]
async fn acting_in_time_defuses_the_old_flag_timer() {
    let (app, store) = new_app(clock_config(200, 1_000));
    let mut a = guest(&app);
    let mut b = guest(&app);
    store.set_deck(&a.id, &[SkillId::Teleportation]).unwrap();
    store.set_deck(&b.id, &[SkillId::Teleportation]).unwrap();
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.color = serde_json::from_value(a.next("deck_select")["you"].clone()).ok();
    a.pick(&[SkillId::Teleportation]);
    b.pick(&[SkillId::Teleportation]);
    let (white, black) = if a.color == Some(chessy_engine::Color::White) {
        (&mut a, &mut b)
    } else {
        (&mut b, &mut a)
    };
    // White spends a skill (an action like any other), black moves.
    white.say(json!({
        "type": "action",
        "action": {"type": "skill", "skill": "teleportation",
                   "target": {"kind": "piece_to", "from": 0, "to": 28}}
    }));
    black.mv("e7", "e5");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        white.try_next("game_over").is_none(),
        "white's first flag timer was stale"
    );
    assert!(black.try_next("game_over").is_none());
    let s = white.last("state");
    assert_eq!(s["ply_count"], 2);
}

#[tokio::test]
async fn time_does_not_run_during_deck_selection() {
    let (app, _) = new_app(clock_config(1_000, 0));
    let mut a = guest(&app);
    let mut b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.pick_nothing();
    tokio::time::sleep(Duration::from_millis(400)).await;
    b.pick_nothing();
    let s = a.last("state");
    let ms = s["clock"]["white_ms"].as_u64().unwrap();
    assert!(ms >= 950, "the clock only starts with the game, got {ms}");
    assert!(b.try_next("game_over").is_none());
}

// ---- draw offers ---------------------------------------------------------

#[tokio::test]
async fn a_draw_offer_can_be_accepted() {
    let (app, _) = new_app(HubConfig::default());
    let (mut white, mut black) = friendly_pair(&app);
    white.send(ClientMsg::OfferDraw);
    assert!(black.try_next("draw_offered").is_some());
    assert!(white.try_next("draw_offered").is_none());

    // The offer shows up in resumed state views.
    let token = black.token.clone();
    let mut back = Client::connect(&app, Some(token));
    assert_eq!(back.next("state")["draw_offer"], "them");
    let token = white.token.clone();
    let mut white2 = Client::connect(&app, Some(token));
    assert_eq!(white2.next("state")["draw_offer"], "you");

    back.send(ClientMsg::RespondDraw { accept: true });
    let over = white2.next("game_over");
    assert_eq!(over["outcome"], json!({"type": "draw_agreed"}));
    assert_eq!(over["reason"], "agreed_draw");
    assert!(over["reward"].is_null(), "a draw rewards nobody");
    assert!(back.next("game_over")["reward"].is_null());
    let _ = (&mut white, &mut black);
}

#[tokio::test]
async fn a_draw_offer_can_be_declined_and_not_spammed() {
    let (app, _) = new_app(HubConfig::default());
    let (mut white, mut black) = friendly_pair(&app);
    white.send(ClientMsg::OfferDraw);
    white.send(ClientMsg::OfferDraw);
    assert_eq!(white.error_code(), "draw_pending");
    black.send(ClientMsg::RespondDraw { accept: false });
    assert!(white.try_next("draw_declined").is_some());
    black.send(ClientMsg::RespondDraw { accept: true });
    assert_eq!(black.error_code(), "no_draw_offer", "the offer is gone");

    white.send(ClientMsg::OfferDraw);
    assert_eq!(white.error_code(), "draw_already_offered", "once per move");
    white.mv("e2", "e4");
    black.mv("e7", "e5");
    white.send(ClientMsg::OfferDraw);
    assert!(
        black.try_next("draw_offered").is_some(),
        "allowed again after moves"
    );
}

#[tokio::test]
async fn a_draw_offer_lapses_when_a_move_is_played() {
    let (app, _) = new_app(HubConfig::default());
    let (mut white, mut black) = friendly_pair(&app);
    white.mv("e2", "e4");
    white.send(ClientMsg::OfferDraw);
    assert!(black.try_next("draw_offered").is_some());
    black.mv("e7", "e5");
    assert_eq!(black.last("state")["draw_offer"], "none");
    black.send(ClientMsg::RespondDraw { accept: true });
    assert_eq!(black.error_code(), "no_draw_offer");
    assert!(white.try_next("game_over").is_none());
}

#[tokio::test]
async fn offering_back_accepts_and_phases_are_checked() {
    let (app, _) = new_app(HubConfig::default());
    let mut a = guest(&app);
    a.send(ClientMsg::OfferDraw);
    assert_eq!(a.error_code(), "not_in_game");
    a.send(ClientMsg::RespondDraw { accept: true });
    assert_eq!(a.error_code(), "not_in_game");

    let b = guest(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    b.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    a.send(ClientMsg::OfferDraw);
    assert_eq!(
        a.error_code(),
        "wrong_phase",
        "no draws during deck selection"
    );

    let (mut white, mut black) = into_game(a, b);
    white.send(ClientMsg::OfferDraw);
    black.send(ClientMsg::OfferDraw);
    assert_eq!(
        white.next("game_over")["outcome"],
        json!({"type": "draw_agreed"})
    );
    let _ = black.next("game_over");
}

// ---- chat ----------------------------------------------------------------

#[tokio::test]
async fn chat_is_cleaned_echoed_and_delivered() {
    let (app, _) = new_app(HubConfig {
        chat_interval: Duration::ZERO,
        ..HubConfig::default()
    });
    let (mut white, mut black) = friendly_pair(&app);
    white.say(json!({"type": "chat", "text": "  hello\u{7}\nthere \u{1b}[0m<b>  "}));
    let theirs = black.next("chat");
    assert_eq!(
        theirs,
        json!({"type": "chat", "text": "hello there [0m<b>", "mine": false})
    );
    let echo = white.next("chat");
    assert_eq!(echo["mine"], true);
    assert_eq!(echo["text"], theirs["text"]);
    assert!(white.try_next("chat").is_none());
    assert!(black.try_next("chat").is_none());
}

#[tokio::test]
async fn chat_length_is_limited_to_140_characters() {
    let (app, _) = new_app(HubConfig {
        chat_interval: Duration::ZERO,
        ..HubConfig::default()
    });
    let (mut white, mut black) = friendly_pair(&app);
    let exact = "\u{e9}".repeat(140);
    white.say(json!({"type": "chat", "text": exact}));
    assert_eq!(
        black.next("chat")["text"],
        exact.as_str(),
        "140 characters, not bytes"
    );
    white.say(json!({"type": "chat", "text": "x".repeat(141)}));
    assert_eq!(white.last("error")["code"], "invalid_message");
    for blank in ["", "   ", "\n\t\u{7}"] {
        white.say(json!({"type": "chat", "text": blank}));
        assert_eq!(white.last("error")["code"], "invalid_message", "{blank:?}");
    }
    assert!(black.try_next("chat").is_none());
}

#[tokio::test]
async fn chat_is_rate_limited() {
    let (app, _) = new_app(HubConfig {
        chat_interval: Duration::from_millis(200),
        ..HubConfig::default()
    });
    let (mut white, mut black) = friendly_pair(&app);
    white.say(json!({"type": "chat", "text": "one"}));
    white.say(json!({"type": "chat", "text": "two"}));
    assert_eq!(white.last("error")["code"], "rate_limited");
    // The other player has their own allowance.
    black.say(json!({"type": "chat", "text": "reply"}));
    assert!(white.try_next("chat").is_some());
    tokio::time::sleep(Duration::from_millis(250)).await;
    white.say(json!({"type": "chat", "text": "three"}));
    assert!(white.try_next("error").is_none());
    let texts: Vec<_> = std::iter::from_fn(|| black.try_next("chat"))
        .filter(|m| m["mine"] == false)
        .map(|m| m["text"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(texts, ["one", "three"]);
}

#[tokio::test]
async fn chat_needs_a_game() {
    let (app, _) = new_app(HubConfig::default());
    let mut a = guest(&app);
    a.say(json!({"type": "chat", "text": "anyone?"}));
    assert_eq!(a.error_code(), "not_in_game");
}

// ---- rematches -----------------------------------------------------------

/// A finished friendly game: both players idle with a rematch available.
fn finished_game(app: &std::sync::Arc<chessy_server::App>) -> (Client, Client) {
    let (mut white, mut black) = friendly_pair(app);
    white.send(ClientMsg::Resign);
    let _ = white.next("game_over");
    let _ = black.next("game_over");
    (white, black)
}

#[tokio::test]
async fn an_accepted_rematch_swaps_colours() {
    let (app, store) = new_app(HubConfig::default());
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bob");
    let (mut a, mut b) = ranked_match(a, b);
    four_plies(&a, &b);
    let first_color = a.color;
    a.send(ClientMsg::Resign);
    let _ = a.next("game_over");
    let _ = b.next("game_over");

    a.send(ClientMsg::RematchRequest);
    assert!(b.try_next("rematch_offered").is_some());
    assert!(a.try_next("rematch_offered").is_none());
    b.send(ClientMsg::RematchRespond { accept: true });
    let da = a.next("deck_select");
    let db = b.next("deck_select");
    assert_ne!(da["you"], json!(first_color), "colours are swapped");
    assert_ne!(da["you"], db["you"]);
    assert_eq!(da["rated"], true, "a ranked game gets a ranked rematch");
    assert_eq!(da["opponent"]["username"], "bob");

    // It is a real game: the rematch chance is spent.
    a.send(ClientMsg::RematchRequest);
    assert_eq!(a.error_code(), "no_rematch");
}

#[tokio::test]
async fn friendly_games_get_friendly_rematches() {
    let (app, _) = new_app(HubConfig::default());
    let (mut a, mut b) = finished_game(&app);
    b.send(ClientMsg::RematchRequest);
    assert!(a.try_next("rematch_offered").is_some());
    a.send(ClientMsg::RematchRespond { accept: true });
    assert_eq!(a.next("deck_select")["rated"], false);
    assert_eq!(b.next("deck_select")["rated"], false);
}

#[tokio::test]
async fn a_declined_rematch_is_over() {
    let (app, _) = new_app(HubConfig::default());
    let (mut a, mut b) = finished_game(&app);
    a.send(ClientMsg::RematchRequest);
    b.send(ClientMsg::RematchRespond { accept: false });
    assert!(a.try_next("rematch_declined").is_some());
    a.send(ClientMsg::RematchRequest);
    assert_eq!(a.error_code(), "no_rematch");
    b.send(ClientMsg::RematchRespond { accept: true });
    assert_eq!(b.error_code(), "no_rematch");
}

#[tokio::test]
async fn leaving_declines_the_rematch() {
    let (app, _) = new_app(HubConfig::default());
    // The opponent disconnects.
    let (mut a, b) = finished_game(&app);
    app.disconnect(&b.id, b.conn);
    assert!(a.try_next("rematch_declined").is_some());
    a.send(ClientMsg::RematchRequest);
    assert_eq!(a.error_code(), "no_rematch");

    // The opponent starts looking for another game.
    let (a, mut b) = finished_game(&app);
    a.send(ClientMsg::QueueJoin {
        ranked: None,
        time: None,
    });
    assert!(b.try_next("rematch_declined").is_some());
    b.send(ClientMsg::RematchRequest);
    assert_eq!(b.error_code(), "no_rematch");
    a.send(ClientMsg::LeaveLobby);

    // The requester leaves after asking.
    let (a, mut b) = finished_game(&app);
    a.send(ClientMsg::RematchRequest);
    assert!(b.try_next("rematch_offered").is_some());
    app.disconnect(&a.id, a.conn);
    assert!(b.try_next("rematch_declined").is_some());
    b.send(ClientMsg::RematchRespond { accept: true });
    assert_eq!(b.error_code(), "no_rematch");
}

#[tokio::test]
async fn crossing_rematch_requests_start_the_game() {
    let (app, _) = new_app(HubConfig::default());
    let (mut a, mut b) = finished_game(&app);
    a.send(ClientMsg::RematchRequest);
    b.send(ClientMsg::RematchRequest);
    assert!(a.try_next("deck_select").is_some());
    assert!(b.try_next("deck_select").is_some());
}

#[tokio::test]
async fn rematches_need_a_finished_game() {
    let (app, _) = new_app(HubConfig::default());
    let mut a = guest(&app);
    a.send(ClientMsg::RematchRequest);
    assert_eq!(a.error_code(), "no_rematch");
    a.send(ClientMsg::RematchRespond { accept: true });
    assert_eq!(a.error_code(), "no_rematch");
    let (mut white, _black) = friendly_pair(&app);
    white.send(ClientMsg::RematchRequest);
    assert_eq!(white.error_code(), "no_rematch", "mid-game");
}
