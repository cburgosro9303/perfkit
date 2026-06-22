//! Modelo canónico (IR) de un escenario de carga.
//!
//! Es el contrato compartido entre importer, engine, reports, CLI y UI.
//! Cambios aquí requieren ADR + actualización de schema, fixtures y docs (ver §8 del plan).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Versión actual del IR (semver). Un cambio incompatible debe subir esta versión.
/// 0.2.0: Throughput/Interleave/Random controllers (ADR-008).
/// 0.3.0: Kafka producer sampler (ADR-009).
pub const IR_VERSION: &str = "0.3.0";

fn default_version() -> String {
    IR_VERSION.to_string()
}

/// Escenario completo de carga.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scenario {
    /// Versión del IR (semver).
    #[serde(default = "default_version")]
    pub version: String,
    /// Nombre legible del escenario (suele venir del Test Plan).
    pub name: String,
    /// Variables definidas por el usuario (User Defined Variables).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
    /// Datasets CSV (CSV Data Set Config).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<Dataset>,
    /// Defaults HTTP (HTTP Request Defaults).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<HttpDefaults>,
    /// Grupos de hilos (Thread Groups).
    pub thread_groups: Vec<ThreadGroup>,
    /// Metadatos de origen/generación.
    #[serde(default)]
    pub metadata: Metadata,
}

impl Scenario {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: default_version(),
            name: name.into(),
            variables: BTreeMap::new(),
            datasets: Vec::new(),
            defaults: None,
            thread_groups: Vec::new(),
            metadata: Metadata::default(),
        }
    }
}

/// Metadatos no semánticos del escenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
pub struct Metadata {
    /// Quién generó el escenario (p.ej. "perfkit-jmx-importer 0.1.0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    /// Ruta del archivo de origen (p.ej. el .jmx importado).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Notas libres para el QA.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Defaults aplicados a todos los samplers HTTP del escenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
pub struct HttpDefaults {
    /// URL base (esquema://host:puerto/ruta) para resolver URLs relativas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Headers por defecto.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
}

/// Dataset CSV (CSV Data Set Config).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dataset {
    pub name: String,
    /// Ruta del archivo CSV (relativa al escenario o absoluta).
    pub path: String,
    /// Delimitador de columnas.
    #[serde(default = "default_delimiter")]
    pub delimiter: char,
    /// Nombres de variable por columna.
    pub variable_names: Vec<String>,
    /// Reciclar al llegar al EOF.
    #[serde(default = "default_true")]
    pub recycle: bool,
    /// La primera línea es encabezado (se ignora como datos).
    #[serde(default)]
    pub first_line_is_header: bool,
}

fn default_delimiter() -> char {
    ','
}
fn default_true() -> bool {
    true
}

/// Grupo de hilos (Thread Group): perfil de carga + pasos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThreadGroup {
    pub name: String,
    pub load: LoadProfile,
    /// Qué hacer ante error de muestra.
    #[serde(default)]
    pub on_error: OnError,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    #[default]
    Continue,
    StopThread,
    StopTest,
}

/// Perfil de carga de un grupo de hilos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LoadProfile {
    /// Número de usuarios virtuales (hilos).
    pub virtual_users: u32,
    /// Periodo de arranque escalonado (segundos).
    #[serde(default)]
    pub ramp_up_secs: u64,
    /// Periodo en régimen estable (segundos). 0 si no se usa.
    #[serde(default)]
    pub hold_secs: u64,
    /// Periodo de descenso (segundos). 0 si no se usa.
    #[serde(default)]
    pub ramp_down_secs: u64,
    /// Iteraciones por VU. None = acotado por `duration_secs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<u64>,
    /// Duración total tope (segundos). None = acotado por `iterations`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
}

impl LoadProfile {
    /// Duración total estimada de la prueba en segundos (para el scheduler).
    pub fn total_secs(&self) -> u64 {
        if let Some(d) = self.duration_secs {
            d.max(self.ramp_up_secs)
        } else {
            self.ramp_up_secs + self.hold_secs + self.ramp_down_secs
        }
    }
}

/// Paso ejecutable dentro de un grupo de hilos o controlador.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Step {
    Http(HttpRequest),
    Transaction(Transaction),
    Loop(LoopController),
    If(IfController),
    While(WhileController),
    /// Throughput Controller (porcentaje de ejecuciones).
    Throughput(ThroughputController),
    /// Interleave Controller (un hijo por pasada, rotatorio).
    Interleave(InterleaveController),
    /// Random Controller (un hijo al azar por pasada).
    Random(RandomController),
    /// Kafka producer sampler (publica un mensaje en un topic).
    Kafka(KafkaRequest),
    Timer(Timer),
}

