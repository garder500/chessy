//! The campaign REST routes: `/api/campaign` and `/api/profile/title`.

mod common;

use axum::http::StatusCode;
use chessy_server::campaign::{
    LevelRef, BOSS_LEVEL, HINT_AFTER_DEFEATS, STAR_CHALLENGE, STAR_OBJECTIVE, STAR_WIN,
};
use chessy_server::hub::HubConfig;
use common::{new_app, Api};
use serde_json::{json, Value};

const ALL_STARS: u8 = STAR_WIN | STAR_OBJECTIVE | STAR_CHALLENGE;
const FIRST_LEVEL: LevelRef = LevelRef {
    chapter: 0,
    level: 0,
};
const FIRST_BOSS: LevelRef = LevelRef {
    chapter: 0,
    level: BOSS_LEVEL,
};

fn first_level(campaign: &Value) -> &Value {
    &campaign["chapters"][0]["levels"][0]
}

#[tokio::test]
async fn the_campaign_totals_out_of_105() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let token = api.register("ana").await;
    let (status, campaign) = api.get("/api/campaign", Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "{campaign}");
    assert_eq!(campaign["total_stars"], 0);
    assert_eq!(campaign["max_stars"], 105);

    let player = store.account_by_name("ana").unwrap().unwrap().id;
    store
        .record_campaign(&player, FIRST_LEVEL, STAR_WIN)
        .unwrap();
    let (_, campaign) = api.get("/api/campaign", Some(&token)).await;
    assert_eq!(campaign["total_stars"], 1);

    let chapter = &campaign["chapters"][0];
    assert!(chapter["forge_table"][0]["percent"].is_u64());
    assert!(chapter["boss_forge"].is_null());
    let level = first_level(&campaign);
    assert!(level["elo"].is_i64());
    assert!(level["lent"].is_array());
}

#[tokio::test]
async fn the_hint_shows_after_three_defeats_in_a_row() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let token = api.register("ana").await;
    let player = store.account_by_name("ana").unwrap().unwrap().id;

    for _ in 1..HINT_AFTER_DEFEATS {
        store.record_campaign_defeat(&player, 0, 0).unwrap();
    }
    let (_, campaign) = api.get("/api/campaign", Some(&token)).await;
    assert!(first_level(&campaign)["hint"].is_null());

    store.record_campaign_defeat(&player, 0, 0).unwrap();
    let (_, campaign) = api.get("/api/campaign", Some(&token)).await;
    assert!(first_level(&campaign)["hint"].is_string());
}

#[tokio::test]
async fn a_title_is_refused_until_the_boss_gives_three_stars() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let token = api.register("ana").await;
    let player = store.account_by_name("ana").unwrap().unwrap().id;
    let choose = |chapter: Value| {
        api.call(
            "POST",
            "/api/profile/title",
            Some(&token),
            Some(json!({ "chapter": chapter })),
        )
    };

    let (status, body) = choose(json!(0)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "title_not_earned");

    store
        .record_campaign(&player, FIRST_BOSS, ALL_STARS)
        .unwrap();
    let (status, body) = choose(json!(0)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        store.player_row(&player).unwrap().unwrap().title_active,
        Some(0)
    );

    let (_, profile) = api.get("/api/players/ana", Some(&token)).await;
    assert_eq!(profile["title"], "Tombeur du Bélier");
    assert_eq!(profile["titles"][0]["chapter"], 0);

    let (_, seen_by_others) = api.get("/api/players/ana", None).await;
    assert!(seen_by_others.get("titles").is_none());
}

#[tokio::test]
async fn a_guest_is_refused() {
    let (app, store) = new_app(HubConfig::default());
    let api = Api::new(&app);
    let (_, guest_token) = store.create_player().unwrap();

    let (status, _) = api.get("/api/campaign", Some(&guest_token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = api
        .call(
            "POST",
            "/api/profile/title",
            Some(&guest_token),
            Some(json!({ "chapter": null })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = api.get("/api/campaign", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
