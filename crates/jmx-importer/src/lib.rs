//! `jmx-importer` — convierte un plan Apache JMeter (`.jmx`) al IR canónico y produce
//! un **reporte de fidelidad**. Regla clave: **nunca falla en silencio**; cada elemento
//! queda `migrated | assisted | unsupported | ignored`.

use roxmltree::Node;
use scenario_ir::migration::{MappedElement, MappingStatus, MigrationReport};
use scenario_ir::model::*;
use std::collections::BTreeMap;

pub mod export;
pub use export::export_jmx;

const GENERATOR: &str = concat!("perfkit-jmx-importer ", env!("CARGO_PKG_VERSION"));

#[derive(thiserror::Error, Debug)]
pub enum ImportError {
    #[error("XML inválido: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("no se encontró un TestPlan en el JMX")]
    NoTestPlan,
}

/// Importa un JMX (contenido XML) y devuelve el escenario + reporte de fidelidad.
pub fn import_jmx(xml: &str, source: &str) -> Result<(Scenario, MigrationReport), ImportError> {
    let doc = roxmltree::Document::parse(xml)?;
    let root = doc.root_element();
    let top = root
        .children()
        .find(|c| c.has_tag_name("hashTree"))
        .ok_or(ImportError::NoTestPlan)?;

    let mut report = MigrationReport::new(source, GENERATOR);
    let mut scenario = Scenario::new("Imported Test Plan");
    scenario.metadata.generator = Some(GENERATOR.to_string());
    scenario.metadata.source = Some(source.to_string());

    let mut found_plan = false;
    for (el, ht) in pairs(top) {
        if el.has_tag_name("TestPlan") {
            found_plan = true;
            scenario.name = testname(el);
            for (k, v) in parse_arguments_under(el) {
                scenario.variables.insert(k, v);
            }
            add(
                &mut report,
                el,
                "Test Plan",
                MappingStatus::Migrated,
                Some("scenario"),
                None,
                None,
            );
            if let Some(ht) = ht {
                walk_plan(ht, &mut scenario, &mut report, "Test Plan");
            }
        } else {
            add(
                &mut report,
                el,
                "Test Plan",
                MappingStatus::Unsupported,
                None,
                Some("elemento inesperado en la raíz del plan"),
                None,
            );
        }
    }
    if !found_plan {
        return Err(ImportError::NoTestPlan);
    }

    report.recompute_summary();
    Ok((scenario, report))
}

/// Recorre el contenido del Test Plan: thread groups y configuración de plan.
fn walk_plan(ht: Node, scenario: &mut Scenario, report: &mut MigrationReport, path: &str) {
    for (el, child) in pairs(ht) {
        let name = el.tag_name().name();
        let p = format!("{path} > {}", testname(el));
        match name {
            "ThreadGroup" | "SetupThreadGroup" | "PostThreadGroup" => {
                let tg = build_thread_group(el, child, scenario, report, &p);
                scenario.thread_groups.push(tg);
            }
            "Arguments" => {
                for (k, v) in parse_arguments_under(el) {
                    scenario.variables.insert(k, v);
                }
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("scenario.variables"),
                    None,
                    None,
                );
            }
            "ConfigTestElement" => apply_config_element(el, scenario, report, path),
            "CSVDataSet" => {
                scenario.datasets.push(parse_csv(el));
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("scenario.datasets"),
                    None,
                    None,
                );
            }
            "HeaderManager" => {
                merge_default_headers(scenario, parse_headers(el));
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("scenario.defaults.headers"),
                    Some("Header Manager a nivel de plan mapeado como headers por defecto"),
                    None,
                );
            }
            "CookieManager" => add(
                report,
                el,
                path,
                MappingStatus::Migrated,
                None,
                Some("cookies habilitadas por VU en el engine"),
                None,
            ),
            "CacheManager" => add(
                report,
                el,
                path,
                MappingStatus::Ignored,
                None,
                Some("cache HTTP no relevante para generación de carga en el MVP"),
                None,
            ),
            "ResultCollector" => add(
                report,
                el,
                path,
                MappingStatus::Ignored,
                None,
                Some("listener: en perfkit el reporte es nativo"),
                None,
            ),
            _ => classify_unknown(el, child, scenario, report, path, &mut Vec::new()),
        }
    }
}

