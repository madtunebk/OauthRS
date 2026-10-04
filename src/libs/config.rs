use std::env;

/// Storage backend, selected with `STORAGE_BACKEND`. When unset, PostgreSQL + Redis
/// is used if `DATABASE_URL` is set (existing deployments), otherwise SQLite.
#[derive(Clone, Debug, PartialEq)]
pub enum Storage {
    /// PostgreSQL for application data, Redis for sessions and temporary state.
    Postgres { database_url: String, redis_url: String },
    /// Standalone: a single SQLite file for application data, sessions and temporary state.
    Sqlite { path: String },
}

#[derive(Clone)]
pub struct Config {
    pub storage: Storage,
    pub jwt_secret: String,
    pub jwt_expiry_secs: u64,
    pub host:         String,
    pub port:         u16,
    pub admin_secret:    String,
    pub invite_ttl_secs: u64,
    pub invite_required: bool,
    pub cookie_secure:   bool,
    pub google_client_id:     String,
    pub google_client_secret: String,
    pub google_redirect_uri:  String,
}

impl Config {
    pub fn load() -> Self {
        Config {
            storage:         Storage::load(),
            jwt_secret:      env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            jwt_expiry_secs: env::var("JWT_EXPIRY_SECS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .expect("JWT_EXPIRY_SECS must be a number"),
            host:         env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port:         env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),
            admin_secret:    env::var("ADMIN_SECRET").expect("ADMIN_SECRET must be set"),
            invite_ttl_secs: env::var("INVITE_TTL_SECS")
                .unwrap_or_else(|_| "86400".to_string())
                .parse()
                .expect("INVITE_TTL_SECS must be a number"),
            invite_required: env::var("INVITE_REQUIRED")
                .unwrap_or_else(|_| "true".to_string())
                .trim()
                .to_lowercase()
                != "false",
            cookie_secure:   env::var("COOKIE_SECURE")
                .unwrap_or_else(|_| "true".to_string())
                .trim()
                .to_lowercase()
                != "false",
            google_client_id:     env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            google_redirect_uri:  env::var("GOOGLE_REDIRECT_URI").unwrap_or_default(),
        }
    }
}

impl Storage {
    fn load() -> Self {
        Self::from_vars(|name| env::var(name).ok())
    }

    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let var = |name: &str| var(name).filter(|v| !v.trim().is_empty());

        let backend = match var("STORAGE_BACKEND") {
            Some(b) => b.trim().to_lowercase(),
            None if var("DATABASE_URL").is_some() => "postgres".to_string(),
            None => "sqlite".to_string(),
        };

        match backend.as_str() {
            "postgres" | "postgresql" => Storage::Postgres {
                database_url: var("DATABASE_URL").expect("DATABASE_URL must be set"),
                redis_url:    var("REDIS_URL").expect("REDIS_URL must be set"),
            },
            "sqlite" => Storage::Sqlite {
                path: var("SQLITE_PATH").unwrap_or_else(|| "oauthrs.db".to_string()),
            },
            other => panic!("STORAGE_BACKEND must be 'postgres' or 'sqlite', got '{}'", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage(vars: &[(&str, &str)]) -> Storage {
        Storage::from_vars(|name| vars.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string()))
    }

    fn postgres() -> Storage {
        Storage::Postgres { database_url: "postgres://db".into(), redis_url: "redis://r".into() }
    }

    #[test]
    fn test_storage_selection() {
        // existing configs (no STORAGE_BACKEND, DATABASE_URL set) keep PostgreSQL + Redis
        assert_eq!(storage(&[("DATABASE_URL", "postgres://db"), ("REDIS_URL", "redis://r")]), postgres());

        // nothing configured -> SQLite standalone
        assert_eq!(storage(&[]), Storage::Sqlite { path: "oauthrs.db".into() });
        assert_eq!(storage(&[("DATABASE_URL", " ")]), Storage::Sqlite { path: "oauthrs.db".into() });

        // explicit selection wins
        assert_eq!(
            storage(&[("STORAGE_BACKEND", "sqlite"), ("SQLITE_PATH", "/data/a.db"), ("DATABASE_URL", "postgres://db")]),
            Storage::Sqlite { path: "/data/a.db".into() },
        );
        assert_eq!(
            storage(&[("STORAGE_BACKEND", "PostgreSQL"), ("DATABASE_URL", "postgres://db"), ("REDIS_URL", "redis://r")]),
            postgres(),
        );
    }

    #[test]
    #[should_panic(expected = "REDIS_URL must be set")]
    fn test_postgres_requires_redis() {
        storage(&[("DATABASE_URL", "postgres://db")]);
    }

    #[test]
    #[should_panic(expected = "DATABASE_URL must be set")]
    fn test_explicit_postgres_requires_database_url() {
        storage(&[("STORAGE_BACKEND", "postgres")]);
    }

    #[test]
    #[should_panic(expected = "STORAGE_BACKEND must be")]
    fn test_unknown_backend() {
        storage(&[("STORAGE_BACKEND", "mysql")]);
    }
}
