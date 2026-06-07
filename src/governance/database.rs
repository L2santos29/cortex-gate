// Governance Database — SQLite-backed persistence for users, budgets, and usage tracking.
//
// Thread-safe via std::sync::Mutex<rusqlite::Connection>.
// Schema migrations tracked via `_schema_version` table.
// Every migration is idempotent — replaying produces the same schema at the same version.

use rusqlite::Connection;
use std::sync::Mutex;

/// Errors originating from the governance database layer.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    /// Wraps a rusqlite error (connection, query, constraint violation, etc.).
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Wraps a Tokio join error from spawn_blocking.
    #[error("async join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    /// An API key string was empty or malformed.
    #[error("invalid api key: {0}")]
    InvalidApiKey(String),

    /// The requested entity was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A budget limit would be exceeded by the requested operation.
    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),
}

/// Thread-safe wrapper around a single SQLite connection.
///
/// All public methods acquire the internal Mutex before touching the database,
/// making `Database` safe to share across threads via `Arc<Database>`.
pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    /// Open (or create) the SQLite database at `path`, run pending migrations,
    /// and return a ready-to-use `Database` handle.
    ///
    /// The underlying `Connection::open` and `run_migrations` are offloaded
    /// to a blocking thread so this async fn never stalls the Tokio runtime.
    pub async fn open_or_create(path: &str) -> Result<Self, DatabaseError> {
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = Connection::open(&path)?;

            // Performance & safety pragmas
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;",
            )?;

            let db = Self {
                conn: Mutex::new(conn),
            };
            db.run_migrations()?;
            Ok(db)
        })
        .await?
    }

    // ------------------------------------------------------------------
    // Schema migrations
    // ------------------------------------------------------------------

    /// Run all pending schema migrations in order.
    ///
    /// Migrations are tracked in the `_schema_version` table. Each version is
    /// applied exactly once. To add a new migration, append a version block
    /// inside this method and bump the target version check.
    fn run_migrations(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().unwrap();

        // Ensure the version-tracking table exists
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _schema_version (
                version    INTEGER PRIMARY KEY,
                applied_at TEXT    NOT NULL
            );",
        )?;

        let current: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // ---- version 1: initial schema -----------------------------------
        if current < 1 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS users (
                    id         TEXT PRIMARY KEY,
                    name       TEXT NOT NULL,
                    email      TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    is_active  INTEGER NOT NULL DEFAULT 1
                );

                CREATE TABLE IF NOT EXISTS api_keys (
                    id         TEXT PRIMARY KEY,
                    user_id    TEXT NOT NULL,
                    key_hash   TEXT NOT NULL UNIQUE,
                    name       TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    expires_at TEXT,
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS budgets (
                    id               TEXT PRIMARY KEY,
                    user_id          TEXT NOT NULL,
                    tokens_per_hour  INTEGER NOT NULL DEFAULT 0,
                    tokens_per_day   INTEGER NOT NULL DEFAULT 0,
                    tokens_per_month INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS token_usage (
                    id        TEXT PRIMARY KEY,
                    user_id   TEXT NOT NULL,
                    model     TEXT NOT NULL,
                    provider  TEXT NOT NULL,
                    tokens_in INTEGER NOT NULL DEFAULT 0,
                    tokens_out INTEGER NOT NULL DEFAULT 0,
                    cost      REAL NOT NULL DEFAULT 0.0,
                    timestamp TEXT NOT NULL,
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS alerts (
                    id         TEXT PRIMARY KEY,
                    user_id    TEXT NOT NULL,
                    alert_type TEXT NOT NULL,
                    threshold  INTEGER NOT NULL DEFAULT 0,
                    enabled    INTEGER NOT NULL DEFAULT 1,
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                );",
            )?;

            conn.execute(
                "INSERT INTO _schema_version (version, applied_at)
                 VALUES (?1, datetime('now'))",
                rusqlite::params![1],
            )?;
        }

        // ---- version 2: unique index on budgets.user_id -------------------
        if current < 2 {
            conn.execute_batch(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_budgets_user_id ON budgets(user_id);"
            )?;

            conn.execute(
                "INSERT INTO _schema_version (version, applied_at)
                 VALUES (?1, datetime('now'))",
                rusqlite::params![2],
            )?;
        }

        // ---- future versions go here (current < 2, current < 3, …) ----

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_or_create_in_memory() {
        // Use a file-based temp path because :memory: is per-connection
        let tmp = std::env::temp_dir().join(format!("cg_test_{}.db", uuid::Uuid::new_v4()));
        let db = Database::open_or_create(tmp.to_str().unwrap())
            .await
            .expect("open_or_create should succeed");
        drop(db);

        // Cleanup
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(tmp.with_extension("db-wal"));
        let _ = std::fs::remove_file(tmp.with_extension("db-shm"));
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let tmp = std::env::temp_dir().join(format!("cg_test_{}.db", uuid::Uuid::new_v4()));
        let path = tmp.to_str().unwrap().to_string();

        // First run — creates everything
        let db = Database::open_or_create(&path)
            .await
            .expect("first open");
        drop(db);

        // Second run — should be a no-op
        let db = Database::open_or_create(&path)
            .await
            .expect("second open");
        drop(db);

        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(tmp.with_extension("db-wal"));
        let _ = std::fs::remove_file(tmp.with_extension("db-shm"));
    }
}
