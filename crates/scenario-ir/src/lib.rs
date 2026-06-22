//! `scenario-ir` — IR canónico de perfkit: el contrato compartido entre importer,
//! engine, reports, CLI y UI.
//!
//! - [`model`]: tipos del escenario (Scenario, ThreadGroup, Step, HttpRequest, ...).
//! - [`migration`]: reporte de fidelidad JMX → IR.
//! - [`validate`]: validación semántica.
//! - Serialización YAML/JSON y generación de JSON Schema (schemars).

pub mod migration;
pub mod model;
pub mod validate;

pub use migration::{FidelitySummary, MappedElement, MappingStatus, MigrationReport};
pub use model::*;
pub use validate::{Severity, ValidationIssue, ValidationReport, validate};

use schemars::schema_for;

#[derive(Debug, thiserror::Error)]
pub enum IrError {
    #[error("error de YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("error de JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("error binario .pkb (encode): {0}")]
    PkbEncode(#[from] rmp_serde::encode::Error),
    #[error("error binario .pkb (decode): {0}")]
    PkbDecode(#[from] rmp_serde::decode::Error),
}

/// Serializa a `.pkb`, el formato binario compacto de perfkit (MessagePack).
///
/// Más pequeño y rápido de parsear que YAML/JSON; ideal para artefactos y
/// transporte. No es legible por humanos (para eso está el YAML).
pub fn to_pkb(s: &Scenario) -> Result<Vec<u8>, IrError> {
    Ok(rmp_serde::to_vec_named(s)?)
}

/// Deserializa un escenario desde `.pkb` (MessagePack).
pub fn from_pkb(bytes: &[u8]) -> Result<Scenario, IrError> {
    Ok(rmp_serde::from_slice(bytes)?)
}

/// Parsea un escenario desde YAML.
pub fn from_yaml(yaml: &str) -> Result<Scenario, IrError> {
    Ok(serde_yaml_ng::from_str(yaml)?)
}

/// Serializa un escenario a YAML.
pub fn to_yaml(s: &Scenario) -> Result<String, IrError> {
    Ok(serde_yaml_ng::to_string(s)?)
}

/// Parsea un escenario desde JSON.
pub fn from_json(json: &str) -> Result<Scenario, IrError> {
    Ok(serde_json::from_str(json)?)
}

/// Serializa un escenario a JSON con indentación.
pub fn to_json_pretty(s: &Scenario) -> Result<String, IrError> {
    Ok(serde_json::to_string_pretty(s)?)
}

/// JSON Schema del escenario (Draft 2020-12 vía schemars).
pub fn scenario_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(Scenario)).expect("schema serializable")
}

/// JSON Schema del reporte de fidelidad.
pub fn migration_report_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(MigrationReport)).expect("schema serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample() -> Scenario {
        let mut s = Scenario::new("demo");
        s.defaults = Some(HttpDefaults {
            base_url: Some("https://example.test".into()),
            ..Default::default()
        });
        let mut headers = BTreeMap::new();
        headers.insert("Accept".into(), "application/json".into());
        s.thread_groups.push(ThreadGroup {
            name: "tg".into(),
            load: LoadProfile {
                virtual_users: 5,
                ramp_up_secs: 1,
                hold_secs: 0,
                ramp_down_secs: 0,
                iterations: Some(2),
                duration_secs: None,
            },
            on_error: OnError::Continue,
            steps: vec![Step::Http(HttpRequest {
                name: "get".into(),
                method: HttpMethod::Get,
                url: "/health".into(),
                headers,
                query: BTreeMap::new(),
                body: None,
                follow_redirects: None,
                timeout_ms: None,
                timers: vec![Timer::Constant { delay_ms: 10 }],
                assertions: vec![Assertion::StatusCode { codes: vec![200] }],
                extractors: vec![],
            })],
        });
        s
    }

    #[test]
    fn roundtrip_yaml() {
        let s = sample();
        let yaml = to_yaml(&s).unwrap();
        let back = from_yaml(&yaml).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn roundtrip_pkb_and_smaller_than_yaml() {
        let s = sample();
        let bytes = to_pkb(&s).unwrap();
        assert_eq!(from_pkb(&bytes).unwrap(), s);
        // El binario compacto debe pesar menos que el YAML.
        assert!(bytes.len() < to_yaml(&s).unwrap().len());
    }

    #[test]
    fn valid_scenario_has_no_errors() {
        let r = validate(&sample());
        assert!(r.is_ok(), "issues: {:?}", r.issues);
    }

    #[test]
    fn relative_url_without_base_is_error() {
        let mut s = sample();
        s.defaults = None;
        let r = validate(&s);
        assert!(!r.is_ok());
    }

    #[test]
    fn schemas_generate() {
        assert!(scenario_schema().is_object());
        assert!(migration_report_schema().is_object());
    }

    #[test]
    fn fidelity_summary_math() {
        let mut rep = MigrationReport::new("x.jmx", "test");
        for st in [
            MappingStatus::Migrated,
            MappingStatus::Migrated,
            MappingStatus::Assisted,
            MappingStatus::Ignored,
        ] {
            rep.push(MappedElement {
                jmx_type: "X".into(),
                jmx_name: "n".into(),
                path: "p".into(),
                status: st,
                ir_ref: None,
                reason: None,
                suggestion: None,
            });
        }
        rep.recompute_summary();
        assert_eq!(rep.summary.total, 4);
        assert_eq!(rep.summary.migrated, 2);
        // migratable = 3 (excluye 1 ignored); 2/3 = 66.66...%
        assert!((rep.summary.fidelity_pct - 66.6667).abs() < 0.01);
    }
}
