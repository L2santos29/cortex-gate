// Token Usage Tracking — estimate, log, and summarize token consumption.
//
// Provides:
// - `estimate_tokens()` — approximate token count from text (~4 chars/token)
// - `TokenTracker` — async wrapper around Database for usage tracking
// - `UsagePeriod` — time window enum (hour / day / month)
// - `UsageSummary` — aggregated statistics with per-model and per-day breakdowns
//
// ## Integration
// ```ignore
// let tracker = TokenTracker::new(db);
// tracker.track_usage(&user_id, "gpt-4o", "openai", 150, 300, 0.015).await?;
// let summary = tracker.get_usage(&user_id, UsagePeriod::LastDay).await?;
// ```

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use crate::governance::database::{Database, DatabaseError};

// ---------------------------------------------------------------------------
// Token estimation
// ---------------------------------------------------------------------------

/// Approximate token count from raw text.
///
/// Rule of thumb: ~4 characters per token for English text.
/// Returns at least 1 for any non-empty input, and 1 for empty strings
/// to avoid zero-token edge cases in rate-limit checks.
pub fn estimate_tokens(text: &str) -> u64 {
    let len = text.len();
    if len == 0 {
        return 1;
    }
    (len / 4).max(1) as u64
}

// ---------------------------------------------------------------------------
// Usage period
// ---------------------------------------------------------------------------

/// Look-back window for usage queries.
///
/// Each variant defines a sliding window relative to `Utc::now()`:
/// - `LastHour`  → past 60 minutes
/// - `LastDay`   → past 24 hours
/// - `LastMonth` → past 30 days
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsagePeriod {
    LastHour,
    LastDay,
    LastMonth,
}

// ---------------------------------------------------------------------------
// Usage summary
// ---------------------------------------------------------------------------

/// Aggregated usage statistics for a user over a given time window.
#[derive(Debug, Clone, Serialize)]
pub struct UsageSummary {
    /// Total tokens consumed (input + output).
    pub total_tokens: u64,
    /// Total input (prompt) tokens.
    pub total_tokens_in: u64,
    /// Total output (completion) tokens.
    pub total_tokens_out: u64,
    /// Total monetary cost in USD (or configured currency).
    pub total_cost: f64,
    /// Tokens grouped by model name.
    pub by_model: HashMap<String, u64>,
    /// Tokens grouped by calendar day (`"YYYY-MM-DD"`).
    pub by_day: HashMap<String, u64>,
    /// Number of individual requests recorded.
    pub request_count: u64,
}

