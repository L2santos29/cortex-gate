// Token Budgets & Quotas — enforce per-user spending limits across time windows.
//
// Provides three budget periods:
// - **Hourly**  — rolling 60-minute window
// - **Daily**   — rolling 24-hour window
// - **Monthly** — rolling 30-day window
//
// ## Integration
// ```ignore
// let enforcer = BudgetEnforcer::new(db);
// let status = enforcer.check_budget(&user_id).await?;
// if !status.allowed {
//     return Err("budget exceeded".into());
// }
// enforcer.enforce_budget(&user_id, estimated_tokens).await?;
// ```

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use crate::governance::database::{Database, DatabaseError};

// ---------------------------------------------------------------------------
// Budget period
// ---------------------------------------------------------------------------

/// A named time window for budget enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BudgetPeriod {
    Hourly,
    Daily,
    Monthly,
}

impl BudgetPeriod {
    /// Return the start of the sliding window for this period.
    pub fn window_start(&self) -> DateTime<Utc> {
        let now = Utc::now();
        match self {
            BudgetPeriod::Hourly => now - Duration::hours(1),
            BudgetPeriod::Daily => now - Duration::days(1),
            BudgetPeriod::Monthly => now - Duration::days(30),
        }
    }

    /// Human-readable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetPeriod::Hourly => "hour",
            BudgetPeriod::Daily => "day",
            BudgetPeriod::Monthly => "month",
        }
    }
}

// ---------------------------------------------------------------------------
// Budget configuration
// ---------------------------------------------------------------------------

/// Budget limits for a single user.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetConfig {
    pub user_id: String,
    /// Maximum tokens allowed per hour (0 = unlimited).
    pub tokens_per_hour: u64,
    /// Maximum tokens allowed per day (0 = unlimited).
    pub tokens_per_day: u64,
    /// Maximum tokens allowed per month (0 = unlimited).
    pub tokens_per_month: u64,
}

// ---------------------------------------------------------------------------
// Budget status
// ---------------------------------------------------------------------------

/// Result of a budget check for a user.
#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    /// Whether the request is allowed to proceed.
    pub allowed: bool,
    /// Tokens remaining in the hourly budget (0 = unlimited / not configured).
    pub remaining_hour: u64,
    /// Tokens remaining in the daily budget.
    pub remaining_day: u64,
    /// Tokens remaining in the monthly budget.
    pub remaining_month: u64,
    /// Human-readable reason if the request is denied.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// BudgetEnforcer — async wrapper
// ---------------------------------------------------------------------------

/// Async budget enforcer backed by the governance database.
pub struct BudgetEnforcer {
    pub db: Arc<Database>,
}

impl BudgetEnforcer {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Check the current budget status for a user (read-only).
    pub async fn check_budget(&self, user_id: &str) -> Result<BudgetStatus, DatabaseError> {
        let db = self.db.clone();
        let user_id = user_id.to_string();

        tokio::task::spawn_blocking(move || db.compute_budget_status(&user_id)).await?
    }

    /// Enforce budget limits for an upcoming request.
    ///
    /// Returns `Ok(())` if the request fits within all active budgets.
    /// Returns `Err(DatabaseError::BudgetExceeded(...))` if the estimated
    /// tokens would exceed any budget limit.
    pub async fn enforce_budget(
        &self,
        user_id: &str,
        estimated_tokens: u64,
    ) -> Result<(), DatabaseError> {
        let db = self.db.clone();
        let user_id = user_id.to_string();

        tokio::task::spawn_blocking(move || {
            let budget = db.get_budget_config(&user_id)?;

            // Unlimited — nothing to enforce
            if budget.tokens_per_hour == 0
                && budget.tokens_per_day == 0
                && budget.tokens_per_month == 0
            {
                return Ok(());
            }

            let now = Utc::now();

            // Check each period
            let checks: [(u64, BudgetPeriod); 3] = [
                (budget.tokens_per_hour, BudgetPeriod::Hourly),
                (budget.tokens_per_day, BudgetPeriod::Daily),
                (budget.tokens_per_month, BudgetPeriod::Monthly),
            ];

            for (limit, period) in checks {
                if limit == 0 {
                    continue;
                }
                let used = db.query_tokens_in_window(&user_id, period.window_start(), now)?;
                let remaining = limit.saturating_sub(used);
                if estimated_tokens > remaining {
                    return Err(DatabaseError::BudgetExceeded(format!(
                        "budget exceeded for {}: estimated={}, remaining={}, limit={}",
                        period.as_str(),
                        estimated_tokens,
                        remaining,
                        limit,
                    )));
                }
            }

            Ok(())
        })
        .await?
    }

