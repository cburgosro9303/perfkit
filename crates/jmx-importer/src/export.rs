//! Exportador **IR → JMX** (Apache JMeter), para el round-trip de migración:
//! construir/editar el plan en perfkit y abrirlo en JMeter.
//!
//! Cubre los elementos Nivel 1 (ver `docs/migration/jmeter-support-matrix.md`).
//! Los pasos sin equivalente JMeter nativo (p.ej. `Step::Kafka`) se anotan como
//! comentario XML y se omiten (JMeter los ignora).

use scenario_ir::model::*;

/// Genera un `.jmx` (JMeter 5.x) a partir de un escenario del IR.
pub fn export_jmx(s: &Scenario) -> String {
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    o.push_str(
        "<jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\">\n<hashTree>\n",
    );

    o.push_str(&format!(
        "<TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"{}\">\n",
        esc(&s.name)
    ));
    o.push_str("<elementProp name=\"TestPlan.user_defined_variables\" elementType=\"Arguments\" guiclass=\"ArgumentsPanel\" testclass=\"Arguments\"><collectionProp name=\"Arguments.arguments\">\n");
    for (k, v) in &s.variables {
        o.push_str(&udv_argument(k, v));
    }
    o.push_str("</collectionProp></elementProp>\n</TestPlan>\n<hashTree>\n");

    if let Some(d) = &s.defaults {
        if d.base_url.is_some() {
            o.push_str(&http_defaults(d));
        }
        if !d.headers.is_empty() {
            o.push_str(&header_manager(
                &d.headers,
                "HTTP Header Manager (defaults)",
            ));
            o.push_str("<hashTree/>\n");
        }
    }
    for ds in &s.datasets {
        o.push_str(&csv_dataset(ds));
    }
    for g in &s.thread_groups {
        o.push_str(&thread_group(g));
    }

    o.push_str("</hashTree>\n</hashTree>\n</jmeterTestPlan>\n");
    o
}

fn thread_group(g: &ThreadGroup) -> String {
    let lp = &g.load;
    let (loops, forever) = match lp.iterations {
        Some(n) => (n.to_string(), false),
        None => ("-1".to_string(), true),
    };
    let scheduler = lp.duration_secs.is_some() || lp.hold_secs > 0;
    let duration = lp.duration_secs.unwrap_or(lp.hold_secs);
    let mut o = String::new();
    o.push_str(&format!(
        "<ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"{}\">\n",
        esc(&g.name)
    ));
    o.push_str(&sprop(
        "ThreadGroup.num_threads",
        &lp.virtual_users.to_string(),
    ));
    o.push_str(&sprop(
        "ThreadGroup.ramp_time",
        &lp.ramp_up_secs.to_string(),
    ));
    o.push_str(&bprop("ThreadGroup.scheduler", scheduler));
    if scheduler {
        o.push_str(&sprop("ThreadGroup.duration", &duration.to_string()));
    }
    o.push_str("<elementProp name=\"ThreadGroup.main_controller\" elementType=\"LoopController\" guiclass=\"LoopControlPanel\" testclass=\"LoopController\">\n");
    o.push_str(&sprop("LoopController.loops", &loops));
    o.push_str(&bprop("LoopController.continue_forever", forever));
    o.push_str("</elementProp>\n</ThreadGroup>\n");
    o.push_str("<hashTree>\n");
    emit_steps(&mut o, &g.steps);
    o.push_str("</hashTree>\n");
    o
}

