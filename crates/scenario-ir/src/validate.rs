//! Validación semántica del IR (más allá de lo que valida el schema JSON).

use crate::model::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json_path::JsonPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationIssue {
    pub severity: Severity,
    /// Ruta lógica al elemento (p.ej. "thread_groups[0].steps[1].url").
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn errors(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }
    pub fn warnings(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }
    pub fn is_ok(&self) -> bool {
        self.errors() == 0
    }
    fn error(&mut self, path: impl Into<String>, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Error,
            path: path.into(),
            message: msg.into(),
        });
    }
    fn warn(&mut self, path: impl Into<String>, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Warning,
            path: path.into(),
            message: msg.into(),
        });
    }
}

/// Valida un escenario y devuelve un reporte con errores y advertencias.
pub fn validate(s: &Scenario) -> ValidationReport {
    let mut r = ValidationReport::default();

    if s.version.trim().is_empty() {
        r.error("version", "la versión del IR no puede estar vacía");
    }
    if s.name.trim().is_empty() {
        r.warn("name", "el escenario no tiene nombre");
    }

    // Datasets
    for (i, d) in s.datasets.iter().enumerate() {
        let base = format!("datasets[{i}]");
        if d.path.trim().is_empty() {
            r.error(format!("{base}.path"), "ruta del dataset vacía");
        }
        if d.variable_names.is_empty() {
            r.error(
                format!("{base}.variable_names"),
                "el dataset no define nombres de variable",
            );
        }
    }

    let has_base_url = s
        .defaults
        .as_ref()
        .and_then(|d| d.base_url.as_ref())
        .map(|u| !u.trim().is_empty())
        .unwrap_or(false);

    if s.thread_groups.is_empty() {
        r.error("thread_groups", "el escenario no tiene grupos de hilos");
    }

    for (gi, g) in s.thread_groups.iter().enumerate() {
        let base = format!("thread_groups[{gi}]");
        if g.load.virtual_users == 0 {
            r.error(
                format!("{base}.load.virtual_users"),
                "virtual_users debe ser >= 1",
            );
        }
        if g.load.iterations.is_none() && g.load.duration_secs.is_none() && g.load.hold_secs == 0 {
            r.error(
                format!("{base}.load"),
                "la carga no está acotada: define iterations, duration_secs o hold_secs",
            );
        }
        if g.steps.is_empty() {
            r.warn(format!("{base}.steps"), "el grupo de hilos no tiene pasos");
        }
        validate_steps(&g.steps, &format!("{base}.steps"), has_base_url, &mut r);
    }

    r
}

fn validate_steps(steps: &[Step], base: &str, has_base_url: bool, r: &mut ValidationReport) {
    for (i, step) in steps.iter().enumerate() {
        let p = format!("{base}[{i}]");
        match step {
            Step::Http(h) => validate_http(h, &p, has_base_url, r),
            Step::Transaction(t) => {
                validate_steps(&t.steps, &format!("{p}.steps"), has_base_url, r)
            }
            Step::Loop(l) => validate_steps(&l.steps, &format!("{p}.steps"), has_base_url, r),
            Step::If(c) => validate_steps(&c.steps, &format!("{p}.steps"), has_base_url, r),
            Step::While(w) => {
                if w.max_iterations == 0 {
                    r.error(
                        format!("{p}.max_iterations"),
                        "max_iterations debe ser >= 1",
                    );
                }
                validate_steps(&w.steps, &format!("{p}.steps"), has_base_url, r)
            }
            Step::Throughput(t) => {
                if !(0.0..=100.0).contains(&t.percent) {
                    r.error(format!("{p}.percent"), "percent debe estar entre 0 y 100");
                }
                validate_steps(&t.steps, &format!("{p}.steps"), has_base_url, r)
            }
            Step::Interleave(c) => validate_steps(&c.steps, &format!("{p}.steps"), has_base_url, r),
            Step::Random(c) => validate_steps(&c.steps, &format!("{p}.steps"), has_base_url, r),
            Step::Kafka(k) => {
                if k.brokers.is_empty() {
                    r.error(format!("{p}.brokers"), "Kafka sampler sin brokers");
                }
                if k.topic.trim().is_empty() {
                    r.error(format!("{p}.topic"), "Kafka sampler sin topic");
                }
            }
            Step::Timer(_) => {}
        }
    }
}

fn validate_http(h: &HttpRequest, p: &str, has_base_url: bool, r: &mut ValidationReport) {
    let url = h.url.trim();
    if url.is_empty() {
        r.error(format!("{p}.url"), "URL vacía");
    } else {
        let absolute = url.starts_with("http://") || url.starts_with("https://");
        if !absolute && !has_base_url {
            r.error(
                format!("{p}.url"),
                "URL relativa sin defaults.base_url definido",
            );
        }
    }

    for (ai, a) in h.assertions.iter().enumerate() {
        let ap = format!("{p}.assertions[{ai}]");
        match a {
            Assertion::BodyMatches { pattern, .. } => {
                if let Err(e) = regex::Regex::new(pattern) {
                    r.error(ap, format!("regex de assertion inválida: {e}"));
                }
            }
            Assertion::JsonPath { path, .. } => {
                if let Err(e) = JsonPath::parse(path) {
                    r.error(ap, format!("JSONPath de assertion inválido: {e}"));
                }
            }
            _ => {}
        }
    }

    for (ei, ex) in h.extractors.iter().enumerate() {
        let ep = format!("{p}.extractors[{ei}]");
        match ex {
            Extractor::Regex { pattern, var, .. } => {
                if var.trim().is_empty() {
                    r.error(format!("{ep}.var"), "el extractor no define variable");
                }
                if let Err(e) = regex::Regex::new(pattern) {
                    r.error(ep, format!("regex de extractor inválida: {e}"));
                }
            }
            Extractor::JsonPath { path, var, .. } => {
                if var.trim().is_empty() {
                    r.error(format!("{ep}.var"), "el extractor no define variable");
                }
                if let Err(e) = JsonPath::parse(path) {
                    r.error(ep, format!("JSONPath de extractor inválido: {e}"));
                }
            }
            Extractor::Boundary {
                var, left, right, ..
            } => {
                if var.trim().is_empty() {
                    r.error(format!("{ep}.var"), "el extractor no define variable");
                }
                if left.is_empty() && right.is_empty() {
                    r.error(ep, "boundary extractor sin left ni right");
                }
            }
        }
    }
}
