//! `engine` — motor de ejecución de carga.
//!
//! Implementa el scheduler (ramp-up/hold), usuarios virtuales asíncronos sobre Tokio,
//! interpolación de variables, datasets CSV, timers, assertions y extractores. El hot
//! path solo emite [`metrics::Sample`]; la agregación corre en una tarea aparte.

use http_adapter::{HttpClient, HttpResponse, PreparedRequest};
use metrics::{Recorder, RunConfig, Sample, SampleDetail, SampleKind};
use rand::RngExt;
use rand_distr::Distribution;
use regex::Regex;
use scenario_ir::model::*;
use serde_json_path::JsonPath;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub use metrics::{LiveSnapshot, RunSummary};

/// Opciones de ejecución.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub run_id: String,
    /// Redirige todas las peticiones a este origen (esquema://host:puerto). Útil para
    /// pruebas locales o para apuntar un escenario a otro entorno sin editar URLs.
    pub base_url_override: Option<String>,
    /// Captura el detalle por petición (request/response) para inspección. Pensado
    /// para corridas cortas de depuración; en runs de carga reales debe ir apagado.
    pub capture: bool,
    /// Tope de peticiones a capturar. **0 = tope por defecto seguro** (5000); un valor
    /// explícito lo sube bajo tu responsabilidad (capturar mucho a alto rate congela).
    pub capture_limit: usize,
    /// Si es `true`, captura cabeceras/variables en texto plano (sin redactar).
    /// Por defecto `false` (se redactan secretos). Opt-in explícito antes del test.
    pub capture_plaintext: bool,
}