fn emit_steps(o: &mut String, steps: &[Step]) {
    for step in steps {
        match step {
            Step::Http(h) => emit_http(o, h),
            Step::Transaction(t) => {
                o.push_str(&format!("<TransactionController guiclass=\"TransactionControllerGui\" testclass=\"TransactionController\" testname=\"{}\"><boolProp name=\"TransactionController.includeTimers\">false</boolProp></TransactionController>\n", esc(&t.name)));
                wrap_children(o, &t.steps);
            }
            Step::Loop(l) => {
                o.push_str(&format!("<LoopController guiclass=\"LoopControlPanel\" testclass=\"LoopController\" testname=\"{}\">", esc(&l.name)));
                o.push_str(&sprop("LoopController.loops", &l.count.to_string()));
                o.push_str(&bprop("LoopController.continue_forever", false));
                o.push_str("</LoopController>\n");
                wrap_children(o, &l.steps);
            }
            Step::If(c) => {
                o.push_str(&format!("<IfController guiclass=\"IfControllerPanel\" testclass=\"IfController\" testname=\"{}\">", esc(&c.name)));
                o.push_str(&sprop("IfController.condition", &c.condition));
                o.push_str("</IfController>\n");
                wrap_children(o, &c.steps);
            }
            Step::While(w) => {
                o.push_str(&format!("<WhileController guiclass=\"WhileControllerGui\" testclass=\"WhileController\" testname=\"{}\">", esc(&w.name)));
                o.push_str(&sprop("WhileController.condition", &w.condition));
                o.push_str("</WhileController>\n");
                wrap_children(o, &w.steps);
            }
            Step::Throughput(t) => {
                o.push_str(&format!("<ThroughputController guiclass=\"ThroughputControllerGui\" testclass=\"ThroughputController\" testname=\"{}\"><intProp name=\"ThroughputController.style\">0</intProp><boolProp name=\"ThroughputController.perThread\">false</boolProp><FloatProperty><name>ThroughputController.percentThroughput</name><value>{}</value><savedValue>0.0</savedValue></FloatProperty></ThroughputController>\n", esc(&t.name), t.percent));
                wrap_children(o, &t.steps);
            }
            Step::Interleave(c) => {
                o.push_str(&format!("<InterleaveControl guiclass=\"InterleaveControlGui\" testclass=\"InterleaveControl\" testname=\"{}\"><intProp name=\"InterleaveControl.style\">1</intProp></InterleaveControl>\n", esc(&c.name)));
                wrap_children(o, &c.steps);
            }
            Step::Random(c) => {
                o.push_str(&format!("<RandomController guiclass=\"RandomControlGui\" testclass=\"RandomController\" testname=\"{}\"><intProp name=\"InterleaveControl.style\">1</intProp></RandomController>\n", esc(&c.name)));
                wrap_children(o, &c.steps);
            }
            Step::Kafka(k) => {
                o.push_str(&format!(
                    "<!-- perfkit: paso Kafka '{}' (topic {}) sin equivalente JMeter nativo; requiere un plugin Kafka -->\n",
                    esc(&k.name),
                    esc(&k.topic)
                ));
            }
            Step::Timer(t) => {
                o.push_str(&timer(t));
                o.push_str("<hashTree/>\n");
            }
        }
    }
}

fn wrap_children(o: &mut String, steps: &[Step]) {
    if steps.is_empty() {
        o.push_str("<hashTree/>\n");
    } else {
        o.push_str("<hashTree>\n");
        emit_steps(o, steps);
        o.push_str("</hashTree>\n");
    }
}

fn emit_http(o: &mut String, h: &HttpRequest) {
    let (proto, domain, port, path) = split_url(&h.url);
    o.push_str(&format!(
        "<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"{}\">\n",
        esc(&h.name)
    ));
    o.push_str(&sprop("HTTPSampler.domain", &domain));
    o.push_str(&sprop("HTTPSampler.port", &port));
    o.push_str(&sprop("HTTPSampler.protocol", &proto));
    o.push_str(&sprop("HTTPSampler.path", &path));
    o.push_str(&sprop("HTTPSampler.method", h.method.as_str()));
    o.push_str(&bprop(
        "HTTPSampler.follow_redirects",
        h.follow_redirects.unwrap_or(true),
    ));

    // Cuerpo / argumentos
    let raw = matches!(&h.body, Some(Body::Raw { .. }));
    if raw {
        o.push_str(&bprop("HTTPSampler.postBodyRaw", true));
    }
    o.push_str("<elementProp name=\"HTTPsampler.Arguments\" elementType=\"Arguments\"><collectionProp name=\"Arguments.arguments\">\n");
    match &h.body {
        Some(Body::Raw { data, .. }) => o.push_str(&raw_body_arg(data)),
        Some(Body::Form { fields }) => {
            for (k, v) in fields {
                o.push_str(&http_argument(k, v));
            }
        }
        None => {
            for (k, v) in &h.query {
                o.push_str(&http_argument(k, v));
            }
        }
    }
    o.push_str("</collectionProp></elementProp>\n</HTTPSamplerProxy>\n");

    // Hijos del sampler
    let has_children = !h.headers.is_empty()
        || !h.timers.is_empty()
        || !h.assertions.is_empty()
        || !h.extractors.is_empty();
    if !has_children {
        o.push_str("<hashTree/>\n");
        return;
    }
    o.push_str("<hashTree>\n");
    if !h.headers.is_empty() {
        o.push_str(&header_manager(&h.headers, "HTTP Header Manager"));
        o.push_str("<hashTree/>\n");
    }
    for t in &h.timers {
        o.push_str(&timer(t));
        o.push_str("<hashTree/>\n");
    }
    for a in &h.assertions {
        o.push_str(&assertion(a));
        o.push_str("<hashTree/>\n");
    }
    for e in &h.extractors {
        o.push_str(&extractor(e));
        o.push_str("<hashTree/>\n");
    }
    o.push_str("</hashTree>\n");
}

