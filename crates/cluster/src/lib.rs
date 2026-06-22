//! `cluster` — ejecución distribuida de perfkit (Fase 6).
//!
//! Modelo: un **coordinator** reparte los VUs entre N **workers** (cada uno corre el
//! engine sobre su parte) y consolida los resultados. Control plane HTTP/JSON (gRPC +
//! mTLS son el objetivo de producción — ver `docs/adr/ADR-009`). La carga se **reparte,
//! no se duplica**, y los fallos de worker se reportan.

use axum::{
    Json, Router,
    routing::{get, post},
};
use scenario_ir::model::Scenario;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub use metrics::RunSummary;

/// Petición que el coordinator envía a cada worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRequest {
    pub scenario: Scenario,
    pub vus: u32,
    pub duration_secs: u64,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub run_id: String,
}

/// Resultado por worker (para reportar fallos claramente).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutcome {
    pub url: String,
    pub vus: u32,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Resultado consolidado de una ejecución distribuida.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedResult {
    pub combined: RunSummary,
    pub workers: Vec<WorkerOutcome>,
}

// ----------------------------------------------------------------- Worker

/// Router del worker (expuesto para tests y para `serve_worker`).
pub fn worker_router() -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/run", post(run_handler))
}

/// Arranca el worker escuchando en `addr` (data plane).
pub async fn serve_worker(addr: std::net::SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("perfkit worker escuchando en http://{addr}");
    serve_worker_on(listener).await
}

/// Sirve el worker sobre un listener ya enlazado (útil para puertos efímeros/tests).
pub async fn serve_worker_on(listener: tokio::net::TcpListener) -> anyhow::Result<()> {
    axum::serve(listener, worker_router()).await?;
    Ok(())
}

async fn run_handler(Json(mut req): Json<RunRequest>) -> Json<RunSummary> {
    apply_overrides(&mut req.scenario, req.vus, req.duration_secs);
    let stop = Arc::new(AtomicBool::new(false));
    let opts = engine::RunOptions {
        run_id: if req.run_id.is_empty() {
            "worker".into()
        } else {
            req.run_id.clone()
        },
        base_url_override: req.base_url.clone(),
        ..Default::default()
    };
    let summary = engine::run(&req.scenario, opts, Path::new("."), None, stop).await;
    Json(summary)
}

// ----------------------------------------------------------------- Coordinator

/// Reparte `total_vus` entre los `workers`, ejecuta en paralelo y consolida.
pub async fn run_distributed(
    scenario: &Scenario,
    total_vus: u32,
    duration_secs: u64,
    base_url: Option<String>,
    workers: &[String],
) -> DistributedResult {
    let shares = split_vus(total_vus, workers.len().max(1));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(duration_secs + 30))
        .build()
        .unwrap_or_default();

    let mut set = tokio::task::JoinSet::new();
    for (i, url) in workers.iter().enumerate() {
        let url = url.trim_end_matches('/').to_string();
        let vus = shares[i];
        let req = RunRequest {
            scenario: scenario.clone(),
            vus,
            duration_secs,
            base_url: base_url.clone(),
            run_id: format!("w{i}"),
        };
        let client = client.clone();
        set.spawn(async move {
            let res = post_run(&client, &url, &req).await;
            (url, vus, res)
        });
    }

    let mut summaries = Vec::new();
    let mut workers_out = Vec::new();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((url, vus, Ok(summary))) => {
                workers_out.push(WorkerOutcome {
                    url,
                    vus,
                    ok: true,
                    error: None,
                });
                summaries.push(summary);
            }
            Ok((url, vus, Err(e))) => {
                workers_out.push(WorkerOutcome {
                    url,
                    vus,
                    ok: false,
                    error: Some(e),
                });
            }
            Err(e) => workers_out.push(WorkerOutcome {
                url: "?".into(),
                vus: 0,
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }
    workers_out.sort_by(|a, b| a.url.cmp(&b.url));

    let combined = if summaries.is_empty() {
        zeroed_summary(&scenario.name, total_vus)
    } else {
        metrics::merge_summaries(&summaries)
    };
    DistributedResult {
        combined,
        workers: workers_out,
    }
}

async fn post_run(
    client: &reqwest::Client,
    url: &str,
    req: &RunRequest,
) -> Result<RunSummary, String> {
    let resp = client
        .post(format!("{url}/run"))
        .json(req)
        .send()
        .await
        .map_err(|e| format!("conexión fallida: {e}"))?
        .error_for_status()
        .map_err(|e| format!("respuesta de error: {e}"))?;
    resp.json::<RunSummary>()
        .await
        .map_err(|e| format!("respuesta inválida: {e}"))
}

/// Reparte `total` VUs en `n` partes lo más parejas posible (no duplica).
pub fn split_vus(total: u32, n: usize) -> Vec<u32> {
    if n == 0 {
        return vec![];
    }
    let base = total / n as u32;
    let rem = (total % n as u32) as usize;
    (0..n).map(|i| base + if i < rem { 1 } else { 0 }).collect()
}

fn apply_overrides(scenario: &mut Scenario, vus: u32, duration_secs: u64) {
    for g in &mut scenario.thread_groups {
        g.load.virtual_users = vus.max(1);
        g.load.duration_secs = Some(duration_secs);
        g.load.iterations = None;
        g.load.hold_secs = duration_secs;
    }
}

fn zeroed_summary(scenario_name: &str, vus: u32) -> RunSummary {
    RunSummary {
        run_id: "distributed".into(),
        scenario_name: scenario_name.into(),
        config: metrics::RunConfig {
            virtual_users: vus,
            thread_groups: 0,
        },
        overall: metrics::LabelStats {
            label: "ALL".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_even_and_total_preserved() {
        assert_eq!(split_vus(10, 3), vec![4, 3, 3]);
        assert_eq!(split_vus(9, 3), vec![3, 3, 3]);
        assert_eq!(split_vus(10, 3).iter().sum::<u32>(), 10);
    }
}
