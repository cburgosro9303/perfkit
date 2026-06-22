//! perfkit Studio — shell Tauri. Los comandos llaman directamente a los crates del
//! engine (sin servidor HTTP) y transmiten métricas en vivo por eventos Tauri.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use metrics::RunSummary;
use scenario_ir::model::*;
use scenario_ir::{MigrationReport, ValidationReport};

#[derive(Serialize)]
struct ImportResult {
    scenario: Scenario,
    report: MigrationReport,
    yaml: String,
}

struct AppState {
    stop: Arc<AtomicBool>,
}

#[tauri::command]
fn import_jmx(path: String) -> Result<ImportResult, String> {
    let xml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    import_from_xml(&xml, &path)
}

#[tauri::command]
fn import_jmx_content(name: String, xml: String) -> Result<ImportResult, String> {
    import_from_xml(&xml, &name)
}

fn import_from_xml(xml: &str, source: &str) -> Result<ImportResult, String> {
    let (scenario, report) = jmx_importer::import_jmx(xml, source).map_err(|e| e.to_string())?;
    let yaml = scenario_ir::to_yaml(&scenario).map_err(|e| e.to_string())?;
    Ok(ImportResult { scenario, report, yaml })
}

/// Carga un ejemplo **ejecutable** empaquetado en el binario. Apunta a `httpbin.org`,
/// así que se puede correr de verdad (a diferencia del demo de navegador, que es
/// simulado y apunta a un dominio ficticio).
#[tauri::command]
fn example_import() -> Result<ImportResult, String> {
    let xml = include_str!("../../../examples/jmx/checkout-demo.jmx");
    import_from_xml(xml, "checkout-demo.jmx")
}

#[tauri::command]
fn validate_scenario(scenario: Scenario) -> ValidationReport {
    scenario_ir::validate(&scenario)
}

#[tauri::command]
fn scenario_to_yaml(scenario: Scenario) -> Result<String, String> {
    scenario_ir::to_yaml(&scenario).map_err(|e| e.to_string())
}

/// Exporta el escenario al `path` dado en el formato indicado:
/// `yaml` | `json` | `jmx` (Apache JMeter) | `pkb` (binario compacto).
#[tauri::command]
fn export_scenario(scenario: Scenario, format: String, path: String) -> Result<(), String> {
    let bytes: Vec<u8> = match format.as_str() {
        "yaml" => scenario_ir::to_yaml(&scenario).map_err(|e| e.to_string())?.into_bytes(),
        "json" => scenario_ir::to_json_pretty(&scenario).map_err(|e| e.to_string())?.into_bytes(),
        "jmx" => jmx_importer::export_jmx(&scenario).into_bytes(),
        "pkb" => scenario_ir::to_pkb(&scenario).map_err(|e| e.to_string())?,
        other => return Err(format!("formato no soportado: {other}")),
    };
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
fn new_scenario() -> Scenario {
    let mut s = Scenario::new("Nuevo escenario");
    s.defaults = Some(HttpDefaults {
        base_url: Some("https://httpbin.org".into()),
        ..Default::default()
    });
    s.thread_groups.push(ThreadGroup {
        name: "Usuarios".into(),
        load: LoadProfile {
            virtual_users: 10,
            ramp_up_secs: 3,
            hold_secs: 0,
            ramp_down_secs: 0,
            iterations: Some(20),
            duration_secs: None,
        },
        on_error: OnError::Continue,
        steps: vec![Step::Http(HttpRequest {
            name: "GET /get".into(),
            method: HttpMethod::Get,
            url: "/get".into(),
            headers: BTreeMap::from([("Accept".into(), "application/json".into())]),
            query: BTreeMap::new(),
            body: None,
            follow_redirects: Some(true),
            timeout_ms: Some(10_000),
            timers: vec![Timer::Constant { delay_ms: 100 }],
            assertions: vec![Assertion::StatusCode { codes: vec![200] }],
            extractors: vec![],
        })],
    });
    s
}

#[tauri::command]
async fn run_scenario(
    app: AppHandle,
    scenario: Scenario,
    base_url: Option<String>,
    vus: Option<u32>,
    duration_secs: Option<u64>,
    capture: Option<bool>,
    capture_plaintext: Option<bool>,
    capture_limit: Option<usize>,
) -> Result<(), String> {
    let mut scenario = scenario;
    apply_overrides(&mut scenario, vus, duration_secs);

    let stop = app.state::<AppState>().stop.clone();
    stop.store(false, Ordering::Relaxed);

    let base_dir = scenario
        .metadata
        .source
        .as_deref()
        .and_then(|s| PathBuf::from(s).parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let app2 = app.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(s) = rx.recv().await {
            let _ = app2.emit("run-metrics", s);
        }
    });

    let opts = engine::RunOptions {
        run_id: "studio".into(),
        base_url_override: base_url,
        capture: capture.unwrap_or(false),
        // 0 ⇒ tope por defecto seguro del engine (5000); la UI puede subirlo.
        capture_limit: capture_limit.unwrap_or(0),
        capture_plaintext: capture_plaintext.unwrap_or(false),
    };
    let summary = engine::run(&scenario, opts, &base_dir, Some(tx), stop).await;
    let _ = forwarder.await;
    app.emit("run-finished", &summary).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn cancel_run(state: State<'_, AppState>) {
    state.stop.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn export_report(summary: RunSummary) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("perfkit-{}", summary.run_id));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("summary.json"), reports::summary_json(&summary)).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("report.html"), reports::html_report(&summary)).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("report.junit.xml"), reports::junit_xml(&summary)).map_err(|e| e.to_string())?;
    Ok(dir.display().to_string())
}

