//! Verifica la ejecución distribuida: la carga se reparte entre N workers (no se
//! duplica) y los resultados se consolidan (Fase 6, DoD §9).

use cluster::{run_distributed, serve_worker_on};
use scenario_ir::model::*;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};

/// Target HTTP/1.1 con keep-alive: 200 a cada request.
fn spawn_target() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let resp =
                    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok";
                let mut buf = [0u8; 2048];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => {
                            let reqs = buf[..n]
                                .windows(4)
                                .filter(|w| *w == b"\r\n\r\n")
                                .count()
                                .max(1);
                            for _ in 0..reqs {
                                if s.write_all(resp.as_bytes()).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            });
        }
    });
    addr
}

async fn spawn_worker() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = serve_worker_on(listener).await;
    });
    format!("http://{addr}")
}

fn scenario() -> Scenario {
    let mut s = Scenario::new("dist");
    s.defaults = Some(HttpDefaults::default());
    s.thread_groups.push(ThreadGroup {
        name: "tg".into(),
        load: LoadProfile {
            virtual_users: 1,
            ramp_up_secs: 0,
            hold_secs: 0,
            ramp_down_secs: 0,
            iterations: None,
            duration_secs: Some(1),
        },
        on_error: OnError::Continue,
        steps: vec![Step::Http(HttpRequest {
            name: "GET /".into(),
            method: HttpMethod::Get,
            url: "/".into(),
            headers: Default::default(),
            query: Default::default(),
            body: None,
            follow_redirects: None,
            timeout_ms: Some(5000),
            timers: vec![],
            assertions: vec![Assertion::StatusCode { codes: vec![200] }],
            extractors: vec![],
        })],
    });
    s
}

#[tokio::test]
async fn distributes_load_across_workers_and_consolidates() {
    let target = spawn_target();
    // Los listeners quedan enlazados antes de llamar al coordinator, así que aceptan
    // conexiones aunque axum aún esté arrancando (backlog del SO).
    let w1 = spawn_worker().await;
    let w2 = spawn_worker().await;

    let result = run_distributed(
        &scenario(),
        6,
        1,
        Some(format!("http://{target}")),
        &[w1.clone(), w2.clone()],
    )
    .await;

    // Ambos workers respondieron OK
    assert_eq!(result.workers.len(), 2);
    assert!(
        result.workers.iter().all(|w| w.ok),
        "workers: {:?}",
        result.workers
    );
    // La carga se repartió 3+3 = 6 (no se duplicó)
    let total_vus: u32 = result.workers.iter().map(|w| w.vus).sum();
    assert_eq!(total_vus, 6);
    assert!(result.workers.iter().all(|w| w.vus == 3));
    // Resultado consolidado con requests reales
    assert!(
        result.combined.overall.count > 0,
        "sin requests consolidadas"
    );
    assert_eq!(result.combined.config.virtual_users, 6);
}

#[tokio::test]
async fn reports_worker_failure() {
    // Un worker que no existe debe reportarse como fallo, sin tumbar el run.
    let target = spawn_target();
    let result = run_distributed(
        &scenario(),
        4,
        1,
        Some(format!("http://{target}")),
        &["http://127.0.0.1:9".to_string()],
    )
    .await;
    assert_eq!(result.workers.len(), 1);
    assert!(!result.workers[0].ok);
    assert!(result.workers[0].error.is_some());
}
