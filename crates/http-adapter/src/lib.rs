//! `http-adapter` — ejecución de una petición HTTP/HTTPS sobre reqwest (rustls).
//!
//! Cada VU usa su propio [`HttpClient`] para tener cookie store aislado.

use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("timeout")]
    Timeout,
    #[error("método inválido: {0}")]
    BadMethod(String),
    #[error("error de transporte: {0}")]
    Transport(String),
}

/// Petición ya interpolada y lista para ejecutar.
#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout_ms: Option<u64>,
}

/// Respuesta normalizada.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub bytes: u64,
    /// Time-to-first-byte (hasta recibir cabeceras), en microsegundos.
    pub ttfb_us: u64,
    /// Latencia total (hasta leer todo el cuerpo), en microsegundos.
    pub latency_us: u64,
}

/// Cliente HTTP por VU (cookie store aislado).
#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new(follow_redirects: bool, default_timeout_ms: Option<u64>) -> Result<Self, HttpError> {
        let redirect = if follow_redirects {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        };
        let mut b = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(redirect)
            .user_agent(concat!("perfkit/", env!("CARGO_PKG_VERSION")));
        if let Some(ms) = default_timeout_ms {
            b = b.timeout(Duration::from_millis(ms));
        }
        let client = b.build().map_err(|e| HttpError::Transport(e.to_string()))?;
        Ok(Self { client })
    }

    /// Ejecuta la petición y mide la latencia con reloj monotónico.
    pub async fn execute(&self, req: &PreparedRequest) -> Result<HttpResponse, HttpError> {
        let method = reqwest::Method::from_bytes(req.method.as_bytes())
            .map_err(|_| HttpError::BadMethod(req.method.clone()))?;
        let mut rb = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if let Some(body) = &req.body {
            rb = rb.body(body.clone());
        }
        if let Some(ms) = req.timeout_ms {
            rb = rb.timeout(Duration::from_millis(ms));
        }

        let start = Instant::now();
        let resp = rb.send().await.map_err(|e| {
            if e.is_timeout() {
                HttpError::Timeout
            } else {
                HttpError::Transport(e.to_string())
            }
        })?;
        // send() resuelve al recibir las cabeceras ⇒ aproxima el TTFB.
        let ttfb_us = start.elapsed().as_micros() as u64;
        let status = resp.status().as_u16();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        let latency_us = start.elapsed().as_micros() as u64;
        let len = bytes.len() as u64;
        let body = String::from_utf8_lossy(&bytes).into_owned();
        Ok(HttpResponse {
            status,
            headers,
            body,
            bytes: len,
            ttfb_us,
            latency_us,
        })
    }
}
