//! `perfkit` — interfaz de línea de comandos.
//!
//! Flujo objetivo (§16): importar JMX → validar → ejecutar local → reporte → gate en CI.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use scenario_ir::migration::{MappingStatus, MigrationReport};
use scenario_ir::model::Scenario;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(
    name = "perfkit",
    version,
    about = "perfkit — performance testing moderno, reemplazo de Apache JMeter"
)]
struct Cli {
    /// Logs detallados.
    #[arg(long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Crea un escenario de ejemplo.
    Init {
        #[arg(default_value = "mi-escenario")]
        name: String,
        #[arg(short, long, default_value = "scenario.yaml")]
        out: PathBuf,
    },
    /// Valida un escenario (schema + reglas semánticas).
    Validate { scenario: PathBuf },
    /// Importa un plan externo al IR.
    Import {
        #[command(subcommand)]
        src: ImportSrc,
    },
    /// Convierte un JMX mostrando el reporte de fidelidad completo.
    Convert {
        #[command(subcommand)]
        src: ImportSrc,
    },
    /// Ejecuta un escenario de carga.
    Run {
        scenario: PathBuf,
        /// Formatos de reporte: html, json, junit (por defecto: todos).
        #[arg(long, value_name = "FORMATO")]
        report: Vec<String>,
        /// Directorio de salida (por defecto reports/<run-id>).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Redirige todas las peticiones a este origen (esquema://host:puerto).
        #[arg(long)]
        base_url: Option<String>,
        /// Sobrescribe el número de VUs por grupo.
        #[arg(long)]
        vus: Option<u32>,
        /// Ejecuta por esta duración en segundos (ignora iterations).
        #[arg(long)]
        duration: Option<u64>,
    },
    /// Corrida corta de depuración que captura y muestra cada petición (request/response).
    Debug {
        scenario: PathBuf,
        #[arg(long, default_value_t = 1)]
        vus: u32,
        #[arg(long, default_value_t = 1)]
        iterations: u64,
        #[arg(long)]
        base_url: Option<String>,
        /// Muestra cabeceras/variables en texto plano (sin redactar secretos).
        #[arg(long)]
        no_redact: bool,
    },
    /// Compara un summary.json contra umbrales (quality gate de CI).
    Gate {
        summary: PathBuf,
        #[arg(long)]
        thresholds: PathBuf,
    },
    /// Genera los JSON Schema (scenario + reporte de fidelidad).
    Schema {
        #[arg(short, long, default_value = "schemas")]
        out: PathBuf,
    },
    /// Ejecución distribuida (coordinator/worker).
    Cluster {
        #[command(subcommand)]
        sub: ClusterCmd,
    },
    /// Histórico de runs, baselines y tendencias (enterprise).
    History {
        #[command(subcommand)]
        sub: HistoryCmd,
    },
    /// Asistente de IA gobernada (local; SaaS apagado por defecto).
    Ai {
        #[command(subcommand)]
        sub: AiCmd,
    },
    /// Plugins WASM firmados.
    Plugin {
        #[command(subcommand)]
        sub: PluginCmd,
    },
    /// Exporta un escenario a otro formato (jmx para JMeter, o pkb binario eficiente).
    Export {
        /// Entrada: .yaml/.json/.pkb del IR, o un .jmx (se importa primero).
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = ExportFormat::Jmx)]
        format: ExportFormat,
        #[arg(short, long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExportFormat {
    /// IR legible (YAML).
    Yaml,
    /// IR como JSON.
    Json,
    /// Apache JMeter (.jmx) — para abrir/ejecutar en JMeter.
    Jmx,
    /// Formato binario compacto de perfkit (.pkb, MessagePack).
    Pkb,
}

