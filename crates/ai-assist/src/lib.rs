//! `ai-assist` — IA **gobernada** para perfkit (Fase 9, ver §2.11 / §6.14 del plan).
//!
//! Principio rector, **no negociable**:
//!
//! - **SaaS apagado por defecto.** El modo por defecto es [`AiMode::Local`], que es
//!   100% heurístico y *no realiza ninguna llamada de red*.
//! - **Ningún dato sale por defecto.** Nada se envía a un proveedor externo salvo que
//!   el usuario lo habilite explícitamente (BYOK, o SaaS con `allow_saas = true`).
//! - **El usuario ve exactamente qué se enviaría.** [`preview_payload`] devuelve el
//!   texto *exacto* (redactado si procede) que saldría, antes de enviar nada.
//! - **Toda sugerencia es revisable y NUNCA se aplica automáticamente.** Cada
//!   [`Suggestion`] lleva `requires_confirmation = true`.
//!
//! Este crate **no contiene ninguna llamada de red**. BYOK/SaaS son responsabilidad
//! del *caller*, que implementa el trait [`AiProvider`]. El único proveedor que se
//! incluye es [`LocalHeuristicProvider`], puramente local y basado en reglas.

use serde::{Deserialize, Serialize};
use serde_json::json;

mod redact;
mod suggest;

pub use redact::{Redactor, redact};
pub use suggest::{
    Suggestion, SuggestionKind, analyze_groovy, explain_results, suggest_correlations,
    suggest_thresholds,
};

/// Modo de operación de la IA. El valor por defecto es [`AiMode::Local`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiMode {
    /// 100% local/heurístico. No sale ningún dato. **Por defecto.**
    #[default]
    Local,
    /// "Bring Your Own Key": el caller usa su propia clave/proveedor.
    Byok,
    /// Servicio SaaS gestionado. Requiere además `allow_saas = true`.
    Saas,
}

/// Configuración de gobernanza de la IA.
///
/// Los valores por defecto son deliberadamente conservadores: modo local, SaaS
/// deshabilitado, redacción activada y allowlist vacía.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Modo de operación.
    pub mode: AiMode,
    /// Interruptor maestro para permitir envíos a SaaS. **Por defecto `false`.**
    pub allow_saas: bool,
    /// Si se redactan secretos antes de previsualizar/enviar. **Por defecto `true`.**
    pub redact: bool,
    /// Campos/cadenas que pueden salir **sin** redactar (excepciones explícitas).
    pub allowlist: Vec<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            mode: AiMode::Local,
            allow_saas: false,
            redact: true,
            allowlist: Vec::new(),
        }
    }
}

/// Vista previa del payload que *se enviaría* (el control clave de gobernanza).
///
/// Permite al usuario ver, byte a byte, exactamente qué saldría antes de que salga.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewedPayload {
    /// Modo bajo el que se generó la vista previa.
    pub mode: AiMode,
    /// Si, con esta configuración, el contenido *realmente* saldría del proceso.
    pub would_send: bool,
    /// El contenido exacto (redactado si `cfg.redact`) que se enviaría.
    pub redacted_content: String,
    /// Tamaño en bytes del contenido que saldría.
    pub bytes: usize,
}

/// Construye la vista previa del payload bajo la configuración dada.
///
/// `would_send` es verdadero solo en BYOK, o en SaaS **con** `allow_saas = true`.
/// En modo local nunca sale nada (`would_send = false`).
pub fn preview_payload(content: &str, cfg: &AiConfig) -> PreviewedPayload {
    let would_send =
        matches!(cfg.mode, AiMode::Byok) || (matches!(cfg.mode, AiMode::Saas) && cfg.allow_saas);
    let redacted_content = if cfg.redact {
        redact(content, &cfg.allowlist)
    } else {
        content.to_string()
    };
    let bytes = redacted_content.len();
    PreviewedPayload {
        mode: cfg.mode,
        would_send,
        redacted_content,
        bytes,
    }
}

/// Verifica que el envío a un SaaS externo esté permitido.
///
/// Devuelve `Err` salvo que `mode == Saas` **y** `allow_saas == true`. Debe usarse
/// para *bloquear* cualquier envío externo: ningún dato sale sin pasar por aquí.
pub fn assert_saas_allowed(cfg: &AiConfig) -> Result<(), AiError> {
    if matches!(cfg.mode, AiMode::Saas) && cfg.allow_saas {
        Ok(())
    } else {
        Err(AiError::SaasDisabled)
    }
}

/// Proveedor de completados de IA. **Lo implementa el caller** para BYOK/SaaS.
///
/// Este crate solo incluye [`LocalHeuristicProvider`], que no toca la red.
pub trait AiProvider {
    /// Produce una respuesta para `prompt`.
    fn complete(&self, prompt: &str) -> Result<String, AiError>;
}

/// Proveedor 100% local basado en reglas. **No realiza llamadas de red.**
///
/// Es el único proveedor incluido en el crate y el respaldo del modo
/// [`AiMode::Local`]. Sus respuestas son deterministas y derivadas de heurísticas.
#[derive(Debug, Clone, Default)]
pub struct LocalHeuristicProvider;

impl LocalHeuristicProvider {
    pub fn new() -> Self {
        Self
    }
}

