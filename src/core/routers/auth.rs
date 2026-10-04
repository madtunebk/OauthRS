use axum::{extract::State, http::{header, HeaderMap, StatusCode}};

use crate::core::state::AppState;
use crate::libs::{jwt, session};

// GET /auth — nginx auth_request subrequest endpoint
// Returns 200 if the request carries a valid JWT (header or cookie)
// Returns 401 if missing or invalid — nginx then redirects to /login
pub async fn handle(State(state): State<AppState>, headers: HeaderMap) -> StatusCode {
    if has_active_session(&state, &headers).await {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    }
}

/// True if the request carries a valid JWT that is still the user's active
/// session (not logged out, revoked or replaced).
pub async fn has_active_session(state: &AppState, headers: &HeaderMap) -> bool {
    let token = match extract_token(headers) {
        Some(t) => t,
        None => return false,
    };

    let claims = match jwt::verify(&token, &state.config.jwt_secret) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let key = format!("session:{}", claims.sub);
    matches!(session::get(&state.sessions, &key).await, Ok(Some(s)) if s == token)
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    // 1. Authorization: Bearer <token>
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        // 2. session cookie
        .or_else(|| {
            headers
                .get(header::COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find(|c| c.trim().starts_with("session="))
                        .map(|c| c.trim().trim_start_matches("session=").to_string())
                })
        })
}
