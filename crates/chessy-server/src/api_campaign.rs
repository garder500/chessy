//! `GET /api/campaign`: the campaign levels and the player's progress.

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::api::{authenticate_account, ApiResult};
use crate::app::App;
use crate::boss_forge_store::{BossForgeRow, BossForgeState};
use crate::campaign::{
    boss_unlocked, chapter_stars, chapter_unlocked, forge_table, level_unlocked, star_flags,
    title_earned, titles_earned, total_stars, Chapter, Level, LevelRef, BOSS_STARS, CHAPTERS,
    HINT_AFTER_DEFEATS, MAX_STARS,
};
use crate::campaign_store::CampaignRow;
use crate::hub::MAX_DECK;
use chessy_engine::SkillId;

/// What the store knows of a player, read once per request.
struct Progress {
    rows: Vec<CampaignRow>,
    defeats: Vec<(LevelRef, u32)>,
    forges: Vec<BossForgeRow>,
    deck_full: bool,
}

fn level_json(chapter: u8, index: usize, level: &Level, progress: &Progress) -> Value {
    let at = LevelRef {
        chapter,
        level: index as u8,
    };
    let row = progress.rows.iter().find(|r| r.at == at);
    let defeats = progress
        .defeats
        .iter()
        .find(|(l, _)| *l == at)
        .map_or(0, |&(_, n)| n);
    json!({
        "level": at.level,
        "name": level.name,
        "elo": at.elo(),
        "boss": at.is_boss(),
        "player_deck": level.player_deck,
        "bot_deck": level.bot_deck,
        "lent": level.lent,
        "deck_choice": level.deck_choice,
        "start_fen": level.start.as_ref().map(|s| s.fen),
        "human_color": level.start.as_ref().map(|s| s.human),
        "move_limit": level.move_limit(),
        "objective": level.objective.map(|o| o.text()),
        "challenge": level.challenge.map(|o| o.text()),
        "hint": (defeats >= HINT_AFTER_DEFEATS).then_some(level.hint),
        "best": star_flags(row.map_or(0, |r| r.stars)),
        "rewarded": row.is_some_and(|r| r.rewarded),
        "unlocked": level_unlocked(&progress.rows, at),
    })
}

fn state_name(state: BossForgeState) -> &'static str {
    match state {
        BossForgeState::Forging => "forging",
        BossForgeState::Pending => "pending",
        BossForgeState::Placed => "placed",
    }
}

fn boss_forge_json(chapter: u8, progress: &Progress) -> Value {
    let Some(forge) = progress.forges.iter().find(|f| f.chapter == chapter) else {
        return Value::Null;
    };
    json!({
        "state": state_name(forge.state),
        "skill": forge.skill_id.map(SkillId::Forged),
        "deck_full": progress.deck_full,
        "legendary_unavailable": false,
    })
}

fn forge_table_json(chapter: u8) -> Vec<Value> {
    forge_table(chapter)
        .weights
        .iter()
        .map(|&(rarity, percent)| json!({ "rarity": rarity, "percent": percent }))
        .collect()
}

fn chapter_json(chapter: u8, content: &Chapter, progress: &Progress) -> Value {
    let rows = &progress.rows;
    let levels: Vec<Value> = content
        .levels
        .iter()
        .enumerate()
        .map(|(i, level)| level_json(chapter, i, level, progress))
        .collect();
    let titles: Vec<&str> = titles_earned(rows)
        .into_iter()
        .filter(|&(c, _)| c == chapter)
        .map(|(_, name)| name)
        .collect();
    json!({
        "chapter": chapter,
        "family": content.family,
        "name": content.name,
        "title": content.title,
        "title_earned": title_earned(rows, chapter),
        "titles": titles,
        "available": content.available(),
        "unlocked": chapter_unlocked(rows, chapter),
        "stars": chapter_stars(rows, chapter),
        "boss_stars_required": BOSS_STARS,
        "boss_unlocked": boss_unlocked(rows, chapter),
        "forge_table": forge_table_json(chapter),
        "boss_forge": boss_forge_json(chapter, progress),
        "levels": levels,
    })
}

fn read_progress(app: &App, player: &str) -> ApiResult<Progress> {
    let store = app.store();
    Ok(Progress {
        rows: store.campaign_rows(player)?,
        defeats: store.campaign_defeats(player)?,
        forges: store.boss_forges(player)?,
        deck_full: store.deck(player)?.len() >= MAX_DECK,
    })
}

/// `{total_stars, max_stars, chapters: [...]}`; a chapter without content has
/// `available: false` and no levels. Accounts only.
pub(crate) async fn campaign(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let player = authenticate_account(&app, &headers)?;
    let progress = read_progress(&app, &player)?;
    let chapters: Vec<Value> = CHAPTERS
        .iter()
        .enumerate()
        .map(|(i, content)| chapter_json(i as u8, content, &progress))
        .collect();
    Ok(Json(json!({
        "total_stars": total_stars(&progress.rows),
        "max_stars": MAX_STARS,
        "chapters": chapters,
    }))
    .into_response())
}
