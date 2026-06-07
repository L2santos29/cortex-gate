// User Management — CRUD for users and API keys backed by the governance database.
//
// API keys are hashed with SHA-256 before storage. The plaintext key is returned
// exactly once at creation time and is never persisted.

use crate::governance::database::{Database, DatabaseError};
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A user record returned by the API.
#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
    pub is_active: bool,
}

/// An API key record (without the plaintext key — that is only returned once
/// at creation time).
#[derive(Debug, Clone, Serialize)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

/// Returned when a new API key is created. The `key` field carries the
/// plaintext value and should be displayed to the caller exactly once.
#[derive(Debug, Clone, Serialize)]
pub struct CreatedApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

// ---------------------------------------------------------------------------
// User CRUD
// ---------------------------------------------------------------------------

impl Database {
    /// Create a new active user with the given `name` and `email`.
    ///
    /// Returns the newly created `User`. The user will have a fresh UUID and
    /// the current timestamp.
    pub fn create_user(&self, name: &str, email: &str) -> Result<User, DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let created_at = now_iso();

        conn.execute(
            "INSERT INTO users (id, name, email, created_at, is_active)
             VALUES (?1, ?2, ?3, ?4, 1)",
            params![id, name, email, created_at],
        )?;

        Ok(User {
            id,
            name: name.to_string(),
            email: email.to_string(),
            created_at,
            is_active: true,
        })
    }

    /// Look up a user by `id`. Returns `None` if no user exists with that id.
    pub fn get_user(&self, id: &str) -> Result<Option<User>, DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, email, created_at, is_active FROM users WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(User {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                created_at: row.get(3)?,
                is_active: row.get::<_, i32>(4)? != 0,
            })),
            None => Ok(None),
        }
    }

    /// Return every user in the database (active or inactive).
    pub fn list_users(&self) -> Result<Vec<User>, DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, email, created_at, is_active FROM users ORDER BY created_at DESC")?;

        let rows = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                created_at: row.get(3)?,
                is_active: row.get::<_, i32>(4)? != 0,
            })
        })?;

        let mut users = Vec::new();
        for user in rows {
            users.push(user?);
        }
        Ok(users)
    }

    /// Soft-delete a user by setting `is_active = 0`.
    ///
    /// Returns an error if no user with that `id` exists.
    pub fn deactivate_user(&self, id: &str) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE users SET is_active = 0 WHERE id = ?1",
            params![id],
        )?;
        if affected == 0 {
            return Err(DatabaseError::NotFound(format!("user {}", id)));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// API key management
// ---------------------------------------------------------------------------

impl Database {
    /// Create a new API key for the given `user_id`.
    ///
    /// The plaintext key is returned exactly once as a `String`. Only the
    /// SHA-256 hash is persisted — the plaintext cannot be recovered from
    /// the database.
    ///
    /// The key is prefixed with `cg_` for easy identification and encoded
    /// as a lower-case hex string (UUID-based).
    pub fn create_api_key(&self, user_id: &str, name: &str) -> Result<String, DatabaseError> {
        let conn = self.conn.lock().unwrap();

        // Verify user exists
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE id = ?1",
                params![user_id],
                |row| row.get::<_, i32>(0),
            )
            .map(|c| c > 0)?;

        if !exists {
            return Err(DatabaseError::NotFound(format!("user {}", user_id)));
        }

        let id = Uuid::new_v4().to_string();
        let key_plain = format!("cg_{}", Uuid::new_v4().to_string().replace('-', ""));
        let key_hash = sha256_hex(&key_plain);
        let created_at = now_iso();

        conn.execute(
            "INSERT INTO api_keys (id, user_id, key_hash, name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, user_id, key_hash, name, created_at],
        )?;

        Ok(key_plain)
    }

    /// Validate a plaintext API key.
    ///
    /// Hashes the input and looks it up in the `api_keys` table. Returns
    /// the associated `user_id` if a match is found, or `None` if the key
    /// is invalid / expired / the owning user is deactivated.
    pub fn validate_api_key(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        if key.is_empty() {
            return Err(DatabaseError::InvalidApiKey("empty key".into()));
        }

        let conn = self.conn.lock().unwrap();
        let key_hash = sha256_hex(key);

        let result: Option<String> = conn
            .query_row(
                "SELECT ak.user_id
                 FROM api_keys ak
                 JOIN users u ON u.id = ak.user_id
                 WHERE ak.key_hash = ?1
                   AND u.is_active = 1
                   AND (ak.expires_at IS NULL OR ak.expires_at > datetime('now'))",
                params![key_hash],
                |row| row.get(0),
            )
            .ok();

        Ok(result)
    }

    /// List all API keys for a given user (without the plaintext secret, of course).
    pub fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKey>, DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, created_at, expires_at
             FROM api_keys
             WHERE user_id = ?1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![user_id], |row| {
            Ok(ApiKey {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                created_at: row.get(3)?,
                expires_at: row.get(4)?,
            })
        })?;

        let mut keys = Vec::new();
        for k in rows {
            keys.push(k?);
        }
        Ok(keys)
    }

    /// Revoke (delete) an API key by its id.
    ///
    /// Returns an error if the key id does not exist.
    pub fn revoke_api_key(&self, id: &str) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DatabaseError::NotFound(format!("api_key {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        let tmp = std::env::temp_dir().join(format!("cg_ut_{}.db", Uuid::new_v4()));
        let path = tmp.to_str().unwrap().to_string();
        Database::open_or_create(&path).await.expect("test db creation")
    }

    #[tokio::test]
    async fn test_create_and_get_user() {
        let db = test_db().await;
        let user = db.create_user("Alice", "alice@example.com").unwrap();
        assert!(user.is_active);

        let fetched = db.get_user(&user.id).unwrap().expect("user should exist");
        assert_eq!(fetched.name, "Alice");
        assert_eq!(fetched.email, "alice@example.com");
    }

    #[tokio::test]
    async fn test_list_users() {
        let db = test_db().await;
        db.create_user("A", "a@x").unwrap();
        db.create_user("B", "b@x").unwrap();
        let users = db.list_users().unwrap();
        assert_eq!(users.len(), 2);
    }

    #[tokio::test]
    async fn test_deactivate_user() {
        let db = test_db().await;
        let user = db.create_user("Bob", "bob@x").unwrap();
        db.deactivate_user(&user.id).unwrap();

        let fetched = db.get_user(&user.id).unwrap().unwrap();
        assert!(!fetched.is_active);
    }

    #[tokio::test]
    async fn test_create_and_validate_api_key() {
        let db = test_db().await;
        let user = db.create_user("Carol", "carol@x").unwrap();

        let key = db.create_api_key(&user.id, "dev-key").unwrap();
        assert!(key.starts_with("cg_"));

        let user_id = db
            .validate_api_key(&key)
            .unwrap()
            .expect("valid key should return user_id");
        assert_eq!(user_id, user.id);
    }

    #[tokio::test]
    async fn test_validate_revoked_key() {
        let db = test_db().await;
        let user = db.create_user("Dave", "dave@x").unwrap();

        // Create key, then fetch its id via list_api_keys to revoke it
        let key = db.create_api_key(&user.id, "temp").unwrap();
        let keys = db.list_api_keys(&user.id).unwrap();
        assert_eq!(keys.len(), 1);
        db.revoke_api_key(&keys[0].id).unwrap();

        let result = db.validate_api_key(&key).unwrap();
        assert!(result.is_none(), "revoked key must not validate");
    }

    #[tokio::test]
    async fn test_deactivated_user_key_invalid() {
        let db = test_db().await;
        let user = db.create_user("Eve", "eve@x").unwrap();
        let key = db.create_api_key(&user.id, "key").unwrap();

        db.deactivate_user(&user.id).unwrap();

        let result = db.validate_api_key(&key).unwrap();
        assert!(result.is_none(), "key of deactivated user must not validate");
    }

    #[tokio::test]
    async fn test_list_api_keys() {
        let db = test_db().await;
        let user = db.create_user("Frank", "frank@x").unwrap();
        db.create_api_key(&user.id, "k1").unwrap();
        db.create_api_key(&user.id, "k2").unwrap();

        let keys = db.list_api_keys(&user.id).unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_create_api_key_returns_string() {
        let db = test_db().await;
        let user = db.create_user("Grace", "grace@x").unwrap();
        let key: String = db.create_api_key(&user.id, "test").unwrap();
        assert!(key.starts_with("cg_"));
        assert_eq!(key.len(), 32 + 3); // "cg_" + 32 hex chars (uuid without dashes)
    }
}