/// Ejecuta el escenario completo y devuelve el resumen agregado.
///
/// - `base_dir`: directorio para resolver rutas de datasets relativas.
/// - `live`: canal opcional para snapshots en vivo (dashboard).
/// - `stop`: bandera de cancelación cooperativa.
pub async fn run(
    scenario: &Scenario,
    options: RunOptions,
    base_dir: &Path,
    live: Option<tokio::sync::mpsc::UnboundedSender<LiveSnapshot>>,
    stop: Arc<AtomicBool>,
) -> RunSummary {
    let start = Instant::now();
    let started_at = chrono::Utc::now().to_rfc3339();
    let total_vus: u32 = scenario
        .thread_groups
        .iter()
        .map(|g| g.load.virtual_users)
        .sum();
    let config = RunConfig {
        virtual_users: total_vus,
        thread_groups: scenario.thread_groups.len() as u32,
    };

    // Carga de datasets (compartidos entre VUs).
    let mut pools: Vec<Arc<DatasetPool>> = Vec::new();
    for ds in &scenario.datasets {
        match DatasetPool::load(ds, base_dir) {
            Ok(p) => pools.push(Arc::new(p)),
            Err(e) => tracing::warn!("dataset '{}' no se pudo cargar: {e}", ds.name),
        }
    }
    let pools = Arc::new(pools);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Sample>();
    let active = Arc::new(AtomicU32::new(0));

    // Captura de detalle por petición (modo depuración/inspección).
    // Salvaguarda obligatoria: capturar sin tope a alto rate agota memoria y congela la
    // app (mismo footgun que el View Results Tree de JMeter, que por eso se desaconseja
    // en pruebas de carga). `capture_limit == 0` ⇒ tope por defecto seguro; un valor
    // explícito lo sube bajo tu responsabilidad.
    let capture = options.capture;
    let capture_plaintext = options.capture_plaintext;
    let cap_limit = match options.capture_limit {
        0 => DEFAULT_CAPTURE_CAP,
        n => n,
    };
    let details: Arc<Mutex<Vec<SampleDetail>>> = Arc::new(Mutex::new(Vec::new()));
    let detail_seq = Arc::new(AtomicUsize::new(0));

    // Tarea agregadora (fuera del hot path).
    let agg_active = active.clone();
    let scenario_name = scenario.name.clone();
    let run_id = if options.run_id.is_empty() {
        "run".to_string()
    } else {
        options.run_id.clone()
    };
    let aggregator = tokio::spawn(async move {
        let mut rec = Recorder::new(run_id, started_at, config);
        let mut tick = tokio::time::interval(Duration::from_millis(500));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                // `biased`: prioriza el tick para que el snapshot en vivo se emita cada
                // 500 ms aunque los VUs inunden el canal de muestras (p.ej. una corrida
                // que falla en bucle). Sin esto, el tick se queda sin turno y las gráficas
                // solo se actualizan al final.
                biased;
                _ = tick.tick() => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    rec.record_vus(elapsed, agg_active.load(Ordering::Relaxed));
                    if let Some(tx) = &live {
                        let _ = tx.send(rec.live_snapshot(elapsed));
                    }
                }
                msg = rx.recv() => match msg {
                    Some(s) => rec.record_sample(&s),
                    None => break,
                },
            }
        }
        let dur = start.elapsed().as_secs_f64();
        rec.finish(scenario_name, dur)
    });

    let defaults = Arc::new(scenario.defaults.clone());
    let base_override = Arc::new(options.base_url_override.clone());

    let mut handles = Vec::new();
    for group in &scenario.thread_groups {
        let lp = group.load.clone();
        let group_deadline =
            if lp.duration_secs.is_some() || lp.hold_secs > 0 || lp.ramp_down_secs > 0 {
                Some(start + Duration::from_secs(lp.total_secs()))
            } else {
                None
            };
        let max_iters = lp.iterations.or(if group_deadline.is_none() {
            Some(1)
        } else {
            None
        });

        for vu in 0..lp.virtual_users {
            let start_delay = Duration::from_millis(
                (lp.ramp_up_secs * 1000 * vu as u64) / lp.virtual_users.max(1) as u64,
            );
            let steps = group.steps.clone();
            let vars0 = scenario.variables.clone();
            let on_error = group.on_error;
            let tx = tx.clone();
            let stop = stop.clone();
            let active = active.clone();
            let pools = pools.clone();
            let defaults = defaults.clone();
            let base_override = base_override.clone();
            let details = details.clone();
            let detail_seq = detail_seq.clone();

            handles.push(tokio::spawn(async move {
                tokio::time::sleep(start_delay).await;
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                active.fetch_add(1, Ordering::Relaxed);

                let follow = defaults
                    .as_ref()
                    .as_ref()
                    .and_then(|d| d.follow_redirects)
                    .unwrap_or(true);
                let default_timeout = defaults
                    .as_ref()
                    .as_ref()
                    .and_then(|d| d.response_timeout_ms);
                let client = match HttpClient::new(follow, default_timeout) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("no se pudo crear el cliente HTTP: {e}");
                        active.fetch_sub(1, Ordering::Relaxed);
                        return;
                    }
                };

                let env = VuEnv {
                    client: &client,
                    tx: &tx,
                    start,
                    deadline: group_deadline,
                    on_error,
                    defaults: defaults.as_ref(),
                    base_override: base_override.as_ref(),
                    stop: &stop,
                    capture,
                    capture_plaintext,
                    capture_limit: cap_limit,
                    details: &details,
                    detail_seq: &detail_seq,
                };
                let mut ctx = Ctx {
                    vars: vars0.into_iter().collect(),
                    failures: 0,
                    interleave: HashMap::new(),
                };

                let mut iter = 0u64;
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(d) = group_deadline
                        && Instant::now() >= d
                    {
                        break;
                    }
                    if let Some(m) = max_iters
                        && iter >= m
                    {
                        break;
                    }
                    for pool in pools.iter() {
                        if let Some(row) = pool.next() {
                            for (k, v) in row {
                                ctx.vars.insert(k, v);
                            }
                        }
                    }
                    let flow = run_steps(&steps, &mut ctx, &env).await;
                    iter += 1;
                    match flow {
                        Flow::Continue => {}
                        Flow::StopThread => break,
                        Flow::StopTest => {
                            stop.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                active.fetch_sub(1, Ordering::Relaxed);
            }));
        }
    }

    drop(tx);
    for h in handles {
        let _ = h.await;
    }
    let mut summary = aggregator
        .await
        .expect("la tarea agregadora no debería panicar");
    if capture {
        let mut d = details.lock().map(|g| g.clone()).unwrap_or_default();
        d.sort_by_key(|x| x.seq);
        summary.details = d;
    }
    summary
}

