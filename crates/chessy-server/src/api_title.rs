//! `POST /api/profile/title`: choose which earned campaign title the profile shows.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{authenticate_account, parse_body, ApiError, ApiResult};
use crate::app::App;
use crate::campaign::titles_earned;

#[derive(Deserialize)]
struct TitleRequest {
    chapter: Option<u8>,
}

/// `[{chapter, name}]`: the titles a player has earned.
pub(crate) fn earned_titles(app: &App, player: &str) -> ApiResult<Value> {
    let rows = app.store().campaign_rows(player)?;
    let titles: Vec<Value> = titles_earned(&rows)
        .into_iter()
        .map(|(chapter, name)| json!({ "chapter": chapter, "name": name }))
        .collect();
    Ok(Value::Array(titles))
}

/// Body `{chapter: u8 | null}`; `null` goes back to the best earned title; 400 `title_not_earned` for a title not won.
pub(crate) async fn set_title(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> ApiResult<Response> {
    let player = authenticate_account(&app, &headers)?;
    let request: TitleRequest = parse_body(body)?;
    if let Some(chapter) = request.chapter {
        let rows = app.store().campaign_rows(&player)?;
        if !titles_earned(&rows).iter().any(|&(c, _)| c == chapter) {
            return Err(ApiError::bad_request("title_not_earned"));
        }
    }
    app.store().set_title_active(&player, request.chapter)?;
    Ok(Json(json!({ "chapter": request.chapter })).into_response())
}