#[derive(Subcommand)]
enum ClusterCmd {
    /// Arranca un worker (data plane).
    Worker {
        #[arg(long, default_value_t = 7711)]
        port: u16,
    },
    /// Coordina una ejecución distribuida sobre varios workers.
    Run {
        scenario: PathBuf,
        /// URLs de workers separadas por coma (http://host:puerto).
        #[arg(long, value_delimiter = ',')]
        workers: Vec<String>,
        #[arg(long, default_value_t = 50)]
        vus: u32,
        #[arg(long, default_value_t = 30)]
        duration: u64,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum HistoryCmd {
    /// Registra un summary.json en el histórico.
    Record {
        summary: PathBuf,
        #[arg(long, default_value = "perfkit-history.db")]
        db: PathBuf,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        environment: Option<String>,
        #[arg(long)]
        build: Option<String>,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Lista runs históricos.
    List {
        #[arg(long, default_value = "perfkit-history.db")]
        db: PathBuf,
        #[arg(long)]
        scenario: Option<String>,
        #[arg(long)]
        environment: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Fija una baseline (branch/environment/scenario → run).
    Baseline {
        #[arg(long, default_value = "perfkit-history.db")]
        db: PathBuf,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        environment: String,
        #[arg(long)]
        scenario: String,
        #[arg(long)]
        run_id: i64,
    },
    /// Compara un run contra su baseline (detecta regresión, exit 1).
    Compare {
        #[arg(long, default_value = "perfkit-history.db")]
        db: PathBuf,
        #[arg(long)]
        run_id: i64,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        environment: String,
        #[arg(long)]
        scenario: String,
    },
}

#[derive(Subcommand)]
enum AiCmd {
    /// Explica un resultado en lenguaje natural (local).
    Explain { summary: PathBuf },
    /// Propone umbrales para CI (revisable, no se aplica solo).
    Thresholds { summary: PathBuf },
    /// Previsualiza exactamente qué se enviaría a una IA (redactado).
    Preview {
        text: String,
        #[arg(long)]
        saas: bool,
    },
}

#[derive(Subcommand)]
enum PluginCmd {
    /// SHA-256 de un .wasm (para el manifiesto).
    Sha256 { wasm: PathBuf },
    /// Verifica firma + hash + ABI de un plugin contra una clave pública (hex).
    Verify {
        wasm: PathBuf,
        manifest: PathBuf,
        #[arg(long)]
        pubkey: String,
    },
}

#[derive(Subcommand)]
enum ImportSrc {
    /// Importa un archivo Apache JMeter (.jmx).
    Jmx {
        input: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        fidelity_report: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    match dispatch(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

async fn dispatch(cli: Cli) -> Result<ExitCode> {
    match cli.cmd {
        Cmd::Init { name, out } => cmd_init(&name, &out),
        Cmd::Validate { scenario } => cmd_validate(&scenario),
        Cmd::Import {
            src:
                ImportSrc::Jmx {
                    input,
                    out,
                    fidelity_report,
                },
        } => cmd_import(&input, out.as_deref(), fidelity_report),
        Cmd::Convert {
            src: ImportSrc::Jmx { input, out, .. },
        } => cmd_import(&input, out.as_deref(), true),
        Cmd::Run {
            scenario,
            report,
            out,
            base_url,
            vus,
            duration,
        } => cmd_run(&scenario, &report, out.as_deref(), base_url, vus, duration).await,
        Cmd::Debug {
            scenario,
            vus,
            iterations,
            base_url,
            no_redact,
        } => cmd_debug(&scenario, vus, iterations, base_url, no_redact).await,
        Cmd::Gate {
            summary,
            thresholds,
        } => cmd_gate(&summary, &thresholds),
        Cmd::Schema { out } => cmd_schema(&out),
        Cmd::Cluster { sub } => cmd_cluster(sub).await,
        Cmd::History { sub } => cmd_history(sub),
        Cmd::Ai { sub } => cmd_ai(sub),
        Cmd::Plugin { sub } => cmd_plugin(sub),
        Cmd::Export { input, format, out } => cmd_export(&input, format, &out),
    }
}

fn cmd_export(input: &Path, format: ExportFormat, out: &Path) -> Result<ExitCode> {
    let scenario = load_scenario_any(input)?;
    let bytes: Vec<u8> = match format {
        ExportFormat::Yaml => scenario_ir::to_yaml(&scenario)?.into_bytes(),
        ExportFormat::Json => scenario_ir::to_json_pretty(&scenario)?.into_bytes(),
        ExportFormat::Jmx => jmx_importer::export_jmx(&scenario).into_bytes(),
        ExportFormat::Pkb => scenario_ir::to_pkb(&scenario)?,
    };
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out, &bytes)
        .with_context(|| format!("no se pudo escribir {}", out.display()))?;
    println!(
        "✔ exportado a {} ({:?}, {} bytes)",
        out.display(),
        format,
        bytes.len()
    );
    if matches!(format, ExportFormat::Jmx) {
        println!("  ábrelo en JMeter: jmeter -t {}", out.display());
    }
    Ok(ExitCode::SUCCESS)
}

/// Carga un escenario desde cualquier formato soportado (por extensión).
fn load_scenario_any(path: &Path) -> Result<Scenario> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("jmx") => {
            let xml = std::fs::read_to_string(path)
                .with_context(|| format!("no se pudo leer {}", path.display()))?;
            Ok(jmx_importer::import_jmx(&xml, &path.display().to_string())?.0)
        }
        Some("pkb") => {
            let bytes = std::fs::read(path)
                .with_context(|| format!("no se pudo leer {}", path.display()))?;
            Ok(scenario_ir::from_pkb(&bytes)?)
        }
        _ => load_scenario(path),
    }
}

async fn cmd_cluster(sub: ClusterCmd) -> Result<ExitCode> {
    match sub {
        ClusterCmd::Worker { port } => {
            let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse()?;
            println!("▶ perfkit worker en http://{addr} (Ctrl-C para salir)");
            cluster::serve_worker(addr).await?;
            Ok(ExitCode::SUCCESS)
        }
        ClusterCmd::Run {
            scenario,
            workers,
            vus,
            duration,
            base_url,
            out,
        } => {
            if workers.is_empty() {
                anyhow::bail!("indica --workers http://host:puerto,http://...");
            }
            let s = load_scenario(&scenario)?;
            println!(
                "▶ distribuyendo {vus} VUs en {} worker(s) por {duration}s …",
                workers.len()
            );
            let res = cluster::run_distributed(&s, vus, duration, base_url, &workers).await;
            for w in &res.workers {
                println!(
                    "  {} {} (vus {}){}",
                    if w.ok { "✔" } else { "✗" },
                    w.url,
                    w.vus,
                    w.error
                        .as_ref()
                        .map(|e| format!(" — {e}"))
                        .unwrap_or_default()
                );
            }
            print_summary(&res.combined);
            if let Some(dir) = out {
                std::fs::create_dir_all(&dir)?;
                std::fs::write(
                    dir.join("summary.json"),
                    reports::summary_json(&res.combined),
                )?;
                std::fs::write(dir.join("report.html"), reports::html_report(&res.combined))?;
                println!("\n✔ reporte consolidado en {}", dir.display());
            }
            Ok(if res.workers.iter().any(|w| w.ok) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
    }
}

fn cmd_history(sub: HistoryCmd) -> Result<ExitCode> {
    use history::{RunMeta, Store};
    match sub {
        HistoryCmd::Record {
            summary,
            db,
            branch,
            environment,
            build,
            commit,
        } => {
            let s: metrics::RunSummary = serde_json::from_str(&std::fs::read_to_string(&summary)?)?;
            let store = Store::open(&db)?;
            let meta = RunMeta {
                branch,
                build,
                environment,
                commit,
                actor: None,
            };
            let id = store.record_run(&s, &meta)?;
            println!("✔ run #{id} registrado en {}", db.display());
            Ok(ExitCode::SUCCESS)
        }
        HistoryCmd::List {
            db,
            scenario,
            environment,
            limit,
        } => {
            let store = Store::open(&db)?;
            let runs = store.list_runs(scenario.as_deref(), environment.as_deref(), limit)?;
            println!(
                "{:<5} {:<24} {:<10} {:>9} {:>9} {:>8}",
                "id", "escenario", "env", "thr/s", "p95ms", "err%"
            );
            for r in runs {
                println!(
                    "{:<5} {:<24} {:<10} {:>9.1} {:>9.1} {:>7.2}%",
                    r.id,
                    trunc(&r.scenario, 24),
                    r.environment.clone().unwrap_or_default(),
                    r.throughput,
                    r.p95_ms,
                    r.error_rate * 100.0
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        HistoryCmd::Baseline {
            db,
            branch,
            environment,
            scenario,
            run_id,
        } => {
            let store = Store::open(&db)?;
            store.set_baseline(&branch, &environment, &scenario, run_id)?;
            println!("✔ baseline: {branch}/{environment}/{scenario} → run #{run_id}");
            Ok(ExitCode::SUCCESS)
        }
        HistoryCmd::Compare {
            db,
            run_id,
            branch,
            environment,
            scenario,
        } => {
            let store = Store::open(&db)?;
            match store.compare_to_baseline(run_id, &branch, &environment, &scenario)? {
                Some(c) => {
                    println!(
                        "p95 {:+.1}% · throughput {:+.1}% · error_rate {:+.4}",
                        c.p95_delta_pct, c.throughput_delta_pct, c.error_rate_delta
                    );
                    if c.is_regression {
                        println!("✗ REGRESIÓN detectada");
                        Ok(ExitCode::from(1))
                    } else {
                        println!("✔ sin regresión");
                        Ok(ExitCode::SUCCESS)
                    }
                }
                None => {
                    println!("no hay baseline para {branch}/{environment}/{scenario}");
                    Ok(ExitCode::SUCCESS)
                }
            }
        }
    }
}

fn cmd_ai(sub: AiCmd) -> Result<ExitCode> {
    use ai_assist::{AiConfig, AiMode, explain_results, preview_payload, suggest_thresholds};
    match sub {
        AiCmd::Explain { summary } => {
            let s: metrics::RunSummary = serde_json::from_str(&std::fs::read_to_string(&summary)?)?;
            let sug = explain_results(&s);
            println!("{}\n\n{}", sug.title, sug.detail);
            Ok(ExitCode::SUCCESS)
        }
        AiCmd::Thresholds { summary } => {
            let s: metrics::RunSummary = serde_json::from_str(&std::fs::read_to_string(&summary)?)?;
            let sug = suggest_thresholds(&s);
            println!("{} (revisable — no se aplica automáticamente):", sug.title);
            println!("{}", serde_json::to_string_pretty(&sug.proposal)?);
            Ok(ExitCode::SUCCESS)
        }
        AiCmd::Preview { text, saas } => {
            let cfg = AiConfig {
                mode: if saas { AiMode::Saas } else { AiMode::Local },
                allow_saas: false,
                redact: true,
                allowlist: vec![],
            };
            let p = preview_payload(&text, &cfg);
            println!(
                "modo {:?} · ¿saldría del proceso?: {} · {} bytes",
                p.mode, p.would_send, p.bytes
            );
            println!(
                "contenido (redactado) que se enviaría:\n{}",
                p.redacted_content
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cmd_plugin(sub: PluginCmd) -> Result<ExitCode> {
    use plugin_host::{PluginHost, PluginManifest, sha256_hex};
    match sub {
        PluginCmd::Sha256 { wasm } => {
            println!("{}", sha256_hex(&std::fs::read(&wasm)?));
            Ok(ExitCode::SUCCESS)
        }
        PluginCmd::Verify {
            wasm,
            manifest,
            pubkey,
        } => {
            let bytes = std::fs::read(&wasm)?;
            let man: PluginManifest = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
            let raw = hex::decode(pubkey.trim()).context("pubkey debe ser hex")?;
            let arr: [u8; 32] = raw
                .as_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("pubkey debe ser 32 bytes"))?;
            let vk =
                ed25519_dalek::VerifyingKey::from_bytes(&arr).context("pubkey ed25519 inválida")?;
            match PluginHost::new().load_signed(&bytes, &man, &[vk]) {
                Ok(_) => {
                    println!(
                        "✔ plugin '{}' v{} válido (firma + hash + ABI OK)",
                        man.name, man.version
                    );
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => {
                    println!("✗ rechazado: {e}");
                    Ok(ExitCode::from(1))
                }
            }
        }
    }
}

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// --- Comandos ---

fn cmd_init(name: &str, out: &Path) -> Result<ExitCode> {
    use scenario_ir::model::*;
    use std::collections::BTreeMap;
    let mut s = Scenario::new(name);
    s.defaults = Some(HttpDefaults {
        base_url: Some("https://httpbin.org".into()),
        ..Default::default()
    });
    s.thread_groups.push(ThreadGroup {
        name: "Usuarios".into(),
        load: LoadProfile {
            virtual_users: 5,
            ramp_up_secs: 2,
            hold_secs: 0,
            ramp_down_secs: 0,
            iterations: Some(10),
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
    let yaml = scenario_ir::to_yaml(&s)?;
    std::fs::write(out, yaml).with_context(|| format!("no se pudo escribir {}", out.display()))?;
    println!("✔ escenario de ejemplo creado en {}", out.display());
    println!(
        "  siguiente: perfkit validate {0} && perfkit run {0}",
        out.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_validate(path: &Path) -> Result<ExitCode> {
    let s = load_scenario(path)?;
    let report = scenario_ir::validate(&s);
    for i in &report.issues {
        let tag = match i.severity {
            scenario_ir::Severity::Error => "ERROR",
            scenario_ir::Severity::Warning => "warn ",
        };
        println!("  [{tag}] {} — {}", i.path, i.message);
    }
    if report.is_ok() {
        println!(
            "✔ {} válido ({} advertencia(s))",
            path.display(),
            report.warnings()
        );
        Ok(ExitCode::SUCCESS)
    } else {
        println!(
            "✗ {} inválido: {} error(es)",
            path.display(),
            report.errors()
        );
        Ok(ExitCode::from(1))
    }
}

fn cmd_import(input: &Path, out: Option<&Path>, show_full: bool) -> Result<ExitCode> {
    let xml = std::fs::read_to_string(input)
        .with_context(|| format!("no se pudo leer {}", input.display()))?;
    let (scenario, report) =
        jmx_importer::import_jmx(&xml, &input.display().to_string()).context("importando JMX")?;

    let out_yaml = out
        .map(PathBuf::from)
        .unwrap_or_else(|| input.with_extension("yaml"));
    if let Some(parent) = out_yaml.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&out_yaml, scenario_ir::to_yaml(&scenario)?)
        .with_context(|| format!("no se pudo escribir {}", out_yaml.display()))?;
    let fid_path = out_yaml.with_extension("fidelity.json");
    std::fs::write(&fid_path, serde_json::to_string_pretty(&report)?)?;

    println!("✔ IR escrito en {}", out_yaml.display());
    println!("✔ reporte de fidelidad en {}", fid_path.display());
    print_fidelity(&report, show_full);
    Ok(ExitCode::SUCCESS)
}

async fn cmd_run(
    path: &Path,
    formats: &[String],
    out: Option<&Path>,
    base_url: Option<String>,
    vus: Option<u32>,
    duration: Option<u64>,
) -> Result<ExitCode> {
    let mut scenario = load_scenario(path)?;
    let v = scenario_ir::validate(&scenario);
    if !v.is_ok() {
        for i in v
            .issues
            .iter()
            .filter(|i| i.severity == scenario_ir::Severity::Error)
        {
            eprintln!("  [ERROR] {} — {}", i.path, i.message);
        }
        anyhow::bail!("el escenario tiene {} error(es) de validación", v.errors());
    }
    apply_overrides(&mut scenario, vus, duration);

    let run_id = new_run_id();
    let out_dir = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("reports").join(&run_id));
    std::fs::create_dir_all(&out_dir)?;
    let base_dir = path.parent().map(PathBuf::from).unwrap_or_default();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let printer = tokio::spawn(live_printer(rx));
    let stop = Arc::new(AtomicBool::new(false));
    install_ctrl_c(stop.clone());

    println!("▶ ejecutando '{}' …", scenario.name);
    let opts = engine::RunOptions {
        run_id: run_id.clone(),
        base_url_override: base_url,
        ..Default::default()
    };
    let summary = engine::run(&scenario, opts, &base_dir, Some(tx), stop).await;
    let _ = printer.await;
    eprintln!();

    write_reports(&summary, &out_dir, formats)?;
    print_summary(&summary);
    println!("\n✔ reportes en {}", out_dir.display());
    Ok(ExitCode::SUCCESS)
}

async fn cmd_debug(
    path: &Path,
    vus: u32,
    iterations: u64,
    base_url: Option<String>,
    no_redact: bool,
) -> Result<ExitCode> {
    let mut scenario = load_scenario(path)?;
    for g in &mut scenario.thread_groups {
        g.load.virtual_users = vus.max(1);
        g.load.iterations = Some(iterations.max(1));
        g.load.duration_secs = None;
        g.load.ramp_up_secs = 0;
    }
    let base_dir = path.parent().map(PathBuf::from).unwrap_or_default();
    let stop = Arc::new(AtomicBool::new(false));
    println!(
        "▶ depuración: {} VU(s) × {} iteración(es) de «{}» …",
        vus.max(1),
        iterations.max(1),
        scenario.name
    );
    let opts = engine::RunOptions {
        run_id: "debug".into(),
        base_url_override: base_url,
        capture: true,
        capture_limit: 0, // sin tope: captura todas en orden
        capture_plaintext: no_redact,
    };
    let summary = engine::run(&scenario, opts, &base_dir, None, stop).await;
    print_details(&summary);
    print_summary(&summary);
    Ok(ExitCode::SUCCESS)
}

fn print_details(s: &metrics::RunSummary) {
    if s.details.is_empty() {
        return;
    }
    println!(
        "\n── Peticiones capturadas ({}) ───────────────────────",
        s.details.len()
    );
    for d in &s.details {
        let mark = if d.success { "✔" } else { "✗" };
        let status = d
            .status
            .map(|c| c.to_string())
            .unwrap_or_else(|| "—".into());
        println!(
            "\n#{} {mark} {} {} → {status} ({:.1} ms, {} B)",
            d.seq + 1,
            d.method,
            d.url,
            d.latency_ms,
            d.bytes
        );
        if let Some(err) = &d.error {
            println!("   error: {err}");
        }
        for (k, v) in &d.req_headers {
            println!("   » {k}: {v}");
        }
        if let Some(b) = &d.req_body
            && !b.is_empty()
        {
            println!("   » body: {}", oneline(b));
        }
        if !d.resp_body.is_empty() {
            println!("   « resp: {}", oneline(&d.resp_body));
        }
        for (k, v) in &d.extracted {
            println!("   ⮑ extraída {k} = {v}");
        }
        if !d.vars.is_empty() {
            let joined = d
                .vars
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("   vars: {joined}");
        }
    }
}

fn oneline(s: &str) -> String {
    let one = s.replace('\n', " ");
    let t: String = one.chars().take(400).collect();
    if t.len() < one.len() {
        format!("{t}…")
    } else {
        t
    }
}

fn cmd_gate(summary_path: &Path, thresholds_path: &Path) -> Result<ExitCode> {
    let summary: metrics::RunSummary = serde_json::from_str(
        &std::fs::read_to_string(summary_path)
            .with_context(|| format!("no se pudo leer {}", summary_path.display()))?,
    )
    .context("parseando summary.json")?;
    let thresholds = reports::load_thresholds(
        &std::fs::read_to_string(thresholds_path)
            .with_context(|| format!("no se pudo leer {}", thresholds_path.display()))?,
    )
    .context("parseando thresholds.yaml")?;

    let result = reports::evaluate_gate(&summary, &thresholds);
    println!("Quality gate:");
    for c in &result.checks {
        println!("  {} {}", if c.passed { "✔" } else { "✗" }, c.detail);
    }
    if result.passed {
        println!("✔ gate OK");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("✗ gate FALLÓ");
        Ok(ExitCode::from(1))
    }
}

fn cmd_schema(out: &Path) -> Result<ExitCode> {
    std::fs::create_dir_all(out)?;
    let s = out.join("scenario-ir.schema.json");
    let m = out.join("migration-report.schema.json");
    std::fs::write(
        &s,
        serde_json::to_string_pretty(&scenario_ir::scenario_schema())?,
    )?;
    std::fs::write(
        &m,
        serde_json::to_string_pretty(&scenario_ir::migration_report_schema())?,
    )?;
    println!("✔ {}", s.display());
    println!("✔ {}", m.display());
    Ok(ExitCode::SUCCESS)
}

// --- Helpers ---

fn load_scenario(path: &Path) -> Result<Scenario> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("no se pudo leer {}", path.display()))?;
    let is_json = path.extension().map(|e| e == "json").unwrap_or(false);
    if is_json {
        Ok(scenario_ir::from_json(&text)?)
    } else {
        Ok(scenario_ir::from_yaml(&text)?)
    }
}

fn apply_overrides(scenario: &mut Scenario, vus: Option<u32>, duration: Option<u64>) {
    for g in &mut scenario.thread_groups {
        if let Some(v) = vus {
            g.load.virtual_users = v.max(1);
        }
        if let Some(d) = duration {
            g.load.duration_secs = Some(d);
            g.load.iterations = None;
            g.load.hold_secs = d;
        }
    }
}

fn write_reports(summary: &metrics::RunSummary, out_dir: &Path, formats: &[String]) -> Result<()> {
    let all = formats.is_empty() || formats.iter().any(|f| f == "all");
    let want = |f: &str| all || formats.iter().any(|x| x == f);

    // summary.json siempre (machine-readable).
    std::fs::write(out_dir.join("summary.json"), reports::summary_json(summary))?;
    if want("html") {
        std::fs::write(out_dir.join("report.html"), reports::html_report(summary))?;
    }
    if want("junit") {
        std::fs::write(
            out_dir.join("report.junit.xml"),
            reports::junit_xml(summary),
        )?;
    }
    Ok(())
}

async fn live_printer(mut rx: tokio::sync::mpsc::UnboundedReceiver<metrics::LiveSnapshot>) {
    use std::io::Write;
    while let Some(s) = rx.recv().await {
        eprint!(
            "\r  [{:>4.0}s] VUs {:>3} · req {:>6} · {:>6.0}/s · p95 {:>6.0}ms · err {:>5.1}%   ",
            s.elapsed_secs,
            s.active_vus,
            s.total_requests,
            s.throughput_per_sec,
            s.p95_ms,
            s.error_rate * 100.0
        );
        let _ = std::io::stderr().flush();
    }
}

fn print_summary(s: &metrics::RunSummary) {
    let o = &s.overall;
    println!("\n── Resumen ──────────────────────────────────────────");
    println!("escenario : {}", s.scenario_name);
    println!(
        "duración  : {:.1}s · VUs {} · {} requests",
        s.duration_secs, s.config.virtual_users, o.count
    );
    println!("throughput: {:.1} req/s", o.throughput_per_sec);
    println!("errores   : {} ({:.2}%)", o.errors, o.error_rate * 100.0);
    println!(
        "latencia  : p50 {:.0} · p90 {:.0} · p95 {:.0} · p99 {:.0} · max {:.0} (ms)",
        o.p50_ms, o.p90_ms, o.p95_ms, o.p99_ms, o.max_ms
    );
    if !s.labels.is_empty() {
        println!(
            "\n  {:<32} {:>7} {:>7} {:>9} {:>9}",
            "etiqueta", "#", "err", "p95(ms)", "p99(ms)"
        );
        for l in &s.labels {
            let name: String = l.label.chars().take(32).collect();
            println!(
                "  {:<32} {:>7} {:>7} {:>9.0} {:>9.0}",
                name, l.count, l.errors, l.p95_ms, l.p99_ms
            );
        }
    }
    if !s.errors.is_empty() {
        println!("\n  errores principales:");
        for e in s.errors.iter().take(5) {
            println!("   · {} ×{}", e.message, e.count);
        }
    }
}

fn print_fidelity(report: &MigrationReport, show_full: bool) {
    let s = &report.summary;
    println!("\n── Fidelidad de migración ───────────────────────────");
    println!(
        "total {} · migrados {} · asistidos {} · no soportados {} · ignorados {} · fidelidad {:.0}%",
        s.total, s.migrated, s.assisted, s.unsupported, s.ignored, s.fidelity_pct
    );
    let attention: Vec<_> = report
        .elements
        .iter()
        .filter(|e| {
            matches!(
                e.status,
                MappingStatus::Assisted | MappingStatus::Unsupported
            )
        })
        .collect();
    if !attention.is_empty() {
        println!("\n  requieren atención:");
        for e in &attention {
            println!("   [{}] {} ({})", e.status.as_str(), e.path, e.jmx_type);
            if let Some(r) = &e.reason {
                println!("        razón: {r}");
            }
            if let Some(sug) = &e.suggestion {
                println!("        sugerencia: {sug}");
            }
        }
    }
    if show_full {
        println!("\n  todos los elementos:");
        for e in &report.elements {
            println!("   [{}] {} ({})", e.status.as_str(), e.path, e.jmx_type);
        }
    }
}

fn new_run_id() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("run-{secs}")
}

fn install_ctrl_c(stop: Arc<AtomicBool>) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("\n⏹ cancelando…");
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
}

fn init_tracing(verbose: bool) {
    let level = if verbose { "debug" } else { "warn" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level))
        .with_target(false)
        .without_time()
        .try_init();
}