fn build_thread_group(
    el: Node,
    ht: Option<Node>,
    scenario: &mut Scenario,
    report: &mut MigrationReport,
    path: &str,
) -> ThreadGroup {
    let vus = prop_str(el, "ThreadGroup.num_threads")
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    let ramp = prop_str(el, "ThreadGroup.ramp_time")
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let main = child_prop(el, "ThreadGroup.main_controller");
    let loops = main
        .and_then(|m| prop_str(m, "LoopController.loops"))
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(1);
    let forever = main
        .and_then(|m| prop_bool(m, "LoopController.continue_forever"))
        .unwrap_or(false)
        || loops < 0;

    let scheduler = prop_bool(el, "ThreadGroup.scheduler").unwrap_or(false);
    let duration = prop_str(el, "ThreadGroup.duration").and_then(|s| s.trim().parse::<u64>().ok());

    let iterations = if forever {
        None
    } else {
        Some(loops.max(0) as u64)
    };
    let duration_secs = if scheduler {
        duration.filter(|d| *d > 0)
    } else {
        None
    };
    let load = LoadProfile {
        virtual_users: vus,
        ramp_up_secs: ramp,
        hold_secs: 0,
        ramp_down_secs: 0,
        iterations: if iterations.is_none() && duration_secs.is_none() {
            Some(1)
        } else {
            iterations
        },
        duration_secs,
    };

    add(
        report,
        el,
        path,
        MappingStatus::Migrated,
        Some("thread_group"),
        None,
        None,
    );

    let mut steps = Vec::new();
    if let Some(ht) = ht {
        walk_steps(ht, &mut steps, scenario, report, path);
    }
    ThreadGroup {
        name: testname(el),
        load,
        on_error: OnError::Continue,
        steps,
    }
}

/// Recorre los hijos de un grupo de hilos o controlador, produciendo pasos.
fn walk_steps(
    ht: Node,
    out: &mut Vec<Step>,
    scenario: &mut Scenario,
    report: &mut MigrationReport,
    path: &str,
) {
    for (el, child) in pairs(ht) {
        let name = el.tag_name().name();
        let p = format!("{path} > {}", testname(el));
        match name {
            "HTTPSamplerProxy" | "HTTPSampler" => {
                let req = build_http(el, child, report, &p);
                out.push(Step::Http(req));
            }
            "TransactionController" => {
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("transaction"),
                    None,
                    None,
                );
                out.push(Step::Transaction(Transaction {
                    name: testname(el),
                    steps: inner,
                }));
            }
            "LoopController" => {
                let count = prop_str(el, "LoopController.loops")
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .unwrap_or(1)
                    .max(0) as u64;
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("loop"),
                    None,
                    None,
                );
                out.push(Step::Loop(LoopController {
                    name: testname(el),
                    count,
                    steps: inner,
                }));
            }
            "IfController" => {
                let condition = prop_str(el, "IfController.condition").unwrap_or_default();
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                let (status, reason) = condition_support(&condition);
                add(report, el, path, status, Some("if"), reason, None);
                out.push(Step::If(IfController {
                    name: testname(el),
                    condition,
                    steps: inner,
                }));
            }
            "WhileController" => {
                let condition = prop_str(el, "WhileController.condition").unwrap_or_default();
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                let (status, reason) = condition_support(&condition);
                add(report, el, path, status, Some("while"), reason, None);
                out.push(Step::While(WhileController {
                    name: testname(el),
                    condition,
                    steps: inner,
                    max_iterations: 10_000,
                }));
            }
            "ThroughputController" => {
                let style = prop_str(el, "ThroughputController.style")
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .unwrap_or(0);
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                if style == 0 {
                    let percent = prop_str(el, "ThroughputController.percentThroughput")
                        .or_else(|| obj_prop(el, "ThroughputController.percentThroughput"))
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .unwrap_or(100.0);
                    add(
                        report,
                        el,
                        path,
                        MappingStatus::Migrated,
                        Some("throughput"),
                        None,
                        None,
                    );
                    out.push(Step::Throughput(ThroughputController {
                        name: testname(el),
                        percent,
                        steps: inner,
                    }));
                } else {
                    add(
                        report,
                        el,
                        path,
                        MappingStatus::Assisted,
                        Some("throughput"),
                        Some(
                            "modo 'total executions' del Throughput Controller no soportado; se ejecuta siempre",
                        ),
                        Some("usar porcentaje, o un Loop/condición para limitar ejecuciones"),
                    );
                    out.push(Step::Throughput(ThroughputController {
                        name: testname(el),
                        percent: 100.0,
                        steps: inner,
                    }));
                }
            }
            "InterleaveControl" => {
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("interleave"),
                    None,
                    None,
                );
                out.push(Step::Interleave(InterleaveController {
                    name: testname(el),
                    steps: inner,
                }));
            }
            "RandomController" => {
                let mut inner = Vec::new();
                if let Some(c) = child {
                    walk_steps(c, &mut inner, scenario, report, &p);
                }
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("random"),
                    None,
                    None,
                );
                out.push(Step::Random(RandomController {
                    name: testname(el),
                    steps: inner,
                }));
            }
            "GenericController" => {
                // Simple Controller: solo agrupa; aplanamos sus hijos.
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("(aplanado)"),
                    None,
                    None,
                );
                if let Some(c) = child {
                    walk_steps(c, out, scenario, report, &p);
                }
            }
            "OnceOnlyController" => {
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Assisted,
                    Some("(aplanado)"),
                    Some("Once Only: en perfkit se ejecuta en cada iteración"),
                    Some("envolver en lógica condicional si se requiere ejecutar solo una vez"),
                );
                if let Some(c) = child {
                    walk_steps(c, out, scenario, report, &p);
                }
            }
            "ConstantTimer"
            | "UniformRandomTimer"
            | "GaussianRandomTimer"
            | "ConstantThroughputTimer" => {
                if let Some(t) = parse_timer(el) {
                    add(
                        report,
                        el,
                        path,
                        MappingStatus::Migrated,
                        Some("timer"),
                        None,
                        None,
                    );
                    out.push(Step::Timer(t));
                }
            }
            "Arguments" => {
                for (k, v) in parse_arguments_under(el) {
                    scenario.variables.insert(k, v);
                }
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("scenario.variables"),
                    None,
                    None,
                );
            }
            "ConfigTestElement" => apply_config_element(el, scenario, report, path),
            "CSVDataSet" => {
                scenario.datasets.push(parse_csv(el));
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("scenario.datasets"),
                    None,
                    None,
                );
            }
            "HeaderManager" => {
                merge_default_headers(scenario, parse_headers(el));
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Migrated,
                    Some("scenario.defaults.headers"),
                    Some("Header Manager a nivel de grupo mapeado como headers por defecto"),
                    None,
                );
            }
            "CookieManager" => add(
                report,
                el,
                path,
                MappingStatus::Migrated,
                None,
                Some("cookies habilitadas por VU en el engine"),
                None,
            ),
            "CacheManager" => add(
                report,
                el,
                path,
                MappingStatus::Ignored,
                None,
                Some("cache HTTP no relevante en el MVP"),
                None,
            ),
            "ResultCollector" => add(
                report,
                el,
                path,
                MappingStatus::Ignored,
                None,
                Some("listener: el reporte es nativo en perfkit"),
                None,
            ),
            "JSR223Sampler" | "BeanShellSampler" => {
                let (reason, suggestion) = analyze_script(el);
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Assisted,
                    None,
                    Some(&reason),
                    suggestion.as_deref(),
                );
            }
            _ if name.to_ascii_lowercase().contains("kafka") => {
                let brokers = prop_str(el, "bootstrap.servers")
                    .or_else(|| prop_str(el, "kafka_brokers"))
                    .or_else(|| prop_str(el, "brokers"))
                    .map(|s| {
                        s.split(',')
                            .map(|x| x.trim().to_string())
                            .filter(|x| !x.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .filter(|v: &Vec<String>| !v.is_empty())
                    .unwrap_or_else(|| vec!["localhost:9092".to_string()]);
                let topic = prop_str(el, "kafka_topic")
                    .or_else(|| prop_str(el, "topic"))
                    .unwrap_or_else(|| "TOPIC".into());
                let payload = prop_str(el, "kafka_message")
                    .or_else(|| prop_str(el, "message"))
                    .or_else(|| prop_str(el, "placeholderMessage"))
                    .unwrap_or_default();
                add(
                    report,
                    el,
                    path,
                    MappingStatus::Assisted,
                    Some("kafka"),
                    Some("plugin Kafka de terceros mapeado a sampler Kafka nativo"),
                    Some("revisar brokers/topic/payload; credenciales como secretos"),
                );
                out.push(Step::Kafka(KafkaRequest {
                    name: testname(el),
                    brokers,
                    topic,
                    key: None,
                    payload,
                    partition: None,
                    headers: BTreeMap::new(),
                }));
            }
            _ => classify_unknown(el, child, scenario, report, path, out),
        }
    }
}

