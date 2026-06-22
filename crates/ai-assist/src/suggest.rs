//! Sugerencias **revisables** generadas localmente (heurísticas, sin red).
//!
//! Toda sugerencia lleva `requires_confirmation = true`: el usuario debe revisarla
//! y aplicarla manualmente. Nunca se aplica nada de forma automática (§6.14).

use crate::{round1, thresholds_json};
use regex::Regex;
use scenario_ir::{Body, HttpRequest, Scenario, Step};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::OnceLock;

/// Categoría de una sugerencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    /// Propuesta de umbrales para el quality gate.
    Threshold,
    /// Propuesta de correlación (extraer un valor dinámico).
    Correlation,
    /// Propuesta de migración de un script (Groovy/JSR223) al IR.
    ScriptMigration,
    /// Explicación en lenguaje natural de un resultado.
    ResultExplanation,
}

/// Sugerencia revisable. **Nunca** se aplica automáticamente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// Categoría.
    pub kind: SuggestionKind,
    /// Título corto.
    pub title: String,
    /// Detalle legible (lenguaje natural).
    pub detail: String,
    /// Propuesta estructurada (forma dependiente del `kind`).
    pub proposal: serde_json::Value,
    /// Siempre `true`: requiere confirmación humana antes de aplicarse.
    pub requires_confirmation: bool,
}

impl Suggestion {
    fn new(
        kind: SuggestionKind,
        title: impl Into<String>,
        detail: impl Into<String>,
        proposal: serde_json::Value,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            detail: detail.into(),
            proposal,
            // Invariante de gobernanza: nunca auto-aplicable.
            requires_confirmation: true,
        }
    }
}

// --------------------------------------------------------------------------
// Umbrales
// --------------------------------------------------------------------------

/// Propone umbrales para el quality gate a partir de un [`RunSummary`].
///
/// La propuesta deja margen sobre lo observado (p95×1.2, p99×1.25, error_rate×1.5
/// con piso de 0.01, throughput×0.8) y tiene la forma de `reports::Thresholds`.
pub fn suggest_thresholds(summary: &metrics::RunSummary) -> Suggestion {
    let o = &summary.overall;
    let max_p95_ms = o.p95_ms * 1.2;
    let max_p99_ms = o.p99_ms * 1.25;
    let max_error_rate = (o.error_rate * 1.5).max(0.01);
    let min_throughput_per_sec = o.throughput_per_sec * 0.8;

    let proposal = thresholds_json(
        max_p95_ms,
        max_p99_ms,
        max_error_rate,
        min_throughput_per_sec,
    );

    let detail = format!(
        "A partir de la línea base observada (p95 {:.1}ms, p99 {:.1}ms, error_rate \
         {:.2}%, throughput {:.1}/s) se proponen umbrales con margen: \
         p95 ≤ {:.1}ms, p99 ≤ {:.1}ms, error_rate ≤ {:.2}%, throughput ≥ {:.1}/s. \
         Revisa estos valores antes de aplicarlos al gate.",
        o.p95_ms,
        o.p99_ms,
        o.error_rate * 100.0,
        o.throughput_per_sec,
        round1(max_p95_ms),
        round1(max_p99_ms),
        max_error_rate * 100.0,
        round1(min_throughput_per_sec),
    );

    Suggestion::new(
        SuggestionKind::Threshold,
        "Umbrales sugeridos para el quality gate",
        detail,
        proposal,
    )
}

// --------------------------------------------------------------------------
// Correlaciones
// --------------------------------------------------------------------------

fn looks_like_var(value: &str) -> bool {
    // Si ya usa una variable ${...}, no hay nada que correlacionar.
    value.contains("${")
}

fn token_regexes() -> &'static (Regex, Regex, Regex) {
    static R: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    R.get_or_init(|| {
        (
            // Esquema de autorización con token literal.
            Regex::new(r"(?i)^\s*(?:bearer|basic|token)\s+(\S+)").unwrap(),
            // Tira larga "densa" que parece token/hash/id opaco.
            Regex::new(r"^[A-Za-z0-9._\-]{16,}$").unwrap(),
            // UUID.
            Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
                .unwrap(),
        )
    })
}

