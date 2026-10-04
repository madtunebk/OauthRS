use crate::libs::config::Config;
use crate::libs::db::Database;
use crate::libs::session::SessionStore;

#[derive(Clone)]
pub struct AppState {
    pub db:       Database,
    pub sessions: SessionStore,
    pub config:   Config,
}
