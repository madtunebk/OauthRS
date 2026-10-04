use axum::{extract::State, http::{HeaderMap, StatusCode}, Json};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::core::state::AppState;
use crate::libs::session;

// POST /api/invite
// Protected by X-Admin-Secret header.
// Generates a single-use invite code stored in Redis with TTL.
pub async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let secret = headers
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if secret != state.config.admin_secret {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let code = Uuid::new_v4().to_string();

    session::set(
        &state.sessions,
        &format!("invite:{}", code),
        "1",
        state.config.invite_ttl_secs,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "invite_code": code,
        "expires_in":  state.config.invite_ttl_secs,
    })))
}
