//! `metrics` — agregación de resultados de ejecución.
//!
//! El hot path del engine solo **emite** [`Sample`]s baratos; este crate los agrega
//! fuera del hot path en histogramas HDR, series por segundo, histograma de latencias
//! con buckets fijos (mergeable), heatmap, conteo por código de estado / tipo de error
//! y estadísticas por etiqueta, produciendo un [`RunSummary`] serializable.

use hdrhistogram::Histogram;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Límites superiores (ms) de los buckets del histograma de latencias. Son **fijos**
/// para que el merge distribuido sea una suma elemento a elemento. El último bucket es
/// el desborde (≥ último límite).
pub const HIST_BOUNDS_MS: &[f64] = &[
    1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
];

fn nbuckets() -> usize {
    HIST_BOUNDS_MS.len() + 1
}

fn bucket_index(ms: f64) -> usize {
    for (i, b) in HIST_BOUNDS_MS.iter().enumerate() {
        if ms < *b {
            return i;
        }
    }
    HIST_BOUNDS_MS.len()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum SampleKind {
    #[default]
    Http,
    Kafka,
    Transaction,
}

/// Resultado de una muestra individual (una request o una transacción).
#[derive(Debug, Clone)]
pub struct Sample {
    pub label: String,
    pub kind: SampleKind,
    /// Offset desde el inicio del run (reloj monotónico).
    pub offset_ms: u64,
    /// Latencia total medida en microsegundos.
    pub latency_us: u64,
    /// Time-to-first-byte (hasta recibir cabeceras), en microsegundos.
    pub ttfb_us: u64,
    pub status: Option<u16>,
    /// Bytes recibidos (cuerpo de respuesta).
    pub bytes: u64,
    /// Bytes enviados (aprox: cuerpo + cabeceras de la request).
    pub sent_bytes: u64,
    pub success: bool,
    pub error: Option<String>,
    /// Categoría del error: status, assertion, timeout, connection, other.
    pub error_kind: Option<String>,
}

/// Estadísticas agregadas de una etiqueta (sampler o transacción) o del total.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LabelStats {
    pub label: String,
    pub kind: SampleKind,
    pub count: u64,
    pub errors: u64,
    pub error_rate: f64,
    pub throughput_per_sec: f64,
    pub min_ms: f64,
    pub mean_ms: f64,
    pub max_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub p999_ms: f64,
    #[serde(default)]
    pub ttfb_mean_ms: f64,
    #[serde(default)]
    pub ttfb_p95_ms: f64,
    pub bytes_total: u64,
    #[serde(default)]
    pub sent_bytes: u64,
}

/// Punto de la serie temporal (resolución 1s).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct TimePoint {
    pub t_secs: u64,
    pub throughput: f64,
    pub error_rate: f64,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub active_vus: u32,
    /// Bytes recibidos en ese segundo.
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBucket {
    pub message: String,
    pub count: u64,
}

/// Fila del heatmap de latencia: para un segundo, conteo por bucket (alineado a
/// [`HIST_BOUNDS_MS`]).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HeatRow {
    pub t_secs: u64,
    pub counts: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct RunConfig {
    pub virtual_users: u32,
    pub thread_groups: u32,
}

/// Detalle de una petición individual (modo depuración / vista previa).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SampleDetail {
    pub seq: u64,
    pub label: String,
    pub kind: SampleKind,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub req_headers: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub req_body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default)]
    pub resp_headers: Vec<(String, String)>,
    #[serde(default)]
    pub resp_body: String,
    pub latency_ms: f64,
    pub bytes: u64,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub extracted: Vec<(String, String)>,
    #[serde(default)]
    pub vars: Vec<(String, String)>,
}

