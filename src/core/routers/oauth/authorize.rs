use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};

use crate::core::routers::auth::has_active_session;
use crate::core::state::AppState;

// GET /oauth/authorize
// Checks if the request carries a valid, active session (header or cookie).
// Valid   → returns authorized user info (or consent page later)
// Invalid → redirects to /api/login
pub async fn handle(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if has_active_session(&state, &headers).await {
        Redirect::to("/").into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}
