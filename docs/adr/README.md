# Architecture Decision Records (ADR)

Registro de las decisiones arquitectónicas de **perfkit** (suite de performance
testing en Rust + Tokio, reemplazo de Apache JMeter). Cada ADR documenta una
decisión, su contexto, sus consecuencias y las alternativas descartadas.

## Formato

Todos los ADR siguen la misma estructura: **Título · Estado · Contexto · Decisión ·
Consecuencias · Alternativas consideradas**. El estado inicial de estos ADR es
**Aceptado**. Si una decisión se revisa, se crea un nuevo ADR que la sustituye o
modifica (no se reescribe la historia).

## Índice

| ADR | Título | Estado | Resumen |
|-----|--------|--------|---------|
| [ADR-001](ADR-001-arquitectura-general.md) | Arquitectura general | Aceptado | Open-core, workspace de crates, separación IR/importer/engine/adapters/reports/UI, toolchain Rust 1.95.0 (desviación del 1.96.0 pedido). |
| [ADR-002](ADR-002-ir-canonico-y-versionado.md) | IR canónico y versionado | Aceptado | IR como contrato central, YAML (serde_yaml_ng) + JSON Schema (schemars), versionado semver (`IR_VERSION = 0.1.0`), regla cambiar-IR ⇒ schema+fixtures+docs. |
| [ADR-003](ADR-003-estrategia-migracion-jmx.md) | Estrategia de migración JMX | Aceptado | Cuatro niveles de fidelidad, reporte `migrated\|assisted\|unsupported\|ignored`, nunca fallar en silencio, catálogo Nivel 1 del MVP. |
| [ADR-004](ADR-004-modelo-ejecucion-local.md) | Modelo de ejecución local | Aceptado | Tokio + reqwest(rustls), VUs async, scheduler ramp-up/hold, cookie store por VU, reloj monotónico, agregación fuera del hot path, cancelación cooperativa. |
| [ADR-005](ADR-005-reportes-vs-observability.md) | Reportes vs observabilidad | Aceptado | Reporte nativo HTML(offline)/JSON/JUnit como primario; Prometheus/OTLP como exports posteriores y opcionales; HTML como artefacto de CI. |
| [ADR-006](ADR-006-seguridad-secretos-ia.md) | Seguridad, secretos e IA | Aceptado | Secretos por entorno (no versionados), redacción de logs/reportes, IA local/BYOK/SaaS-opt-in (SaaS off por defecto), plugins firmados/WASM (futuro). |
| [ADR-007](ADR-007-ui-nativa-tauri.md) | UI nativa con Tauri 2 | Aceptado | App de escritorio Tauri 2 + React/TS/Tailwind, shell llama in-process a los crates (sin servidor HTTP), métricas en vivo por eventos; el QA nunca escribe TypeScript. |
| [ADR-008](ADR-008-controllers-ir-v0.2.md) | Throughput/Interleave/Random controllers | Aceptado | IR v0.2.0: nuevos controladores con semántica verificada por tests; importer + engine + schema + fixtures. |
| [ADR-009](ADR-009-kafka-sampler-ir-v0.3.md) | Kafka producer sampler | Aceptado | IR v0.3.0: `Step::Kafka` + `SampleKind::Kafka` + crate `kafka-adapter` (rskafka); reporte distingue HTTP/Kafka; broker real diferido. |
| [ADR-010](ADR-010-fases-6-a-10.md) | Fases 6–10 (distribuido, plugins, IA, enterprise) | Aceptado | Slices MVP de cluster (HTTP/JSON), plugins WASM firmados (wasmi), IA gobernada (SaaS off) e histórico SQLite (RBAC/baseline/regresión); infra real diferida. |

## Documentos relacionados

- [Visión general de la arquitectura](../architecture/overview.md) — diagrama del
  flujo JMX→IR→engine→reporte y tabla de crates.
- [Migrar tu primer JMX](../migration/migrar-tu-primer-jmx.md) — guía paso a paso para QA.
- `CLAUDE_OPUS_IMPLEMENTATION_PLAN.md` (raíz del repo) — plan de implementación y
  decisiones no negociables (§2, §4, §5) que sustentan estos ADR.