/// Resumen completo de un run (machine-readable → `summary.json`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct RunSummary {
    pub run_id: String,
    pub scenario_name: String,
    pub started_at: String,
    pub duration_secs: f64,
    pub config: RunConfig,
    pub overall: LabelStats,
    pub labels: Vec<LabelStats>,
    pub timeseries: Vec<TimePoint>,
    pub errors: Vec<ErrorBucket>,
    /// Histograma de latencias (buckets fijos): límites superiores en ms.
    #[serde(default)]
    pub histogram_bounds_ms: Vec<f64>,
    /// Conteo por bucket del histograma de latencias (overall).
    #[serde(default)]
    pub histogram_counts: Vec<u64>,
    /// Conteo por código de estado HTTP.
    #[serde(default)]
    pub status_codes: Vec<(u16, u64)>,
    /// Conteo por categoría de error (status/assertion/timeout/connection/other).
    #[serde(default)]
    pub error_kinds: Vec<(String, u64)>,
    #[serde(default)]
    pub bytes_received: u64,
    #[serde(default)]
    pub bytes_sent: u64,
    /// Heatmap latencia×tiempo (filas por segundo, conteo por bucket).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub latency_heatmap: Vec<HeatRow>,
    /// Detalle por petición (solo si se activó la captura). Vacío en runs normales.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<SampleDetail>,
}

/// Snapshot ligero para el dashboard en vivo.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LiveSnapshot {
    pub elapsed_secs: f64,
    pub active_vus: u32,
    pub total_requests: u64,
    pub total_errors: u64,
    pub throughput_per_sec: f64,
    pub error_rate: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

// --- Internos de agregación ---

struct Acc {
    kind: SampleKind,
    hist: Histogram<u64>,
    ttfb: Histogram<u64>,
    count: u64,
    errors: u64,
    bytes: u64,
    sent: u64,
}

impl Acc {
    fn new(kind: SampleKind) -> Self {
        Self {
            kind,
            hist: Histogram::<u64>::new(3).expect("hdr histogram"),
            ttfb: Histogram::<u64>::new(3).expect("hdr histogram"),
            count: 0,
            errors: 0,
            bytes: 0,
            sent: 0,
        }
    }
    fn record(&mut self, s: &Sample) {
        let _ = self.hist.record(s.latency_us.max(1));
        let _ = self.ttfb.record(s.ttfb_us.max(1));
        self.count += 1;
        self.bytes += s.bytes;
        self.sent += s.sent_bytes;
        if !s.success {
            self.errors += 1;
        }
    }
    fn stats(&self, label: &str, duration_secs: f64) -> LabelStats {
        let q = |p: f64| self.hist.value_at_quantile(p) as f64 / 1000.0;
        let tp = if duration_secs > 0.0 {
            self.count as f64 / duration_secs
        } else {
            0.0
        };
        LabelStats {
            label: label.to_string(),
            kind: self.kind,
            count: self.count,
            errors: self.errors,
            error_rate: if self.count > 0 {
                self.errors as f64 / self.count as f64
            } else {
                0.0
            },
            throughput_per_sec: tp,
            min_ms: self.hist.min() as f64 / 1000.0,
            mean_ms: self.hist.mean() / 1000.0,
            max_ms: self.hist.max() as f64 / 1000.0,
            p50_ms: q(0.50),
            p90_ms: q(0.90),
            p95_ms: q(0.95),
            p99_ms: q(0.99),
            p999_ms: q(0.999),
            ttfb_mean_ms: self.ttfb.mean() / 1000.0,
            ttfb_p95_ms: self.ttfb.value_at_quantile(0.95) as f64 / 1000.0,
            bytes_total: self.bytes,
            sent_bytes: self.sent,
        }
    }
}

struct SecondAcc {
    count: u64,
    errors: u64,
    sum_us: u64,
    bytes: u64,
    hist_p95: Option<Histogram<u64>>,
    buckets: Vec<u64>,
    active_vus: u32,
}