/// Construye una request HTTP y le adjunta los hijos (headers/assertions/extractores/timers).
fn build_http(el: Node, ht: Option<Node>, report: &mut MigrationReport, path: &str) -> HttpRequest {
    let method = parse_method(&prop_str(el, "HTTPSampler.method").unwrap_or_else(|| "GET".into()));
    let domain = prop_str(el, "HTTPSampler.domain").unwrap_or_default();
    let port = prop_str(el, "HTTPSampler.port").unwrap_or_default();
    let protocol = prop_str(el, "HTTPSampler.protocol").unwrap_or_default();
    let raw_path = prop_str(el, "HTTPSampler.path").unwrap_or_default();
    let follow = prop_bool(el, "HTTPSampler.follow_redirects");

    let url = if domain.trim().is_empty() {
        if raw_path.is_empty() {
            "/".to_string()
        } else {
            raw_path.clone()
        }
    } else {
        let proto = if protocol.trim().is_empty() {
            "http".into()
        } else {
            protocol
        };
        let portpart = if port.trim().is_empty() {
            String::new()
        } else {
            format!(":{}", port.trim())
        };
        format!("{proto}://{domain}{portpart}{}", ensure_path(&raw_path))
    };

    let (body, query) = parse_payload(el, method);
    let mut req = HttpRequest {
        name: testname(el),
        method,
        url,
        headers: BTreeMap::new(),
        query,
        body,
        follow_redirects: follow,
        timeout_ms: prop_str(el, "HTTPSampler.response_timeout")
            .and_then(|s| s.trim().parse().ok()),
        timers: Vec::new(),
        assertions: Vec::new(),
        extractors: Vec::new(),
    };

    add(
        report,
        el,
        path,
        MappingStatus::Migrated,
        Some("http"),
        None,
        None,
    );

    if let Some(ht) = ht {
        for (c, _) in pairs(ht) {
            let cp = format!("{path} > {}", testname(c));
            match c.tag_name().name() {
                "HeaderManager" => {
                    for (k, v) in parse_headers(c) {
                        req.headers.insert(k, v);
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.headers"),
                        None,
                        None,
                    );
                }
                "ResponseAssertion" => {
                    let n = req.assertions.len();
                    req.assertions.extend(parse_response_assertion(c));
                    let st = if req.assertions.len() > n {
                        MappingStatus::Migrated
                    } else {
                        MappingStatus::Assisted
                    };
                    add(
                        report,
                        c,
                        &cp,
                        st,
                        Some("http.assertions"),
                        if st == MappingStatus::Assisted {
                            Some("Response Assertion sin patrón soportado")
                        } else {
                            None
                        },
                        None,
                    );
                }
                "DurationAssertion" => {
                    if let Some(ms) = prop_str(c, "DurationAssertion.duration")
                        .and_then(|s| s.trim().parse().ok())
                    {
                        req.assertions
                            .push(Assertion::DurationBelowMs { max_ms: ms });
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.assertions"),
                        None,
                        None,
                    );
                }
                "SizeAssertion" => {
                    if let Some(b) =
                        prop_str(c, "SizeAssertion.size").and_then(|s| s.trim().parse().ok())
                    {
                        req.assertions
                            .push(Assertion::SizeBelowBytes { max_bytes: b });
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.assertions"),
                        None,
                        None,
                    );
                }
                "JSONPathAssertion" => {
                    if let Some(p) = prop_str(c, "JSON_PATH") {
                        let expected = prop_str(c, "EXPECTED_VALUE").filter(|s| !s.is_empty());
                        let validate = prop_bool(c, "JSONVALIDATION").unwrap_or(false);
                        req.assertions.push(Assertion::JsonPath {
                            path: p,
                            equals: if validate { expected } else { None },
                            exists: Some(true),
                        });
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.assertions"),
                        None,
                        None,
                    );
                }
                "RegexExtractor" => {
                    if let Some(e) = parse_regex_extractor(c) {
                        req.extractors.push(e);
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.extractors"),
                        None,
                        None,
                    );
                }
                "JSONPostProcessor" => {
                    if let Some(e) = parse_json_extractor(c) {
                        req.extractors.push(e);
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.extractors"),
                        None,
                        None,
                    );
                }
                "BoundaryExtractor" => {
                    if let Some(e) = parse_boundary_extractor(c) {
                        req.extractors.push(e);
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.extractors"),
                        None,
                        None,
                    );
                }
                "ConstantTimer"
                | "UniformRandomTimer"
                | "GaussianRandomTimer"
                | "ConstantThroughputTimer" => {
                    if let Some(t) = parse_timer(c) {
                        req.timers.push(t);
                    }
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Migrated,
                        Some("http.timers"),
                        None,
                        None,
                    );
                }
                "JSR223PreProcessor"
                | "JSR223PostProcessor"
                | "BeanShellPreProcessor"
                | "BeanShellPostProcessor" => {
                    let (reason, suggestion) = analyze_script(c);
                    add(
                        report,
                        c,
                        &cp,
                        MappingStatus::Assisted,
                        None,
                        Some(&reason),
                        suggestion.as_deref(),
                    );
                }
                _ => add(
                    report,
                    c,
                    &cp,
                    MappingStatus::Unsupported,
                    None,
                    Some("elemento hijo de sampler no soportado en el MVP"),
                    None,
                ),
            }
        }
    }
    req
}