// --- Sub-emisores ---

fn http_defaults(d: &HttpDefaults) -> String {
    let (proto, domain, port, path) = split_url(d.base_url.as_deref().unwrap_or(""));
    let mut o = String::from(
        "<ConfigTestElement guiclass=\"HttpDefaultsGui\" testclass=\"ConfigTestElement\" testname=\"HTTP Request Defaults\">\n",
    );
    o.push_str(&sprop("HTTPSampler.domain", &domain));
    o.push_str(&sprop("HTTPSampler.protocol", &proto));
    o.push_str(&sprop("HTTPSampler.port", &port));
    o.push_str(&sprop("HTTPSampler.path", &path));
    o.push_str("</ConfigTestElement>\n<hashTree/>\n");
    o
}

fn csv_dataset(ds: &Dataset) -> String {
    let delim = if ds.delimiter == '\t' {
        "\\t".to_string()
    } else {
        ds.delimiter.to_string()
    };
    let mut o = String::from(
        "<CSVDataSet guiclass=\"TestBeanGUI\" testclass=\"CSVDataSet\" testname=\"CSV Data Set Config\">\n",
    );
    o.push_str(&sprop("filename", &ds.path));
    o.push_str(&sprop("variableNames", &ds.variable_names.join(",")));
    o.push_str(&sprop("delimiter", &delim));
    o.push_str(&bprop("recycle", ds.recycle));
    o.push_str(&bprop("ignoreFirstLine", ds.first_line_is_header));
    o.push_str("</CSVDataSet>\n<hashTree/>\n");
    o
}

fn header_manager(headers: &std::collections::BTreeMap<String, String>, name: &str) -> String {
    let mut o = format!(
        "<HeaderManager guiclass=\"HeaderPanel\" testclass=\"HeaderManager\" testname=\"{}\"><collectionProp name=\"HeaderManager.headers\">",
        esc(name)
    );
    for (k, v) in headers {
        o.push_str(&format!(
            "<elementProp name=\"\" elementType=\"Header\">{}{}</elementProp>",
            sprop("Header.name", k),
            sprop("Header.value", v)
        ));
    }
    o.push_str("</collectionProp></HeaderManager>\n");
    o
}

fn timer(t: &Timer) -> String {
    match t {
        Timer::Constant { delay_ms } => format!(
            "<ConstantTimer guiclass=\"ConstantTimerGui\" testclass=\"ConstantTimer\" testname=\"Constant Timer\">{}</ConstantTimer>\n",
            sprop("ConstantTimer.delay", &delay_ms.to_string())
        ),
        Timer::UniformRandom { base_ms, range_ms } => format!(
            "<UniformRandomTimer guiclass=\"UniformRandomTimerGui\" testclass=\"UniformRandomTimer\" testname=\"Uniform Random Timer\">{}{}</UniformRandomTimer>\n",
            sprop("ConstantTimer.delay", &base_ms.to_string()),
            sprop("RandomTimer.range", &range_ms.to_string())
        ),
        Timer::Gaussian {
            offset_ms,
            deviation_ms,
        } => format!(
            "<GaussianRandomTimer guiclass=\"GaussianRandomTimerGui\" testclass=\"GaussianRandomTimer\" testname=\"Gaussian Random Timer\">{}{}</GaussianRandomTimer>\n",
            sprop("ConstantTimer.delay", &offset_ms.to_string()),
            sprop("RandomTimer.range", &deviation_ms.to_string())
        ),
        Timer::ConstantThroughput { target_per_minute } => format!(
            "<ConstantThroughputTimer guiclass=\"TestBeanGUI\" testclass=\"ConstantThroughputTimer\" testname=\"Constant Throughput Timer\"><doubleProp><name>throughput</name><value>{}</value><savedValue>0.0</savedValue></doubleProp></ConstantThroughputTimer>\n",
            target_per_minute
        ),
    }
}

