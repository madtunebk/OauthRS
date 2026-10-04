use axum::{extract::State, http::{header, HeaderMap, StatusCode}};

use crate::core::state::AppState;
use crate::libs::{jwt, session};

// GET /auth — nginx auth_request subrequest endpoint
// Returns 200 if the request carries a valid JWT (header or cookie)
// Returns 401 if missing or invalid — nginx then redirects to /login
pub async fn handle(State(state): State<AppState>, headers: HeaderMap) -> StatusCode {
    let token = match extract_token(&headers) {
        Some(t) => t,
        None => return StatusCode::UNAUTHORIZED,
    };

    let claims = match jwt::verify(&token, &state.config.jwt_secret) {
        Ok(c) => c,
        Err(_) => return StatusCode::UNAUTHORIZED,
    };

    let key = format!("session:{}", claims.sub);
    let stored = match session::get(&state.sessions, &key).await {
        Ok(v) => v,
        Err(_) => return StatusCode::UNAUTHORIZED,
    };

    match stored {
        Some(s) if s == token => StatusCode::OK,
        _ => StatusCode::UNAUTHORIZED,
    }
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
