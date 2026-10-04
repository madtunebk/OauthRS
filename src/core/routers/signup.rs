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
use crate::libs::{jwt, models::{AuthResponse, SignupRequest}, password, session, templates};

// GET /api/signup — serve the signup form
pub async fn form() -> Html<String> {
    Html(templates::render("signup.tpl", &Context::new()))
}

// POST /api/signup — JSON (API clients)
pub async fn handle(
    State(state): State<AppState>,
    Json(body): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let user_id = create_user(&state, &body.email, &body.username, &body.password, &body.invite_code)
        .await
        .map_err(|e| e)?;

    let token = jwt::sign(user_id, &state.config.jwt_secret, state.config.jwt_expiry_secs);
    session::set(&state.sessions, &format!("session:{}", user_id), &token, state.config.jwt_expiry_secs)
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AuthResponse { token, token_type: "Bearer".to_string(), expires_in: state.config.jwt_expiry_secs }))
}

// POST /api/signup/form — form submission (browser)
#[derive(Deserialize)]
pub struct SignupForm {
    pub email:       String,
    pub username:    String,
    pub password:    String,
    pub invite_code: String,
}

pub async fn handle_form(
    State(state): State<AppState>,
    Form(body): Form<SignupForm>,
) -> Response {
    match create_user(&state, &body.email, &body.username, &body.password, &body.invite_code).await {
        Ok(user_id) => {
            let token = jwt::sign(user_id, &state.config.jwt_secret, state.config.jwt_expiry_secs);
            let _ = session::set(&state.sessions, &format!("session:{}", user_id), &token, state.config.jwt_expiry_secs).await;

            axum::response::Response::builder()
                .status(302)
                .header("Location", "/")
                .header("Set-Cookie", session::session_cookie(&token, state.config.cookie_secure))
                .body(axum::body::Body::empty())
                .unwrap()
                .into_response()
        }
        Err(StatusCode::FORBIDDEN)  => render_error("Invalid or expired invite code."),
        Err(StatusCode::CONFLICT)   => render_error("Email or username already taken."),
        Err(_)                      => render_error("Something went wrong. Please try again."),
    }
}

async fn create_user(state: &AppState, email: &str, username: &str, password: &str, invite_code: &str) -> Result<Uuid, StatusCode> {
    let invite_key = format!("invite:{}", invite_code);

    // Claim the invite atomically so concurrent signups cannot share one code
    let invite = if state.config.invite_required {
        let taken = session::take(&state.sessions, &invite_key)
            .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match taken {
            Some(invite) => Some(invite),
            None => return Err(StatusCode::FORBIDDEN),
        }
    } else {
        None
    };

    let password_hash = password::hash(password);

    let result = state.db.create_user(email, username, &password_hash).await;

    if result.is_err() {
        // Signup failed — give the invite back with its remaining TTL
        if let Some((value, ttl)) = invite.filter(|(_, ttl)| *ttl > 0) {
            let _ = session::set(&state.sessions, &invite_key, &value, ttl).await;
        }
    }

    match result {
        Ok(user_id) => Ok(user_id),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(StatusCode::CONFLICT)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn render_error(msg: &str) -> Response {
    let mut ctx = Context::new();
    ctx.insert("error", msg);
    Html(templates::render("signup.tpl", &ctx)).into_response()
}