// ---------------------------------------------------------------------------

struct Ctx {
    vars: HashMap<String, String>,
    failures: u64,
    /// Contador rotatorio por Interleave Controller (clave = nombre).
    interleave: HashMap<String, usize>,
}

#[derive(Clone, Copy)]
enum Flow {
    Continue,
    StopThread,
    StopTest,
}

struct VuEnv<'a> {
    client: &'a HttpClient,
    tx: &'a tokio::sync::mpsc::UnboundedSender<Sample>,
    start: Instant,
    deadline: Option<Instant>,
    on_error: OnError,
    defaults: &'a Option<HttpDefaults>,
    base_override: &'a Option<String>,
    stop: &'a Arc<AtomicBool>,
    capture: bool,
    capture_plaintext: bool,
    capture_limit: usize,
    details: &'a Arc<Mutex<Vec<SampleDetail>>>,
    detail_seq: &'a Arc<AtomicUsize>,
}

fn run_steps<'a>(
    steps: &'a [Step],
    ctx: &'a mut Ctx,
    env: &'a VuEnv<'a>,
) -> Pin<Box<dyn Future<Output = Flow> + Send + 'a>> {
    Box::pin(async move {
        for step in steps {
            if env.stop.load(Ordering::Relaxed) {
                return Flow::StopThread;
            }
            if let Some(d) = env.deadline
                && Instant::now() >= d
            {
                return Flow::StopThread;
            }
            let flow = match step {
                Step::Timer(t) => {
                    apply_timer(t).await;
                    Flow::Continue
                }
                Step::Http(h) => execute_http(h, &mut *ctx, env).await,
                Step::Transaction(t) => {
                    let before = ctx.failures;
                    let tstart = Instant::now();
                    let flow = run_steps(&t.steps, &mut *ctx, env).await;
                    let elapsed_us = tstart.elapsed().as_micros() as u64;
                    let offset_ms = tstart.saturating_duration_since(env.start).as_millis() as u64;
                    let _ = env.tx.send(Sample {
                        label: t.name.clone(),
                        kind: SampleKind::Transaction,
                        offset_ms,
                        latency_us: elapsed_us,
                        ttfb_us: elapsed_us,
                        status: None,
                        bytes: 0,
                        sent_bytes: 0,
                        success: ctx.failures == before,
                        error: None,
                        error_kind: None,
                    });
                    flow
                }
                Step::Loop(l) => {
                    let mut flow = Flow::Continue;
                    for _ in 0..l.count {
                        flow = run_steps(&l.steps, &mut *ctx, env).await;
                        if !matches!(flow, Flow::Continue) {
                            break;
                        }
                    }
                    flow
                }
                Step::If(c) => {
                    if eval_condition(&c.condition, ctx) {
                        run_steps(&c.steps, &mut *ctx, env).await
                    } else {
                        Flow::Continue
                    }
                }
                Step::While(w) => {
                    let mut flow = Flow::Continue;
                    let mut n = 0u64;
                    while n < w.max_iterations && eval_condition(&w.condition, ctx) {
                        flow = run_steps(&w.steps, &mut *ctx, env).await;
                        if !matches!(flow, Flow::Continue) {
                            break;
                        }
                        n += 1;
                    }
                    flow
                }
                Step::Throughput(t) => {
                    let run = if t.percent >= 100.0 {
                        true
                    } else if t.percent <= 0.0 {
                        false
                    } else {
                        rand::rng().random::<f64>() * 100.0 < t.percent
                    };
                    if run {
                        run_steps(&t.steps, &mut *ctx, env).await
                    } else {
                        Flow::Continue
                    }
                }
                Step::Interleave(c) => {
                    if c.steps.is_empty() {
                        Flow::Continue
                    } else {
                        let counter = ctx.interleave.entry(c.name.clone()).or_insert(0);
                        let idx = *counter % c.steps.len();
                        *counter += 1;
                        run_steps(std::slice::from_ref(&c.steps[idx]), &mut *ctx, env).await
                    }
                }
                Step::Random(c) => {
                    if c.steps.is_empty() {
                        Flow::Continue
                    } else {
                        let r: f64 = rand::rng().random::<f64>();
                        let idx = ((r * c.steps.len() as f64) as usize).min(c.steps.len() - 1);
                        run_steps(std::slice::from_ref(&c.steps[idx]), &mut *ctx, env).await
                    }
                }
                Step::Kafka(k) => execute_kafka(k, &mut *ctx, env).await,
            };
            if !matches!(flow, Flow::Continue) {
                return flow;
            }
        }
        Flow::Continue
    })
}

