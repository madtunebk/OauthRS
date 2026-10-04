use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    Form,
};
use serde::Deserialize;
use tera::Context;
use uuid::Uuid;

use crate::core::state::AppState;
use crate::libs::{jwt, models::{AuthResponse, LoginRequest}, password, session, templates};

// GET /api/login — serve the login form
pub async fn form(State(state): State<AppState>) -> Html<String> {
    Html(templates::render("login.tpl", &page_context(&state)))
}

// POST /api/login — JSON (API clients)
pub async fn handle(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let (user_id, password_hash) = fetch_user(&state, &body.login).await?;

    if !password::verify(&body.password, &password_hash) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = jwt::sign(user_id, &state.config.jwt_secret, state.config.jwt_expiry_secs);
    session::set(&state.sessions, &format!("session:{}", user_id), &token, state.config.jwt_expiry_secs)
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AuthResponse { token, token_type: "Bearer".to_string(), expires_in: state.config.jwt_expiry_secs }))
}

// POST /api/login/form — form submission (browser)
#[derive(Deserialize)]
pub struct LoginForm {
    pub login:    String,
    pub password: String,
}

pub async fn handle_form(
    State(state): State<AppState>,
    Form(body): Form<LoginForm>,
) -> Response {
    let result = fetch_user(&state, &body.login).await;

    let (user_id, password_hash) = match result {
        Ok(u)  => u,
        Err(_) => return render_error(&state, "Invalid email or password."),
    };

    if !password::verify(&body.password, &password_hash) {
        return render_error(&state, "Invalid email or password.");
    }

    let token = jwt::sign(user_id, &state.config.jwt_secret, state.config.jwt_expiry_secs);
    let _ = session::set(&state.sessions, &format!("session:{}", user_id), &token, state.config.jwt_expiry_secs).await;

    // set session cookie and redirect back to home
    axum::response::Response::builder()
        .status(302)
        .header("Location", "/")
        .header("Set-Cookie", session::session_cookie(&token, state.config.cookie_secure))
        .body(axum::body::Body::empty())
        .unwrap()
        .into_response()
}

async fn fetch_user(state: &AppState, login: &str) -> Result<(Uuid, String), StatusCode> {
    state.db.find_active_credentials(login)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn render_error(state: &AppState, msg: &str) -> Response {
    let mut ctx = page_context(state);
    ctx.insert("error", msg);
    Html(templates::render("login.tpl", &ctx)).into_response()
}

fn page_context(state: &AppState) -> Context {
    let mut ctx = Context::new();
    ctx.insert("google_enabled", &state.config.google_enabled);
    ctx
}
