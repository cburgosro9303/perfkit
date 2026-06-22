//! Quality gate: compara un `RunSummary` contra umbrales para CI.

use metrics::RunSummary;
use serde::{Deserialize, Serialize};

/// Umbrales (cargados desde `thresholds.yaml`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Thresholds {
    #[serde(default)]
    pub max_error_rate: Option<f64>,
    #[serde(default)]
    pub max_p95_ms: Option<f64>,
    #[serde(default)]
    pub max_p99_ms: Option<f64>,
    #[serde(default)]
    pub min_throughput_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateResult {
    pub passed: bool,
    pub checks: Vec<GateCheck>,
}

pub fn load_thresholds(yaml: &str) -> Result<Thresholds, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(yaml)
}

pub fn evaluate_gate(s: &RunSummary, t: &Thresholds) -> GateResult {
    let o = &s.overall;
    let mut checks = Vec::new();
    let mut check = |name: &str, passed: bool, detail: String| {
        checks.push(GateCheck {
            name: name.to_string(),
            passed,
            detail,
        });
    };

    if let Some(max) = t.max_error_rate {
        check(
            "error_rate",
            o.error_rate <= max,
            format!("error_rate {:.4} (límite ≤ {:.4})", o.error_rate, max),
        );
    }
    if let Some(max) = t.max_p95_ms {
        check(
            "p95_ms",
            o.p95_ms <= max,
            format!("p95 {:.1}ms (límite ≤ {:.1}ms)", o.p95_ms, max),
        );
    }
    if let Some(max) = t.max_p99_ms {
        check(
            "p99_ms",
            o.p99_ms <= max,
            format!("p99 {:.1}ms (límite ≤ {:.1}ms)", o.p99_ms, max),
        );
    }
    if let Some(min) = t.min_throughput_per_sec {
        check(
            "throughput",
            o.throughput_per_sec >= min,
            format!(
                "throughput {:.1}/s (mínimo ≥ {:.1}/s)",
                o.throughput_per_sec, min
            ),
        );
    }

    let passed = checks.iter().all(|c| c.passed);
    GateResult { passed, checks }
}