impl AiProvider for LocalHeuristicProvider {
    fn complete(&self, prompt: &str) -> Result<String, AiError> {
        // Heurístico, sin red: clasifica la intención por palabras clave y responde
        // con texto de orientación. Nunca "inventa" datos externos.
        let p = prompt.to_lowercase();
        let answer = if p.contains("threshold") || p.contains("umbral") {
            "Sugerencia local: deriva umbrales de los percentiles observados \
             (p95/p99) con un margen, y revísalos antes de aplicarlos."
        } else if p.contains("correlat") || p.contains("token") || p.contains("extract") {
            "Sugerencia local: busca valores literales que parezcan tokens/ids en \
             headers o cuerpo y conviértelos en extractores + variables."
        } else if p.contains("groovy") || p.contains("jsr223") || p.contains("script") {
            "Sugerencia local: mapea el script a elementos del IR (extractores, \
             timers, samplers) y omite logging; revisa cada paso."
        } else if p.contains("explain") || p.contains("result") || p.contains("resultado") {
            "Sugerencia local: revisa throughput, error rate, p95/p99 y la etiqueta \
             más lenta para interpretar el resultado."
        } else {
            "Proveedor local (sin red): no hay análisis específico para esta \
             petición; usa las funciones de sugerencia dedicadas."
        };
        Ok(answer.to_string())
    }
}

/// Errores del crate.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// Se intentó un envío a SaaS sin haberlo habilitado explícitamente.
    #[error("envío a SaaS deshabilitado: requiere mode=saas y allow_saas=true")]
    SaasDisabled,
    /// Error reportado por un proveedor (BYOK/SaaS) implementado por el caller.
    #[error("error del proveedor de IA: {0}")]
    Provider(String),
}

/// Redondea a 1 decimal (helper compartido por sugerencias y previews).
pub(crate) fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Construye el objeto JSON con la forma de `reports::Thresholds`.
///
/// Expuesto para que el caller pueda inspeccionar/serializar la propuesta sin
/// depender de `reports`.
pub(crate) fn thresholds_json(
    max_p95_ms: f64,
    max_p99_ms: f64,
    max_error_rate: f64,
    min_throughput_per_sec: f64,
) -> serde_json::Value {
    json!({
        "max_p95_ms": round1(max_p95_ms),
        "max_p99_ms": round1(max_p99_ms),
        "max_error_rate": (max_error_rate * 10000.0).round() / 10000.0,
        "min_throughput_per_sec": round1(min_throughput_per_sec),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saas_on() -> AiConfig {
        AiConfig {
            mode: AiMode::Saas,
            allow_saas: true,
            ..AiConfig::default()
        }
    }

    #[test]
    fn defaults_are_conservative() {
        let c = AiConfig::default();
        assert_eq!(c.mode, AiMode::Local);
        assert!(!c.allow_saas);
        assert!(c.redact);
        assert!(c.allowlist.is_empty());
    }

    #[test]
    fn preview_local_never_sends() {
        let p = preview_payload("Authorization: Bearer abc", &AiConfig::default());
        assert!(!p.would_send, "modo local nunca debe enviar");
    }

    #[test]
    fn preview_saas_disabled_does_not_send() {
        let cfg = AiConfig {
            mode: AiMode::Saas,
            allow_saas: false,
            ..AiConfig::default()
        };
        let p = preview_payload("hola", &cfg);
        assert!(!p.would_send, "SaaS sin allow_saas no debe enviar");
    }

    #[test]
    fn preview_saas_enabled_sends_redacted() {
        let cfg = saas_on();
        let p = preview_payload("Authorization: Bearer SECRETTOKEN12345", &cfg);
        assert!(p.would_send, "SaaS con allow_saas debe poder enviar");
        assert!(
            p.redacted_content.contains("***"),
            "el contenido debe ir redactado: {}",
            p.redacted_content
        );
        assert!(
            !p.redacted_content.contains("SECRETTOKEN12345"),
            "el token no debe filtrarse: {}",
            p.redacted_content
        );
        assert_eq!(p.bytes, p.redacted_content.len());
    }

    #[test]
    fn preview_byok_sends() {
        let cfg = AiConfig {
            mode: AiMode::Byok,
            ..AiConfig::default()
        };
        let p = preview_payload("hola", &cfg);
        assert!(p.would_send, "BYOK debe poder enviar");
    }

    #[test]
    fn preview_respects_redact_off() {
        let cfg = AiConfig {
            mode: AiMode::Byok,
            redact: false,
            ..AiConfig::default()
        };
        let p = preview_payload("password=secret", &cfg);
        assert_eq!(p.redacted_content, "password=secret");
    }

    #[test]
    fn assert_saas_allowed_errs_by_default() {
        assert!(assert_saas_allowed(&AiConfig::default()).is_err());
    }

    #[test]
    fn assert_saas_allowed_ok_when_enabled() {
        assert!(assert_saas_allowed(&saas_on()).is_ok());
    }

    #[test]
    fn local_provider_has_no_network_and_responds() {
        let prov = LocalHeuristicProvider::new();
        let out = prov.complete("explain my results").unwrap();
        assert!(!out.is_empty());
    }
}
