//! `GET /api/campaign`: the campaign levels and the player's progress.

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::api::{authenticate, ApiResult};
use crate::app::App;
use crate::campaign::{
    boss_unlocked, chapter_stars, star_flags, Chapter, Level, LevelRef, BOSS_STARS, CHAPTERS,
};
use crate::campaign_store::CampaignRow;

fn level_json(chapter: u8, index: usize, level: &Level, rows: &[CampaignRow]) -> Value {
    let at = LevelRef {
        chapter,
        level: index as u8,
    };
    let row = rows.iter().find(|r| r.at == at);
    json!({
        "level": at.level,
        "name": level.name,
        "elo": at.elo(),
        "boss": at.is_boss(),
        "player_deck": level.player_deck,
        "bot_deck": level.bot_deck,
        "objective": level.objective.map(|o| o.text()),
        "challenge": level.challenge.map(|o| o.text()),
        "best": star_flags(row.map_or(0, |r| r.stars)),
        "rewarded": row.is_some_and(|r| r.rewarded),
    })
}

fn chapter_json(chapter: u8, content: &Chapter, rows: &[CampaignRow]) -> Value {
    let levels: Vec<Value> = content
        .levels
        .iter()
        .enumerate()
        .map(|(i, level)| level_json(chapter, i, level, rows))
        .collect();
    json!({
        "chapter": chapter,
        "family": content.family,
        "name": content.name,
        "available": content.available(),
        "stars": chapter_stars(rows, chapter),
        "boss_stars_required": BOSS_STARS,
        "boss_unlocked": boss_unlocked(rows, chapter),
        "levels": levels,
    })
}

/// `{chapters: [...]}`; a chapter without content has `available: false` and no levels.
pub(crate) async fn campaign(State(app): State<Arc<App>>, headers: HeaderMap) -> ApiResult<Response> {
    let player = authenticate(&app, &headers)?;
    let rows = app.store().campaign_rows(&player)?;
    let chapters: Vec<Value> = CHAPTERS
        .iter()
        .enumerate()
        .map(|(i, content)| chapter_json(i as u8, content, &rows))
        .collect();
    Ok(Json(json!({ "chapters": chapters })).into_response())
}
