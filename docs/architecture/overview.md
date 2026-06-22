# Visión general de la arquitectura (MVP)

`perfkit` es una suite de performance testing en Rust + Tokio cuyo objetivo es
reemplazar Apache JMeter para QA tradicional. El núcleo del MVP es un flujo simple:
**importar un JMX → obtener un IR canónico (YAML) → ejecutar carga HTTP local →
producir un reporte familiar y un quality gate para CI**. Las decisiones que
sustentan esta arquitectura están en los ADR-001 a ADR-007.

## Principios

- **El IR es el contrato central.** El motor ejecuta el IR, no el JMX. Importador,
  engine, reports, CLI y UI se comunican a través del IR (ADR-002).
- **Migración por niveles, sin fallar en silencio.** Todo elemento JMX queda
  clasificado `migrated | assisted | unsupported | ignored` (ADR-003).
- **Reporte nativo offline primero.** Prometheus/OTLP son exports posteriores y
  opcionales, no reemplazo (ADR-005).
- **Separación de responsabilidades por crate.** Workspace Cargo, edición 2024,
  toolchain Rust 1.95.0 (ADR-001).

## Flujo JMX → IR → engine → reporte

```text
   archivo.jmx
       │
       ▼
┌──────────────────┐     escenario.yaml (IR)
│  jmx-importer    │────────────────────────────────┐
│  (roxmltree)     │                                 │
│  mapeo Nivel 1   │──► migration-report.json        │
└──────────────────┘    (migrated|assisted|          │
       ▲                  unsupported|ignored)        │
       │                                              ▼
       │                                    ┌──────────────────┐
   perfkit import jmx                       │   scenario-ir    │
                                            │  IR + serde +    │
   escenario.yaml ──── perfkit validate ──►│  JSON Schema +   │
                                            │  validador       │
                                            └────────┬─────────┘
                                                     │  IR validado
                                                     ▼
                                            ┌──────────────────┐
                                            │     engine       │
                                            │  Tokio scheduler │
                                            │  ramp-up/hold    │
                                            │  VUs async       │──► http-adapter
                                            │  timers/assert/  │   (reqwest/rustls,
                                            │  extractores     │    cookie store/VU)
                                            └────────┬─────────┘
                                       Sample (mpsc) │
                                                     ▼
                                            ┌──────────────────┐
                                            │     metrics      │
                                            │  hdrhistogram    │
                                            │  p50/p90/p95/    │
                                            │  p99/p99.9,      │
                                            │  throughput,     │
                                            │  error rate,     │
                                            │  series/seg      │
                                            └────────┬─────────┘
                                          RunSummary │
                                                     ▼
                                            ┌──────────────────┐
                                            │     reports      │
                                            │  HTML offline    │──► reports/<run>/index.html
                                            │  JSON            │──► summary.json
                                            │  JUnit XML       │──► junit.xml
                                            │  quality gate    │──► perfkit gate (exit code)
                                            └──────────────────┘

   La UI (Tauri 2 + React/TS) llama in-process a estos crates y muestra
   métricas en vivo (LiveSnapshot) por eventos Tauri.   (ADR-007)
```

## Tabla de crates

| Crate          | Responsabilidad                                                              | Dependencias clave        |
|----------------|------------------------------------------------------------------------------|---------------------------|
| `scenario-ir`  | IR canónico, serde, JSON Schema (schemars), validador, reporte de fidelidad  | serde, serde_yaml_ng, schemars |
| `jmx-importer` | Parser JMX, mapeo Nivel 1 a IR, reporte de fidelidad (nunca falla en silencio) | roxmltree, scenario-ir    |
| `http-adapter` | Ejecución HTTP/HTTPS, cookie store por VU, headers/redirects/timeouts        | reqwest (rustls)          |
| `engine`       | Scheduler ramp-up/hold, VUs async, timers, assertions, extractores, datasets | tokio, http-adapter, scenario-ir |
| `metrics`      | Histogramas, percentiles, throughput, error rate, series por segundo         | hdrhistogram, serde       |
| `reports`      | HTML offline / JSON / JUnit + quality gate                                   | html-escape, metrics      |
| `security`     | Secretos por entorno + redacción (stub de la política completa)              | std env                   |
| `cli`          | Binario `perfkit` (validate / import / convert / run / debug / gate / schema)| clap, todos los anteriores |
| `ui/src-tauri` | App nativa de escritorio (Tauri 2 + React/TS/Tailwind), in-process al core   | tauri, crates por path    |

## Componentes diferidos (post-MVP)

`observability` (Prometheus/OTLP), `plugin-host` (WASM/WASI), modo compatibilidad
JVM (Nivel 3), ejecución distribuida (coordinator/worker), Kafka e IA gobernada.
Ninguno bloquea el MVP local; ver las fases del plan (§9) y los ADR-005/006/007.