async fn execute_http(h: &HttpRequest, ctx: &mut Ctx, env: &VuEnv<'_>) -> Flow {
    for t in &h.timers {
        apply_timer(t).await;
    }
    let prepared = build_request(h, ctx, env);
    let sent_bytes = estimate_sent(&prepared);
    let offset_ms = env.start.elapsed().as_millis() as u64;
    let (sample, resp_opt) = match env.client.execute(&prepared).await {
        Ok(resp) => {
            let (success, err) = eval_assertions(&h.assertions, &resp);
            if success {
                apply_extractors(&h.extractors, &resp, ctx);
            }
            let error_kind = if success {
                None
            } else if err
                .as_deref()
                .map(|m| m.starts_with("HTTP "))
                .unwrap_or(false)
            {
                Some("status".to_string())
            } else {
                Some("assertion".to_string())
            };
            let s = Sample {
                label: h.name.clone(),
                kind: SampleKind::Http,
                offset_ms,
                latency_us: resp.latency_us,
                ttfb_us: resp.ttfb_us,
                status: Some(resp.status),
                bytes: resp.bytes,
                sent_bytes,
                success,
                error: err,
                error_kind,
            };
            (s, Some(resp))
        }
        Err(e) => {
            let kind = match &e {
                http_adapter::HttpError::Timeout => "timeout",
                http_adapter::HttpError::Transport(_) => "connection",
                http_adapter::HttpError::BadMethod(_) => "other",
            };
            (
                Sample {
                    label: h.name.clone(),
                    kind: SampleKind::Http,
                    offset_ms,
                    latency_us: 0,
                    ttfb_us: 0,
                    status: None,
                    bytes: 0,
                    sent_bytes,
                    success: false,
                    error: Some(e.to_string()),
                    error_kind: Some(kind.to_string()),
                },
                None,
            )
        }
    };

    if env.capture {
        let n = env.detail_seq.fetch_add(1, Ordering::Relaxed);
        if n < env.capture_limit {
            let detail = build_detail(
                n as u64,
                h,
                &prepared,
                resp_opt.as_ref(),
                &sample,
                ctx,
                env.capture_plaintext,
            );
            if let Ok(mut g) = env.details.lock() {
                g.push(detail);
            }
        }
    }

    let success = sample.success;
    let _ = env.tx.send(sample);
    if success {
        Flow::Continue
    } else {
        ctx.failures += 1;
        match env.on_error {
            OnError::Continue => Flow::Continue,
            OnError::StopThread => Flow::StopThread,
            OnError::StopTest => Flow::StopTest,
        }
    }
}

/// Tope de seguridad por defecto de peticiones capturadas (evita OOM/congelamiento a
/// alto rate). Suficiente para inspeccionar corridas cortas; para carga real, apaga la
/// captura. Un `capture_limit` explícito > 0 lo sobreescribe.
const DEFAULT_CAPTURE_CAP: usize = 5000;

/// Claves de cabecera/variable cuyo valor se enmascara por defecto.
const SENSITIVE_KEYS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "api-key",
    "apikey",
    "x-api-key",
    "token",
    "secret",
    "password",
    "passwd",
];