/// Carga un `summary.json` producido por `perfkit run --out <dir>` (o por el benchmark)
/// para visualizarlo en la UI. Valida que sea un `RunSummary` real.
#[tauri::command]
fn load_summary(path: String) -> Result<RunSummary, String> {
    let txt = std::fs::read_to_string(&path).map_err(|e| format!("no se pudo leer {path}: {e}"))?;
    serde_json::from_str::<RunSummary>(&txt).map_err(|e| format!("summary.json inválido: {e}"))
}

// --- Quality gate ---

#[tauri::command]
fn evaluate_gate(summary: RunSummary, thresholds: reports::Thresholds) -> reports::GateResult {
    reports::evaluate_gate(&summary, &thresholds)
}

// --- Histórico (Fase 10) ---

fn history_db() -> Result<history::Store, String> {
    history::Store::open(std::path::Path::new("perfkit-history.db")).map_err(|e| e.to_string())
}

#[tauri::command]
fn history_record(
    summary: RunSummary,
    branch: Option<String>,
    build: Option<String>,
    environment: Option<String>,
    commit: Option<String>,
) -> Result<i64, String> {
    let store = history_db()?;
    let meta = history::RunMeta { branch, build, environment, commit, actor: Some("studio".into()) };
    store.record_run(&summary, &meta).map_err(|e| e.to_string())
}

#[tauri::command]
fn history_list(
    scenario: Option<String>,
    environment: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<history::RunRecord>, String> {
    history_db()?
        .list_runs(scenario.as_deref(), environment.as_deref(), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn history_set_baseline(
    branch: String,
    environment: String,
    scenario: String,
    run_id: i64,
) -> Result<(), String> {
    history_db()?
        .set_baseline(&branch, &environment, &scenario, run_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn history_compare(
    run_id: i64,
    branch: String,
    environment: String,
    scenario: String,
) -> Result<Option<history::Comparison>, String> {
    history_db()?
        .compare_to_baseline(run_id, &branch, &environment, &scenario)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn history_trend(
    scenario: String,
    environment: String,
    metric: String,
    limit: Option<usize>,
) -> Result<Vec<history::TrendPoint>, String> {
    let m = match metric.as_str() {
        "throughput" => history::Metric::Throughput,
        "error_rate" => history::Metric::ErrorRate,
        _ => history::Metric::P95,
    };
    history_db()?
        .trend(&scenario, &environment, m, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

fn apply_overrides(scenario: &mut Scenario, vus: Option<u32>, duration_secs: Option<u64>) {
    for g in &mut scenario.thread_groups {
        if let Some(v) = vus {
            g.load.virtual_users = v.max(1);
        }
        if let Some(d) = duration_secs {
            g.load.duration_secs = Some(d);
            g.load.iterations = None;
            g.load.hold_secs = d;
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            stop: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            import_jmx,
            import_jmx_content,
            example_import,
            validate_scenario,
            scenario_to_yaml,
            export_scenario,
            new_scenario,
            run_scenario,
            cancel_run,
            export_report,
            load_summary,
            evaluate_gate,
            history_record,
            history_list,
            history_set_baseline,
            history_compare,
            history_trend
        ])
        .run(tauri::generate_context!())
        .expect("error iniciando perfkit Studio");
}
