use axum::{extract::State, http::StatusCode, Json};
use uuid::Uuid;

use crate::core::state::AppState;
use crate::libs::{jwt, models::RevokeRequest, session};

// POST /oauth/revoke
// Validates the token, then deletes the session from Redis immediately.
// The JWT itself expires on its own (TTL), but the session is gone instantly.
pub async fn handle(
    State(state): State<AppState>,
    Json(body): Json<RevokeRequest>,
) -> StatusCode {
    let claims = match jwt::verify(&body.token, &state.config.jwt_secret) {
        Ok(c)  => c,
        Err(_) => return StatusCode::UNAUTHORIZED,
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id)  => id,
        Err(_) => return StatusCode::UNAUTHORIZED,
    };

    let _ = session::del(&state.sessions, &format!("session:{}", user_id)).await;

    StatusCode::OK
}
