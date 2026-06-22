//! Reporte JUnit XML para integración en CI.

use metrics::RunSummary;

fn esc(s: &str) -> String {
    html_escape::encode_quoted_attribute(s).into_owned()
}

/// Genera JUnit XML: un `<testcase>` por etiqueta; falla si tuvo errores.
pub fn junit_xml(s: &RunSummary) -> String {
    let failures = s.labels.iter().filter(|l| l.errors > 0).count();
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<testsuite name=\"perfkit: {}\" tests=\"{}\" failures=\"{}\" time=\"{:.3}\">\n",
        esc(&s.scenario_name),
        s.labels.len(),
        failures,
        s.duration_secs
    ));
    for l in &s.labels {
        out.push_str(&format!(
            "  <testcase classname=\"perfkit\" name=\"{}\" time=\"{:.3}\">",
            esc(&l.label),
            l.mean_ms / 1000.0
        ));
        if l.errors > 0 {
            out.push('\n');
            out.push_str(&format!(
                "    <failure message=\"{} de {} muestras con error ({:.2}%)\">p95={:.1}ms p99={:.1}ms</failure>\n",
                l.errors,
                l.count,
                l.error_rate * 100.0,
                l.p95_ms,
                l.p99_ms
            ));
            out.push_str("  </testcase>\n");
        } else {
            out.push_str("</testcase>\n");
        }
    }
    out.push_str("</testsuite>\n");
    out
}