/// ¿El valor parece un token/id/secreto literal (no una `${var}`)?
fn looks_hardcoded_secret(value: &str) -> Option<&'static str> {
    if looks_like_var(value) {
        return None;
    }
    let (scheme, dense, uuid) = token_regexes();
    let trimmed = value.trim();
    if scheme.is_match(trimmed) {
        return Some("auth_scheme_token");
    }
    if uuid.is_match(trimmed) {
        return Some("uuid");
    }
    if dense.is_match(trimmed) {
        return Some("opaque_token");
    }
    None
}

/// Nombre de variable sugerido a partir del nombre del header/campo.
fn suggest_var_name(source: &str) -> String {
    let cleaned: String = source
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "extracted_value".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Recorre el escenario buscando valores literales que parezcan tokens/ids
/// hardcodeados en headers o cuerpo, y sugiere extraerlos a variables.
pub fn suggest_correlations(scenario: &Scenario) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for tg in &scenario.thread_groups {
        for step in &tg.steps {
            walk_step(step, &mut out);
        }
    }
    out
}

fn walk_step(step: &Step, out: &mut Vec<Suggestion>) {
    match step {
        Step::Http(req) => scan_http(req, out),
        Step::Transaction(t) => t.steps.iter().for_each(|s| walk_step(s, out)),
        Step::Loop(c) => c.steps.iter().for_each(|s| walk_step(s, out)),
        Step::If(c) => c.steps.iter().for_each(|s| walk_step(s, out)),
        Step::While(c) => c.steps.iter().for_each(|s| walk_step(s, out)),
        Step::Throughput(c) => c.steps.iter().for_each(|s| walk_step(s, out)),
        Step::Interleave(c) => c.steps.iter().for_each(|s| walk_step(s, out)),
        Step::Random(c) => c.steps.iter().for_each(|s| walk_step(s, out)),
        Step::Kafka(_) | Step::Timer(_) => {}
    }
}

fn scan_http(req: &HttpRequest, out: &mut Vec<Suggestion>) {
    // Headers
    for (name, value) in &req.headers {
        if let Some(reason) = looks_hardcoded_secret(value) {
            let var = suggest_var_name(name);
            let is_auth =
                name.eq_ignore_ascii_case("authorization") || reason == "auth_scheme_token";
            let detail = format!(
                "El header '{name}' de la petición '{}' contiene un valor literal que \
                 parece {} ({reason}). Considera extraerlo de una respuesta previa a la \
                 variable ${{{var}}} y referenciarlo con un extractor, en lugar de \
                 dejarlo hardcodeado.",
                req.name,
                if is_auth {
                    "un token de autorización"
                } else {
                    "un identificador/secreto"
                },
            );
            // Propuesta: un extractor (regex/json) + el reemplazo del header por ${var}.
            let proposal = json!({
                "location": "header",
                "request": req.name,
                "header": name,
                "current_value_redacted": crate::redact(value, &[]),
                "suggested_variable": var,
                "replace_with": format!("${{{var}}}"),
                "extractor": {
                    "extract": "json_path",
                    "var": var,
                    "path": "$..token",
                    "note": "Ajusta el path/regex al cuerpo real de la respuesta que provee este valor."
                },
                "alternative_extractor": {
                    "extract": "regex",
                    "var": var,
                    "pattern": "\"token\"\\s*:\\s*\"([^\"]+)\"",
                    "group": 1
                }
            });
            out.push(Suggestion::new(
                SuggestionKind::Correlation,
                format!("Correlacionar header '{name}'"),
                detail,
                proposal,
            ));
        }
    }

    // Query params
    for (name, value) in &req.query {
        if let Some(reason) = looks_hardcoded_secret(value) {
            let var = suggest_var_name(name);
            let detail = format!(
                "El parámetro de query '{name}' de '{}' tiene un valor literal que parece \
                 {reason}. Considera extraerlo a ${{{var}}}.",
                req.name
            );
            let proposal = json!({
                "location": "query",
                "request": req.name,
                "param": name,
                "current_value_redacted": crate::redact(value, &[]),
                "suggested_variable": var,
                "replace_with": format!("${{{var}}}"),
                "extractor": {
                    "extract": "regex",
                    "var": var,
                    "pattern": format!("{}=([^&\\s]+)", regex::escape(name)),
                    "group": 1
                }
            });
            out.push(Suggestion::new(
                SuggestionKind::Correlation,
                format!("Correlacionar query '{name}'"),
                detail,
                proposal,
            ));
        }
    }

    // Cuerpo: buscamos pares "clave": "valor-opaco".
    if let Some(Body::Raw { data, .. }) = &req.body {
        scan_body(req, data, out);
    }
    if let Some(Body::Form { fields }) = &req.body {
        for (name, value) in fields {
            if let Some(reason) = looks_hardcoded_secret(value) {
                let var = suggest_var_name(name);
                let detail = format!(
                    "El campo de formulario '{name}' de '{}' tiene un valor literal que \
                     parece {reason}. Considera extraerlo a ${{{var}}}.",
                    req.name
                );
                let proposal = json!({
                    "location": "body_form",
                    "request": req.name,
                    "field": name,
                    "current_value_redacted": crate::redact(value, &[]),
                    "suggested_variable": var,
                    "replace_with": format!("${{{var}}}"),
                });
                out.push(Suggestion::new(
                    SuggestionKind::Correlation,
                    format!("Correlacionar campo '{name}'"),
                    detail,
                    proposal,
                ));
            }
        }
    }
}