// --- Parsers de elementos ---

fn apply_config_element(
    el: Node,
    scenario: &mut Scenario,
    report: &mut MigrationReport,
    path: &str,
) {
    let gui = el.attribute("guiclass").unwrap_or("");
    if gui.contains("HttpDefaults") {
        let d = parse_defaults(el);
        let entry = scenario.defaults.get_or_insert_with(HttpDefaults::default);
        if entry.base_url.is_none() {
            entry.base_url = d.base_url;
        }
        for (k, v) in d.headers {
            entry.headers.entry(k).or_insert(v);
        }
        if d.connect_timeout_ms.is_some() {
            entry.connect_timeout_ms = d.connect_timeout_ms;
        }
        if d.response_timeout_ms.is_some() {
            entry.response_timeout_ms = d.response_timeout_ms;
        }
        add(
            report,
            el,
            path,
            MappingStatus::Migrated,
            Some("scenario.defaults"),
            None,
            None,
        );
    } else {
        add(
            report,
            el,
            path,
            MappingStatus::Unsupported,
            None,
            Some("config element no soportado en el MVP"),
            None,
        );
    }
}

fn parse_defaults(el: Node) -> HttpDefaults {
    let domain = prop_str(el, "HTTPSampler.domain").unwrap_or_default();
    let port = prop_str(el, "HTTPSampler.port").unwrap_or_default();
    let protocol = prop_str(el, "HTTPSampler.protocol").unwrap_or_default();
    let path = prop_str(el, "HTTPSampler.path").unwrap_or_default();
    let base_url = if domain.trim().is_empty() {
        None
    } else {
        let proto = if protocol.trim().is_empty() {
            "http".into()
        } else {
            protocol
        };
        let portpart = if port.trim().is_empty() {
            String::new()
        } else {
            format!(":{}", port.trim())
        };
        let tail = if path.trim().is_empty() {
            String::new()
        } else {
            ensure_path(&path)
        };
        Some(format!("{proto}://{domain}{portpart}{tail}"))
    };
    HttpDefaults {
        base_url,
        headers: BTreeMap::new(),
        connect_timeout_ms: prop_str(el, "HTTPSampler.connect_timeout")
            .and_then(|s| s.trim().parse().ok()),
        response_timeout_ms: prop_str(el, "HTTPSampler.response_timeout")
            .and_then(|s| s.trim().parse().ok()),
        follow_redirects: prop_bool(el, "HTTPSampler.follow_redirects"),
    }
}

