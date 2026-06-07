// Cost Governance — Token budgets, multi-user quotas, rate limiting.
//
// Sistema completo de control de gastos para equipos empresariales.
// Permite definir límites de consumo de tokens por:
// - Usuario individual
// - Proyecto / equipo
// - Período: hora, día, semana, mes
//
// ## Features
// - Topes de tokens configurables por período
// - Multi-usuario con cuotas independientes
// - Multi-API (varios proveedores simultáneos)
// - Alertas automáticas al acercarse al límite
// - Corte automático al superar el límite
// - Logs de uso por usuario, modelo, proyecto
// - Reportes y estadísticas

pub mod database;
pub mod quotas;
pub mod alerts;
pub mod tracking;
pub mod users;

// -- Re-exports ------------------------------------------------

pub use database::{Database, DatabaseError};
pub use users::{ApiKey, User};
pub use tracking::{TokenTracker, UsagePeriod, UsageSummary, estimate_tokens};
pub use quotas::{BudgetEnforcer, BudgetPeriod, BudgetConfig, BudgetStatus};
pub use alerts::{AlertManager, Alert, AlertType, AlertThresholds, Severity};
