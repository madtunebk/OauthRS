use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redis::{Client, AsyncCommands};
use sqlx::SqlitePool;

/// Sessions and temporary state (invites, OAuth state) with per-key TTL.
#[derive(Clone)]
pub enum SessionStore {
    Redis(Client),
    /// Uses the `kv_store` table; expired keys are ignored on read and purged periodically.
    Sqlite(SqlitePool),
}

#[derive(Debug)]
pub enum SessionError {
    Redis(redis::RedisError),
    Sqlite(sqlx::Error),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Redis(e)  => write!(f, "redis: {}", e),
            SessionError::Sqlite(e) => write!(f, "sqlite: {}", e),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<redis::RedisError> for SessionError {
    fn from(e: redis::RedisError) -> Self { SessionError::Redis(e) }
}

impl From<sqlx::Error> for SessionError {
    fn from(e: sqlx::Error) -> Self { SessionError::Sqlite(e) }
}

pub type SessionResult<T> = Result<T, SessionError>;

const PURGE_INTERVAL: Duration = Duration::from_secs(60);

pub fn connect(redis_url: &str) -> Client {
    Client::open(redis_url).expect("Failed to connect to Redis")
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

pub async fn set(store: &SessionStore, key: &str, value: &str, ttl_secs: u64) -> SessionResult<()> {
    match store {
        SessionStore::Redis(client) => {
            let mut conn = client.get_multiplexed_async_connection().await?;
            Ok(conn.set_ex(key, value, ttl_secs).await?)
        }
        SessionStore::Sqlite(pool) => {
            sqlx::query(
                "INSERT INTO kv_store (key, value, expires_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, expires_at = excluded.expires_at"
            )
            .bind(key)
            .bind(value)
            .bind(now().saturating_add(ttl_secs.min(i64::MAX as u64) as i64))
            .execute(pool)
            .await?;
            Ok(())
        }
    }
}

pub async fn get(store: &SessionStore, key: &str) -> SessionResult<Option<String>> {
    match store {
        SessionStore::Redis(client) => {
            let mut conn = client.get_multiplexed_async_connection().await?;
            Ok(conn.get(key).await?)
        }
        SessionStore::Sqlite(pool) => {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT value FROM kv_store WHERE key = ?1 AND expires_at > ?2"
            )
            .bind(key)
            .bind(now())
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|(v,)| v))
        }
    }
}

pub async fn del(store: &SessionStore, key: &str) -> SessionResult<()> {
    match store {
        SessionStore::Redis(client) => {
            let mut conn = client.get_multiplexed_async_connection().await?;
            Ok(conn.del(key).await?)
        }
        SessionStore::Sqlite(pool) => {
            sqlx::query("DELETE FROM kv_store WHERE key = ?1")
                .bind(key)
                .execute(pool)
                .await?;
            Ok(())
        }
    }
}

/// Atomically reads and deletes `key`, returning its value and remaining TTL in seconds.
/// Two concurrent callers can never both receive the same value.
pub async fn take(store: &SessionStore, key: &str) -> SessionResult<Option<(String, u64)>> {
    match store {
        SessionStore::Redis(client) => {
            let mut conn = client.get_multiplexed_async_connection().await?;
            // MULTI/EXEC: GET + TTL + DEL run as one atomic transaction
            let (value, ttl, _): (Option<String>, i64, i64) = redis::pipe()
                .atomic()
                .get(key)
                .ttl(key)
                .del(key)
                .query_async(&mut conn)
                .await?;
            Ok(value.map(|v| (v, ttl.max(0) as u64)))
        }
        SessionStore::Sqlite(pool) => {
            let now = now();
            let row: Option<(String, i64)> = sqlx::query_as(
                "DELETE FROM kv_store WHERE key = ?1 AND expires_at > ?2 RETURNING value, expires_at"
            )
            .bind(key)
            .bind(now)
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|(v, exp)| (v, (exp - now).max(0) as u64)))
        }
    }
}

