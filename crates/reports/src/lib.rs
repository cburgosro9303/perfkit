//! `reports` — reportes nativos (JSON, JUnit, HTML offline) y quality gate.
//!
//! Regla (§6.7): OTel/Prometheus no reemplazan el reporte nativo; el HTML debe abrir
//! offline y ser compartible como artefacto de pipeline.

mod gate;
mod html;
mod junit;

pub use gate::{GateCheck, GateResult, Thresholds, evaluate_gate, load_thresholds};
pub use html::html_report;
pub use junit::junit_xml;

use metrics::RunSummary;

/// summary.json (machine-readable).
pub fn summary_json(s: &RunSummary) -> String {
    serde_json::to_string_pretty(s).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::{LabelStats, RunConfig, RunSummary, SampleKind};

    fn stats(label: &str, count: u64, errors: u64, p95: f64) -> LabelStats {
        LabelStats {
            label: label.into(),
            kind: SampleKind::Http,
            count,
            errors,
            error_rate: errors as f64 / count as f64,
            throughput_per_sec: count as f64,
            min_ms: 1.0,
            mean_ms: 10.0,
            max_ms: 50.0,
            p50_ms: 8.0,
            p90_ms: p95 - 5.0,
            p95_ms: p95,
            p99_ms: p95 + 10.0,
            p999_ms: p95 + 20.0,
            bytes_total: 1024,
            ..Default::default()
        }
    }

    fn summary() -> RunSummary {
        RunSummary {
            run_id: "run-1".into(),
            scenario_name: "demo".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            duration_secs: 10.0,
            config: RunConfig {
                virtual_users: 5,
                thread_groups: 1,
            },
            overall: stats("ALL", 100, 1, 120.0),
            labels: vec![stats("GET /", 100, 1, 120.0)],
            timeseries: vec![],
            errors: vec![metrics::ErrorBucket {
                message: "HTTP 500".into(),
                count: 1,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn html_is_offline_selfcontained() {
        let h = html_report(&summary());
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("perfkit"));
        // sin recursos externos: nada de <script src=...> ni <link href=...> a la red
        assert!(!h.contains("src=\"http"));
        assert!(!h.contains("href=\"http"));
        assert!(!h.contains("//cdn"));
        assert!(h.contains("GET /"));
    }

    #[test]
    fn junit_marks_failures() {
        let x = junit_xml(&summary());
        assert!(x.contains("<testsuite"));
        assert!(x.contains("failures=\"1\""));
        assert!(x.contains("<failure"));
    }

    #[test]
    fn gate_passes_and_fails() {
        let s = summary();
        let pass = evaluate_gate(
            &s,
            &Thresholds {
                max_error_rate: Some(0.05),
                max_p95_ms: Some(200.0),
                ..Default::default()
            },
        );
        assert!(pass.passed);
        let fail = evaluate_gate(
            &s,
            &Thresholds {
                max_p95_ms: Some(100.0),
                ..Default::default()
            },
        );
        assert!(!fail.passed);
    }
}
