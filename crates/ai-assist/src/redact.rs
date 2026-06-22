//! Redacción de secretos (gobernanza, §6.14).
//!
//! El objetivo es que *ningún* secreto salga del proceso sin que el usuario lo
//! permita. [`redact`] enmascara a `***` los patrones sensibles más comunes; lo que
//! aparezca en la `allowlist` se restaura intacto tras el enmascarado.

use regex::Regex;
use std::sync::OnceLock;

const MASK: &str = "***";

/// Conjunto de expresiones regulares compiladas una sola vez.
struct Patterns {
    /// `Authorization:` / `Bearer <token>` (cabecera completa o solo el esquema).
    auth_header: Regex,
    /// `Authorization: Basic xxxx` no encaja arriba si el token es corto; cubierto aquí.
    bearer_inline: Regex,
    /// `password=...`, `token=...`, `secret=...`, `apikey=...`, `api_key=...`.
    kv_secret: Regex,
    /// Direcciones de correo.
    email: Regex,
    /// JWT (tres segmentos base64url separados por puntos).
    jwt: Regex,
    /// Claves tipo AWS (AKIA/ASIA + 16 alfanum).
    aws_key: Regex,
    /// Tiras largas hex/base64 (≥20 chars) — tokens, hashes, claves.
    long_blob: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // "Authorization: <lo-que-sea-hasta-fin-de-línea>" — el valor (token, esquema
        // + token, etc.) se consume entero hasta el salto de línea.
        auth_header: Regex::new(r"(?i)\bauthorization\s*[:=]\s*[^\r\n]+").unwrap(),
        // "Bearer <token>" o "Basic <token>" sueltos.
        bearer_inline: Regex::new(r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]+").unwrap(),
        // clave=valor para secretos comunes. Captura la clave (1) y el valor (2),
        // donde el valor va opcionalmente entrecomillado. `regex` no soporta
        // backreferences, así que enumeramos las dos formas (comillas / sin comillas).
        kv_secret: Regex::new(
            r#"(?i)\b(password|passwd|pwd|token|secret|api[_-]?key|access[_-]?key|client[_-]?secret)\s*[:=]\s*(?:"([^"]+)"|([^"\s,&]+))"#,
        )
        .unwrap(),
        email: Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap(),
        // JWT: header.payload.signature, todos base64url, payload razonablemente largo.
        jwt: Regex::new(r"\beyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\b").unwrap(),
        aws_key: Regex::new(r"\b(?:AKIA|ASIA|AGPA|AIDA|AROA)[A-Z0-9]{16}\b").unwrap(),
        // Cadena larga y "densa" (hex o base64) de 20+ chars.
        long_blob: Regex::new(r"\b[A-Za-z0-9+/=_\-]{20,}\b").unwrap(),
    })
}

/// Enmascara secretos en `text`, dejando intacto lo que esté en `allowlist`.
///
/// El orden importa: primero se enmascaran los patrones más específicos (auth, kv,
/// jwt, aws), luego email, y por último los blobs largos genéricos. Tras enmascarar,
/// cualquier substring exacto presente en la allowlist se restaura.
pub fn redact(text: &str, allowlist: &[String]) -> String {
    let p = patterns();
    // Aplicamos los patrones en orden de especificidad (más específico primero).
    let mut out = p.auth_header.replace_all(text, MASK).into_owned();
    out = p.bearer_inline.replace_all(&out, MASK).into_owned();
    out = p
        .kv_secret
        .replace_all(&out, |c: &regex::Captures| {
            // Conservar "clave=" y enmascarar solo el valor → claridad para el usuario.
            format!("{}={}", &c[1], MASK)
        })
        .into_owned();
    out = p.jwt.replace_all(&out, MASK).into_owned();
    out = p.aws_key.replace_all(&out, MASK).into_owned();
    out = p.email.replace_all(&out, MASK).into_owned();
    out = p.long_blob.replace_all(&out, MASK).into_owned();

    // Restaurar las excepciones explícitas de la allowlist.
    //
    // Si un valor de la allowlist fue enmascarado, no podemos recuperarlo del texto
    // ya redactado; por eso el contrato es: "lo que esté en la allowlist se deja
    // intacto". Implementamos esto re-protegiendo esos substrings en el ORIGINAL:
    // si aparecen tal cual en el texto original, restauramos esa porción.
    if allowlist.is_empty() {
        return out;
    }
    restore_allowlisted(text, out, allowlist)
}