impl UsageSummary {
    /// An empty summary with all counters at zero.
    pub fn empty() -> Self {
        Self {
            total_tokens: 0,
            total_tokens_in: 0,
            total_tokens_out: 0,
            total_cost: 0.0,
            by_model: HashMap::new(),
            by_day: HashMap::new(),
            request_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// TokenTracker — async wrapper
// ---------------------------------------------------------------------------

/// Async token usage tracker backed by the governance database.
///
/// Wraps an `Arc<Database>` and offloads all SQLite operations to blocking
/// threads via `tokio::task::spawn_blocking`.
pub struct TokenTracker {
    pub db: Arc<Database>,
}

impl TokenTracker {
    /// Create a new tracker from an existing database handle.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Record a token usage event for a user.
    ///
    /// Inserts a row into `token_usage` with the given user, model, provider,
    /// token counts, and cost. Timestamp is set automatically to the current
    /// UTC time.
    pub async fn track_usage(
        &self,
        user_id: &str,
        model: &str,
        provider: &str,
        tokens_in: u64,
        tokens_out: u64,
        cost: f64,
    ) -> Result<(), DatabaseError> {
        let db = self.db.clone();
        let user_id = user_id.to_string();
        let model = model.to_string();
        let provider = provider.to_string();

        tokio::task::spawn_blocking(move || {
            db.insert_token_usage(&user_id, &model, &provider, tokens_in, tokens_out, cost)
        })
        .await?
    }

    /// Get aggregated usage summary for a user over a time window.
    ///
    /// Returns total tokens, cost, per-model breakdown, per-day breakdown,
    /// and request count for the given `UsagePeriod`.
    pub async fn get_usage(
        &self,
        user_id: &str,
        period: UsagePeriod,
    ) -> Result<UsageSummary, DatabaseError> {
        let db = self.db.clone();
        let user_id = user_id.to_string();
        let (start, end) = period_to_range(period);

        tokio::task::spawn_blocking(move || db.query_usage_summary(&user_id, start, end)).await?
    }
}

// ---------------------------------------------------------------------------
// Database methods (sync, used internally by TokenTracker)
// ---------------------------------------------------------------------------

impl Database {
    /// Insert a single token usage row.
    ///
    /// Generates a UUID primary key and records the current UTC timestamp.
    pub fn insert_token_usage(
        &self,
        user_id: &str,
        model: &str,
        provider: &str,
        tokens_in: u64,
        tokens_out: u64,
        cost: f64,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        conn.execute(
            "INSERT INTO token_usage (id, user_id, model, provider, tokens_in, tokens_out, cost, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, user_id, model, provider, tokens_in as i64, tokens_out as i64, cost, timestamp],
        )?;

        Ok(())
    }

    /// Query and aggregate usage for a user between two ISO timestamps.
    ///
    /// Returns a `UsageSummary` with aggregated totals, per-model breakdown
    /// (by model name), and per-day breakdown (by `YYYY-MM-DD` date string).
    pub fn query_usage_summary(
        &self,
        user_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<UsageSummary, DatabaseError> {
        let conn = self.conn.lock().unwrap();

        let start_str = start.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let end_str = end.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let mut stmt = conn.prepare(
            "SELECT tokens_in, tokens_out, cost, model, timestamp
             FROM token_usage
             WHERE user_id = ?1 AND timestamp >= ?2 AND timestamp < ?3
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![user_id, start_str, end_str], |row| {
            let tokens_in: i64 = row.get(0)?;
            let tokens_out: i64 = row.get(1)?;
            let cost: f64 = row.get(2)?;
            let model: String = row.get(3)?;
            let timestamp: String = row.get(4)?;
            Ok((tokens_in, tokens_out, cost, model, timestamp))
        })?;

        let mut summary = UsageSummary::empty();

        for row in rows {
            let (t_in, t_out, cost, model, ts) = row?;
            let t_in = t_in.max(0) as u64;
            let t_out = t_out.max(0) as u64;
            let tokens = t_in + t_out;

            summary.total_tokens += tokens;
            summary.total_tokens_in += t_in;
            summary.total_tokens_out += t_out;
            summary.total_cost += cost;
            summary.request_count += 1;

            *summary.by_model.entry(model).or_insert(0) += tokens;

            // Extract date portion (YYYY-MM-DD) for the daily breakdown
            let day = if ts.len() >= 10 {
                ts[..10].to_string()
            } else {
                ts.clone()
            };
            *summary.by_day.entry(day).or_insert(0) += tokens;
        }

        Ok(summary)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `UsagePeriod` to a (start, end) UTC range using a sliding window.
fn period_to_range(period: UsagePeriod) -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    match period {
        UsagePeriod::LastHour => (now - Duration::hours(1), now),
        UsagePeriod::LastDay => (now - Duration::days(1), now),
        UsagePeriod::LastMonth => (now - Duration::days(30), now),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test Database backed by a temporary file.
    fn test_db() -> Database {
        let tmp = std::env::temp_dir().join(format!("cg_trk_{}.db", Uuid::new_v4()));
        let path = tmp.to_str().unwrap().to_string();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(Database::open_or_create(&path))
            .expect("test db creation")
    }

    async fn test_db_async() -> Database {
        let tmp = std::env::temp_dir().join(format!("cg_trk_{}.db", Uuid::new_v4()));
        let path = tmp.to_str().unwrap().to_string();
        Database::open_or_create(&path).await.expect("test db creation")
    }

    #[test]
    fn test_estimate_tokens_english() {
        // "hello world" = 11 chars → 11/4 = 2
        assert_eq!(estimate_tokens("hello world"), 2);
    }

    #[test]
    fn test_estimate_tokens_minimum() {
        assert_eq!(estimate_tokens("a"), 1); // 1/4 = 0 → clamped to 1
        assert_eq!(estimate_tokens(""), 1); // empty → returns 1
    }

    #[test]
    fn test_estimate_tokens_long_text() {
        let text = "The quick brown fox jumps over the lazy dog. ";
        let repeated = text.repeat(100); // 4500 chars → ~1125 tokens
        let estimated = estimate_tokens(&repeated);
        assert!(estimated > 1000);
        assert!(estimated < 1200);
    }

    #[test]
    fn test_insert_and_query_usage() {
        let db = test_db();
        let user = db.create_user("test_user", "test@example.com").unwrap();

        // Insert two usage records
        db.insert_token_usage(&user.id, "gpt-4o", "openai", 100, 200, 0.015)
            .unwrap();
        db.insert_token_usage(&user.id, "claude-3", "anthropic", 50, 150, 0.025)
            .unwrap();

        // Query all time (use a wide range)
        let start = Utc::now() - Duration::days(1);
        let end = Utc::now() + Duration::hours(1);
        let summary = db.query_usage_summary(&user.id, start, end).unwrap();

        assert_eq!(summary.total_tokens, 500); // 300 + 200
        assert_eq!(summary.total_tokens_in, 150);
        assert_eq!(summary.total_tokens_out, 350);
        assert_eq!(summary.request_count, 2);
        assert!(summary.total_cost > 0.0);

        // Check per-model breakdown
        assert_eq!(*summary.by_model.get("gpt-4o").unwrap(), 300);
        assert_eq!(*summary.by_model.get("claude-3").unwrap(), 200);
    }

    #[test]
    fn test_query_empty_usage() {
        let db = test_db();
        let user = db.create_user("empty_user", "empty@example.com").unwrap();

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let summary = db.query_usage_summary(&user.id, start, end).unwrap();

        assert_eq!(summary.total_tokens, 0);
        assert_eq!(summary.request_count, 0);
        assert!(summary.by_model.is_empty());
        assert!(summary.by_day.is_empty());
    }

    #[tokio::test]
    async fn test_token_tracker_roundtrip() {
        let db = test_db_async().await;
        let user = db.create_user("tracker_user", "tracker@example.com").unwrap();

        let tracker = TokenTracker::new(Arc::new(db));

        tracker
            .track_usage(&user.id, "gpt-4o-mini", "openai", 75, 120, 0.003)
            .await
            .unwrap();

        let summary = tracker
            .get_usage(&user.id, UsagePeriod::LastDay)
            .await
            .unwrap();

        assert_eq!(summary.total_tokens, 195);
        assert_eq!(summary.request_count, 1);
    }

    #[test]
    fn test_usage_summary_empty() {
        let empty = UsageSummary::empty();
        assert_eq!(empty.total_tokens, 0);
        assert_eq!(empty.total_cost, 0.0);
        assert!(empty.by_model.is_empty());
        assert!(empty.by_day.is_empty());
        assert_eq!(empty.request_count, 0);
    }
}
