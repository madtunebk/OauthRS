use axum::{extract::State, http::StatusCode};

use crate::core::extractors::AuthSession;
use crate::core::state::AppState;
use crate::libs::session;

pub async fn handle(
    State(state): State<AppState>,
    auth: AuthSession,
) -> StatusCode {
    let _ = session::del(&state.sessions, &format!("session:{}", auth.user_id)).await;
    StatusCode::OK
}