/// Restaura porciones permitidas. Estrategia simple y robusta: para cada entrada de
/// la allowlist presente en el texto original, reconstruimos el resultado redactando
/// todo *salvo* esa porción. Como las entradas son substrings literales, basta con
/// volver a aplicar la redacción sobre segmentos separados por las porciones
/// permitidas y reensamblar con dichas porciones intactas.
fn restore_allowlisted(original: &str, redacted_fallback: String, allowlist: &[String]) -> String {
    // Recolectar ocurrencias (inicio, fin) de cualquier entrada de la allowlist.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for entry in allowlist {
        if entry.is_empty() {
            continue;
        }
        let mut from = 0;
        while let Some(rel) = original[from..].find(entry.as_str()) {
            let start = from + rel;
            let end = start + entry.len();
            spans.push((start, end));
            from = end;
        }
    }
    if spans.is_empty() {
        return redacted_fallback;
    }
    // Ordenar y fusionar solapamientos.
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        if let Some(last) = merged.last_mut()
            && s <= last.1
        {
            last.1 = last.1.max(e);
            continue;
        }
        merged.push((s, e));
    }
    // Reconstruir: redactar segmentos fuera de los spans permitidos; dejar intactos
    // los segmentos permitidos.
    let mut result = String::with_capacity(original.len());
    let mut cursor = 0;
    for (s, e) in merged {
        if s > cursor {
            result.push_str(&redact_segment(&original[cursor..s]));
        }
        result.push_str(&original[s..e]); // porción permitida, intacta
        cursor = e;
    }
    if cursor < original.len() {
        result.push_str(&redact_segment(&original[cursor..]));
    }
    result
}

/// Redacta un segmento sin re-procesar la allowlist (evita recursión).
fn redact_segment(seg: &str) -> String {
    let p = patterns();
    let mut out = p.auth_header.replace_all(seg, MASK).into_owned();
    out = p.bearer_inline.replace_all(&out, MASK).into_owned();
    out = p
        .kv_secret
        .replace_all(&out, |c: &regex::Captures| format!("{}={}", &c[1], MASK))
        .into_owned();
    out = p.jwt.replace_all(&out, MASK).into_owned();
    out = p.aws_key.replace_all(&out, MASK).into_owned();
    out = p.email.replace_all(&out, MASK).into_owned();
    out = p.long_blob.replace_all(&out, MASK).into_owned();
    out
}

/// Redactor reutilizable con una allowlist fija.
///
/// Equivalente a llamar a [`redact`] con la misma allowlist, pero cómodo cuando se
/// redacta repetidamente con la misma configuración.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    allowlist: Vec<String>,
}

impl Redactor {
    /// Crea un redactor con la allowlist dada (campos a dejar intactos).
    pub fn new(allowlist: Vec<String>) -> Self {
        Self { allowlist }
    }

    /// Redacta `text` aplicando la allowlist de este redactor.
    pub fn redact(&self, text: &str) -> String {
        redact(text, &self.allowlist)
    }

    /// Acceso a la allowlist configurada.
    pub fn allowlist(&self) -> &[String] {
        &self.allowlist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_bearer_email_and_password() {
        let input = "Authorization: Bearer abcDEF123456 user=jane@example.com password=secret";
        let out = redact(input, &[]);
        assert!(!out.contains("abcDEF123456"), "bearer token leaked: {out}");
        assert!(!out.contains("jane@example.com"), "email leaked: {out}");
        assert!(!out.contains("secret"), "password leaked: {out}");
        assert!(out.contains(MASK));
    }

    #[test]
    fn keeps_allowlisted_field() {
        // El email de soporte está permitido y debe conservarse intacto, mientras
        // el password sigue redactado.
        let input = "contact support@perfkit.io password=hunter2value";
        let allow = vec!["support@perfkit.io".to_string()];
        let out = redact(input, &allow);
        assert!(
            out.contains("support@perfkit.io"),
            "allowlisted dropped: {out}"
        );
        assert!(!out.contains("hunter2value"), "password leaked: {out}");
    }

    #[test]
    fn masks_jwt_and_aws_and_long_blob() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N";
        let input =
            format!("tok={jwt} key=AKIAIOSFODNN7EXAMPLE hash=0123456789abcdef0123456789abcdef");
        let out = redact(&input, &[]);
        assert!(!out.contains(jwt), "jwt leaked: {out}");
        assert!(
            !out.contains("AKIAIOSFODNN7EXAMPLE"),
            "aws key leaked: {out}"
        );
        assert!(
            !out.contains("0123456789abcdef0123456789abcdef"),
            "long blob leaked: {out}"
        );
    }

    #[test]
    fn redactor_struct_matches_function() {
        let r = Redactor::new(vec![]);
        let input = "password=topsecret";
        assert_eq!(r.redact(input), redact(input, &[]));
        assert!(!r.redact(input).contains("topsecret"));
    }
}
