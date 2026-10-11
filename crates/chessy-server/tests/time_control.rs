//! The game length is persisted and exposed (docs/spec-v4.md, « Cadence »):
//! `time_control` in `GET /api/me/games` and `GET /api/games/{id}`.

mod common;

use chessy_engine::{Action, Outcome};
use chessy_server::games_store::GameKind;
use chessy_server::hub::HubConfig;
use chessy_server::protocol::{ClientMsg, TimeControl};
use chessy_server::store::{GameRecord, Store};
use chessy_server::App;
use common::*;
use serde_json::Value;

fn record(store: &Store, id: &str, w: &str, b: &str, tc: Option<TimeControl>) {
    let none: Vec<Action> = Vec::new();
    store
        .record_game(&GameRecord {
            id,
            white: w,
            black: b,
            outcome: &Outcome::Stalemate,
            reason: "stalemate",
            plies: 0,
            rated: false,
            started_unix: 0,
            kind: GameKind::Room,
            loadouts: &[vec![], vec![]],
            start_fen: None,
            actions: &none,
            solo_elo: None,
            time_control: tc,
        })
        .unwrap();
}

#[tokio::test]
async fn the_schema_is_at_version_12_with_a_nullable_column() {
    let db = TempDb::new();
    let _store = Store::open(db.path_str()).unwrap();
    let raw = db.raw();
    let version: i64 = raw
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 12);
    let notnull: i64 = raw
        .query_row(
            "SELECT \"notnull\" FROM pragma_table_info('games') WHERE name = 'time_control'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(notnull, 0);
}

#[tokio::test]
async fn each_length_round_trips_and_null_or_unknown_text_reads_as_none() {
    let db = TempDb::new();
    let store = Store::open(db.path_str()).unwrap();
    let app = App::new(store.clone(), HubConfig::default());
    let api = Api::new(&app);
    let a = account(&app, &store, "alice");
    let b = account(&app, &store, "bobby");
    let (aid, bid) = (a.id.clone(), b.id.clone());
    for (i, tc) in [
        Some(TimeControl::Short),
        Some(TimeControl::Medium),
        Some(TimeControl::Long),
        None,
        None,
    ]
    .into_iter()
    .enumerate()
    {
        record(&store, &format!("g{i}"), &aid, &bid, tc);
    }
    // g3 stays NULL (an old row looks the same); g4 holds text we do not know.
    let raw = db.raw();
    let stored: Option<String> = raw
        .query_row("SELECT time_control FROM games WHERE id = 'g3'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(stored, None);
    let stored: String = raw
        .query_row("SELECT time_control FROM games WHERE id = 'g1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(stored, "medium");
    raw.execute(
        "UPDATE games SET time_control = 'bogus' WHERE id = 'g4'",
        [],
    )
    .unwrap();

    let expected = [
        ("g0", Value::from("short")),
        ("g1", Value::from("medium")),
        ("g2", Value::from("long")),
        ("g3", Value::Null),
        ("g4", Value::Null),
    ];
    let (_, list) = api.get("/api/me/games", Some(&a.token)).await;
    for (id, want) in &expected {
        let row = list["games"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["game_id"] == *id)
            .unwrap();
        assert!(row.as_object().unwrap().contains_key("time_control"));
        assert_eq!(&row["time_control"], want, "list {id}");
        let (_, replay) = api.get(&format!("/api/games/{id}"), None).await;
        assert!(replay.as_object().unwrap().contains_key("time_control"));
        assert_eq!(&replay["time_control"], want, "replay {id}");
    }
}

#[tokio::test]
async fn a_played_game_keeps_the_length_it_was_queued_with() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    for (time, want) in [
        (Some(TimeControl::Short), Value::from("short")),
        (None, Value::Null),
    ] {
        let name = |s: &str| format!("{s}{}", time.map_or("d", |t| t.as_str()));
        let a = account(&app, &store, &name("aa"));
        let b = account(&app, &store, &name("bb"));
        for c in [&a, &b] {
            c.send(ClientMsg::QueueJoin { ranked: None, time });
        }
        let (white, black) = into_game(a, b);
        fools_mate(&white, &black);
        let (_, list) = api.get("/api/me/games", Some(&white.token)).await;
        assert_eq!(list["games"][0]["time_control"], want, "{list}");
        let id = list["games"][0]["game_id"].as_str().unwrap();
        let (_, replay) = api.get(&format!("/api/games/{id}"), None).await;
        assert_eq!(replay["time_control"], want);
    }
}