fn parse_csv(el: Node) -> Dataset {
    let names = prop_str(el, "variableNames").unwrap_or_default();
    let variable_names: Vec<String> = names
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let delim = prop_str(el, "delimiter").unwrap_or_else(|| ",".into());
    let delimiter = delim.chars().next().unwrap_or(',');
    let delimiter = if delim == "\\t" { '\t' } else { delimiter };
    Dataset {
        name: testname(el),
        path: prop_str(el, "filename").unwrap_or_default(),
        delimiter,
        variable_names,
        recycle: prop_bool(el, "recycle").unwrap_or(true),
        first_line_is_header: prop_bool(el, "ignoreFirstLine").unwrap_or(false),
    }
}

fn parse_timer(el: Node) -> Option<Timer> {
    match el.tag_name().name() {
        "ConstantTimer" => Some(Timer::Constant {
            delay_ms: prop_str(el, "ConstantTimer.delay")?.trim().parse().ok()?,
        }),
        "UniformRandomTimer" => Some(Timer::UniformRandom {
            base_ms: prop_str(el, "ConstantTimer.delay")
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0),
            range_ms: prop_str(el, "RandomTimer.range")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0) as u64,
        }),
        "GaussianRandomTimer" => Some(Timer::Gaussian {
            offset_ms: prop_str(el, "ConstantTimer.delay")
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0),
            deviation_ms: prop_str(el, "RandomTimer.range")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0) as u64,
        }),
        "ConstantThroughputTimer" => Some(Timer::ConstantThroughput {
            target_per_minute: double_prop(el, "throughput").unwrap_or(0.0),
        }),
        _ => None,
    }
}

fn parse_response_assertion(el: Node) -> Vec<Assertion> {
    let field = prop_str(el, "Assertion.test_field").unwrap_or_default();
    let test_type = prop_str(el, "Assertion.test_type")
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(2);
    let negate = test_type & 4 != 0;
    let regex = test_type & 1 != 0 || test_type & 2 != 0;
    let is_code = field.contains("response_code");

    let mut out = Vec::new();
    if let Some(coll) = el.children().find(|c| c.has_tag_name("collectionProp")) {
        for s in coll.children().filter(|c| c.is_element()) {
            let Some(val) = s.text().map(|t| t.to_string()) else {
                continue;
            };
            if val.is_empty() {
                continue;
            }
            if is_code && let Ok(code) = val.trim().parse::<u16>() {
                out.push(Assertion::StatusCode { codes: vec![code] });
                continue;
            }
            if regex {
                out.push(Assertion::BodyMatches {
                    pattern: val,
                    negate,
                });
            } else {
                out.push(Assertion::BodyContains {
                    substring: val,
                    negate,
                });
            }
        }
    }
    out
}

fn parse_regex_extractor(el: Node) -> Option<Extractor> {
    let var = prop_str(el, "RegexExtractor.refname")?;
    let pattern = prop_str(el, "RegexExtractor.regex")?;
    let template = prop_str(el, "RegexExtractor.template").unwrap_or_else(|| "$1$".into());
    let group = template.trim_matches('$').parse::<usize>().unwrap_or(1);
    Some(Extractor::Regex {
        var,
        pattern,
        group,
        default: prop_str(el, "RegexExtractor.default").filter(|s| !s.is_empty()),
    })
}

