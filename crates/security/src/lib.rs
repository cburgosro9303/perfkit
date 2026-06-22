//! `security` — utilidades de seguridad para el MVP (ver ADR-006).
//!
//! Alcance MVP: (1) cargar variables/secretos desde el entorno sin escribirlos en el
//! escenario, y (2) redactar secretos comunes de logs/reportes. La política completa
//! (firma de plugins, permisos WASM, IA gobernada) es trabajo de fases posteriores.

use std::collections::BTreeMap;

/// Claves que disparan redacción (comparación case-insensitive por substring).
const SENSITIVE: &[&str] = &[
    "authorization",
    "bearer",
    "password",
    "passwd",
    "token",
    "api-key",
    "apikey",
    "x-api-key",
    "secret",
    "set-cookie",
    "cookie",
];

/// Carga variables desde el entorno con un prefijo dado, devolviéndolas sin el prefijo.
///
/// Ej: `PERFKIT_VAR_TOKEN=abc` con `prefix = "PERFKIT_VAR_"` → `{"TOKEN": "abc"}`.
/// Permite inyectar secretos en el escenario sin versionarlos en el YAML.
pub fn vars_from_env(prefix: &str) -> BTreeMap<String, String> {
    std::env::vars()
        .filter_map(|(k, v)| k.strip_prefix(prefix).map(|name| (name.to_string(), v)))
        .collect()
}

/// Redacta secretos comunes de un texto multilínea (headers, query params, etc.).
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let ends_nl = text.ends_with('\n');
    for line in text.lines() {
        out.push_str(&redact_line(line));
        out.push('\n');
    }
    if !ends_nl {
        out.pop();
    }
    out
}

fn redact_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if SENSITIVE.iter().any(|k| lower.contains(k))
        && let Some(pos) = line.find([':', '='])
    {
        let head = &line[..=pos];
        return format!("{head} ***REDACTED***");
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_values() {
        let input = "Authorization: Bearer abc.def.ghi\nAccept: application/json\npassword=hunter2";
        let out = redact(input);
        assert!(out.contains("Authorization: ***REDACTED***"));
        assert!(out.contains("Accept: application/json"));
        assert!(out.contains("password= ***REDACTED***"));
        assert!(!out.contains("hunter2"));
        assert!(!out.contains("abc.def.ghi"));
    }
}