/// Kafka producer sampler (Fase 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct KafkaRequest {
    pub name: String,
    /// Lista de brokers (host:puerto). Soporta `${var}`.
    pub brokers: Vec<String>,
    pub topic: String,
    /// Clave del mensaje (opcional, soporta `${var}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Payload del mensaje (plantilla con `${var}`).
    pub payload: String,
    /// Partición destino (opcional; por defecto 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<i32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

/// Petición HTTP (HTTP Sampler).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HttpRequest {
    pub name: String,
    pub method: HttpMethod,
    /// URL (puede ser relativa a `defaults.base_url`) y contener `${var}`.
    pub url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timers: Vec<Timer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<Assertion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extractors: Vec<Extractor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }
}

/// Cuerpo de una petición HTTP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    /// Cuerpo crudo (JSON, texto, XML, ...).
    Raw {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
        data: String,
    },
    /// Formulario application/x-www-form-urlencoded.
    Form { fields: BTreeMap<String, String> },
}

/// Controlador de transacción (agrupa pasos como una unidad de reporte).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transaction {
    pub name: String,
    pub steps: Vec<Step>,
}

/// Loop Controller: repite sus pasos `count` veces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LoopController {
    pub name: String,
    pub count: u64,
    pub steps: Vec<Step>,
}

/// If Controller: ejecuta sus pasos si `condition` es verdadera (expresión simple).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IfController {
    pub name: String,
    pub condition: String,
    pub steps: Vec<Step>,
}

/// While Controller: repite mientras `condition` sea verdadera (con tope de seguridad).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WhileController {
    pub name: String,
    pub condition: String,
    pub steps: Vec<Step>,
    /// Tope de iteraciones para evitar bucles infinitos.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u64,
}

fn default_max_iterations() -> u64 {
    10_000
}

/// Throughput Controller: ejecuta sus pasos en un porcentaje de las pasadas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThroughputController {
    pub name: String,
    /// Porcentaje de pasadas en que se ejecutan los hijos (0.0–100.0).
    pub percent: f64,
    pub steps: Vec<Step>,
}

/// Interleave Controller: en cada pasada ejecuta el siguiente hijo (orden rotatorio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InterleaveController {
    pub name: String,
    pub steps: Vec<Step>,
}

/// Random Controller: en cada pasada ejecuta un hijo elegido al azar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RandomController {
    pub name: String,
    pub steps: Vec<Step>,
}

/// Temporizadores (think time / pacing).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "timer", rename_all = "snake_case")]
pub enum Timer {
    /// Constant Timer.
    Constant { delay_ms: u64 },
    /// Uniform Random Timer: delay = base + U(0, range).
    UniformRandom { base_ms: u64, range_ms: u64 },
    /// Gaussian Random Timer: delay = offset + N(0, deviation).
    Gaussian { offset_ms: u64, deviation_ms: u64 },
    /// Constant Throughput Timer (objetivo de muestras por minuto).
    ConstantThroughput { target_per_minute: f64 },
}

/// Aserciones sobre la respuesta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "assert", rename_all = "snake_case")]
pub enum Assertion {
    /// Response Assertion (código de estado).
    StatusCode { codes: Vec<u16> },
    /// Response Assertion (substring en el cuerpo).
    BodyContains {
        substring: String,
        #[serde(default)]
        negate: bool,
    },
    /// Response Assertion (regex en el cuerpo).
    BodyMatches {
        pattern: String,
        #[serde(default)]
        negate: bool,
    },
    /// JSON Assertion (JSONPath).
    JsonPath {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        equals: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exists: Option<bool>,
    },
    /// Duration Assertion.
    DurationBelowMs { max_ms: u64 },
    /// Size Assertion.
    SizeBelowBytes { max_bytes: u64 },
}

/// Extractores de variables desde la respuesta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "extract", rename_all = "snake_case")]
pub enum Extractor {
    /// Regular Expression Extractor.
    Regex {
        var: String,
        pattern: String,
        #[serde(default = "default_group")]
        group: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
    /// JSON Extractor (JSONPath).
    JsonPath {
        var: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
    /// Boundary Extractor (texto entre `left` y `right`).
    Boundary {
        var: String,
        left: String,
        right: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
}

fn default_group() -> usize {
    1
}
