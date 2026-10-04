-- SQLite standalone schema: equivalent of migrations/0001..0003 plus a
-- key/value table that replaces Redis (sessions, invites, OAuth state).

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY NOT NULL,  -- hyphenated UUID
    email         TEXT UNIQUE NOT NULL CHECK (length(email)    <= 255),
    username      TEXT UNIQUE NOT NULL CHECK (length(username) <= 100),
    password_hash TEXT,
    google_id     TEXT UNIQUE CHECK (google_id IS NULL OR length(google_id) <= 255),
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    deleted_at    TEXT DEFAULT NULL,
    disabled_at   TEXT DEFAULT NULL,
    deleted_by    TEXT DEFAULT NULL  -- admin user_id
);

CREATE TABLE IF NOT EXISTS user_audit_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      TEXT NOT NULL,
    action       TEXT NOT NULL,  -- 'deleted', 'disabled', 'enabled', 'created', 'google_linked'
    performed_by TEXT DEFAULT NULL,  -- NULL = system
    note         TEXT DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_user    ON user_audit_log (user_id);
CREATE INDEX IF NOT EXISTS idx_audit_created ON user_audit_log (created_at DESC);

CREATE TABLE IF NOT EXISTS kv_store (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    expires_at INTEGER NOT NULL  -- unix timestamp (seconds)
);

CREATE INDEX IF NOT EXISTS idx_kv_expires ON kv_store (expires_at);