impl Default for SecondAcc {
    fn default() -> Self {
        Self {
            count: 0,
            errors: 0,
            sum_us: 0,
            bytes: 0,
            hist_p95: None,
            buckets: vec![0; nbuckets()],
            active_vus: 0,
        }
    }
}

/// Agregador de muestras. No usa async ni locks: el engine lo conduce en un único
/// punto, alimentándolo con eventos y consultando snapshots.
pub struct Recorder {
    run_id: String,
    started_at: String,
    config: RunConfig,
    overall: Acc,
    by_label: BTreeMap<String, Acc>,
    seconds: BTreeMap<u64, SecondAcc>,
    errors: BTreeMap<String, u64>,
    hist_counts: Vec<u64>,
    status: BTreeMap<u16, u64>,
    error_kinds: BTreeMap<String, u64>,
    current_vus: u32,
}

impl Recorder {
    pub fn new(
        run_id: impl Into<String>,
        started_at: impl Into<String>,
        config: RunConfig,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            started_at: started_at.into(),
            config,
            overall: Acc::new(SampleKind::Http),
            by_label: BTreeMap::new(),
            seconds: BTreeMap::new(),
            errors: BTreeMap::new(),
            hist_counts: vec![0; nbuckets()],
            status: BTreeMap::new(),
            error_kinds: BTreeMap::new(),
            current_vus: 0,
        }
    }

    pub fn record_vus(&mut self, offset_ms: u64, active: u32) {
        self.current_vus = active;
        let sec = offset_ms / 1000;
        let e = self.seconds.entry(sec).or_default();
        e.active_vus = e.active_vus.max(active);
    }

    pub fn record_sample(&mut self, s: &Sample) {
        if s.kind != SampleKind::Transaction {
            self.overall.record(s);
            let bi = bucket_index(s.latency_us as f64 / 1000.0);
            self.hist_counts[bi] += 1;
            if let Some(code) = s.status {
                *self.status.entry(code).or_insert(0) += 1;
            }
            if let Some(k) = &s.error_kind {
                *self.error_kinds.entry(k.clone()).or_insert(0) += 1;
            }
            let sec = s.offset_ms / 1000;
            let acc = self.seconds.entry(sec).or_default();
            acc.count += 1;
            acc.sum_us += s.latency_us;
            acc.bytes += s.bytes;
            acc.buckets[bi] += 1;
            if !s.success {
                acc.errors += 1;
            }
            let h = acc
                .hist_p95
                .get_or_insert_with(|| Histogram::<u64>::new(2).expect("hdr"));
            let _ = h.record(s.latency_us.max(1));
            if acc.active_vus == 0 {
                acc.active_vus = self.current_vus;
            }
        }
        self.by_label
            .entry(s.label.clone())
            .or_insert_with(|| Acc::new(s.kind))
            .record(s);
        if let Some(err) = &s.error {
            *self.errors.entry(err.clone()).or_insert(0) += 1;
        }
    }

    pub fn live_snapshot(&self, elapsed_ms: u64) -> LiveSnapshot {
        let elapsed_secs = elapsed_ms as f64 / 1000.0;
        let q = |p: f64| self.overall.hist.value_at_quantile(p) as f64 / 1000.0;
        LiveSnapshot {
            elapsed_secs,
            active_vus: self.current_vus,
            total_requests: self.overall.count,
            total_errors: self.overall.errors,
            throughput_per_sec: if elapsed_secs > 0.0 {
                self.overall.count as f64 / elapsed_secs
            } else {
                0.0
            },
            error_rate: if self.overall.count > 0 {
                self.overall.errors as f64 / self.overall.count as f64
            } else {
                0.0
            },
            p50_ms: q(0.50),
            p95_ms: q(0.95),
            p99_ms: q(0.99),
        }
    }

    pub fn finish(self, scenario_name: impl Into<String>, duration_secs: f64) -> RunSummary {
        let overall = self.overall.stats("ALL", duration_secs);
        let mut labels: Vec<LabelStats> = self
            .by_label
            .iter()
            .map(|(k, v)| v.stats(k, duration_secs))
            .collect();
        labels.sort_by(|a, b| {
            b.p95_ms
                .partial_cmp(&a.p95_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let timeseries: Vec<TimePoint> = self
            .seconds
            .iter()
            .map(|(sec, acc)| TimePoint {
                t_secs: *sec,
                throughput: acc.count as f64,
                error_rate: if acc.count > 0 {
                    acc.errors as f64 / acc.count as f64
                } else {
                    0.0
                },
                avg_ms: if acc.count > 0 {
                    (acc.sum_us as f64 / acc.count as f64) / 1000.0
                } else {
                    0.0
                },
                p95_ms: acc
                    .hist_p95
                    .as_ref()
                    .map(|h| h.value_at_quantile(0.95) as f64 / 1000.0)
                    .unwrap_or(0.0),
                active_vus: acc.active_vus,
                bytes: acc.bytes,
            })
            .collect();

        let latency_heatmap: Vec<HeatRow> = self
            .seconds
            .iter()
            .map(|(sec, acc)| HeatRow {
                t_secs: *sec,
                counts: acc.buckets.clone(),
            })
            .collect();

        let mut errors: Vec<ErrorBucket> = self
            .errors
            .into_iter()
            .map(|(message, count)| ErrorBucket { message, count })
            .collect();
        errors.sort_by_key(|e| std::cmp::Reverse(e.count));

        let status_codes: Vec<(u16, u64)> = self.status.into_iter().collect();
        let mut error_kinds: Vec<(String, u64)> = self.error_kinds.into_iter().collect();
        error_kinds.sort_by_key(|e| std::cmp::Reverse(e.1));

        RunSummary {
            run_id: self.run_id,
            scenario_name: scenario_name.into(),
            started_at: self.started_at,
            duration_secs,
            config: self.config,
            bytes_received: overall.bytes_total,
            bytes_sent: overall.sent_bytes,
            overall,
            labels,
            timeseries,
            errors,
            histogram_bounds_ms: HIST_BOUNDS_MS.to_vec(),
            histogram_counts: self.hist_counts,
            status_codes,
            error_kinds,
            latency_heatmap,
            details: Vec::new(),
        }
    }
}

/// Consolida varios [`RunSummary`] de workers en uno (ejecución distribuida).
/// Conteos/throughput/bytes/histogramas/heatmap se suman; los percentiles por etiqueta
/// se combinan con promedio ponderado por conteo (aproximación).
pub fn merge_summaries(parts: &[RunSummary]) -> RunSummary {
    let first = &parts[0];
    let duration = parts
        .iter()
        .map(|p| p.duration_secs)
        .fold(0.0_f64, f64::max)
        .max(0.001);

    let overalls: Vec<&LabelStats> = parts.iter().map(|p| &p.overall).collect();
    let overall = merge_label_stats("ALL", SampleKind::Http, &overalls);

    let mut groups: BTreeMap<String, Vec<&LabelStats>> = BTreeMap::new();
    for p in parts {
        for l in &p.labels {
            groups.entry(l.label.clone()).or_default().push(l);
        }
    }
    let mut labels: Vec<LabelStats> = groups
        .iter()
        .map(|(name, v)| merge_label_stats(name, v[0].kind, v))
        .collect();
    labels.sort_by(|a, b| {
        b.p95_ms
            .partial_cmp(&a.p95_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut tsmap: BTreeMap<u64, Vec<&TimePoint>> = BTreeMap::new();
    for p in parts {
        for t in &p.timeseries {
            tsmap.entry(t.t_secs).or_default().push(t);
        }
    }
    let timeseries = tsmap
        .iter()
        .map(|(t, v)| {
            let throughput: f64 = v.iter().map(|x| x.throughput).sum();
            let errsum: f64 = v.iter().map(|x| x.error_rate * x.throughput).sum();
            let avgsum: f64 = v.iter().map(|x| x.avg_ms * x.throughput.max(1.0)).sum();
            let wsum: f64 = v.iter().map(|x| x.throughput.max(1.0)).sum();
            TimePoint {
                t_secs: *t,
                throughput,
                error_rate: if throughput > 0.0 {
                    errsum / throughput
                } else {
                    0.0
                },
                avg_ms: if wsum > 0.0 { avgsum / wsum } else { 0.0 },
                p95_ms: v.iter().map(|x| x.p95_ms).fold(0.0, f64::max),
                active_vus: v.iter().map(|x| x.active_vus).sum(),
                bytes: v.iter().map(|x| x.bytes).sum(),
            }
        })
        .collect();

    let mut errmap: BTreeMap<String, u64> = BTreeMap::new();
    for p in parts {
        for e in &p.errors {
            *errmap.entry(e.message.clone()).or_insert(0) += e.count;
        }
    }
    let mut errors: Vec<ErrorBucket> = errmap
        .into_iter()
        .map(|(message, count)| ErrorBucket { message, count })
        .collect();
    errors.sort_by_key(|e| std::cmp::Reverse(e.count));

    // Histograma: suma elemento a elemento (buckets fijos).
    let nb = nbuckets();
    let mut histogram_counts = vec![0u64; nb];
    for p in parts {
        for (i, c) in p.histogram_counts.iter().enumerate().take(nb) {
            histogram_counts[i] += c;
        }
    }
    // Status / error kinds: suma por clave.
    let mut statusmap: BTreeMap<u16, u64> = BTreeMap::new();
    let mut kindmap: BTreeMap<String, u64> = BTreeMap::new();
    for p in parts {
        for (k, c) in &p.status_codes {
            *statusmap.entry(*k).or_insert(0) += c;
        }
        for (k, c) in &p.error_kinds {
            *kindmap.entry(k.clone()).or_insert(0) += c;
        }
    }
    let mut error_kinds: Vec<(String, u64)> = kindmap.into_iter().collect();
    error_kinds.sort_by_key(|e| std::cmp::Reverse(e.1));
    // Heatmap: por segundo, suma de vectores de bucket.
    let mut heatmap: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for p in parts {
        for row in &p.latency_heatmap {
            let e = heatmap.entry(row.t_secs).or_insert_with(|| vec![0; nb]);
            for (i, c) in row.counts.iter().enumerate().take(nb) {
                e[i] += c;
            }
        }
    }
    let latency_heatmap: Vec<HeatRow> = heatmap
        .into_iter()
        .map(|(t_secs, counts)| HeatRow { t_secs, counts })
        .collect();

    RunSummary {
        run_id: "distributed".to_string(),
        scenario_name: first.scenario_name.clone(),
        started_at: first.started_at.clone(),
        duration_secs: duration,
        config: RunConfig {
            virtual_users: parts.iter().map(|p| p.config.virtual_users).sum(),
            thread_groups: first.config.thread_groups,
        },
        bytes_received: parts.iter().map(|p| p.bytes_received).sum(),
        bytes_sent: parts.iter().map(|p| p.bytes_sent).sum(),
        overall,
        labels,
        timeseries,
        errors,
        histogram_bounds_ms: HIST_BOUNDS_MS.to_vec(),
        histogram_counts,
        status_codes: statusmap.into_iter().collect(),
        error_kinds,
        latency_heatmap,
        details: Vec::new(),
    }
}

fn merge_label_stats(label: &str, kind: SampleKind, parts: &[&LabelStats]) -> LabelStats {
    let count: u64 = parts.iter().map(|p| p.count).sum();
    let errors: u64 = parts.iter().map(|p| p.errors).sum();
    let total = count.max(1) as f64;
    let wavg = |sel: fn(&LabelStats) -> f64| -> f64 {
        parts.iter().map(|p| sel(p) * p.count as f64).sum::<f64>() / total
    };
    LabelStats {
        label: label.to_string(),
        kind,
        count,
        errors,
        error_rate: if count > 0 {
            errors as f64 / count as f64
        } else {
            0.0
        },
        throughput_per_sec: parts.iter().map(|p| p.throughput_per_sec).sum(),
        min_ms: parts
            .iter()
            .map(|p| p.min_ms)
            .fold(f64::MAX, f64::min)
            .min(f64::MAX),
        mean_ms: wavg(|p| p.mean_ms),
        max_ms: parts.iter().map(|p| p.max_ms).fold(0.0, f64::max),
        p50_ms: wavg(|p| p.p50_ms),
        p90_ms: wavg(|p| p.p90_ms),
        p95_ms: wavg(|p| p.p95_ms),
        p99_ms: wavg(|p| p.p99_ms),
        p999_ms: wavg(|p| p.p999_ms),
        ttfb_mean_ms: wavg(|p| p.ttfb_mean_ms),
        ttfb_p95_ms: wavg(|p| p.ttfb_p95_ms),
        bytes_total: parts.iter().map(|p| p.bytes_total).sum(),
        sent_bytes: parts.iter().map(|p| p.sent_bytes).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(i: u64) -> Sample {
        Sample {
            label: "GET /".into(),
            kind: SampleKind::Http,
            offset_ms: i * 10,
            latency_us: (i + 1) * 1000,
            ttfb_us: (i + 1) * 500,
            status: Some(if i.is_multiple_of(10) { 500 } else { 200 }),
            bytes: 128,
            sent_bytes: 64,
            success: !i.is_multiple_of(10),
            error: if i.is_multiple_of(10) {
                Some("boom".into())
            } else {
                None
            },
            error_kind: if i.is_multiple_of(10) {
                Some("status".into())
            } else {
                None
            },
        }
    }

    #[test]
    fn aggregates_basic() {
        let mut r = Recorder::new("run-1", "2026-01-01T00:00:00Z", RunConfig::default());
        for i in 0..100u64 {
            r.record_sample(&sample(i));
        }
        let s = r.finish("demo", 1.0);
        assert_eq!(s.overall.count, 100);
        assert_eq!(s.overall.errors, 10);
        assert!((s.overall.error_rate - 0.10).abs() < 1e-9);
        assert!(s.overall.p95_ms > 0.0);
        assert!(s.overall.ttfb_p95_ms > 0.0);
        assert_eq!(s.errors[0].message, "boom");
        // Histograma suma al total
        assert_eq!(s.histogram_counts.iter().sum::<u64>(), 100);
        assert_eq!(s.histogram_bounds_ms, HIST_BOUNDS_MS.to_vec());
        // Status codes
        let s200 = s
            .status_codes
            .iter()
            .find(|(c, _)| *c == 200)
            .map(|(_, n)| *n)
            .unwrap_or(0);
        assert_eq!(s200, 90);
        assert_eq!(
            s.error_kinds
                .iter()
                .find(|(k, _)| k == "status")
                .map(|(_, n)| *n),
            Some(10)
        );
        assert!(s.bytes_received >= 100 * 128);
        assert!(!s.latency_heatmap.is_empty());
    }

    #[test]
    fn merge_sums_histograms() {
        let mk = || {
            let mut r = Recorder::new(
                "w",
                "t",
                RunConfig {
                    virtual_users: 5,
                    thread_groups: 1,
                },
            );
            for i in 0..50u64 {
                r.record_sample(&sample(i));
            }
            r.finish("demo", 1.0)
        };
        let merged = merge_summaries(&[mk(), mk()]);
        assert_eq!(merged.overall.count, 100);
        assert_eq!(merged.histogram_counts.iter().sum::<u64>(), 100);
        assert_eq!(merged.config.virtual_users, 10);
    }
}