fn body_kv_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // "clave": "valor"  →  capturamos clave (1) y valor (2).
    R.get_or_init(|| Regex::new(r#""([A-Za-z0-9_\-]+)"\s*:\s*"([^"]+)""#).unwrap())
}

fn scan_body(req: &HttpRequest, data: &str, out: &mut Vec<Suggestion>) {
    if looks_like_var(data) && !body_kv_regex().is_match(data) {
        // Ya parametrizado y sin pares literales evidentes.
    }
    for cap in body_kv_regex().captures_iter(data) {
        let key = &cap[1];
        let value = &cap[2];
        let key_is_sensitive = matches!(
            key.to_ascii_lowercase().as_str(),
            "token"
                | "access_token"
                | "id_token"
                | "session"
                | "sessionid"
                | "session_id"
                | "csrf"
                | "csrf_token"
                | "apikey"
                | "api_key"
                | "auth"
        );
        let value_opaque = looks_hardcoded_secret(value).is_some();
        if !key_is_sensitive && !value_opaque {
            continue;
        }
        if looks_like_var(value) {
            continue;
        }
        let var = suggest_var_name(key);
        let detail = format!(
            "El cuerpo de '{}' incluye '\"{key}\": \"…\"' con un valor literal que parece \
             dinámico. Considera extraerlo de una respuesta previa a ${{{var}}} y \
             referenciarlo aquí, en vez de hardcodearlo.",
            req.name
        );
        let proposal = json!({
            "location": "body_json",
            "request": req.name,
            "json_key": key,
            "current_value_redacted": crate::redact(value, &[]),
            "suggested_variable": var,
            "replace_with": format!("${{{var}}}"),
            "extractor": {
                "extract": "json_path",
                "var": var,
                "path": format!("$..{key}"),
            }
        });
        out.push(Suggestion::new(
            SuggestionKind::Correlation,
            format!("Correlacionar valor del cuerpo '{key}'"),
            detail,
            proposal,
        ));
    }
}

// --------------------------------------------------------------------------
// Migración de Groovy/JSR223
// --------------------------------------------------------------------------

/// Mapea heurísticamente un script Groovy/JSR223 a elementos del IR.
///
/// Reglas planas (sin ejecutar el script):
/// - `vars.put(...)` → extractor / variable.
/// - `Thread.sleep(...)` / `sleep(...)` → Timer (think time).
/// - cripto (`MessageDigest`, `Mac`, `Base64`, `Cipher`) → mantener / plugin.
/// - HTTP (`HttpURLConnection`, `HttpClient`, `new URL(...).openConnection`) → sampler.
/// - logging (`log.info`, `println`) → omitir.
pub fn analyze_groovy(code: &str) -> Suggestion {
    let mut mappings: Vec<serde_json::Value> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    let push = |mappings: &mut Vec<serde_json::Value>,
                pattern: &str,
                ir: &str,
                action: &str,
                hint: &str| {
        mappings.push(json!({
            "groovy": pattern,
            "ir_target": ir,
            "action": action,
            "hint": hint,
        }));
    };

    if code.contains("vars.put") || code.contains("vars.putObject") {
        push(
            &mut mappings,
            "vars.put(...)",
            "extractor/variable",
            "convert",
            "Reemplaza la asignación de variable por un Extractor (json_path/regex/boundary) o una variable del escenario.",
        );
    }
    if code.contains("vars.get") {
        push(
            &mut mappings,
            "vars.get(...)",
            "variable_reference",
            "convert",
            "Las lecturas de variable se vuelven referencias ${var} en el IR.",
        );
    }
    if code.contains("Thread.sleep") || regex_sleep().is_match(code) {
        push(
            &mut mappings,
            "Thread.sleep(ms) / sleep(ms)",
            "timer.constant",
            "convert",
            "Sustituye la pausa por un Timer (Constant/UniformRandom) como think time.",
        );
    }
    if code.contains("MessageDigest")
        || code.contains("Mac.getInstance")
        || code.contains("Cipher")
        || code.contains("Base64")
        || code.contains("Signature.getInstance")
    {
        push(
            &mut mappings,
            "MessageDigest/Mac/Cipher/Base64/Signature",
            "keep_or_plugin",
            "keep",
            "La lógica criptográfica no tiene equivalente declarativo: mantenla como paso de script o muévela a un plugin dedicado.",
        );
    }
    if code.contains("HttpURLConnection")
        || code.contains("HttpClient")
        || code.contains("openConnection")
        || code.contains(".toURL()")
        || regex_new_url().is_match(code)
    {
        push(
            &mut mappings,
            "new URL(...).openConnection / HttpClient",
            "http_sampler",
            "convert",
            "Las peticiones HTTP hechas a mano se modelan como un HTTP Sampler del IR.",
        );
    }
    if code.contains("log.info")
        || code.contains("log.warn")
        || code.contains("log.error")
        || code.contains("log.debug")
        || code.contains("println")
        || code.contains("System.out")
    {
        push(
            &mut mappings,
            "log.* / println / System.out",
            "(omit)",
            "omit",
            "El logging no se traduce al IR; se omite (usa el reporting integrado).",
        );
    }

    if mappings.is_empty() {
        notes.push(
            "No se reconocieron construcciones mapeables. Revisa el script manualmente; \
             puede requerir mantenerse como paso de script o trasladarse a un plugin."
                .to_string(),
        );
    } else {
        notes.push(
            "Mapeo heurístico: revisa cada elemento antes de aplicarlo. No se ejecutó el script."
                .to_string(),
        );
    }

    let proposal = json!({
        "language": "groovy/jsr223",
        "mappings": mappings,
        "notes": notes,
    });

    Suggestion::new(
        SuggestionKind::ScriptMigration,
        "Mapeo sugerido de script Groovy/JSR223 al IR",
        "Se identificaron construcciones del script y su equivalente en el IR. \
         Cada conversión debe revisarse y aplicarse manualmente.",
        proposal,
    )
}

fn regex_sleep() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\bsleep\s*\(").unwrap())
}

fn regex_new_url() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\bnew\s+URL\s*\(").unwrap())
}

// --------------------------------------------------------------------------
// Explicación de resultados
// --------------------------------------------------------------------------

/// Genera una narrativa en español llano del resultado de un run.
///
/// Menciona: total de requests, throughput, error rate (marcado si >1%), p95/p99,
/// la etiqueta más lenta y el mayor bucket de errores (si lo hay).
pub fn explain_results(summary: &metrics::RunSummary) -> Suggestion {
    let o = &summary.overall;
    let error_pct = o.error_rate * 100.0;
    let error_flag = if o.error_rate > 0.01 {
        " (POR ENCIMA del 1% — revisar)"
    } else {
        " (dentro de lo aceptable)"
    };

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "Se ejecutaron {} requests en {:.1}s con {} usuarios virtuales.",
        o.count, summary.duration_secs, summary.config.virtual_users
    ));
    lines.push(format!(
        "Throughput: {:.1} req/s. Tasa de error: {:.2}%{}.",
        o.throughput_per_sec, error_pct, error_flag
    ));
    lines.push(format!(
        "Latencia: p95 {:.1}ms, p99 {:.1}ms (media {:.1}ms, máx {:.1}ms).",
        o.p95_ms, o.p99_ms, o.mean_ms, o.max_ms
    ));

    // Etiqueta más lenta (por p95). `overall` se llama "ALL"; lo excluimos.
    let slowest = summary
        .labels
        .iter()
        .filter(|l| l.label != "ALL")
        .max_by(|a, b| {
            a.p95_ms
                .partial_cmp(&b.p95_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    if let Some(s) = slowest {
        lines.push(format!(
            "La operación más lenta es '{}' con p95 {:.1}ms ({} muestras).",
            s.label, s.p95_ms, s.count
        ));
    }

    // Mayor bucket de errores.
    let top_error = summary.errors.iter().max_by_key(|e| e.count);
    if let Some(e) = top_error {
        lines.push(format!(
            "El error más frecuente es \"{}\" ({} veces).",
            e.message, e.count
        ));
    } else {
        lines.push("No se registraron errores con mensaje.".to_string());
    }

    let detail = lines.join(" ");

    let proposal = json!({
        "total_requests": o.count,
        "throughput_per_sec": round1(o.throughput_per_sec),
        "error_rate": (o.error_rate * 10000.0).round() / 10000.0,
        "error_rate_exceeds_1pct": o.error_rate > 0.01,
        "p95_ms": round1(o.p95_ms),
        "p99_ms": round1(o.p99_ms),
        "slowest_label": slowest.map(|s| json!({ "label": s.label, "p95_ms": round1(s.p95_ms) })),
        "top_error": top_error.map(|e| json!({ "message": e.message, "count": e.count })),
    });

    Suggestion::new(
        SuggestionKind::ResultExplanation,
        "Explicación del resultado",
        detail,
        proposal,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics::{ErrorBucket, LabelStats, RunConfig, RunSummary, SampleKind};
    use scenario_ir::{HttpMethod, HttpRequest, LoadProfile, Scenario, Step, ThreadGroup};
    use std::collections::BTreeMap;

    fn label(name: &str, p95: f64, p99: f64, count: u64) -> LabelStats {
        LabelStats {
            label: name.to_string(),
            kind: SampleKind::Http,
            count,
            errors: 0,
            error_rate: 0.0,
            throughput_per_sec: 50.0,
            min_ms: 1.0,
            mean_ms: p95 / 2.0,
            max_ms: p99 * 1.5,
            p50_ms: p95 / 2.0,
            p90_ms: p95 * 0.9,
            p95_ms: p95,
            p99_ms: p99,
            p999_ms: p99 * 1.2,
            bytes_total: 1024,
            ..Default::default()
        }
    }

    fn sample_summary() -> RunSummary {
        let mut overall = label("ALL", 200.0, 350.0, 1000);
        overall.errors = 25;
        overall.error_rate = 0.025; // 2.5% → debe marcarse
        overall.throughput_per_sec = 120.0;
        RunSummary {
            run_id: "run-1".into(),
            scenario_name: "demo".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            duration_secs: 10.0,
            config: RunConfig {
                virtual_users: 20,
                thread_groups: 1,
            },
            overall,
            labels: vec![
                label("GET /fast", 50.0, 80.0, 600),
                label("GET /slow", 900.0, 1500.0, 400),
            ],
            timeseries: vec![],
            errors: vec![
                ErrorBucket {
                    message: "500 Internal Server Error".into(),
                    count: 20,
                },
                ErrorBucket {
                    message: "timeout".into(),
                    count: 5,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn thresholds_p95_proposal_exceeds_current() {
        let s = sample_summary();
        let sug = suggest_thresholds(&s);
        let proposed_p95 = sug.proposal["max_p95_ms"].as_f64().unwrap();
        assert!(
            proposed_p95 > s.overall.p95_ms,
            "p95 propuesto {proposed_p95} debe superar el actual {}",
            s.overall.p95_ms
        );
        // Forma compatible con reports::Thresholds.
        assert!(sug.proposal.get("max_p99_ms").is_some());
        assert!(sug.proposal.get("max_error_rate").is_some());
        assert!(sug.proposal.get("min_throughput_per_sec").is_some());
        assert!(sug.requires_confirmation);
    }

    #[test]
    fn thresholds_error_rate_has_floor() {
        // Con error_rate 0, el piso 0.01 debe aplicarse.
        let mut s = sample_summary();
        s.overall.error_rate = 0.0;
        let sug = suggest_thresholds(&s);
        let er = sug.proposal["max_error_rate"].as_f64().unwrap();
        assert!((er - 0.01).abs() < 1e-9, "piso de error_rate: {er}");
    }

    #[test]
    fn explain_mentions_throughput_and_error_rate() {
        let s = sample_summary();
        let sug = explain_results(&s);
        let d = sug.detail.to_lowercase();
        assert!(d.contains("throughput"), "falta throughput: {}", sug.detail);
        assert!(
            d.contains("error") || sug.detail.contains("%"),
            "falta error rate: {}",
            sug.detail
        );
        // 2.5% supera el umbral del 1% → debe marcarse.
        assert!(sug.proposal["error_rate_exceeds_1pct"].as_bool().unwrap());
        // La operación más lenta es GET /slow.
        assert!(sug.detail.contains("GET /slow"), "slowest: {}", sug.detail);
        assert_eq!(sug.kind, SuggestionKind::ResultExplanation);
    }

    fn scenario_with_auth_token() -> Scenario {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer abc123DEF456ghi789JKL".to_string(),
        );
        let req = HttpRequest {
            name: "GET protected".into(),
            method: HttpMethod::Get,
            url: "/api/protected".into(),
            headers,
            query: BTreeMap::new(),
            body: None,
            follow_redirects: None,
            timeout_ms: None,
            timers: vec![],
            assertions: vec![],
            extractors: vec![],
        };
        let mut s = Scenario::new("auth-demo");
        s.thread_groups.push(ThreadGroup {
            name: "tg".into(),
            load: LoadProfile {
                virtual_users: 1,
                ramp_up_secs: 0,
                hold_secs: 0,
                ramp_down_secs: 0,
                iterations: Some(1),
                duration_secs: None,
            },
            on_error: Default::default(),
            steps: vec![Step::Http(req)],
        });
        s
    }

    #[test]
    fn correlations_flag_hardcoded_authorization() {
        let s = scenario_with_auth_token();
        let sugs = suggest_correlations(&s);
        assert!(
            !sugs.is_empty(),
            "debe detectar el token de autorización hardcodeado"
        );
        let auth = sugs
            .iter()
            .find(|x| x.kind == SuggestionKind::Correlation)
            .expect("una sugerencia de correlación");
        assert_eq!(auth.proposal["location"], "header");
        assert_eq!(auth.proposal["header"], "Authorization");
        // El valor en la propuesta debe ir redactado.
        let redacted = auth.proposal["current_value_redacted"].as_str().unwrap();
        assert!(
            !redacted.contains("abc123DEF456ghi789JKL"),
            "el token no debe filtrarse en la propuesta: {redacted}"
        );
        assert!(auth.requires_confirmation);
    }

    #[test]
    fn correlations_ignore_already_parameterized() {
        let mut s = scenario_with_auth_token();
        // Reemplazar por una variable: no debe sugerir nada.
        if let Step::Http(req) = &mut s.thread_groups[0].steps[0] {
            req.headers
                .insert("Authorization".into(), "Bearer ${token}".into());
        }
        let sugs = suggest_correlations(&s);
        assert!(
            sugs.iter().all(|x| x.proposal["header"] != "Authorization"),
            "no debe sugerir sobre un header ya parametrizado"
        );
    }

    #[test]
    fn groovy_maps_known_constructs() {
        let code = r#"
            vars.put("id", resp);
            Thread.sleep(500);
            def md = MessageDigest.getInstance("SHA-256");
            log.info("done");
        "#;
        let sug = analyze_groovy(code);
        assert_eq!(sug.kind, SuggestionKind::ScriptMigration);
        let maps = sug.proposal["mappings"].as_array().unwrap();
        let targets: Vec<&str> = maps
            .iter()
            .map(|m| m["ir_target"].as_str().unwrap())
            .collect();
        assert!(targets.iter().any(|t| t.contains("extractor")));
        assert!(targets.iter().any(|t| t.contains("timer")));
        assert!(targets.iter().any(|t| t.contains("keep")));
        assert!(targets.contains(&"(omit)"));
        assert!(sug.requires_confirmation);
    }

    #[test]
    fn groovy_unknown_yields_note() {
        let sug = analyze_groovy("def x = 1 + 2");
        let maps = sug.proposal["mappings"].as_array().unwrap();
        assert!(maps.is_empty());
        let notes = sug.proposal["notes"].as_array().unwrap();
        assert!(!notes.is_empty());
    }
}
