use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{PgPool, SqlitePool};
use uuid::{fmt::Hyphenated, Uuid};

/// Application data store. Every query has a PostgreSQL and a SQLite variant;
/// SQLite stores UUIDs as hyphenated text.
#[derive(Clone)]
pub enum Database {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

pub async fn connect(database_url: &str) -> PgPool {
    PgPool::connect(database_url)
        .await
        .expect("Failed to connect to PostgreSQL")
}

pub async fn connect_sqlite(path: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path))
        .expect("Invalid SQLITE_PATH")
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to open SQLite database")
}

impl Database {
    pub async fn run_migrations(&self) {
        match self {
            Database::Postgres(pool) => sqlx::migrate!("./migrations").run(pool).await,
            Database::Sqlite(pool)   => sqlx::migrate!("./migrations_sqlite").run(pool).await,
        }
        .expect("Failed to run migrations");
    }

    /// (id, password_hash) of an active (not deleted / disabled) user, by email or username.
    pub async fn find_active_credentials(&self, login: &str) -> Result<Option<(Uuid, String)>, sqlx::Error> {
        match self {
            Database::Postgres(pool) => sqlx::query_as(
                "SELECT id, password_hash FROM users
                 WHERE (email = $1 OR username = $1)
                   AND deleted_at IS NULL AND disabled_at IS NULL"
            )
            .bind(login)
            .fetch_optional(pool)
            .await,

            Database::Sqlite(pool) => sqlx::query_as::<_, (Hyphenated, String)>(
                "SELECT id, password_hash FROM users
                 WHERE (email = ?1 OR username = ?1)
                   AND deleted_at IS NULL AND disabled_at IS NULL"
            )
            .bind(login)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(|(id, hash)| (id.into_uuid(), hash))),
        }
    }

    /// Whether the user exists and is not deleted or disabled.
    pub async fn is_active_user(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let row: Option<(i32,)> = match self {
            Database::Postgres(pool) => sqlx::query_as(
                "SELECT 1 FROM users WHERE id = $1 AND deleted_at IS NULL AND disabled_at IS NULL"
            )
            .bind(id)
            .fetch_optional(pool)
            .await?,

            Database::Sqlite(pool) => sqlx::query_as(
                "SELECT 1 FROM users WHERE id = ?1 AND deleted_at IS NULL AND disabled_at IS NULL"
            )
            .bind(id.hyphenated())
            .fetch_optional(pool)
            .await?,
        };
        Ok(row.is_some())
    }

    pub async fn create_user(&self, email: &str, username: &str, password_hash: &str) -> Result<Uuid, sqlx::Error> {
        match self {
            Database::Postgres(pool) => sqlx::query_as::<_, (Uuid,)>(
                "INSERT INTO users (email, username, password_hash) VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(email)
            .bind(username)
            .bind(password_hash)
            .fetch_one(pool)
            .await
            .map(|(id,)| id),

            Database::Sqlite(pool) => {
                let id = Uuid::new_v4();
                sqlx::query("INSERT INTO users (id, email, username, password_hash) VALUES (?1, ?2, ?3, ?4)")
                    .bind(id.hyphenated())
                    .bind(email)
                    .bind(username)
                    .bind(password_hash)
                    .execute(pool)
                    .await?;
                Ok(id)
            }
        }
    }

    pub async fn find_active_by_google_id(&self, google_id: &str) -> Result<Option<Uuid>, sqlx::Error> {
        match self {
            Database::Postgres(pool) => sqlx::query_as::<_, (Uuid,)>(
                "SELECT id FROM users WHERE google_id = $1 AND deleted_at IS NULL AND disabled_at IS NULL"
            )
            .bind(google_id)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(|(id,)| id)),

            Database::Sqlite(pool) => sqlx::query_as::<_, (Hyphenated,)>(
                "SELECT id FROM users WHERE google_id = ?1 AND deleted_at IS NULL AND disabled_at IS NULL"
            )
            .bind(google_id)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(|(id,)| id.into_uuid())),
        }
    }

    /// Links `google_id` to the active user with `email`, returning that user's id.
    pub async fn link_google_by_email(&self, google_id: &str, email: &str) -> Result<Option<Uuid>, sqlx::Error> {
        match self {
            Database::Postgres(pool) => sqlx::query_as::<_, (Uuid,)>(
                "UPDATE users SET google_id = $1 WHERE email = $2 AND deleted_at IS NULL AND disabled_at IS NULL RETURNING id"
            )
            .bind(google_id)
            .bind(email)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(|(id,)| id)),

            Database::Sqlite(pool) => sqlx::query_as::<_, (Hyphenated,)>(
                "UPDATE users SET google_id = ?1 WHERE email = ?2 AND deleted_at IS NULL AND disabled_at IS NULL RETURNING id"
            )
            .bind(google_id)
            .bind(email)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(|(id,)| id.into_uuid())),
        }
    }

    pub async fn create_google_user(&self, email: &str, username: &str, google_id: &str) -> Result<Uuid, sqlx::Error> {
        match self {
            Database::Postgres(pool) => sqlx::query_as::<_, (Uuid,)>(
                "INSERT INTO users (email, username, google_id) VALUES ($1, $2, $3) RETURNING id"
            )
            .bind(email)
            .bind(username)
            .bind(google_id)
            .fetch_one(pool)
            .await
            .map(|(id,)| id),

            Database::Sqlite(pool) => {
                let id = Uuid::new_v4();
                sqlx::query("INSERT INTO users (id, email, username, google_id) VALUES (?1, ?2, ?3, ?4)")
                    .bind(id.hyphenated())
                    .bind(email)
                    .bind(username)
                    .bind(google_id)
                    .execute(pool)
                    .await?;
                Ok(id)
            }
        }
    }

    pub async fn username_exists(&self, username: &str) -> Result<bool, sqlx::Error> {
        let row: Option<(String,)> = match self {
            Database::Postgres(pool) => sqlx::query_as("SELECT username FROM users WHERE username = $1")
                .bind(username)
                .fetch_optional(pool)
                .await?,

            Database::Sqlite(pool) => sqlx::query_as("SELECT username FROM users WHERE username = ?1")
                .bind(username)
                .fetch_optional(pool)
                .await?,
        };
        Ok(row.is_some())
    }
}

