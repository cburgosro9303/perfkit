# Benchmark perfkit vs Apache JMeter

Escenario HTTP de referencia (GET), mismo plan (`examples/jmx/bench-http.jmx`), mismo target local, 20s, igual concurrencia. perfkit en `--release`; JMeter `-n` (no-GUI) sobre JDK 21.

| Métrica | perfkit | JMeter | Ventaja |
|---|---:|---:|---:|
| Throughput (req/s) | 116,587 | 123,863 | 0.94x |
| Requests totales | 2,331,797 | 2,477,253 | 0.94x |
| Errores | 0 | 0 |  |
| Latencia p50 (ms) | 0.4 | 0.0 |  |
| Latencia p95 (ms) | 0.7 | 1.0 |  |
| Latencia p99 (ms) | 0.9 | 1.0 |  |
| Memoria pico RSS (MB) | 20 | 906 | 46.16x menos |

**Resumen:** perfkit ≈ 0.94x throughput y 46.2x menos memoria pico que JMeter en este escenario. La latencia debe ser comparable (ambos saturan el mismo target); la ventaja real del motor Rust/Tokio está en memoria por VU y overhead de reporte.

> Reproducir: `bash tools/benchmark.sh [threads] [duration]`
