//! Reporte de fidelidad de migración JMX → IR.
//!
//! Contrato clave del producto: el importador **nunca falla en silencio**. Cada
//! elemento del JMX queda clasificado como `migrated | assisted | unsupported | ignored`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Estado de migración de un elemento JMeter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MappingStatus {
    /// Migrado 1:1 a IR nativo (Nivel 1).
    Migrated,
    /// Migración asistida: requiere revisión/acción manual (Nivel 2).
    Assisted,
    /// No soportado: reportado explícitamente, no ejecutado (Nivel 4).
    Unsupported,
    /// Ignorado a propósito con razón (p.ej. listeners como metadata).
    Ignored,
}

impl MappingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MappingStatus::Migrated => "migrated",
            MappingStatus::Assisted => "assisted",
            MappingStatus::Unsupported => "unsupported",
            MappingStatus::Ignored => "ignored",
        }
    }
}

/// Un elemento JMeter y cómo se mapeó al IR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MappedElement {
    /// Tipo JMeter (p.ej. "HTTPSamplerProxy", "ThreadGroup").
    pub jmx_type: String,
    /// Nombre legible (testname) del elemento.
    pub jmx_name: String,
    /// Ruta en el árbol (p.ej. "Test Plan > Thread Group > Login").
    pub path: String,
    /// Estado de migración.
    pub status: MappingStatus,
    /// Referencia a dónde quedó en el IR, si aplica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir_ref: Option<String>,
    /// Razón (obligatoria para unsupported/ignored/assisted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Sugerencia de acción para el QA.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Resumen agregado de fidelidad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
pub struct FidelitySummary {
    pub total: usize,
    pub migrated: usize,
    pub assisted: usize,
    pub unsupported: usize,
    pub ignored: usize,
    /// Porcentaje migrado 1:1 sobre el total (0–100).
    pub fidelity_pct: f64,
}

/// Reporte completo de fidelidad de una migración.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MigrationReport {
    /// Archivo .jmx de origen.
    pub source: String,
    /// Generador (p.ej. "perfkit-jmx-importer 0.1.0").
    pub generated_by: String,
    pub summary: FidelitySummary,
    pub elements: Vec<MappedElement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl MigrationReport {
    pub fn new(source: impl Into<String>, generated_by: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            generated_by: generated_by.into(),
            summary: FidelitySummary::default(),
            elements: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn push(&mut self, el: MappedElement) {
        self.elements.push(el);
    }

    /// Recalcula el resumen a partir de los elementos.
    pub fn recompute_summary(&mut self) {
        let mut s = FidelitySummary {
            total: self.elements.len(),
            ..Default::default()
        };
        for e in &self.elements {
            match e.status {
                MappingStatus::Migrated => s.migrated += 1,
                MappingStatus::Assisted => s.assisted += 1,
                MappingStatus::Unsupported => s.unsupported += 1,
                MappingStatus::Ignored => s.ignored += 1,
            }
        }
        // La fidelidad mide migración declarativa exitosa: migrated sobre lo "migrable"
        // (todo menos lo ignorado a propósito).
        let migratable = s.total.saturating_sub(s.ignored);
        s.fidelity_pct = if migratable == 0 {
            100.0
        } else {
            (s.migrated as f64 / migratable as f64) * 100.0
        };
        self.summary = s;
    }

    /// ¿Hay elementos que requieren atención (assisted o unsupported)?
    pub fn needs_attention(&self) -> bool {
        self.summary.assisted > 0 || self.summary.unsupported > 0
    }
}