fn assertion(a: &Assertion) -> String {
    match a {
        Assertion::StatusCode { codes } => response_assertion(
            "Assertion.response_code",
            8,
            &codes.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        ),
        Assertion::BodyContains { substring, negate } => response_assertion(
            "Assertion.response_data",
            2 | if *negate { 4 } else { 0 },
            std::slice::from_ref(substring),
        ),
        Assertion::BodyMatches { pattern, negate } => response_assertion(
            "Assertion.response_data",
            1 | if *negate { 4 } else { 0 },
            std::slice::from_ref(pattern),
        ),
        Assertion::JsonPath { path, equals, .. } => format!(
            "<JSONPathAssertion guiclass=\"JSONPathAssertionGui\" testclass=\"JSONPathAssertion\" testname=\"JSON Assertion\">{}{}{}</JSONPathAssertion>\n",
            sprop("JSON_PATH", path),
            sprop("EXPECTED_VALUE", equals.as_deref().unwrap_or("")),
            bprop("JSONVALIDATION", equals.is_some())
        ),
        Assertion::DurationBelowMs { max_ms } => format!(
            "<DurationAssertion guiclass=\"DurationAssertionGui\" testclass=\"DurationAssertion\" testname=\"Duration Assertion\">{}</DurationAssertion>\n",
            sprop("DurationAssertion.duration", &max_ms.to_string())
        ),
        Assertion::SizeBelowBytes { max_bytes } => format!(
            "<SizeAssertion guiclass=\"SizeAssertionGui\" testclass=\"SizeAssertion\" testname=\"Size Assertion\">{}<intProp name=\"SizeAssertion.operator\">2</intProp></SizeAssertion>\n",
            sprop("SizeAssertion.size", &max_bytes.to_string())
        ),
    }
}

fn response_assertion(field: &str, test_type: i64, strings: &[String]) -> String {
    let mut coll = String::new();
    for (i, s) in strings.iter().enumerate() {
        coll.push_str(&format!("<stringProp name=\"{i}\">{}</stringProp>", esc(s)));
    }
    format!(
        "<ResponseAssertion guiclass=\"AssertionGui\" testclass=\"ResponseAssertion\" testname=\"Response Assertion\"><collectionProp name=\"Asserion.test_strings\">{coll}</collectionProp>{}<intProp name=\"Assertion.test_type\">{test_type}</intProp></ResponseAssertion>\n",
        sprop("Assertion.test_field", field)
    )
}

fn extractor(e: &Extractor) -> String {
    match e {
        Extractor::Regex {
            var,
            pattern,
            group,
            default,
        } => format!(
            "<RegexExtractor guiclass=\"RegexExtractorGui\" testclass=\"RegexExtractor\" testname=\"Regular Expression Extractor\">{}{}{}{}</RegexExtractor>\n",
            sprop("RegexExtractor.refname", var),
            sprop("RegexExtractor.regex", pattern),
            sprop("RegexExtractor.template", &format!("${group}$")),
            sprop("RegexExtractor.default", default.as_deref().unwrap_or(""))
        ),
        Extractor::JsonPath { var, path, default } => format!(
            "<JSONPostProcessor guiclass=\"JSONPostProcessorGui\" testclass=\"JSONPostProcessor\" testname=\"JSON Extractor\">{}{}{}</JSONPostProcessor>\n",
            sprop("JSONPostProcessor.referenceNames", var),
            sprop("JSONPostProcessor.jsonPathExprs", path),
            sprop(
                "JSONPostProcessor.defaultValues",
                default.as_deref().unwrap_or("")
            )
        ),
        Extractor::Boundary {
            var,
            left,
            right,
            default,
        } => format!(
            "<BoundaryExtractor guiclass=\"BoundaryExtractorGui\" testclass=\"BoundaryExtractor\" testname=\"Boundary Extractor\">{}{}{}{}</BoundaryExtractor>\n",
            sprop("BoundaryExtractor.refname", var),
            sprop("BoundaryExtractor.lboundary", left),
            sprop("BoundaryExtractor.rboundary", right),
            sprop(
                "BoundaryExtractor.default",
                default.as_deref().unwrap_or("")
            )
        ),
    }
}

fn udv_argument(name: &str, value: &str) -> String {
    format!(
        "<elementProp name=\"{n}\" elementType=\"Argument\">{}{}</elementProp>\n",
        sprop("Argument.name", name),
        sprop("Argument.value", value),
        n = esc(name)
    )
}