/// `Set-Cookie` value for the session token.
pub fn session_cookie(token: &str, secure: bool) -> String {
    let mut cookie = format!("session={}; Path=/; HttpOnly; SameSite=Lax", token);
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Deletes expired keys from the SQLite store (Redis expires keys itself).
pub async fn purge_expired(store: &SessionStore) -> SessionResult<u64> {
    match store {
        SessionStore::Redis(_) => Ok(0),
        SessionStore::Sqlite(pool) => Ok(
            sqlx::query("DELETE FROM kv_store WHERE expires_at <= ?1")
                .bind(now())
                .execute(pool)
                .await?
                .rows_affected()
        ),
    }
}

/// Spawns a background task that periodically purges expired keys (SQLite only).
pub fn spawn_purge_task(store: &SessionStore) {
    if !matches!(store, SessionStore::Sqlite(_)) {
        return;
    }
    let store = store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(PURGE_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(e) = purge_expired(&store).await {
                tracing::warn!("Failed to purge expired sessions: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::db::{remove_temp_sqlite, temp_sqlite};
    use dotenvy::dotenv;
    use std::env;

    #[tokio::test]
    async fn test_redis_connection_and_dummy_user() {
        dotenv().ok();
        let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");
        let client = connect(&redis_url);

        // ping
        let mut conn = client.get_multiplexed_async_connection().await.expect("Failed to connect");
        let pong: String = redis::cmd("PING").query_async(&mut conn).await.expect("Ping failed");
        assert_eq!(pong, "PONG");
        println!("Redis ping: {}", pong);

        let store = SessionStore::Redis(client);

        // store dummy user (TTL 60s)
        let dummy = r#"{"id":"00000000-0000-0000-0000-000000000001","email":"dummy@oauthrs.dev","username":"dummy"}"#;
        set(&store, "user:dummy", dummy, 60).await.expect("Set failed");
        println!("Stored dummy user");

        // read it back
        let result = get(&store, "user:dummy").await.expect("Get failed");
        assert_eq!(result.as_deref(), Some(dummy));
        println!("Retrieved: {}", result.unwrap());

        // take is single-use
        set(&store, "invite:test-take", "1", 60).await.expect("Set failed");
        let taken = take(&store, "invite:test-take").await.expect("Take failed");
        assert!(matches!(taken, Some((ref v, ttl)) if v == "1" && ttl > 0 && ttl <= 60));
        assert_eq!(take(&store, "invite:test-take").await.expect("Take failed"), None);
    }

    #[test]
    fn test_session_cookie() {
        assert_eq!(session_cookie("t", true),  "session=t; Path=/; HttpOnly; SameSite=Lax; Secure");
        assert_eq!(session_cookie("t", false), "session=t; Path=/; HttpOnly; SameSite=Lax");
    }

    #[tokio::test]
    async fn test_sqlite_set_get_del_expiry() {
        let (pool, path) = temp_sqlite().await;
        let store = SessionStore::Sqlite(pool.clone());

        assert_eq!(get(&store, "k").await.unwrap(), None);

        set(&store, "k", "v1", 60).await.unwrap();
        assert_eq!(get(&store, "k").await.unwrap().as_deref(), Some("v1"));

        // overwrite replaces value (like SETEX)
        set(&store, "k", "v2", 60).await.unwrap();
        assert_eq!(get(&store, "k").await.unwrap().as_deref(), Some("v2"));

        del(&store, "k").await.unwrap();
        assert_eq!(get(&store, "k").await.unwrap(), None);

        // take is single-use
        set(&store, "inv", "1", 60).await.unwrap();
        let (v, ttl) = take(&store, "inv").await.unwrap().unwrap();
        assert_eq!(v, "1");
        assert!(ttl > 0 && ttl <= 60);
        assert_eq!(take(&store, "inv").await.unwrap(), None);
        assert_eq!(get(&store, "inv").await.unwrap(), None);

        // expired keys are invisible and get purged
        set(&store, "old", "x", 60).await.unwrap();
        sqlx::query("UPDATE kv_store SET expires_at = ?1 WHERE key = 'old'")
            .bind(now() - 1)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(get(&store, "old").await.unwrap(), None);
        assert_eq!(purge_expired(&store).await.unwrap(), 1);

        remove_temp_sqlite(pool, path).await;
    }
}
