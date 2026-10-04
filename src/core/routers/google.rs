use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use uuid::Uuid;

use crate::core::state::AppState;
use crate::libs::{jwt, session};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const STATE_TTL: u64 = 300; // 5 minutes

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code:  Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    id:             String,
    email:          String,
    name:           Option<String>,
    given_name:     Option<String>,
}

// GET /auth/google — redirect to Google consent screen
pub async fn initiate(State(state): State<AppState>) -> impl IntoResponse {
    if state.config.google_client_id.is_empty() {
        return Redirect::to("/login").into_response();
    }

    let oauth_state: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let key = format!("oauth_state:{}", oauth_state);
    let _ = session::set(&state.sessions, &key, "1", STATE_TTL).await;

    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope=email+profile&state={}",
        GOOGLE_AUTH_URL,
        urlencoding::encode(&state.config.google_client_id),
        urlencoding::encode(&state.config.google_redirect_uri),
        oauth_state,
    );

    Redirect::to(&url).into_response()
}

// GET /auth/google/callback
pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    // User denied or error
    if params.error.is_some() || params.code.is_none() {
        return Redirect::to("/login").into_response();
    }

    let code        = params.code.unwrap();
    let oauth_state = params.state.unwrap_or_default();

    // Verify state (CSRF)
    let state_key = format!("oauth_state:{}", oauth_state);
    let valid = session::get(&state.sessions, &state_key).await.unwrap_or(None);
    if valid.is_none() {
        return Redirect::to("/login").into_response();
    }
    let _ = session::del(&state.sessions, &state_key).await;

    // Exchange code for access token
    let client = reqwest::Client::new();
    let token_res = client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code",          code.as_str()),
            ("client_id",     &state.config.google_client_id),
            ("client_secret", &state.config.google_client_secret),
            ("redirect_uri",  &state.config.google_redirect_uri),
            ("grant_type",    "authorization_code"),
        ])
        .send()
        .await;

    let access_token = match token_res {
        Ok(r) => match r.json::<GoogleTokenResponse>().await {
            Ok(t) => t.access_token,
            Err(_) => return Redirect::to("/login").into_response(),
        },
        Err(_) => return Redirect::to("/login").into_response(),
    };

    // Get user info from Google
    let user_info = match client
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(&access_token)
        .send()
        .await
    {
        Ok(r) => match r.json::<GoogleUserInfo>().await {
            Ok(u) => u,
            Err(_) => return Redirect::to("/login").into_response(),
        },
        Err(_) => return Redirect::to("/login").into_response(),
    };

    // Find or create user
    let user_id = match find_or_create_user(&state, &user_info).await {
        Ok(id) => id,
        Err(StatusCode::FORBIDDEN) => return Redirect::to("/signup?error=invite_required").into_response(),
        Err(_) => return Redirect::to("/login").into_response(),
    };

    // Create session
    let token = jwt::sign(user_id, &state.config.jwt_secret, state.config.jwt_expiry_secs);
    let _ = session::set(
        &state.sessions,
        &format!("session:{}", user_id),
        &token,
        state.config.jwt_expiry_secs,
    ).await;

    axum::response::Response::builder()
        .status(302)
        .header("Location", "/")
        .header("Set-Cookie", session::session_cookie(&token, state.config.cookie_secure))
        .body(axum::body::Body::empty())
        .unwrap()
        .into_response()
}

async fn find_or_create_user(state: &AppState, info: &GoogleUserInfo) -> Result<Uuid, StatusCode> {
    // Try find by google_id
    let existing = state.db.find_active_by_google_id(&info.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(id) = existing {
        return Ok(id);
    }

    // Try find by email — link google_id to existing account
    let by_email = state.db.link_google_by_email(&info.id, &info.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(id) = by_email {
        return Ok(id);
    }

    // New user — block if invite required
    if state.config.invite_required {
        return Err(StatusCode::FORBIDDEN);
    }

    // New user — generate username from name or email prefix
    let base = info.given_name.as_deref()
        .or(info.name.as_deref())
        .unwrap_or(&info.email)
        .split('@').next().unwrap_or("user")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(20)
        .collect::<String>();

    let username = make_unique_username(state, &base).await?;

    let id = state.db.create_google_user(&info.email, &username, &info.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(id)
}

async fn make_unique_username(state: &AppState, base: &str) -> Result<String, StatusCode> {
    let base = if base.is_empty() { "user" } else { base };

    // Try base first, then base2, base3, ...
    let existing = state.db.username_exists(base)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !existing {
        return Ok(base.to_string());
    }

    for i in 2..=99 {
        let candidate = format!("{}{}", base, i);
        let taken = state.db.username_exists(&candidate)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if !taken {
            return Ok(candidate);
        }
    }

    // Fallback to UUID suffix
    Ok(format!("{}{}", base, &Uuid::new_v4().to_string()[..8]))
}
