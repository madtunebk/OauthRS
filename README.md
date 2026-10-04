# OauthRS

A lightweight authentication microservice built with Rust and [Axum](https://github.com/tokio-rs/axum). Handles user registration, login, session management, Google OAuth, and exposes a `/auth` endpoint compatible with **nginx `auth_request`**.

## Features

- Email/password login and signup (Argon2 password hashing)
- Google OAuth 2.0 login (optional, `GOOGLE_ENABLED=on/off`)
- JWT session tokens stored in Redis (or SQLite in standalone mode)
- Two storage modes: PostgreSQL + Redis, or a single-file SQLite standalone mode
- Invite-only registration mode
- `GET /auth` — nginx `auth_request` subrequest endpoint (validates JWT from `Authorization` header or `session` cookie)
- OAuth 2.0 token issuance and revoke endpoints
- Tera templates (Jinja2 syntax) for login/signup pages
- Auto-runs SQLx migrations on startup

## Stack

| Layer | Tech |
|---|---|
| HTTP | Axum 0.8 + Tokio |
| Database | PostgreSQL 16 via SQLx (or SQLite) |
| Sessions | Redis (or SQLite) |
| Auth | JWT (jsonwebtoken) + Argon2 |
| Templates | Tera |

## Getting started

**Requirements:** Rust (stable) — plus PostgreSQL 16 and Redis for the PostgreSQL mode; nothing else in SQLite standalone mode

```bash
git clone https://github.com/madtunebk/OauthRS.git
cd OauthRS
cp .env.example .env          # PostgreSQL + Redis
# or: cp .env.sqlite.example .env   # SQLite standalone
# fill in your values
cargo run
```

Server starts on `http://127.0.0.1:8080` by default.

## Storage modes

| Mode | `STORAGE_BACKEND` | Application data | Sessions, invites, OAuth state |
|---|---|---|---|
| PostgreSQL + Redis | `postgres` | PostgreSQL (`DATABASE_URL`) | Redis (`REDIS_URL`) |
| Standalone | `sqlite` | SQLite file (`SQLITE_PATH`) | Same SQLite file (`kv_store` table) |

When `STORAGE_BACKEND` is not set, the mode is chosen automatically:

- `DATABASE_URL` is set → PostgreSQL + Redis, so existing configurations keep working unchanged;
- otherwise → SQLite standalone, so a fresh install runs with no database setup.

Set `STORAGE_BACKEND` explicitly in production to make the choice independent of other variables.

Standalone mode needs no external services:

```bash
STORAGE_BACKEND=sqlite SQLITE_PATH=/var/lib/oauthrs/oauthrs.db \
JWT_SECRET=... ADMIN_SECRET=... cargo run
```

The database file is created on first start and migrated from `migrations_sqlite/` (PostgreSQL uses `migrations/`). Expired session keys are ignored on read and purged every minute. SQLite runs in WAL mode, so keep the `-wal`/`-shm` files next to the database and run a single instance per file.

## Environment variables

Copy `.env.example` to `.env` and set:

| Variable | Required | Default | Description |
|---|---|---|---|
| `STORAGE_BACKEND` | no | auto | `postgres` (PostgreSQL + Redis) or `sqlite` (standalone); if unset: `postgres` when `DATABASE_URL` is set, else `sqlite` |
| `DATABASE_URL` | postgres mode | — | PostgreSQL connection string |
| `REDIS_URL` | postgres mode | — | Redis connection string |
| `SQLITE_PATH` | no | `oauthrs.db` | SQLite database file (sqlite mode only) |
| `JWT_SECRET` | yes | — | Secret key for signing JWTs |
| `JWT_EXPIRY_SECS` | no | `3600` | Token lifetime in seconds |
| `HOST` | no | `127.0.0.1` | Bind address |
| `PORT` | no | `8080` | Bind port |
| `ADMIN_SECRET` | yes | — | Secret for admin operations |
| `INVITE_REQUIRED` | no | `true` | Require invite code to register |
| `INVITE_TTL_SECS` | no | `86400` | Invite link expiry |
| `COOKIE_SECURE` | no | `true` | Add `Secure` to the session cookie; set `false` only for plain-HTTP development |
| `GOOGLE_ENABLED` | no | auto | `on`/`off` switch for Google login; if unset, enabled when `GOOGLE_CLIENT_ID` is set. When off, the Google button is hidden and `/auth/google*` redirect to `/login` |
| `GOOGLE_CLIENT_ID` | no | — | Google OAuth client ID |
| `GOOGLE_CLIENT_SECRET` | no | — | Google OAuth client secret |
| `GOOGLE_REDIRECT_URI` | no | — | OAuth callback URL |
| `TEMPLATES_DIR` | no | `src/core/templates` | Directory with the `*.tpl` page templates |

## API routes

| Method | Path | Description |
|---|---|---|
| `GET` | `/` | Home page |
| `GET` | `/auth` | nginx `auth_request` — returns 200 or 401 |
| `GET/POST` | `/login` | Login page / submit |
| `GET/POST` | `/signup` | Signup page / submit |
| `POST` | `/logout` | Invalidate session |
| `GET` | `/auth/google` | Start Google OAuth flow |
| `GET` | `/auth/google/callback` | Google OAuth callback |
| `POST` | `/oauth/token` | Issue OAuth token |
| `POST` | `/oauth/revoke` | Revoke OAuth token |
| `GET` | `/oauth/authorize` | OAuth authorization |
| `POST` | `/api/invite` | Generate invite link |

## nginx integration

```nginx
location /protected {
    auth_request /auth;
    auth_request_set $auth_status $upstream_status;
    error_page 401 = /login;
    # ... your proxy config
}

location = /auth {
    internal;
    proxy_pass http://127.0.0.1:8080/auth;
    proxy_pass_request_body off;
    proxy_set_header Content-Length "";
}
```

The `/auth` endpoint reads the JWT from:
1. `Authorization: Bearer <token>` header
2. `session` cookie

Returns `200 OK` for a valid, active session — `401 Unauthorized` otherwise.

## Development

```bash
cargo check      # fast type-check
cargo clippy     # lint
cargo test       # run tests
cargo build      # compile
cargo run        # compile + start server
```

## License

MIT
