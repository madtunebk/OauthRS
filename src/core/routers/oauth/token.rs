use axum::{extract::State, http::StatusCode, Json};
use uuid::Uuid;

use crate::core::state::AppState;
use crate::libs::{jwt, models::{TokenRequest, TokenResponse}, password, session};

// POST /oauth/token
// Supported grant types:
//   password      — exchange email+password for an access token
//   refresh_token — exchange a valid token for a new one
pub async fn handle(
    State(state): State<AppState>,
    Json(body): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, StatusCode> {
    match body.grant_type.as_str() {
        "password" => {
            let login    = body.login.ok_or(StatusCode::BAD_REQUEST)?;
            let password = body.password.ok_or(StatusCode::BAD_REQUEST)?;

            let (user_id, password_hash) = state.db.find_credentials(&login)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

            if !password::verify(&password, &password_hash) {
                return Err(StatusCode::UNAUTHORIZED);
            }

            issue_token(&state, user_id).await
        }

        "refresh_token" => {
            let refresh_token = body.refresh_token.ok_or(StatusCode::BAD_REQUEST)?;

            let claims = jwt::verify(&refresh_token, &state.config.jwt_secret)
                .map_err(|_| StatusCode::UNAUTHORIZED)?;

            let user_id = Uuid::parse_str(&claims.sub)
                .map_err(|_| StatusCode::UNAUTHORIZED)?;

            // the token must still be the active session — a logged-out or
            // revoked token cannot be exchanged for a new one
            let key = format!("session:{}", user_id);
            let stored = session::get(&state.sessions, &key)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if stored.as_deref() != Some(refresh_token.as_str()) {
                return Err(StatusCode::UNAUTHORIZED);
            }

            // revoke old token before issuing new one
            let _ = session::del(&state.sessions, &format!("session:{}", user_id)).await;

            issue_token(&state, user_id).await
        }

        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn issue_token(state: &AppState, user_id: Uuid) -> Result<Json<TokenResponse>, StatusCode> {
    let token = jwt::sign(user_id, &state.config.jwt_secret, state.config.jwt_expiry_secs);

    session::set(
        &state.sessions,
        &format!("session:{}", user_id),
        &token,
        state.config.jwt_expiry_secs,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type:   "Bearer".to_string(),
        expires_in:   state.config.jwt_expiry_secs,
    }))
}
