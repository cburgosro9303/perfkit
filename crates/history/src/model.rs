//! Tipos de datos (DTOs) del histórico: metadatos de run, registros, comparaciones,
//! tendencias, anotaciones y entradas de auditoría.

use serde::{Deserialize, Serialize};

/// Metadatos de contexto de una ejecución (CI/CD, autor, entorno).
///
/// Todos los campos son opcionales para no acoplar el motor a un pipeline concreto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMeta {
    /// Rama de VCS (p. ej. `main`, `feature/x`).
    pub branch: Option<String>,
    /// Identificador de build/pipeline (p. ej. número de job de CI).
    pub build: Option<String>,
    /// Entorno objetivo (p. ej. `staging`, `prod`).
    pub environment: Option<String>,
    /// Commit SHA.
    pub commit: Option<String>,
    /// Actor que lanzó la prueba (usuario o servicio).
    pub actor: Option<String>,
}

/// Vista resumida de un run persistido (lo que devuelven los listados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Id interno (clave primaria autoincremental).
    pub id: i64,
    /// Nombre del escenario.
    pub scenario: String,
    pub branch: Option<String>,
    pub build: Option<String>,
    pub environment: Option<String>,
    /// Timestamp de inicio (ISO-8601, tal cual lo emite `metrics`).
    pub started_at: String,
    /// Duración del run en segundos.
    pub duration_secs: f64,
    /// Throughput global (req/s).
    pub throughput: f64,
    /// Tasa de error global (0.0–1.0).
    pub error_rate: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    /// Número total de requests del run.
    pub requests: u64,
}

/// Resultado de comparar un run contra su baseline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// Variación porcentual de p95 (positivo = más lento = peor).
    pub p95_delta_pct: f64,
    /// Variación porcentual de throughput (positivo = más rápido = mejor).
    pub throughput_delta_pct: f64,
    /// Variación absoluta de la tasa de error en puntos porcentuales (pp).
    pub error_rate_delta: f64,
    /// `true` si la comparación viola la [`crate::RegressionPolicy`].
    pub is_regression: bool,
}

/// Métrica seleccionable para series de tendencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Percentil 95 de latencia (ms).
    P95,
    /// Throughput (req/s).
    Throughput,
    /// Tasa de error (0.0–1.0).
    ErrorRate,
}

/// Punto de una serie de tendencia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendPoint {
    /// Timestamp de inicio del run correspondiente.
    pub started_at: String,
    /// Valor de la métrica elegida.
    pub value: f64,
}

/// Anotación humana asociada a un run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: i64,
    pub run_id: i64,
    pub text: String,
    pub actor: Option<String>,
    /// Momento de creación (ISO-8601 UTC).
    pub created_at: String,
}

/// Entrada del registro de auditoría.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub actor: Option<String>,
    /// Acción registrada (texto libre, p. ej. `record_run`, `set_baseline`).
    pub action: String,
    /// Detalle adicional.
    pub detail: String,
    /// Momento del evento (ISO-8601 UTC).
    pub at: String,
}

/// Umbrales que definen cuándo una comparación es una regresión.
///
/// Por defecto (DoD §9): p95 peor en >10%, **o** tasa de error sube >1pp,
/// **o** throughput cae >10%.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegressionPolicy {
    /// Empeoramiento máximo tolerado de p95, en porcentaje (10.0 = 10%).
    pub max_p95_increase_pct: f64,
    /// Aumento máximo tolerado de la tasa de error, en puntos porcentuales (0.01 = 1pp).
    pub max_error_rate_increase: f64,
    /// Caída máxima tolerada de throughput, en porcentaje (10.0 = 10%).
    pub max_throughput_drop_pct: f64,
}

impl Default for RegressionPolicy {
    fn default() -> Self {
        Self {
            max_p95_increase_pct: 10.0,
            max_error_rate_increase: 0.01, // 1 punto porcentual
            max_throughput_drop_pct: 10.0,
        }
    }
}
