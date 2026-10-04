mod cli;
mod core;
mod libs;

use core::server::start_server;
use libs::config::{Config, Storage};
use libs::db::{self, Database};
use libs::session::{self, SessionStore};

pub const APP_NAME: &str = "OauthRS";
pub const APP_ENV: &str = "dev";

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(cli::run(&args).await);
    }

    tracing_subscriber::fmt::init();

    tracing::info!("Starting {} in {} mode", APP_NAME, APP_ENV);

    let config = Config::load();

    let (database, sessions) = match &config.storage {
        Storage::Postgres { database_url, redis_url } => {
            tracing::info!("Storage: PostgreSQL + Redis");
            let db_pool = db::connect(database_url).await;
            let redis   = session::connect(redis_url);
            (Database::Postgres(db_pool), SessionStore::Redis(redis))
        }
        Storage::Sqlite { path } => {
            tracing::info!("Storage: SQLite standalone ({})", path);
            let pool = db::connect_sqlite(path).await;
            (Database::Sqlite(pool.clone()), SessionStore::Sqlite(pool))
        }
    };

    database.run_migrations().await;
    tracing::info!("Migrations applied");

    session::spawn_purge_task(&sessions);

    start_server(config, database, sessions).await;
}