#[cfg(test)]
pub(crate) async fn temp_sqlite() -> (SqlitePool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("oauthrs-test-{}.db", Uuid::new_v4()));
    let pool = connect_sqlite(path.to_str().unwrap()).await;
    Database::Sqlite(pool.clone()).run_migrations().await;
    (pool, path)
}

#[cfg(test)]
pub(crate) async fn remove_temp_sqlite(pool: SqlitePool, path: std::path::PathBuf) {
    pool.close().await;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenvy::dotenv;
    use std::env;

    #[tokio::test]
    async fn test_postgres_connection() {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = connect(&database_url).await;

        let row: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("Query failed");

        assert_eq!(row.0, 1);
        println!("PostgreSQL ping: ok");

        let version: (String,) = sqlx::query_as("SELECT version()")
            .fetch_one(&pool)
            .await
            .expect("Version query failed");

        println!("PostgreSQL version: {}", version.0);
    }

    #[tokio::test]
    async fn test_sqlite_users() {
        let (pool, path) = temp_sqlite().await;
        let db = Database::Sqlite(pool.clone());

        // signup + login lookup by email and username
        let id = db.create_user("a@x.dev", "alice", "hash").await.unwrap();
        assert_eq!(db.find_active_credentials("a@x.dev").await.unwrap(), Some((id, "hash".to_string())));
        assert_eq!(db.find_active_credentials("alice").await.unwrap(), Some((id, "hash".to_string())));
        assert!(db.is_active_user(id).await.unwrap());
        assert!(!db.is_active_user(Uuid::new_v4()).await.unwrap());
        assert!(db.find_active_credentials("nobody").await.unwrap().is_none());

        // duplicates are reported as unique violations (signup maps these to 409)
        match db.create_user("a@x.dev", "other", "hash").await {
            Err(sqlx::Error::Database(e)) => assert!(e.is_unique_violation()),
            other => panic!("expected unique violation, got {:?}", other),
        }

        // over-length values are rejected like PostgreSQL VARCHAR limits
        assert!(db.create_user(&"e".repeat(256), "bob", "hash").await.is_err());

        // google linking and lookup
        assert!(db.find_active_by_google_id("g1").await.unwrap().is_none());
        assert_eq!(db.link_google_by_email("g1", "a@x.dev").await.unwrap(), Some(id));
        assert_eq!(db.find_active_by_google_id("g1").await.unwrap(), Some(id));
        assert!(db.link_google_by_email("g2", "missing@x.dev").await.unwrap().is_none());

        let gid = db.create_google_user("g@x.dev", "gina", "g3").await.unwrap();
        assert_eq!(db.find_active_by_google_id("g3").await.unwrap(), Some(gid));
        assert!(db.username_exists("gina").await.unwrap());
        assert!(!db.username_exists("gina2").await.unwrap());

        // soft-deleted / disabled users are excluded from active lookups
        sqlx::query("UPDATE users SET disabled_at = CURRENT_TIMESTAMP WHERE id = ?1")
            .bind(id.hyphenated())
            .execute(&pool)
            .await
            .unwrap();
        assert!(db.find_active_credentials("alice").await.unwrap().is_none());
        assert!(db.find_active_by_google_id("g1").await.unwrap().is_none());
        assert!(!db.is_active_user(id).await.unwrap());

        remove_temp_sqlite(pool, path).await;
    }
}