/// Construye el detalle de una petición (para inspección). Con `plaintext = false`
/// (por defecto) enmascara cabeceras/variables sensibles; con `true` las muestra
/// en claro (opt-in explícito). Los cuerpos se muestran siempre (solo recortados).
fn build_detail(
    seq: u64,
    h: &HttpRequest,
    prepared: &PreparedRequest,
    resp: Option<&HttpResponse>,
    sample: &Sample,
    ctx: &Ctx,
    plaintext: bool,
) -> SampleDetail {
    let truncate = |s: &str| -> String {
        let t: String = s.chars().take(8000).collect();
        if t.len() < s.len() {
            format!("{t}… (truncado)")
        } else {
            t
        }
    };
    let mask = |k: &str, v: &str| -> String {
        if plaintext {
            return v.to_string();
        }
        let kl = k.to_ascii_lowercase();
        if SENSITIVE_KEYS.iter().any(|s| kl.contains(s)) {
            "***REDACTED***".to_string()
        } else {
            security::redact(v)
        }
    };
    let kv = |list: &[(String, String)]| -> Vec<(String, String)> {
        list.iter().map(|(k, v)| (k.clone(), mask(k, v))).collect()
    };
    let extracted: Vec<(String, String)> = h
        .extractors
        .iter()
        .filter_map(|ex| {
            let var = extractor_var(ex);
            ctx.vars.get(var).map(|v| (var.to_string(), mask(var, v)))
        })
        .collect();
    let mut vars: Vec<(String, String)> = ctx
        .vars
        .iter()
        .map(|(k, v)| (k.clone(), mask(k, v)))
        .collect();
    vars.sort();
    SampleDetail {
        seq,
        label: h.name.clone(),
        kind: SampleKind::Http,
        method: prepared.method.clone(),
        url: prepared.url.clone(),
        req_headers: kv(&prepared.headers),
        req_body: prepared
            .body
            .as_ref()
            .map(|b| truncate(&String::from_utf8_lossy(b))),
        status: sample.status,
        resp_headers: resp.map(|r| kv(&r.headers)).unwrap_or_default(),
        resp_body: resp.map(|r| truncate(&r.body)).unwrap_or_default(),
        latency_ms: sample.latency_us as f64 / 1000.0,
        bytes: sample.bytes,
        success: sample.success,
        error: sample.error.clone(),
        extracted,
        vars,
    }
}

fn extractor_var(ex: &Extractor) -> &str {
    match ex {
        Extractor::Regex { var, .. } => var,
        Extractor::JsonPath { var, .. } => var,
        Extractor::Boundary { var, .. } => var,
    }
}

async fn execute_kafka(k: &KafkaRequest, ctx: &mut Ctx, env: &VuEnv<'_>) -> Flow {
    let rec = kafka_adapter::prepare(k, &ctx.vars);
    let offset_ms = env.start.elapsed().as_millis() as u64;
    let sent = rec.value.len() as u64;
    let sample = match kafka_adapter::produce(&rec).await {
        Ok(latency_us) => Sample {
            label: k.name.clone(),
            kind: SampleKind::Kafka,
            offset_ms,
            latency_us,
            ttfb_us: latency_us,
            status: None,
            bytes: 0,
            sent_bytes: sent,
            success: true,
            error: None,
            error_kind: None,
        },
        Err(e) => Sample {
            label: k.name.clone(),
            kind: SampleKind::Kafka,
            offset_ms,
            latency_us: 0,
            ttfb_us: 0,
            status: None,
            bytes: 0,
            sent_bytes: sent,
            success: false,
            error: Some(e.to_string()),
            error_kind: Some("connection".to_string()),
        },
    };
    let success = sample.success;
    let _ = env.tx.send(sample);
    if success {
        Flow::Continue
    } else {
        ctx.failures += 1;
        match env.on_error {
            OnError::Continue => Flow::Continue,
            OnError::StopThread => Flow::StopThread,
            OnError::StopTest => Flow::StopTest,
        }
    }
}

/// Estima los bytes enviados de una request (método + url + cabeceras + cuerpo).
fn estimate_sent(p: &PreparedRequest) -> u64 {
    let body = p.body.as_ref().map(|b| b.len()).unwrap_or(0);
    let headers: usize = p.headers.iter().map(|(k, v)| k.len() + v.len() + 4).sum();
    (p.method.len() + p.url.len() + headers + body + 12) as u64
}

