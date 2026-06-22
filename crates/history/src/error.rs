//! Errores del crate `history`.

use thiserror::Error;

/// Error unificado de la capa de histórico.
#[derive(Debug, Error)]
pub enum HistoryError {
    /// Error proveniente de SQLite (rusqlite).
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Error de (de)serialización JSON (serde_json).
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Una fila referenciada no existe (p. ej. baseline apunta a un run borrado).
    #[error("not found: {0}")]
    NotFound(String),
}

/// Alias de resultado del crate.
pub type Result<T> = std::result::Result<T, HistoryError>;