fn parse_json_extractor(el: Node) -> Option<Extractor> {
    let var = first_token(&prop_str(el, "JSONPostProcessor.referenceNames")?);
    let path = first_token(&prop_str(el, "JSONPostProcessor.jsonPathExprs")?);
    Some(Extractor::JsonPath {
        var,
        path,
        default: prop_str(el, "JSONPostProcessor.defaultValues").filter(|s| !s.is_empty()),
    })
}

fn parse_boundary_extractor(el: Node) -> Option<Extractor> {
    Some(Extractor::Boundary {
        var: prop_str(el, "BoundaryExtractor.refname")?,
        left: prop_str(el, "BoundaryExtractor.lboundary").unwrap_or_default(),
        right: prop_str(el, "BoundaryExtractor.rboundary").unwrap_or_default(),
        default: prop_str(el, "BoundaryExtractor.default").filter(|s| !s.is_empty()),
    })
}

fn parse_payload(el: Node, method: HttpMethod) -> (Option<Body>, BTreeMap<String, String>) {
    let raw = prop_bool(el, "HTTPSampler.postBodyRaw").unwrap_or(false);
    let args = sampler_args(el);
    if raw {
        let data = args.into_iter().next().map(|(_, v)| v).unwrap_or_default();
        return (
            Some(Body::Raw {
                content_type: None,
                data,
            }),
            BTreeMap::new(),
        );
    }
    let body_method = matches!(
        method,
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
    );
    if body_method && !args.is_empty() {
        (
            Some(Body::Form {
                fields: args.into_iter().collect(),
            }),
            BTreeMap::new(),
        )
    } else {
        (None, args.into_iter().collect())
    }
}

fn sampler_args(el: Node) -> Vec<(String, String)> {
    let Some(ap) = child_prop(el, "HTTPsampler.Arguments") else {
        return Vec::new();
    };
    let Some(coll) = ap.descendants().find(|c| c.has_tag_name("collectionProp")) else {
        return Vec::new();
    };
    coll.children()
        .filter(|c| c.has_tag_name("elementProp"))
        .map(|a| {
            (
                prop_str(a, "Argument.name").unwrap_or_default(),
                prop_str(a, "Argument.value").unwrap_or_default(),
            )
        })
        .collect()
}

fn parse_headers(el: Node) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(coll) = el.children().find(|c| c.has_tag_name("collectionProp")) {
        for h in coll.children().filter(|c| c.has_tag_name("elementProp")) {
            let name = prop_str(h, "Header.name").unwrap_or_default();
            if !name.is_empty() {
                out.push((name, prop_str(h, "Header.value").unwrap_or_default()));
            }
        }
    }
    out
}

fn classify_unknown(
    el: Node,
    child: Option<Node>,
    scenario: &mut Scenario,
    report: &mut MigrationReport,
    path: &str,
    out: &mut Vec<Step>,
) {
    let assisted =
        matches!(el.tag_name().name(), n if n.contains("JSR223") || n.contains("BeanShell"));
    let status = if assisted {
        MappingStatus::Assisted
    } else {
        MappingStatus::Unsupported
    };
    add(
        report,
        el,
        path,
        status,
        None,
        Some("elemento no modelado en el MVP; sus hijos conocidos se aplanan"),
        None,
    );
    if let Some(c) = child {
        let p = format!("{path} > {}", testname(el));
        walk_steps(c, out, scenario, report, &p);
    }
}

// --- Helpers de propiedades JMX ---

/// Análisis heurístico de un sampler/processor de script (JSR223/BeanShell) para
/// generar una sugerencia de migración concreta (Fase 4 — migración asistida).
fn analyze_script(el: Node) -> (String, Option<String>) {
    let lang = prop_str(el, "scriptLanguage").unwrap_or_else(|| "groovy".into());
    let code = prop_str(el, "script").unwrap_or_default();
    if code.trim().is_empty() {
        return (
            format!("script {lang}: vacío o referenciado por archivo"),
            Some("revisar el script externo y portarlo manualmente".into()),
        );
    }
    let c = code.to_lowercase();
    let suggestion = if c.contains("vars.put") || c.contains("vars.get") {
        "manejo de variables: usar extractores (regex/jsonpath/boundary) y variables del IR"
    } else if c.contains("prev.") || c.contains("getresponsedata") || c.contains("responsecode") {
        "lee/parsea la respuesta: reemplazar por un extractor o assertion declarativa"
    } else if c.contains("messagedigest")
        || c.contains("hmac")
        || c.contains("mac.")
        || c.contains("signature")
        || c.contains("base64")
        || c.contains("cipher")
    {
        "firma/cripto de payload: mantener como pre-procesador (soporte nativo pendiente) o plugin firmado"
    } else if c.contains("thread.sleep") {
        "pausa explícita: usar un Timer del IR"
    } else if c.contains("props.put") || c.contains("props.get") {
        "estado global entre hilos: modelar con variables de escenario o dataset"
    } else if c.contains("openconnection") || c.contains("httpclient") || c.contains("url(") {
        "petición HTTP manual: convertir a un HTTP Sampler declarativo"
    } else if c.lines().all(|l| {
        let t = l.trim();
        t.is_empty() || t.starts_with("log.") || t.starts_with("//")
    }) {
        "solo logging: puede omitirse en la migración"
    } else {
        "lógica de script no trivial: revisar manualmente; candidato a migración asistida por IA"
    };
    (
        format!("script {lang} ({} líneas)", code.lines().count()),
        Some(suggestion.to_string()),
    )
}

