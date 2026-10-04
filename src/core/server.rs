use axum::{Router, middleware as axum_middleware, routing::{get, post}};
use tokio::net::TcpListener;

use super::routers::{auth, home, login, logout, signup, invite, oauth, google};
use super::middleware::handle_errors;
use super::state::AppState;
use crate::libs::config::Config;
use crate::libs::db::Database;
use crate::libs::session::SessionStore;

pub async fn start_server(config: Config, db: Database, sessions: SessionStore) {
    let state = AppState { db, sessions, config };
    let addr  = format!("{}:{}", state.config.host, state.config.port);

    let api_routes = Router::new()
        .route("/login",        get(login::form).post(login::handle))
        .route("/login/form",   post(login::handle_form))
        .route("/signup",       get(signup::form).post(signup::handle_form))
        .route("/signup/form",  post(signup::handle_form))
        .route("/logout",       post(logout::handle))
        .route("/invite",       post(invite::handle));

    let oauth_routes = Router::new()
        .route("/token",     post(oauth::token::handle))
        .route("/revoke",    post(oauth::revoke::handle))
        .route("/authorize", get(oauth::authorize::handle));

    let app = Router::new()
        .route("/",       get(home::handle))
        // nginx auth_request subrequest
        .route("/auth",   get(auth::handle))
        // top-level routes nginx proxies directly
        .route("/login",  get(login::form).post(login::handle_form))
        .route("/logout", post(logout::handle))
        .route("/signup", get(signup::form).post(signup::handle_form))
        // Google OAuth
        .route("/auth/google",          get(google::initiate))
        .route("/auth/google/callback", get(google::callback))
        .nest("/api",   api_routes)
        .nest("/oauth", oauth_routes)
        .with_state(state)
        .layer(axum_middleware::from_fn(handle_errors));

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind to address");
    tracing::info!("Server running on http://{}", addr);
    axum::serve(listener, app).await.expect("Server error");
}