fn http_argument(name: &str, value: &str) -> String {
    format!(
        "<elementProp name=\"{n}\" elementType=\"HTTPArgument\"><boolProp name=\"HTTPArgument.always_encode\">false</boolProp>{}{}</elementProp>",
        sprop("Argument.value", value),
        sprop("Argument.name", name),
        n = esc(name)
    )
}

fn raw_body_arg(data: &str) -> String {
    format!(
        "<elementProp name=\"\" elementType=\"HTTPArgument\"><boolProp name=\"HTTPArgument.always_encode\">false</boolProp>{}<stringProp name=\"Argument.name\"></stringProp></elementProp>",
        sprop("Argument.value", data)
    )
}

fn split_url(url: &str) -> (String, String, String, String) {
    let (proto, rest) = if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else {
        ("", url)
    };
    if proto.is_empty() {
        let path = url.split('?').next().unwrap_or(url);
        return (
            String::new(),
            String::new(),
            String::new(),
            path.to_string(),
        );
    }
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (domain, port) = match authority.find(':') {
        Some(i) => (&authority[..i], &authority[i + 1..]),
        None => (authority, ""),
    };
    let path = path.split('?').next().unwrap_or(path);
    (
        proto.to_string(),
        domain.to_string(),
        port.to_string(),
        path.to_string(),
    )
}

fn sprop(name: &str, value: &str) -> String {
    format!(
        "<stringProp name=\"{}\">{}</stringProp>",
        esc(name),
        esc(value)
    )
}
fn bprop(name: &str, value: bool) -> String {
    format!("<boolProp name=\"{}\">{}</boolProp>", esc(name), value)
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            _ => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_jmx;

    #[test]
    fn roundtrip_jmx_via_perfkit() {
        // Importa un escenario, expórtalo a JMX y re-impórtalo: debe conservar lo esencial.
        let mut s = Scenario::new("Round Trip");
        s.defaults = Some(HttpDefaults {
            base_url: Some("https://api.test:8443".into()),
            ..Default::default()
        });
        s.variables.insert("token".into(), "abc".into());
        s.thread_groups.push(ThreadGroup {
            name: "TG".into(),
            load: LoadProfile {
                virtual_users: 7,
                ramp_up_secs: 2,
                hold_secs: 0,
                ramp_down_secs: 0,
                iterations: Some(4),
                duration_secs: None,
            },
            on_error: OnError::Continue,
            steps: vec![Step::Transaction(Transaction {
                name: "Checkout".into(),
                steps: vec![Step::Http(HttpRequest {
                    name: "POST /order".into(),
                    method: HttpMethod::Post,
                    url: "https://api.test:8443/order".into(),
                    headers: std::collections::BTreeMap::from([(
                        "Authorization".to_string(),
                        "Bearer ${token}".to_string(),
                    )]),
                    query: Default::default(),
                    body: Some(Body::Raw {
                        content_type: None,
                        data: "{\"x\":1}".into(),
                    }),
                    follow_redirects: Some(true),
                    timeout_ms: None,
                    timers: vec![Timer::Constant { delay_ms: 150 }],
                    assertions: vec![Assertion::StatusCode { codes: vec![200] }],
                    extractors: vec![Extractor::JsonPath {
                        var: "id".into(),
                        path: "$.id".into(),
                        default: None,
                    }],
                })],
            })],
        });

        let jmx = export_jmx(&s);
        let (back, report) = import_jmx(&jmx, "exported.jmx").expect("re-import");
        assert_eq!(back.name, "Round Trip");
        assert_eq!(back.variables.get("token").map(String::as_str), Some("abc"));
        assert_eq!(back.thread_groups.len(), 1);
        let tg = &back.thread_groups[0];
        assert_eq!(tg.load.virtual_users, 7);
        assert_eq!(tg.load.iterations, Some(4));
        // La transacción con la request anidada sobrevive
        match &tg.steps[0] {
            Step::Transaction(t) => match &t.steps[0] {
                Step::Http(h) => {
                    assert_eq!(h.method, HttpMethod::Post);
                    assert_eq!(h.url, "https://api.test:8443/order");
                    assert_eq!(
                        h.headers.get("Authorization").map(String::as_str),
                        Some("Bearer ${token}")
                    );
                    assert!(matches!(h.body, Some(Body::Raw { .. })));
                    assert_eq!(h.assertions.len(), 1);
                    assert_eq!(h.extractors.len(), 1);
                }
                _ => panic!("esperaba http"),
            },
            _ => panic!("esperaba transacción"),
        }
        // Sin fallos silenciosos en el re-import
        assert!(report.summary.total > 0);
    }
}