fn condition_support(cond: &str) -> (MappingStatus, Option<&'static str>) {
    let c = cond.trim();
    let simple = c.is_empty()
        || c.eq_ignore_ascii_case("true")
        || c.eq_ignore_ascii_case("false")
        || c.contains("==")
        || c.contains("!=");
    if simple {
        (MappingStatus::Migrated, None)
    } else {
        (
            MappingStatus::Assisted,
            Some("condición compleja: el engine solo evalúa ==, != y true/false"),
        )
    }
}

fn testname(n: Node) -> String {
    n.attribute("testname")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| n.tag_name().name())
        .to_string()
}

fn child_prop<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.attribute("name") == Some(name))
}

fn prop_str(n: Node, name: &str) -> Option<String> {
    child_prop(n, name)
        .and_then(|p| p.text())
        .map(resolve_jmeter_funcs)
}

/// Resuelve funciones de propiedad de JMeter `${__P(nombre,default)}` /
/// `${__property(nombre,default)}` a su valor por defecto (en el importador no hay
/// propiedades de JMeter, así que se usa el default). Las variables normales `${var}`
/// se conservan tal cual (perfkit las interpola en runtime). Otras funciones `${__...}`
/// sin default se vacían (no tienen equivalente en el importador).
fn resolve_jmeter_funcs(s: &str) -> String {
    if !s.contains("${__") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find("${__") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let Some(close) = after.find('}') else {
            out.push_str(after);
            return out;
        };
        let inner = &after[2..close]; // p.ej. "__P(host,127.0.0.1)"
        if let Some(open) = inner.find('(') {
            let args = inner[open + 1..].trim_end_matches(')');
            // El default es el 2º argumento (si existe).
            if let Some((_, def)) = args.split_once(',') {
                out.push_str(def.trim());
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

fn prop_bool(n: Node, name: &str) -> Option<bool> {
    prop_str(n, name).map(|s| s.trim().eq_ignore_ascii_case("true"))
}

fn double_prop(n: Node, name: &str) -> Option<f64> {
    // doubleProp: <doubleProp><name>throughput</name><value>X</value></doubleProp>
    n.children()
        .filter(|c| c.has_tag_name("doubleProp"))
        .find(|dp| {
            dp.children()
                .find(|x| x.has_tag_name("name"))
                .and_then(|x| x.text())
                == Some(name)
        })
        .and_then(|dp| dp.children().find(|x| x.has_tag_name("value")))
        .and_then(|v| v.text())
        .and_then(|s| s.trim().parse().ok())
}

/// Lee propiedades de objeto JMeter `<FloatProperty><name>X</name><value>V</value>...`.
fn obj_prop(n: Node, name: &str) -> Option<String> {
    n.children()
        .filter(|c| c.is_element())
        .find(|c| {
            c.children()
                .find(|x| x.has_tag_name("name"))
                .and_then(|x| x.text())
                == Some(name)
        })
        .and_then(|c| c.children().find(|x| x.has_tag_name("value")))
        .and_then(|v| v.text())
        .map(|s| s.to_string())
}

fn parse_arguments_under(node: Node) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(coll) = node.descendants().find(|c| {
        c.has_tag_name("collectionProp") && c.attribute("name") == Some("Arguments.arguments")
    }) {
        for a in coll.children().filter(|c| c.has_tag_name("elementProp")) {
            let n = prop_str(a, "Argument.name").unwrap_or_default();
            if !n.is_empty() {
                out.push((n, prop_str(a, "Argument.value").unwrap_or_default()));
            }
        }
    }
    out
}

fn merge_default_headers(scenario: &mut Scenario, headers: Vec<(String, String)>) {
    if headers.is_empty() {
        return;
    }
    let d = scenario.defaults.get_or_insert_with(HttpDefaults::default);
    for (k, v) in headers {
        d.headers.insert(k, v);
    }
}

fn pairs<'a, 'i>(htree: Node<'a, 'i>) -> Vec<(Node<'a, 'i>, Option<Node<'a, 'i>>)> {
    let mut res = Vec::new();
    let mut pending: Option<Node> = None;
    for c in htree.children().filter(|c| c.is_element()) {
        if c.has_tag_name("hashTree") {
            if let Some(e) = pending.take() {
                res.push((e, Some(c)));
            }
        } else {
            if let Some(e) = pending.take() {
                res.push((e, None));
            }
            pending = Some(c);
        }
    }
    if let Some(e) = pending.take() {
        res.push((e, None));
    }
    res
}

fn add(
    report: &mut MigrationReport,
    el: Node,
    path: &str,
    status: MappingStatus,
    ir_ref: Option<&str>,
    reason: Option<&str>,
    suggestion: Option<&str>,
) {
    report.push(MappedElement {
        jmx_type: el.tag_name().name().to_string(),
        jmx_name: testname(el),
        path: path.to_string(),
        status,
        ir_ref: ir_ref.map(|s| s.to_string()),
        reason: reason.map(|s| s.to_string()),
        suggestion: suggestion.map(|s| s.to_string()),
    });
}

fn parse_method(s: &str) -> HttpMethod {
    match s.trim().to_ascii_uppercase().as_str() {
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "DELETE" => HttpMethod::Delete,
        "PATCH" => HttpMethod::Patch,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => HttpMethod::Get,
    }
}

fn ensure_path(p: &str) -> String {
    if p.is_empty() {
        "/".to_string()
    } else if p.starts_with('/') || p.starts_with("${") {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

fn first_token(s: &str) -> String {
    s.split([';', ',']).next().unwrap_or(s).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const JMX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2">
  <hashTree>
    <TestPlan testname="Demo Plan"><elementProp name="TestPlan.user_defined_variables" elementType="Arguments">
      <collectionProp name="Arguments.arguments">
        <elementProp name="base" elementType="Argument"><stringProp name="Argument.name">base</stringProp><stringProp name="Argument.value">v1</stringProp></elementProp>
      </collectionProp></elementProp>
    </TestPlan>
    <hashTree>
      <ThreadGroup testname="TG">
        <stringProp name="ThreadGroup.num_threads">3</stringProp>
        <stringProp name="ThreadGroup.ramp_time">1</stringProp>
        <elementProp name="ThreadGroup.main_controller" elementType="LoopController">
          <stringProp name="LoopController.loops">5</stringProp>
          <boolProp name="LoopController.continue_forever">false</boolProp>
        </elementProp>
      </ThreadGroup>
      <hashTree>
        <HTTPSamplerProxy testname="Home">
          <stringProp name="HTTPSampler.domain">example.test</stringProp>
          <stringProp name="HTTPSampler.protocol">https</stringProp>
          <stringProp name="HTTPSampler.path">/home</stringProp>
          <stringProp name="HTTPSampler.method">GET</stringProp>
        </HTTPSamplerProxy>
        <hashTree>
          <ResponseAssertion testname="200 ok">
            <collectionProp name="Asserion.test_strings"><stringProp name="0">200</stringProp></collectionProp>
            <stringProp name="Assertion.test_field">Assertion.response_code</stringProp>
            <intProp name="Assertion.test_type">8</intProp>
          </ResponseAssertion>
          <hashTree/>
        </hashTree>
        <JSR223Sampler testname="script"><stringProp name="script">x</stringProp></JSR223Sampler>
        <hashTree/>
      </hashTree>
    </hashTree>
  </hashTree>
</jmeterTestPlan>"#;

    #[test]
    fn imports_basic_plan() {
        let (s, r) = import_jmx(JMX, "demo.jmx").unwrap();
        assert_eq!(s.name, "Demo Plan");
        assert_eq!(s.variables.get("base").map(String::as_str), Some("v1"));
        assert_eq!(s.thread_groups.len(), 1);
        let tg = &s.thread_groups[0];
        assert_eq!(tg.load.virtual_users, 3);
        assert_eq!(tg.load.iterations, Some(5));
        assert_eq!(tg.steps.len(), 1); // el JSR223 no produce step
        match &tg.steps[0] {
            Step::Http(h) => {
                assert_eq!(h.url, "https://example.test/home");
                assert_eq!(h.assertions.len(), 1);
                assert!(matches!(h.assertions[0], Assertion::StatusCode { .. }));
            }
            _ => panic!("esperaba http"),
        }
        // fidelidad: el JSR223 quedó como assisted, nada silencioso
        assert!(r.summary.assisted >= 1);
        assert!(
            r.elements
                .iter()
                .any(|e| e.jmx_type == "JSR223Sampler" && e.status == MappingStatus::Assisted)
        );
        assert!(r.summary.total >= 5);
    }

    #[test]
    fn no_testplan_errors() {
        let bad =
            r#"<?xml version="1.0"?><jmeterTestPlan><hashTree><Foo/></hashTree></jmeterTestPlan>"#;
        assert!(
            import_jmx(bad, "x").is_err()
                || import_jmx(bad, "x").unwrap().1.summary.unsupported > 0
        );
    }
}