    /// Query how many tokens remain in a specific budget period for a user.
    ///
    /// Returns the budget limit minus tokens already used. Returns `u64::MAX`
    /// if the period has no limit (unlimited).
    pub async fn tokens_remaining_in_period(
        &self,
        user_id: &str,
        period: BudgetPeriod,
    ) -> Result<u64, DatabaseError> {
        let db = self.db.clone();
        let user_id = user_id.to_string();

        tokio::task::spawn_blocking(move || db.remaining_for_period(&user_id, period)).await?
    }
}

// ---------------------------------------------------------------------------
// Database methods (sync)
// ---------------------------------------------------------------------------

impl Database {
    /// Retrieve the budget configuration for a user.
    ///
    /// If no explicit row exists in the `budgets` table, returns a default
    /// config with all limits set to `0` (unlimited).
    pub fn get_budget_config(&self, user_id: &str) -> Result<BudgetConfig, DatabaseError> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT tokens_per_hour, tokens_per_day, tokens_per_month
             FROM budgets WHERE user_id = ?1",
            params![user_id],
            |row| {
                Ok(BudgetConfig {
                    user_id: user_id.to_string(),
                    tokens_per_hour: row.get::<_, i64>(0)? as u64,
                    tokens_per_day: row.get::<_, i64>(1)? as u64,
                    tokens_per_month: row.get::<_, i64>(2)? as u64,
                })
            },
        );

        match result {
            Ok(config) => Ok(config),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // No budget row = unlimited
                Ok(BudgetConfig {
                    user_id: user_id.to_string(),
                    tokens_per_hour: 0,
                    tokens_per_day: 0,
                    tokens_per_month: 0,
                })
            }
            Err(e) => Err(DatabaseError::Sqlite(e)),
        }
    }

    /// Upsert budget limits for a user.
    ///
    /// Requires the `idx_budgets_user_id` unique index (created in migration v2).
    /// Creates a new row if none exists, otherwise updates the existing one.
    pub fn set_budget_config(
        &self,
        user_id: &str,
        tokens_per_hour: u64,
        tokens_per_day: u64,
        tokens_per_month: u64,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO budgets (id, user_id, tokens_per_hour, tokens_per_day, tokens_per_month)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(user_id) DO UPDATE SET
                 tokens_per_hour = excluded.tokens_per_hour,
                 tokens_per_day = excluded.tokens_per_day,
                 tokens_per_month = excluded.tokens_per_month",
            params![id, user_id, tokens_per_hour as i64, tokens_per_day as i64, tokens_per_month as i64],
        )?;

        Ok(())
    }

    /// Sum token usage for a user within a (start, end) window.
    pub fn query_tokens_in_window(
        &self,
        user_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<u64, DatabaseError> {
        let conn = self.conn.lock().unwrap();
        let start_str = start.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let end_str = end.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let total: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(tokens_in + tokens_out), 0)
                 FROM token_usage
                 WHERE user_id = ?1 AND timestamp >= ?2 AND timestamp < ?3",
                params![user_id, start_str, end_str],
                |row| row.get(0),
            )?;

        Ok(total.max(0) as u64)
    }

    /// Compute how many tokens remain in a given budget period.
    pub fn remaining_for_period(
        &self,
        user_id: &str,
        period: BudgetPeriod,
    ) -> Result<u64, DatabaseError> {
        let config = self.get_budget_config(user_id)?;

        let limit = match period {
            BudgetPeriod::Hourly => config.tokens_per_hour,
            BudgetPeriod::Daily => config.tokens_per_day,
            BudgetPeriod::Monthly => config.tokens_per_month,
        };

        if limit == 0 {
            return Ok(u64::MAX); // Unlimited
        }

        let used = self.query_tokens_in_window(user_id, period.window_start(), Utc::now())?;
        Ok(limit.saturating_sub(used))
    }

    /// Compute the full budget status for a user (all periods at once).
    fn compute_budget_status(&self, user_id: &str) -> Result<BudgetStatus, DatabaseError> {
        let config = self.get_budget_config(user_id)?;
        let now = Utc::now();

        let used_hour = if config.tokens_per_hour > 0 {
            self.query_tokens_in_window(user_id, BudgetPeriod::Hourly.window_start(), now)?
        } else {
            0
        };
        let used_day = if config.tokens_per_day > 0 {
            self.query_tokens_in_window(user_id, BudgetPeriod::Daily.window_start(), now)?
        } else {
            0
        };
        let used_month = if config.tokens_per_month > 0 {
            self.query_tokens_in_window(user_id, BudgetPeriod::Monthly.window_start(), now)?
        } else {
            0
        };

        let remaining_hour = config.tokens_per_hour.saturating_sub(used_hour);
        let remaining_day = config.tokens_per_day.saturating_sub(used_day);
        let remaining_month = config.tokens_per_month.saturating_sub(used_month);

        let mut reason = None;

        if config.tokens_per_hour > 0 && used_hour >= config.tokens_per_hour {
            reason = Some(format!(
                "hourly budget exceeded: used={}, limit={}",
                used_hour, config.tokens_per_hour
            ));
        } else if config.tokens_per_day > 0 && used_day >= config.tokens_per_day {
            reason = Some(format!(
                "daily budget exceeded: used={}, limit={}",
                used_day, config.tokens_per_day
            ));
        } else if config.tokens_per_month > 0 && used_month >= config.tokens_per_month {
            reason = Some(format!(
                "monthly budget exceeded: used={}, limit={}",
                used_month, config.tokens_per_month
            ));
        }

        Ok(BudgetStatus {
            allowed: reason.is_none(),
            remaining_hour,
            remaining_day,
            remaining_month,
            reason,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let tmp = std::env::temp_dir().join(format!("cg_qta_{}.db", Uuid::new_v4()));
        let path = tmp.to_str().unwrap().to_string();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(Database::open_or_create(&path))
            .expect("test db creation")
    }

    async fn test_db_async() -> Database {
        let tmp = std::env::temp_dir().join(format!("cg_qta_{}.db", Uuid::new_v4()));
        let path = tmp.to_str().unwrap().to_string();
        Database::open_or_create(&path).await.expect("test db creation")
    }

    #[test]
    fn test_budget_period_window() {
        let now = Utc::now();
        let hourly_start = BudgetPeriod::Hourly.window_start();
        let daily_start = BudgetPeriod::Daily.window_start();
        let monthly_start = BudgetPeriod::Monthly.window_start();

        assert!(hourly_start <= now);
        assert!(daily_start <= now);
        assert!(monthly_start <= now);
        assert!(hourly_start > daily_start);
        assert!(daily_start > monthly_start);
    }

    #[test]
    fn test_budget_period_as_str() {
        assert_eq!(BudgetPeriod::Hourly.as_str(), "hour");
        assert_eq!(BudgetPeriod::Daily.as_str(), "day");
        assert_eq!(BudgetPeriod::Monthly.as_str(), "month");
    }

    #[test]
    fn test_get_budget_config_default() {
        let db = test_db();
        let user = db.create_user("budget_user", "budget@example.com").unwrap();

        let config = db.get_budget_config(&user.id).unwrap();
        assert_eq!(config.tokens_per_hour, 0);
        assert_eq!(config.tokens_per_day, 0);
        assert_eq!(config.tokens_per_month, 0);
    }

    #[test]
    fn test_set_and_get_budget_config() {
        let db = test_db();
        let user = db.create_user("config_user", "config@example.com").unwrap();

        db.set_budget_config(&user.id, 1000, 10000, 100000)
            .unwrap();

        let config = db.get_budget_config(&user.id).unwrap();
        assert_eq!(config.tokens_per_hour, 1000);
        assert_eq!(config.tokens_per_day, 10000);
        assert_eq!(config.tokens_per_month, 100000);
    }

    #[test]
    fn test_set_budget_config_idempotent() {
        let db = test_db();
        let user = db.create_user("idempotent", "idempotent@example.com").unwrap();

        // Two consecutive calls must not error
        db.set_budget_config(&user.id, 500, 5000, 50000)
            .unwrap();
        db.set_budget_config(&user.id, 1000, 10000, 100000)
            .unwrap();

        let config = db.get_budget_config(&user.id).unwrap();
        assert_eq!(config.tokens_per_hour, 1000);
    }

    #[test]
    fn test_query_tokens_in_window() {
        let db = test_db();
        let user = db.create_user("usage_user", "usage@example.com").unwrap();

        db.insert_token_usage(&user.id, "gpt-4o", "openai", 100, 200, 0.01)
            .unwrap();

        let now = Utc::now();
        let total = db
            .query_tokens_in_window(&user.id, now - Duration::hours(1), now + Duration::hours(1))
            .unwrap();
        assert_eq!(total, 300);
    }

    #[test]
    fn test_query_tokens_in_window_empty() {
        let db = test_db();
        let user = db.create_user("empty_window", "empty@example.com").unwrap();

        let total = db
            .query_tokens_in_window(
                &user.id,
                Utc::now() - Duration::hours(1),
                Utc::now(),
            )
            .unwrap();
        assert_eq!(total, 0);
    }

    #[test]
    fn test_remaining_for_period() {
        let db = test_db();
        let user = db.create_user("remaining_user", "remaining@example.com").unwrap();

        db.set_budget_config(&user.id, 1000, 0, 0).unwrap();
        db.insert_token_usage(&user.id, "gpt-4o", "openai", 300, 200, 0.01)
            .unwrap();

        let remaining = db
            .remaining_for_period(&user.id, BudgetPeriod::Hourly)
            .unwrap();
        assert_eq!(remaining, 500);
    }

    #[test]
    fn test_remaining_for_unlimited_period() {
        let db = test_db();
        let user = db.create_user("unlimited_user", "unlimited@example.com").unwrap();

        let remaining = db
            .remaining_for_period(&user.id, BudgetPeriod::Hourly)
            .unwrap();
        assert_eq!(remaining, u64::MAX);
    }

    #[test]
    fn test_compute_budget_status_allowed() {
        let db = test_db();
        let user = db.create_user("status_user", "status@example.com").unwrap();
        db.set_budget_config(&user.id, 1000, 5000, 50000).unwrap();

        let status = db.compute_budget_status(&user.id).unwrap();
        assert!(status.allowed);
        assert_eq!(status.remaining_hour, 1000);
    }

    #[test]
    fn test_compute_budget_status_exceeded() {
        let db = test_db();
        let user = db.create_user("exceed_user", "exceed@example.com").unwrap();
        db.set_budget_config(&user.id, 100, 0, 0).unwrap();
        db.insert_token_usage(&user.id, "gpt-4o", "openai", 60, 60, 0.01)
            .unwrap();

        let status = db.compute_budget_status(&user.id).unwrap();
        assert!(!status.allowed);
        assert!(status.reason.unwrap().contains("hourly budget exceeded"));
    }

    #[tokio::test]
    async fn test_enforcer_check_budget() {
        let db = test_db_async().await;
        let user = db.create_user("enforcer_user", "enforcer@example.com").unwrap();
        db.set_budget_config(&user.id, 500, 5000, 50000).unwrap();

        let enforcer = BudgetEnforcer::new(Arc::new(db));
        let status = enforcer.check_budget(&user.id).await.unwrap();
        assert!(status.allowed);
    }

    #[tokio::test]
    async fn test_enforcer_enforce_budget_ok() {
        let db = test_db_async().await;
        let user = db.create_user("enforce_ok", "enforce_ok@example.com").unwrap();
        db.set_budget_config(&user.id, 1000, 0, 0).unwrap();

        let enforcer = BudgetEnforcer::new(Arc::new(db));
        let result = enforcer.enforce_budget(&user.id, 200).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_enforcer_enforce_budget_exceeded() {
        let db = test_db_async().await;
        let user = db
            .create_user("enforce_exceed", "enforce_exceed@example.com")
            .unwrap();
        db.set_budget_config(&user.id, 100, 0, 0).unwrap();
        db.insert_token_usage(&user.id, "gpt-4o", "openai", 80, 0, 0.01)
            .unwrap();

        let enforcer = BudgetEnforcer::new(Arc::new(db));
        let result = enforcer.enforce_budget(&user.id, 50).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            DatabaseError::BudgetExceeded(_) => {} // Expected
            other => panic!("expected BudgetExceeded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_enforcer_tokens_remaining_period() {
        let db = test_db_async().await;
        let user = db
            .create_user("tokens_remaining", "tokens_remaining@example.com")
            .unwrap();
        db.set_budget_config(&user.id, 1000, 0, 0).unwrap();
        db.insert_token_usage(&user.id, "gpt-4o", "openai", 300, 0, 0.01)
            .unwrap();

        let enforcer = BudgetEnforcer::new(Arc::new(db));
        let remaining = enforcer
            .tokens_remaining_in_period(&user.id, BudgetPeriod::Hourly)
            .await
            .unwrap();
        assert_eq!(remaining, 700);
    }

    #[tokio::test]
    async fn test_enforcer_unlimited_budget() {
        let db = test_db_async().await;
        let user = db
            .create_user("unlimited_enforcer", "unlimited_enforcer@example.com")
            .unwrap();

        let enforcer = BudgetEnforcer::new(Arc::new(db));
        let result = enforcer.enforce_budget(&user.id, 1_000_000).await;
        assert!(result.is_ok());
    }
}
