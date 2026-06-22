# ADR-005: Reportes nativos vs export de observabilidad

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** reporting-analytics-lead, observability-lead

## Contexto

El público objetivo es QA tradicional que hoy lee reportes de JMeter. El plan es
explícito (§2.8, §6.7, §6.10, §11.7): **OpenTelemetry y Prometheus son exports de
observabilidad, no sustitutos del reporte nativo**; el reporte debe funcionar
offline y ser compartible como artefacto de pipeline; y la telemetría nunca debe
ser requisito para ver resultados ni contaminar el hot path. Un QA debe poder
abrir un archivo y entender el resultado sin levantar Grafana ni un collector.

## Decisión

### El reporte nativo es el artefacto primario

El crate `reports` genera tres formatos a partir del `RunSummary` de `metrics`:

- **HTML offline autocontenido** (`html_report`): un único `.html` que **abre sin
  servidor ni red** (datos embebidos en el propio archivo, empieza con
  `<!doctype html>`). Es el artefacto compartible de CI.
- **JSON machine-readable** (`summary_json` → `summary.json`): consumible por la
  CLI, por el quality gate y por integraciones.
- **JUnit XML** (`junit_xml`): para que los pipelines de CI muestren el resultado
  como un test suite.

El `RunSummary` incluye lo que el QA reconoce: percentiles **p50/p90/p95/p99/p99.9**,
throughput, error rate, series temporales por segundo, estadísticas por etiqueta y
resumen de errores.

### Quality gate integrado

`reports::evaluate_gate` compara el `summary.json` contra un `thresholds.yaml`
(`max_error_rate`, `max_p95_ms`, `max_p99_ms`, `min_throughput_per_sec`) y devuelve
un resultado pass/fail por check. La CLI (`perfkit gate`) lo usa para fallar el
pipeline con exit codes confiables.

### Prometheus / OTLP: exports posteriores y opcionales

La observabilidad (export Prometheus, OTLP metrics/logs/traces, inyección de trace
context) se añade **después** como capa opcional y **asíncrona**, sin bloquear el
hot path. **Nunca** reemplaza al reporte nativo: un run sin collector configurado
sigue produciendo HTML/JSON/JUnit completos.

## Consecuencias

**Positivas**

- El QA obtiene un reporte familiar y offline sin infraestructura.
- El HTML autocontenido es trivial de adjuntar como artefacto de CI o compartir.
- El JSON + gate habilitan CI con umbrales y exit codes desde el MVP.
- Desacopla "ver resultados" de "tener observabilidad", reduciendo barreras de adopción.

**Negativas / costos**

- Mantener tres formatos de reporte tiene costo (consistencia entre HTML/JSON/JUnit).
- El HTML autocontenido puede crecer si se embeben muchas series; hay que vigilar
  el tamaño del artefacto.
- Habrá dos caminos de salida (reporte nativo y export OTel/Prometheus) que deben
  mantenerse coherentes en sus métricas.

## Alternativas consideradas

- **Solo Prometheus/Grafana (sin reporte nativo):** descartado por §2.8; obligaría a
  montar infraestructura para leer un resultado y alejaría al QA tradicional.
- **HTML que carga datos por red / CDN:** descartado; rompe el requisito offline y de
  artefacto autocontenido de pipeline.
- **Reporte solo JSON (sin HTML):** descartado; el QA espera un reporte visual
  reconocible, no solo datos crudos.
- **OTel como requisito de ejecución:** descartado; la telemetría es opcional y nunca
  prerequisito para ver resultados.