fn build_request(h: &HttpRequest, ctx: &Ctx, env: &VuEnv<'_>) -> PreparedRequest {
    let mut url = interpolate(&h.url, &ctx.vars);
    let absolute = url.starts_with("http://") || url.starts_with("https://");
    if !absolute {
        let base = env
            .base_override
            .clone()
            .or_else(|| env.defaults.as_ref().and_then(|d| d.base_url.clone()));
        if let Some(b) = base {
            url = join_url(&b, &url);
        }
    } else if let Some(b) = env.base_override {
        url = override_authority(b, &url);
    }

    if !h.query.is_empty() {
        let qs: Vec<String> = h
            .query
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    urlencode(&interpolate(k, &ctx.vars)),
                    urlencode(&interpolate(v, &ctx.vars))
                )
            })
            .collect();
        let sep = if url.contains('?') { '&' } else { '?' };
        url = format!("{url}{sep}{}", qs.join("&"));
    }

    let mut headers: Vec<(String, String)> = Vec::new();
    if let Some(d) = env.defaults {
        for (k, v) in &d.headers {
            headers.push((k.clone(), interpolate(v, &ctx.vars)));
        }
    }
    for (k, v) in &h.headers {
        headers.push((k.clone(), interpolate(v, &ctx.vars)));
    }

    let (body, ctype) = match &h.body {
        Some(Body::Raw { content_type, data }) => (
            Some(interpolate(data, &ctx.vars).into_bytes()),
            content_type.clone(),
        ),
        Some(Body::Form { fields }) => {
            let enc = fields
                .iter()
                .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(&interpolate(v, &ctx.vars))))
                .collect::<Vec<_>>()
                .join("&");
            (
                Some(enc.into_bytes()),
                Some("application/x-www-form-urlencoded".to_string()),
            )
        }
        None => (None, None),
    };
    if let Some(ct) = ctype
        && !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        headers.push(("Content-Type".to_string(), ct));
    }

    PreparedRequest {
        method: h.method.as_str().to_string(),
        url,
        headers,
        body,
        timeout_ms: h.timeout_ms,
    }
}

