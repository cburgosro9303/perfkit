//! `history` — histórico local de runs, baselines, tendencias, detección de
//! regresión, anotaciones, retención, RBAC y auditoría (Fase 10, DoD §9).
//!
//! Persiste en SQLite (vía `rusqlite`, feature `bundled`: SQLite se compila dentro,
//! sin librería del sistema). El punto de entrada es [`Store`], que crea el esquema
//! de forma idempotente al abrirse.
//!
//! ```
//! use history::{Store, RunMeta};
//! let store = Store::open_in_memory().unwrap();
//! // store.record_run(&summary, &RunMeta::default()).unwrap();
//! let _ = (&store, RunMeta::default());
//! ```

mod error;
mod model;
mod rbac;

pub use error::{HistoryError, Result};
pub use model::{
    Annotation, AuditEntry, Comparison, Metric, RegressionPolicy, RunMeta, RunRecord, TrendPoint,
};
pub use rbac::{Action, Role, can};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::Path;

/// Almacén de histórico respaldado por SQLite.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Abre (o crea) la base en `path`, aplicando el esquema si falta.
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Abre una base en memoria (útil para tests). El esquema se crea al vuelo.
    pub fn open_in_memory() -> Result<Store> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    /// Aplica PRAGMAs y migraciones idempotentes.
    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    /// Acceso de bajo nivel a la conexión (para usos avanzados/tests).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // ----------------------------------------------------------------- runs

    /// Inserta un run a partir de su [`metrics::RunSummary`] + metadatos.
    ///
    /// Persiste métricas clave en columnas (para consultas baratas) y el resumen
    /// completo en `summary_json`. Registra además una entrada de auditoría.
    /// Devuelve el id del run insertado.
    pub fn record_run(&self, summary: &metrics::RunSummary, meta: &RunMeta) -> Result<i64> {
        let overall = &summary.overall;
        let summary_json = serde_json::to_string(summary)?;

        self.conn.execute(
            "INSERT INTO runs (
                scenario, branch, build, environment, commit_sha,
                started_at, duration_secs, throughput, error_rate,
                p50_ms, p95_ms, p99_ms, requests, summary_json
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14
            )",
            params![
                summary.scenario_name,
                meta.branch,
                meta.build,
                meta.environment,
                meta.commit,
                summary.started_at,
                summary.duration_secs,
                overall.throughput_per_sec,
                overall.error_rate,
                overall.p50_ms,
                overall.p95_ms,
                overall.p99_ms,
                overall.count as i64,
                summary_json,
            ],
        )?;
        let id = self.conn.last_insert_rowid();

        self.audit(
            meta.actor.as_deref(),
            "record_run",
            &format!(
                "run {id} scenario='{}' env={:?} branch={:?}",
                summary.scenario_name, meta.environment, meta.branch
            ),
        )?;
        Ok(id)
    }

    /// Lista runs (más recientes primero), con filtros opcionales por escenario y entorno.
    pub fn list_runs(
        &self,
        scenario: Option<&str>,
        environment: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>> {
        let mut sql = String::from(
            "SELECT id, scenario, branch, build, environment, started_at,
                    duration_secs, throughput, error_rate, p95_ms, p99_ms, requests
             FROM runs WHERE 1=1",
        );
        // Filtros aplicados como parámetros posicionales según presencia.
        if scenario.is_some() {
            sql.push_str(" AND scenario = ?1");
        }
        if environment.is_some() {
            // El índice del parámetro depende de si hay escenario.
            if scenario.is_some() {
                sql.push_str(" AND environment = ?2");
            } else {
                sql.push_str(" AND environment = ?1");
            }
        }
        sql.push_str(" ORDER BY datetime(started_at) DESC, id DESC LIMIT ");
        sql.push_str(&limit.to_string());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = match (scenario, environment) {
            (Some(s), Some(e)) => stmt.query_map(params![s, e], row_to_run_record)?,
            (Some(s), None) => stmt.query_map(params![s], row_to_run_record)?,
            (None, Some(e)) => stmt.query_map(params![e], row_to_run_record)?,
            (None, None) => stmt.query_map([], row_to_run_record)?,
        };
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Recupera un run por id, si existe.
    pub fn get_run(&self, run_id: i64) -> Result<Option<RunRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scenario, branch, build, environment, started_at,
                    duration_secs, throughput, error_rate, p95_ms, p99_ms, requests
             FROM runs WHERE id = ?1",
        )?;
        let rec = stmt
            .query_row(params![run_id], row_to_run_record)
            .optional()?;
        Ok(rec)
    }

    // ------------------------------------------------------------- baselines

    /// Fija (o reemplaza) la baseline para la clave `(branch, environment, scenario)`.
    pub fn set_baseline(
        &self,
        branch: &str,
        environment: &str,
        scenario: &str,
        run_id: i64,
    ) -> Result<()> {
        // Validamos que el run exista para evitar baselines colgantes.
        if self.get_run(run_id)?.is_none() {
            return Err(HistoryError::NotFound(format!("run {run_id}")));
        }
        self.conn.execute(
            "INSERT INTO baselines (branch, environment, scenario, run_id, set_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(branch, environment, scenario)
             DO UPDATE SET run_id = excluded.run_id, set_at = excluded.set_at",
            params![branch, environment, scenario, run_id, now()],
        )?;
        Ok(())
    }

    /// Devuelve el [`RunRecord`] de la baseline para la clave dada, si existe.
    pub fn get_baseline(
        &self,
        branch: &str,
        environment: &str,
        scenario: &str,
    ) -> Result<Option<RunRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.scenario, r.branch, r.build, r.environment, r.started_at,
                    r.duration_secs, r.throughput, r.error_rate, r.p95_ms, r.p99_ms, r.requests
             FROM baselines b
             JOIN runs r ON r.id = b.run_id
             WHERE b.branch = ?1 AND b.environment = ?2 AND b.scenario = ?3",
        )?;
        let rec = stmt
            .query_row(params![branch, environment, scenario], row_to_run_record)
            .optional()?;
        Ok(rec)
    }

    // ------------------------------------------------------------ comparison

    /// Compara el run `run_id` con su baseline `(branch, environment, scenario)`.
    ///
    /// Usa la [`RegressionPolicy`] por defecto. Devuelve `None` si no hay baseline
    /// o el run no existe.
    pub fn compare_to_baseline(
        &self,
        run_id: i64,
        branch: &str,
        environment: &str,
        scenario: &str,
    ) -> Result<Option<Comparison>> {
        self.compare_to_baseline_with(
            run_id,
            branch,
            environment,
            scenario,
            &RegressionPolicy::default(),
        )
    }

    /// Igual que [`Store::compare_to_baseline`] pero con política explícita.
    pub fn compare_to_baseline_with(
        &self,
        run_id: i64,
        branch: &str,
        environment: &str,
        scenario: &str,
        policy: &RegressionPolicy,
    ) -> Result<Option<Comparison>> {
        let candidate = match self.get_run(run_id)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let baseline = match self.get_baseline(branch, environment, scenario)? {
            Some(b) => b,
            None => return Ok(None),
        };
        Ok(Some(compare(&baseline, &candidate, policy)))
    }

    // ---------------------------------------------------------------- trends

    /// Serie de tendencia de `metric` para `(scenario, environment)`, en orden
    /// cronológico ascendente (más antiguo primero), limitada a los `limit` runs
    /// más recientes.
    pub fn trend(
        &self,
        scenario: &str,
        environment: &str,
        metric: Metric,
        limit: usize,
    ) -> Result<Vec<TrendPoint>> {
        let col = match metric {
            Metric::P95 => "p95_ms",
            Metric::Throughput => "throughput",
            Metric::ErrorRate => "error_rate",
        };
        // Tomamos los N más recientes, luego invertimos para devolver ascendente.
        let sql = format!(
            "SELECT started_at, {col} AS value FROM (
                 SELECT started_at, {col}, id
                 FROM runs
                 WHERE scenario = ?1 AND environment = ?2
                 ORDER BY datetime(started_at) DESC, id DESC
                 LIMIT {limit}
             ) ORDER BY datetime(started_at) ASC, id ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![scenario, environment], |row| {
            Ok(TrendPoint {
                started_at: row.get(0)?,
                value: row.get(1)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ----------------------------------------------------------- annotations

    /// Añade una anotación a un run. Registra auditoría.
    pub fn annotate(&self, run_id: i64, text: &str, actor: Option<&str>) -> Result<()> {
        if self.get_run(run_id)?.is_none() {
            return Err(HistoryError::NotFound(format!("run {run_id}")));
        }
        self.conn.execute(
            "INSERT INTO annotations (run_id, text, actor, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, text, actor, now()],
        )?;
        self.audit(actor, "annotate", &format!("run {run_id}"))?;
        Ok(())
    }

    /// Lista las anotaciones de un run (más antiguas primero).
    pub fn annotations(&self, run_id: i64) -> Result<Vec<Annotation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, text, actor, created_at
             FROM annotations WHERE run_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                run_id: row.get(1)?,
                text: row.get(2)?,
                actor: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ------------------------------------------------------------- retention

    /// Borra los runs con `started_at` anterior a `days` días respecto a ahora (UTC).
    ///
    /// Las anotaciones asociadas se eliminan en cascada (FK). Las baselines que
    /// apuntaban a un run borrado también desaparecen (FK ON DELETE CASCADE).
    /// Devuelve el número de runs eliminados. Registra auditoría.
    pub fn purge_older_than(&self, days: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();
        let deleted = self.conn.execute(
            "DELETE FROM runs WHERE datetime(started_at) < datetime(?1)",
            params![cutoff_str],
        )?;
        self.audit(
            None,
            "purge_older_than",
            &format!("days={days} cutoff={cutoff_str} deleted={deleted}"),
        )?;
        Ok(deleted)
    }

    // ----------------------------------------------------------------- audit

    /// Inserta una entrada en el registro de auditoría.
    pub fn audit(&self, actor: Option<&str>, action: &str, detail: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit (actor, action, detail, at)
             VALUES (?1, ?2, ?3, ?4)",
            params![actor, action, detail, now()],
        )?;
        Ok(())
    }

    /// Devuelve las últimas `limit` entradas de auditoría (más recientes primero).
    pub fn audit_log(&self, limit: usize) -> Result<Vec<AuditEntry>> {
        let sql = format!(
            "SELECT id, actor, action, detail, at FROM audit
             ORDER BY id DESC LIMIT {limit}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(AuditEntry {
                id: row.get(0)?,
                actor: row.get(1)?,
                action: row.get(2)?,
                detail: row.get(3)?,
                at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

/// Calcula la comparación entre baseline y candidato bajo `policy`.
fn compare(baseline: &RunRecord, candidate: &RunRecord, policy: &RegressionPolicy) -> Comparison {
    // p95: positivo = candidato más lento (peor).
    let p95_delta_pct = pct_change(baseline.p95_ms, candidate.p95_ms);
    // throughput: positivo = candidato más rápido (mejor).
    let throughput_delta_pct = pct_change(baseline.throughput, candidate.throughput);
    // error rate: positivo = candidato con más errores (peor). En fracción (1pp = 0.01).
    let error_rate_delta = candidate.error_rate - baseline.error_rate;

    let p95_regressed = p95_delta_pct > policy.max_p95_increase_pct;
    let error_regressed = error_rate_delta > policy.max_error_rate_increase;
    // Caída de throughput: throughput_delta_pct negativo cuyo valor absoluto supera el umbral.
    let throughput_regressed = throughput_delta_pct < -policy.max_throughput_drop_pct;

    Comparison {
        p95_delta_pct,
        throughput_delta_pct,
        error_rate_delta,
        is_regression: p95_regressed || error_regressed || throughput_regressed,
    }
}

/// Variación porcentual de `from` a `to`. Si `from == 0`, devuelve 0 si `to == 0`,
/// si no `+inf`/`-inf` evitado devolviendo 100*to (crecimiento desde nada).
fn pct_change(from: f64, to: f64) -> f64 {
    if from == 0.0 {
        if to == 0.0 {
            0.0
        } else {
            // Crecimiento desde cero: lo tratamos como 100% por cada unidad relativa.
            // Devolver un valor grande garantiza que dispare la regresión si aplica.
            to.signum() * f64::INFINITY
        }
    } else {
        (to - from) / from * 100.0
    }
}

/// Timestamp UTC actual en RFC-3339.
fn now() -> String {
    Utc::now().to_rfc3339()
}

/// Mapea una fila SELECT (orden de columnas de [`RunRecord`]) a la struct.
fn row_to_run_record(row: &Row<'_>) -> rusqlite::Result<RunRecord> {
    let requests: i64 = row.get(11)?;
    Ok(RunRecord {
        id: row.get(0)?,
        scenario: row.get(1)?,
        branch: row.get(2)?,
        build: row.get(3)?,
        environment: row.get(4)?,
        started_at: row.get(5)?,
        duration_secs: row.get(6)?,
        throughput: row.get(7)?,
        error_rate: row.get(8)?,
        p95_ms: row.get(9)?,
        p99_ms: row.get(10)?,
        requests: requests.max(0) as u64,
    })
}

/// Esquema SQL. Las migraciones son idempotentes (`IF NOT EXISTS`).
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS runs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    scenario      TEXT    NOT NULL,
    branch        TEXT,
    build         TEXT,
    environment   TEXT,
    commit_sha    TEXT,
    started_at    TEXT    NOT NULL,
    duration_secs REAL    NOT NULL,
    throughput    REAL    NOT NULL,
    error_rate    REAL    NOT NULL,
    p50_ms        REAL    NOT NULL,
    p95_ms        REAL    NOT NULL,
    p99_ms        REAL    NOT NULL,
    requests      INTEGER NOT NULL,
    summary_json  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_scenario_env
    ON runs (scenario, environment, started_at);

CREATE TABLE IF NOT EXISTS baselines (
    branch      TEXT NOT NULL,
    environment TEXT NOT NULL,
    scenario    TEXT NOT NULL,
    run_id      INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    set_at      TEXT NOT NULL,
    PRIMARY KEY (branch, environment, scenario)
);

CREATE TABLE IF NOT EXISTS annotations (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id     INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    text       TEXT NOT NULL,
    actor      TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_annotations_run ON annotations (run_id);

CREATE TABLE IF NOT EXISTS audit (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    actor  TEXT,
    action TEXT NOT NULL,
    detail TEXT NOT NULL,
    at     TEXT NOT NULL
);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::{LabelStats, RunConfig, RunSummary, SampleKind};

    /// Construye un `RunSummary` mínimo con las métricas globales que nos interesan.
    fn summary(
        scenario: &str,
        started_at: &str,
        p95: f64,
        throughput: f64,
        error_rate: f64,
        count: u64,
    ) -> RunSummary {
        let overall = LabelStats {
            label: "ALL".into(),
            kind: SampleKind::Http,
            count,
            errors: (count as f64 * error_rate).round() as u64,
            error_rate,
            throughput_per_sec: throughput,
            min_ms: 1.0,
            mean_ms: p95 / 2.0,
            max_ms: p95 * 1.5,
            p50_ms: p95 / 2.0,
            p90_ms: p95 * 0.95,
            p95_ms: p95,
            p99_ms: p95 * 1.2,
            p999_ms: p95 * 1.3,
            bytes_total: count * 100,
            ..Default::default()
        };
        RunSummary {
            run_id: format!("run-{started_at}"),
            scenario_name: scenario.into(),
            started_at: started_at.into(),
            duration_secs: 60.0,
            config: RunConfig::default(),
            overall,
            labels: vec![],
            timeseries: vec![],
            errors: vec![],
            ..Default::default()
        }
    }

    fn meta() -> RunMeta {
        RunMeta {
            branch: Some("main".into()),
            build: Some("123".into()),
            environment: Some("staging".into()),
            commit: Some("abc123".into()),
            actor: Some("ci-bot".into()),
        }
    }

    #[test]
    fn open_in_memory_creates_schema_and_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        // Re-aplicar el esquema no debe fallar.
        store.connection().execute_batch(SCHEMA).unwrap();
        assert!(store.list_runs(None, None, 10).unwrap().is_empty());
    }

    #[test]
    fn record_and_list_runs() {
        let store = Store::open_in_memory().unwrap();
        let id1 = store
            .record_run(
                &summary("checkout", "2026-06-01T10:00:00Z", 100.0, 500.0, 0.01, 1000),
                &meta(),
            )
            .unwrap();
        let id2 = store
            .record_run(
                &summary("checkout", "2026-06-02T10:00:00Z", 120.0, 480.0, 0.02, 1000),
                &meta(),
            )
            .unwrap();
        assert!(id2 > id1);

        let runs = store
            .list_runs(Some("checkout"), Some("staging"), 10)
            .unwrap();
        assert_eq!(runs.len(), 2);
        // Más reciente primero.
        assert_eq!(runs[0].id, id2);
        assert_eq!(runs[0].requests, 1000);
        assert_eq!(runs[0].scenario, "checkout");
        assert_eq!(runs[0].environment.as_deref(), Some("staging"));

        // Filtro que no coincide.
        assert!(store.list_runs(Some("nope"), None, 10).unwrap().is_empty());
        // Sin filtros.
        assert_eq!(store.list_runs(None, None, 10).unwrap().len(), 2);
    }

    #[test]
    fn baseline_set_get_and_regression_detection() {
        let store = Store::open_in_memory().unwrap();
        // Run base: p95=100, throughput=500, error=1%.
        let base = store
            .record_run(
                &summary("api", "2026-06-01T00:00:00Z", 100.0, 500.0, 0.01, 1000),
                &meta(),
            )
            .unwrap();
        // Run peor: p95=150 (+50% > 10%) → regresión.
        let worse = store
            .record_run(
                &summary("api", "2026-06-02T00:00:00Z", 150.0, 500.0, 0.01, 1000),
                &meta(),
            )
            .unwrap();
        // Run mejor: p95=90 (-10%), throughput igual, error igual → no regresión.
        let better = store
            .record_run(
                &summary("api", "2026-06-03T00:00:00Z", 90.0, 520.0, 0.005, 1000),
                &meta(),
            )
            .unwrap();

        store.set_baseline("main", "staging", "api", base).unwrap();
        let got = store
            .get_baseline("main", "staging", "api")
            .unwrap()
            .unwrap();
        assert_eq!(got.id, base);

        let cmp_worse = store
            .compare_to_baseline(worse, "main", "staging", "api")
            .unwrap()
            .unwrap();
        assert!(cmp_worse.is_regression, "p95 +50% must be a regression");
        assert!((cmp_worse.p95_delta_pct - 50.0).abs() < 1e-6);

        let cmp_better = store
            .compare_to_baseline(better, "main", "staging", "api")
            .unwrap()
            .unwrap();
        assert!(!cmp_better.is_regression, "improvement is not a regression");
        assert!(cmp_better.p95_delta_pct < 0.0);

        // Sin baseline para otra clave → None.
        assert!(
            store
                .compare_to_baseline(worse, "main", "prod", "api")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn baseline_upsert_replaces_run() {
        let store = Store::open_in_memory().unwrap();
        let a = store
            .record_run(
                &summary("s", "2026-06-01T00:00:00Z", 100.0, 500.0, 0.0, 10),
                &meta(),
            )
            .unwrap();
        let b = store
            .record_run(
                &summary("s", "2026-06-02T00:00:00Z", 100.0, 500.0, 0.0, 10),
                &meta(),
            )
            .unwrap();
        store.set_baseline("main", "staging", "s", a).unwrap();
        store.set_baseline("main", "staging", "s", b).unwrap();
        let got = store.get_baseline("main", "staging", "s").unwrap().unwrap();
        assert_eq!(got.id, b, "upsert should replace the run_id");
    }

    #[test]
    fn regression_from_error_rate_and_throughput() {
        let store = Store::open_in_memory().unwrap();
        let base = store
            .record_run(
                &summary("e", "2026-06-01T00:00:00Z", 100.0, 1000.0, 0.01, 1000),
                &meta(),
            )
            .unwrap();
        store.set_baseline("main", "staging", "e", base).unwrap();

        // Error sube de 1% a 3% (+2pp > 1pp) → regresión.
        let err_up = store
            .record_run(
                &summary("e", "2026-06-02T00:00:00Z", 100.0, 1000.0, 0.03, 1000),
                &meta(),
            )
            .unwrap();
        assert!(
            store
                .compare_to_baseline(err_up, "main", "staging", "e")
                .unwrap()
                .unwrap()
                .is_regression
        );

        // Throughput cae de 1000 a 800 (-20% > 10%) → regresión.
        let tp_down = store
            .record_run(
                &summary("e", "2026-06-03T00:00:00Z", 100.0, 800.0, 0.01, 1000),
                &meta(),
            )
            .unwrap();
        assert!(
            store
                .compare_to_baseline(tp_down, "main", "staging", "e")
                .unwrap()
                .unwrap()
                .is_regression
        );
    }

    #[test]
    fn trend_is_chronological() {
        let store = Store::open_in_memory().unwrap();
        // Insertamos fuera de orden cronológico para probar el orden de salida.
        store
            .record_run(
                &summary("t", "2026-06-03T00:00:00Z", 130.0, 500.0, 0.0, 100),
                &meta(),
            )
            .unwrap();
        store
            .record_run(
                &summary("t", "2026-06-01T00:00:00Z", 110.0, 500.0, 0.0, 100),
                &meta(),
            )
            .unwrap();
        store
            .record_run(
                &summary("t", "2026-06-02T00:00:00Z", 120.0, 500.0, 0.0, 100),
                &meta(),
            )
            .unwrap();

        let points = store.trend("t", "staging", Metric::P95, 10).unwrap();
        assert_eq!(points.len(), 3);
        // Orden cronológico ascendente.
        assert_eq!(points[0].started_at, "2026-06-01T00:00:00Z");
        assert_eq!(points[1].started_at, "2026-06-02T00:00:00Z");
        assert_eq!(points[2].started_at, "2026-06-03T00:00:00Z");
        // Valores de p95 en orden.
        assert!((points[0].value - 110.0).abs() < 1e-6);
        assert!((points[1].value - 120.0).abs() < 1e-6);
        assert!((points[2].value - 130.0).abs() < 1e-6);

        // Otra métrica.
        let tp = store.trend("t", "staging", Metric::Throughput, 10).unwrap();
        assert_eq!(tp.len(), 3);
        assert!((tp[0].value - 500.0).abs() < 1e-6);
    }

    #[test]
    fn annotations_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        let id = store
            .record_run(
                &summary("a", "2026-06-01T00:00:00Z", 100.0, 500.0, 0.0, 10),
                &meta(),
            )
            .unwrap();
        store.annotate(id, "looks slow", Some("alice")).unwrap();
        store.annotate(id, "investigating", Some("bob")).unwrap();
        let anns = store.annotations(id).unwrap();
        assert_eq!(anns.len(), 2);
        assert_eq!(anns[0].text, "looks slow");
        assert_eq!(anns[0].actor.as_deref(), Some("alice"));
        assert_eq!(anns[1].text, "investigating");

        // Anotar un run inexistente falla.
        assert!(store.annotate(9999, "x", None).is_err());
    }

    #[test]
    fn retention_purges_old_rows() {
        let store = Store::open_in_memory().unwrap();
        // Run antiguo (100 días atrás) y reciente (hoy).
        let old_ts = (Utc::now() - chrono::Duration::days(100)).to_rfc3339();
        let new_ts = Utc::now().to_rfc3339();
        let old_id = store
            .record_run(&summary("r", &old_ts, 100.0, 500.0, 0.0, 10), &meta())
            .unwrap();
        let new_id = store
            .record_run(&summary("r", &new_ts, 100.0, 500.0, 0.0, 10), &meta())
            .unwrap();

        let deleted = store.purge_older_than(30).unwrap();
        assert_eq!(deleted, 1);
        assert!(store.get_run(old_id).unwrap().is_none());
        assert!(store.get_run(new_id).unwrap().is_some());
    }

    #[test]
    fn audit_log_records_entries() {
        let store = Store::open_in_memory().unwrap();
        // record_run ya escribe una entrada de auditoría.
        store
            .record_run(
                &summary("au", "2026-06-01T00:00:00Z", 100.0, 500.0, 0.0, 10),
                &meta(),
            )
            .unwrap();
        store.audit(Some("admin"), "login", "from cli").unwrap();

        let log = store.audit_log(10).unwrap();
        assert!(log.len() >= 2);
        // Más reciente primero.
        assert_eq!(log[0].action, "login");
        assert_eq!(log[0].actor.as_deref(), Some("admin"));
        assert!(log.iter().any(|e| e.action == "record_run"));
    }

    #[test]
    fn rbac_matrix() {
        assert!(!can(Role::Viewer, Action::SetBaseline));
        assert!(can(Role::Admin, Action::ManageRetention));
        assert!(can(Role::Viewer, Action::ViewRuns));
        assert!(can(Role::Operator, Action::Annotate));
        assert!(!can(Role::Operator, Action::ManageRetention));
    }

    #[test]
    fn set_baseline_rejects_missing_run() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.set_baseline("main", "staging", "x", 42).is_err());
    }
}
