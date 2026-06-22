//! Tests de semántica de controladores contra un servidor HTTP local real.
//! Verifica que Loop, Interleave y Throughput se comporten como en JMeter (Fase 4).

use engine::{RunOptions, run};
use scenario_ir::model::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Servidor HTTP/1.1 mínimo con keep-alive: responde 200 a cada request.
fn spawn_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let body = "{\"ok\":true}";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                    body.len(),
                    body
                );
                let mut buf = [0u8; 4096];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => {
                            // Una respuesta por request completa recibida en el chunk.
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

fn http(name: &str, path: &str) -> Step {
    Step::Http(HttpRequest {
        name: name.into(),
        method: HttpMethod::Get,
        url: path.into(),
        headers: Default::default(),
        query: Default::default(),
        body: None,
        follow_redirects: None,
        timeout_ms: Some(5000),
        timers: vec![],
        assertions: vec![Assertion::StatusCode { codes: vec![200] }],
        extractors: vec![],
    })
}

async fn run_counts(steps: Vec<Step>, iterations: u64) -> HashMap<String, u64> {
    let addr = spawn_server();
    let mut s = Scenario::new("sem");
    s.defaults = Some(HttpDefaults {
        base_url: Some(format!("http://{addr}")),
        ..Default::default()
    });
    s.thread_groups.push(ThreadGroup {
        name: "tg".into(),
        load: LoadProfile {
            virtual_users: 1,
            ramp_up_secs: 0,
            hold_secs: 0,
            ramp_down_secs: 0,
            iterations: Some(iterations),
            duration_secs: None,
        },
        on_error: OnError::Continue,
        steps,
    });
    let stop = Arc::new(AtomicBool::new(false));
    let summary = run(&s, RunOptions::default(), Path::new("."), None, stop).await;
    summary
        .labels
        .into_iter()
        .map(|l| (l.label, l.count))
        .collect()
}

#[tokio::test]
async fn loop_runs_count_times_per_iteration() {
    let steps = vec![Step::Loop(LoopController {
        name: "lp".into(),
        count: 3,
        steps: vec![http("Y", "/y")],
    })];
    let counts = run_counts(steps, 2).await;
    assert_eq!(counts.get("Y"), Some(&6), "3 loops x 2 iteraciones = 6");
}

#[tokio::test]
async fn interleave_round_robins_children() {
    let steps = vec![Step::Interleave(InterleaveController {
        name: "il".into(),
        steps: vec![http("A", "/a"), http("B", "/b"), http("C", "/c")],
    })];
    let counts = run_counts(steps, 6).await;
    assert_eq!(counts.get("A"), Some(&2));
    assert_eq!(counts.get("B"), Some(&2));
    assert_eq!(counts.get("C"), Some(&2));
}

#[tokio::test]
async fn throughput_zero_never_hundred_always() {
    let zero = vec![Step::Throughput(ThroughputController {
        name: "z".into(),
        percent: 0.0,
        steps: vec![http("Z", "/z")],
    })];
    assert_eq!(run_counts(zero, 5).await.get("Z"), None, "0% nunca ejecuta");

    let hundred = vec![Step::Throughput(ThroughputController {
        name: "h".into(),
        percent: 100.0,
        steps: vec![http("H", "/h")],
    })];
    assert_eq!(
        run_counts(hundred, 5).await.get("H"),
        Some(&5),
        "100% siempre ejecuta"
    );
}