async fn apply_timer(t: &Timer) {
    let ms = match t {
        Timer::Constant { delay_ms } => *delay_ms,
        Timer::UniformRandom { base_ms, range_ms } => {
            let mut rng = rand::rng();
            base_ms + (rng.random::<f64>() * (*range_ms as f64)) as u64
        }
        Timer::Gaussian {
            offset_ms,
            deviation_ms,
        } => {
            let dev = (*deviation_ms as f64).max(0.0001);
            let normal = rand_distr::Normal::new(0.0, dev).expect("normal");
            let mut rng = rand::rng();
            let v: f64 = normal.sample(&mut rng);
            (*offset_ms as f64 + v).max(0.0) as u64
        }
        // Aproximación MVP del Constant Throughput Timer (pacing por VU). Refinar en
        // qa-performance-semantics; JMeter lo calcula global o por-hilo.
        Timer::ConstantThroughput { target_per_minute } => {
            if *target_per_minute > 0.0 {
                (60_000.0 / target_per_minute) as u64
            } else {
                0
            }
        }
    };
    if ms > 0 {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}

fn eval_assertions(
    asserts: &[Assertion],
    resp: &http_adapter::HttpResponse,
) -> (bool, Option<String>) {
    for a in asserts {
        let ok = match a {
            Assertion::StatusCode { codes } => codes.contains(&resp.status),
            Assertion::BodyContains { substring, negate } => {
                let c = resp.body.contains(substring.as_str());
                if *negate { !c } else { c }
            }
            Assertion::BodyMatches { pattern, negate } => {
                let m = cached_regex(pattern)
                    .map(|re| re.is_match(&resp.body))
                    .unwrap_or(false);
                if *negate { !m } else { m }
            }
            Assertion::JsonPath {
                path,
                equals,
                exists,
            } => eval_jsonpath_assert(&resp.body, path, equals, exists),
            Assertion::DurationBelowMs { max_ms } => (resp.latency_us / 1000) <= *max_ms,
            Assertion::SizeBelowBytes { max_bytes } => resp.bytes <= *max_bytes,
        };
        if !ok {
            return (false, Some(assertion_msg(a, resp)));
        }
    }
    // Sin assertion de status explícita: >=400 es fallo (como el comportamiento por
    // defecto de un sampler JMeter).
    if !asserts
        .iter()
        .any(|a| matches!(a, Assertion::StatusCode { .. }))
        && resp.status >= 400
    {
        return (false, Some(format!("HTTP {}", resp.status)));
    }
    (true, None)
}

fn eval_jsonpath_assert(
    body: &str,
    path: &str,
    equals: &Option<String>,
    exists: &Option<bool>,
) -> bool {
    let val: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let jp = match JsonPath::parse(path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let nodes = jp.query(&val);
    let present = !nodes.is_empty();
    if let Some(ex) = exists
        && present != *ex
    {
        return false;
    }
    if let Some(want) = equals {
        return nodes.first().map(node_to_string).as_deref() == Some(want.as_str());
    }
    if exists.is_none() && equals.is_none() {
        return present;
    }
    true
}

fn apply_extractors(exs: &[Extractor], resp: &http_adapter::HttpResponse, ctx: &mut Ctx) {
    for ex in exs {
        match ex {
            Extractor::Regex {
                var,
                pattern,
                group,
                default,
            } => {
                let v = cached_regex(pattern).and_then(|re| {
                    re.captures(&resp.body)
                        .and_then(|c| c.get(*group))
                        .map(|m| m.as_str().to_string())
                });
                set_var(ctx, var, v, default);
            }
            Extractor::JsonPath { var, path, default } => {
                let v = JsonPath::parse(path).ok().and_then(|jp| {
                    let val: serde_json::Value = serde_json::from_str(&resp.body).ok()?;
                    jp.query(&val).first().map(node_to_string)
                });
                set_var(ctx, var, v, default);
            }
            Extractor::Boundary {
                var,
                left,
                right,
                default,
            } => {
                set_var(ctx, var, extract_boundary(&resp.body, left, right), default);
            }
        }
    }
}

fn set_var(ctx: &mut Ctx, var: &str, value: Option<String>, default: &Option<String>) {
    if let Some(v) = value.or_else(|| default.clone()) {
        ctx.vars.insert(var.to_string(), v);
    }
}

fn extract_boundary(body: &str, left: &str, right: &str) -> Option<String> {
    let start = if left.is_empty() {
        0
    } else {
        body.find(left)? + left.len()
    };
    let rest = &body[start..];
    let end = if right.is_empty() {
        rest.len()
    } else {
        rest.find(right)?
    };
    Some(rest[..end].to_string())
}

fn eval_condition(cond: &str, ctx: &Ctx) -> bool {
    let c = interpolate(cond, &ctx.vars);
    let c = c.trim();
    if c.eq_ignore_ascii_case("true") {
        return true;
    }
    if c.eq_ignore_ascii_case("false") || c.is_empty() {
        return false;
    }
    if let Some((l, r)) = c.split_once("==") {
        return strip(l) == strip(r);
    }
    if let Some((l, r)) = c.split_once("!=") {
        return strip(l) != strip(r);
    }
    false
}

fn strip(s: &str) -> String {
    s.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn node_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn assertion_msg(a: &Assertion, resp: &http_adapter::HttpResponse) -> String {
    match a {
        Assertion::StatusCode { codes } => format!("status {} no está en {:?}", resp.status, codes),
        Assertion::BodyContains { substring, negate } => {
            format!(
                "body {}contiene \"{}\"",
                if *negate { "no debía " } else { "debía " },
                substring
            )
        }
        Assertion::BodyMatches { pattern, negate } => {
            format!(
                "body {}coincide con /{}/",
                if *negate { "no debía " } else { "debía " },
                pattern
            )
        }
        Assertion::JsonPath { path, .. } => format!("jsonpath {path} falló"),
        Assertion::DurationBelowMs { max_ms } => {
            format!("duración {}ms > {}ms", resp.latency_us / 1000, max_ms)
        }
        Assertion::SizeBelowBytes { max_bytes } => {
            format!("tamaño {}B > {}B", resp.bytes, max_bytes)
        }
    }
}

fn var_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").expect("regex de variables"))
}

fn interpolate(s: &str, vars: &HashMap<String, String>) -> String {
    if !s.contains("${") {
        return s.to_string();
    }
    var_re()
        .replace_all(s, |caps: &regex::Captures| {
            let name = &caps[1];
            vars.get(name)
                .cloned()
                .unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned()
}

thread_local! {
    static RE_CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
}

fn cached_regex(pat: &str) -> Option<Regex> {
    RE_CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if let Some(r) = m.get(pat) {
            return Some(r.clone());
        }
        match Regex::new(pat) {
            Ok(r) => {
                m.insert(pat.to_string(), r.clone());
                Some(r)
            }
            Err(_) => None,
        }
    })
}

fn join_url(base: &str, rel: &str) -> String {
    let b = base.trim_end_matches('/');
    if rel.starts_with('/') {
        format!("{b}{rel}")
    } else {
        format!("{b}/{rel}")
    }
}

fn override_authority(base: &str, url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after = &url[scheme_end + 3..];
        if let Some(slash) = after.find('/') {
            return format!("{}{}", base.trim_end_matches('/'), &after[slash..]);
        }
        return base.trim_end_matches('/').to_string();
    }
    url.to_string()
}

fn urlencode(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                o.push(b as char)
            }
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

// --- Datasets CSV ---

struct DatasetPool {
    var_names: Vec<String>,
    rows: Vec<Vec<String>>,
    cursor: AtomicUsize,
    recycle: bool,
}

impl DatasetPool {
    fn load(ds: &Dataset, base_dir: &Path) -> std::io::Result<Self> {
        let path = if Path::new(&ds.path).is_absolute() {
            PathBuf::from(&ds.path)
        } else {
            base_dir.join(&ds.path)
        };
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(ds.delimiter as u8)
            .has_headers(ds.first_line_is_header)
            .flexible(true)
            .from_path(&path)?;
        let mut rows = Vec::new();
        for rec in rdr.records() {
            let rec = rec?;
            rows.push(rec.iter().map(|s| s.to_string()).collect());
        }
        Ok(Self {
            var_names: ds.variable_names.clone(),
            rows,
            cursor: AtomicUsize::new(0),
            recycle: ds.recycle,
        })
    }

    fn next(&self) -> Option<Vec<(String, String)>> {
        if self.rows.is_empty() {
            return None;
        }
        let i = self.cursor.fetch_add(1, Ordering::Relaxed);
        let idx = if self.recycle {
            i % self.rows.len()
        } else if i < self.rows.len() {
            i
        } else {
            return None;
        };
        let row = &self.rows[idx];
        Some(
            self.var_names
                .iter()
                .enumerate()
                .map(|(j, name)| (name.clone(), row.get(j).cloned().unwrap_or_default()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_replaces_known_and_keeps_unknown() {
        let mut vars = HashMap::new();
        vars.insert("host".to_string(), "example.test".to_string());
        assert_eq!(
            interpolate("https://${host}/x", &vars),
            "https://example.test/x"
        );
        assert_eq!(interpolate("${missing}", &vars), "${missing}");
    }

    #[test]
    fn condition_eval() {
        let ctx = Ctx {
            vars: HashMap::from([("a".to_string(), "1".to_string())]),
            failures: 0,
            interleave: HashMap::new(),
        };
        assert!(eval_condition("true", &ctx));
        assert!(!eval_condition("false", &ctx));
        assert!(eval_condition("${a} == 1", &ctx));
        assert!(!eval_condition("${a} == 2", &ctx));
        assert!(eval_condition("${a} != 2", &ctx));
    }

    #[test]
    fn boundary_extraction() {
        assert_eq!(
            extract_boundary("xx[token=ABC]yy", "token=", "]"),
            Some("ABC".to_string())
        );
        assert_eq!(extract_boundary("nope", "a", "b"), None);
    }

    #[test]
    fn url_helpers() {
        assert_eq!(join_url("https://h.test/", "/a"), "https://h.test/a");
        assert_eq!(join_url("https://h.test", "a"), "https://h.test/a");
        assert_eq!(
            override_authority("http://127.0.0.1:9", "https://prod.test/a?b=1"),
            "http://127.0.0.1:9/a?b=1"
        );
    }
}
