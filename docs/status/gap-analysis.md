# Análisis de brechas — perfkit vs. plan original

> Documento honesto y basado en evidencia. Compara lo que el `CLAUDE_OPUS_IMPLEMENTATION_PLAN.md` pidió contra lo que está **realmente implementado en el código**, distinguiendo siempre entre *implementado a profundidad MVP* y *completo a nivel producción*. Cada afirmación cita rutas de archivo como evidencia.
>
> Fecha del análisis: 2026-06-21 · IR actual: `0.3.0` (`crates/scenario-ir/src/model.rs:13`)

---

## 1. Resumen ejecutivo

- **El flujo central QA está completo end-to-end** (el "principio rector" del §16): importar un JMX → IR/YAML → reporte de fidelidad → ejecutar carga HTTP local (CLI y UI nativa Tauri) → reporte HTML/JSON/JUnit offline → quality gate con exit codes en CI. Esto cubre los 10 criterios de aceptación del MVP (§11) salvo matices en el benchmark (criterio 9) que se explican abajo.
- **El motor Rust/Tokio es real, no simulado**: VUs asíncronas, scheduler ramp-up/hold/ramp-down, timers, assertions, extractores, controllers (loop/if/while/transaction/throughput/interleave/random), medición con reloj monotónico y agregación con HdrHistogram fuera del hot path (`crates/engine/src/lib.rs`, `crates/metrics/src/lib.rs`).
- **Las Fases 6–10 se recorrieron como "slices MVP funcionales", no como producto productivo.** Cluster (HTTP/JSON, sin gRPC/mTLS ni operador K8s), Kafka (rskafka real pero sin SASL/SSL ni broker validado), plugins WASM (firma + sandbox reales, pero solo plugins puros y registry de primera parte), IA (gobernanza real pero proveedor solo heurístico local; sin LLM real), y enterprise (SQLite real con baselines/regresión/auditoría, pero RBAC no forzado y sin SSO).
- **Diferidos explícitos y documentados** (ADR-010 principalmente): gRPC+mTLS y operador K8s real, broker Kafka real, registry de terceros con firma en producción, SaaS de IA, SSO/multi-tenant gestionado, export Prometheus/OTLP, DSL TypeScript, y modo compatibilidad JVM sidecar para JSR223.
- **Brecha de calidad detectada y RESUELTA en esta sesión**: este análisis descubrió que `cargo test --workspace` **no compilaba** — tres módulos `#[cfg(test)]` (`engine`, `history`, `ai-assist`) habían quedado desincronizados de structs que ganaron campos (`Ctx.interleave`, `LabelStats.ttfb_*`/`sent_bytes`, `RunSummary.bytes_*`/`details`). Se corrigieron los inicializadores de test y **ahora el suite está en verde: 28 suites OK, 79 tests pasando, 0 fallos**. Queda como deuda real que los directorios `tests/golden|integration|benchmarks` existen pero están **vacíos** (no hay golden snapshots ni suite de paridad formal vs JMeter).

---

## 2. Estado por fase (0–10)

| Fase | Alcance del plan | Estado | Evidencia | Qué falta |
|---|---|---|---|---|
| **0 — Inception** | ADR-001..006, schema IR, schema fidelidad, 10 fixtures JMX, matriz JMeter→nivel, wireframes UX, plan de benchmark | ✅ Implementado | `docs/adr/ADR-001..010`, `schemas/scenario-ir.schema.json`, `schemas/migration-report.schema.json`, 14 JMX en `examples/jmx/`, `docs/migration/jmeter-support-matrix.md`, `docs/benchmarks/` | Wireframes formales no presentes (se saltó a UI real). ADRs van más allá (007–010). |
| **1 — Bootstrap core + CLI** | Workspace, `scenario-ir`, YAML→IR, validador, CLI `validate`/`import jmx`, parser XML/JMX, mapping TestPlan/ThreadGroup/HTTP/Header/CSV, fidelidad JSON, golden | ✅ Implementado | `crates/scenario-ir/{model,validate,lib}.rs`, `crates/jmx-importer/src/lib.rs`, `crates/cli/src/main.rs:526-568`, `crates/jmx-importer/tests/fixtures.rs` | Golden formal es un smoke test (importa los 14 fixtures sin fallo silencioso), no golden snapshots versionados. |
| **2 — MVP engine HTTP local** | Scheduler, VUs async, HTTP/S, vars+CSV, cookies/headers, timers, assertions, extractores, CLI `run`, summary JSON, HTML, Prometheus opcional | 🟡 Parcial | `crates/engine/src/lib.rs` (todo lo de carga), `crates/http-adapter/src/lib.rs`, `crates/reports/{html,junit,lib}.rs`, `crates/cli/src/main.rs:570-614` | **Export Prometheus NO existe** (solo se menciona en doc-comment `crates/reports/src/lib.rs:3`). Constant Throughput Timer es aproximación por-VU, no global (ver §3/§4). TTFB es aproximado (tiempo a headers). El resto: ✅. |
| **3 — UI MVP para QA** | App UI, import JMX, árbol, vista fidelidad, editores HTTP/vars/assertions/timers, run local, dashboard live, reporte post-run | ✅ Implementado | `ui/src/views/{Import,Plan,Run,Report,History,Help}View.tsx`, `ui/src/components/{PlanTree,StepEditor,FidelityPanel,Chart,Bars,Heatmap}.tsx`, `ui/src-tauri/src/lib.rs` | UI **edita el IR en memoria**; persiste solo vía "Exportar" (no auto-save). Datasets CSV se ven pero no se editan inline. Controllers Interleave/Random/Throughput soportados en IR pero con edición limitada en UI. |
| **4 — Migración JMX profunda + semántica** | Más controllers/timers, Transaction, Throughput, extractores/assertions avanzados, detección JSR223/Groovy/BeanShell, migración asistida, fidelidad enriquecida, comparativas JMeter, spike JVM | 🟡 Parcial | `crates/jmx-importer/src/lib.rs` (clasificador `classify_unknown` :680-695, `analyze_script` :702-745, `condition_support` :747-759), `crates/engine/tests/semantics.rs`, ADR-008 | **Sin XPath** (no está en el IR). Condiciones if/while solo `== / != / true / false` (resto → assisted). **Spike de modo compatibilidad JVM NO se hizo.** Comparativa JMeter existe pero como script de benchmark, no como suite automatizada de paridad semántica. |
| **5 — CI, Docker, gates, release** | Docker image, GitHub Actions, JUnit, gates, thresholds YAML, exit codes, artefactos, release multi-arch, SBOM, signing | 🟡 Parcial | `Dockerfile`, `.github/`, `crates/reports/{junit,gate}.rs`, `crates/cli/src/main.rs:706-730` (gate), `docs/deploy/{ci-cd,docker}.md`, `Makefile` | JUnit/gates/exit codes/Docker: ✅. **SBOM y signing de release: no hay evidencia en repo** (solo signing de *plugins* en plugin-host). Release multi-arch documentado pero no verificado. |
| **6 — Distribuido + Kubernetes** | Coordinator, worker, gRPC, mTLS, distribución real, barrier start, health, agregación, Compose, CRD `LoadTest`, operator, Helm | 🟡 Parcial | `crates/cluster/src/lib.rs` (split de VUs :85-172, merge :141), `deploy/{kubernetes,helm,operator,docker-compose.distributed.yml}` | **Transporte es HTTP/JSON, NO gRPC+mTLS** (`crates/cluster/src/lib.rs:4-5`). Reparte VUs y consolida métricas (real), pero **sin barrier start** (workers arrancan independientes), health solo `/health`→"ok". **Operator es solo diseño/manifiestos YAML, sin binario** (`deploy/operator/README.md`). |
| **7 — Kafka y eventos** | Producer sampler, SSL/SASL, templating, data-driven, assertions sobre publish, consumer validation, Schema Registry, métricas por topic/partición | 🟡 Parcial | `crates/kafka-adapter/src/lib.rs` (rskafka real :81-111, templating :32-75), `Step::Kafka` en IR, ADR-009 | **rskafka real, pero sin SASL/SSL** (solo comentario `:6`), compresión hardcodeada a None (`:107`). **Sin consumer validation, sin Schema Registry, sin métricas por partición.** Broker real **no validado** (ADR-009: "fuera del alcance verificado aquí"). |
| **8 — Plugins WASM + registry** | WASM host, ABI, manifest, permisos declarativos, firma/verificación, version pinning, revocación, registry curado, SDK | 🟡 Parcial | `crates/plugin-host/src/lib.rs` (wasmi :217-311, ed25519 :164-198, fuel :276, registry+revocación :378-434) | **Firma ed25519 + hash SHA-256 + sandbox con fuel: reales y bien testeados.** Pero MVP solo permite **plugins puros (sin imports)**; `allow_net/env/fs` declarados pero no implementados (siempre `false`). **Registry de terceros diferido**; solo primera parte. SDK de plugin no presente. |
| **9 — IA gobernada** | IA local, BYOK, SaaS opt-in, redacción/anonimización, allowlist, análisis Groovy, sugerencias correlación/thresholds, explicación | 🟡 Parcial | `crates/ai-assist/{lib,suggest,redact}.rs` (modos :31-41, `assert_saas_allowed` :110-116, `preview_payload` :89-104, redacción regex :12-28) | **Gobernanza real y sólida** (SaaS off por defecto, nada sale sin opt-in, todo `requires_confirmation`, redacción con 7 patrones). Pero el proveedor "local" es **heurístico por reglas, no un LLM**; BYOK/SaaS son *traits* sin implementación de red (responsabilidad del caller). No hay IA generativa real. |
| **10 — Enterprise histórico + colaboración** | Histórico centralizado, baseline, trends, regression detection, RBAC, SSO, auditoría, projects/teams, annotations, retention | 🟡 Parcial | `crates/history/{lib,model,rbac}.rs` (SQLite :36-52, baselines :168-207, regresión :215-249, trends :256-290, auditoría :355-386, RBAC :rbac.rs) | **Persistencia/baselines/regresión/trends/auditoría/annotations: reales (SQLite local).** Pero **RBAC es solo lógica `can(role,action)`, NO forzada en los métodos del `Store`**. **Sin SSO, sin multi-tenant, sin almacenamiento centralizado gestionado, sin cifrado en reposo.** |

Leyenda: ✅ Implementado (a la profundidad que el MVP pedía) · 🟡 Parcial (slice funcional con brechas o solo MVP) · ⛔ Diferido (no implementado).

---

## 3. Estado por componente/área (§6.1–6.9)

| Área (lead del plan) | Estado | Evidencia | Brechas |
|---|---|---|---|
| **6.2 platform-architect** (IR, schemas, contratos, ADRs, matriz JMX) | ✅ | `crates/scenario-ir/`, `schemas/*.json`, `docs/adr/ADR-001..010`, `docs/migration/jmeter-support-matrix.md` | DSL TypeScript diferido (por diseño, ADR-002). IR versionado 0.1→0.2→0.3 con ADRs. |
| **6.3 rust-engine-lead** (hot path, scheduler, VUs, timers, assertions, backpressure, benchmarks) | ✅ (MVP sólido) | `crates/engine/src/lib.rs`, reloj monotónico (`http-adapter:79`), HdrHistogram (`metrics:206`), cancelación cooperativa `Arc<AtomicBool>` | Constant Throughput Timer aproximado por-VU (no global, multiplica por nº VUs); TTFB ≈ tiempo a headers; condiciones limitadas. Sin proxy HTTP. |
| **6.4 jmx-migration-lead** (parser, mapper, fidelidad, golden, roundtrip) | ✅ (MVP sólido) | `crates/jmx-importer/src/lib.rs` + `export.rs` (roundtrip test :413-483), clasificación sin fallo silencioso | XPath no soportado. Modo compatibilidad JVM (Nivel 3) no abordado. Golden = smoke test, no snapshots. |
| **6.5 qa-performance-semantics** (suite compatibilidad JMeter, matriz equivalencia, benchmarks VUs/core+memoria) | 🟡 | `crates/engine/tests/semantics.rs`, `docs/benchmarks/perfkit-vs-jmeter.md`, `tools/benchmark.sh` | Benchmark real ejecutado (ver §6), pero "2x VUs/core" no demostrado por throughput. **Sin suite formal de paridad semántica** vs JMeter (solo tests de semántica internos). |
| **6.6 frontend-ux-lead** (UI: import, árbol, editores, run, dashboard, fidelidad, reporte) | ✅ | `ui/src/views/*`, `ui/src/components/*`, `ui/src-tauri/src/lib.rs` (15 comandos Tauri) | Edición del IR no persiste sin export explícito; CSV no editable inline. |
| **6.7 reporting-analytics-lead** (aggregate, percentiles, throughput, errores, series, HTML/JSON/JUnit) | ✅ | `crates/reports/{html,junit,gate,lib}.rs`, `crates/metrics/src/lib.rs` (percentiles, heatmap, status codes, error kinds, TTFB, bytes) | OTel/Prometheus (que el plan ponía como export, no reporte) no implementados. |
| **6.8 cli-dx-lead** (comandos, help, exit codes, logs, verbose, config) | ✅ (excede) | `crates/cli/src/main.rs`: `init/validate/import/convert/run/debug/gate/schema/cluster/history/ai/plugin/export` | Cubre todos los comandos del §6.8 **y añade** `export` (jmx/pkb) no pedido. |
| **6.9 security-governance-lead** (secretos, redacción, firma plugins, permisos WASM, IA gobernada) | 🟡 | `crates/security/src/lib.rs` (env vars + redacción, 73 líneas), `crates/plugin-host` (firma real), `crates/ai-assist` (gobernanza real) | `security` es stub explícito (redacción frágil por substring vs regex de ai-assist). Permisos WASM declarados pero no forzados. Sin threat model formal versionado. |

Áreas no numeradas como §6.x pero en el plan: **6.10 observability** ⛔ (sin Prometheus/OTLP), **6.11 devops/k8s** 🟡 (Docker/CI sí; operator solo diseño), **6.12 plugins** 🟡, **6.13 kafka** 🟡, **6.14 IA** 🟡, **6.15 docs** ✅ (`docs/migration/migrar-tu-primer-jmx.md`, `HelpView`, guías de deploy).

---

## 4. Niveles de migración JMX (§5)

Clasificador en `crates/jmx-importer/src/lib.rs`. **Garantía cumplida: nunca falla en silencio** — todo elemento se registra como `migrated | assisted | unsupported | ignored` con razón (`classify_unknown` :680-695; los hijos conocidos de un elemento desconocido se aplanan, no se descartan).

### Nivel 1 — Migración nativa 1:1 (debía entrar en MVP)

| Elemento JMeter | Estado real | Evidencia |
|---|---|---|
| Test Plan, Thread Group, setUp/tearDown TG | migrated | `lib.rs:39-45, 68-154` |
| HTTP Request Defaults, HTTP Sampler | migrated | `lib.rs:480-524, 168-476` |
| Header Manager, Cookie Manager | migrated | `lib.rs:83-89, 313-319` |
| Cache Manager | **ignored** (no relevante para carga en MVP) | `lib.rs:90-91, 320-321` |
| User Defined Variables / Arguments | migrated | `lib.rs:72-74, 302-306` |
| CSV Data Set Config | migrated | `lib.rs:79-81, 309-311` |
| Constant / Uniform / Gaussian / Constant Throughput Timer | migrated | `lib.rs:546-565` |
| Response / Duration / Size / JSON Assertion | migrated (Response: patrones no soportados → assisted) | `lib.rs:409-438` |
| Regex / JSON / Boundary Extractor | migrated | `lib.rs:440-456` |
| **XPath Extractor/Assertion** | **unsupported** (no hay motor XML ni nodo IR) | matriz :58; no hay variante en `model.rs` |
| Loop / If / While / Transaction Controller | migrated (If/While complejos → assisted) | `lib.rs:183-226, 172-181, 747-759` |
| Once Only Controller | **assisted** (en perfkit se ejecuta cada iteración) | `lib.rs:287-293` |
| Throughput Controller (percent) | migrated; modo "total executions" → assisted | `lib.rs:228-256` |
| Simple / Interleave / Random Controller | migrated | `lib.rs:280-285, 258-278` |
| Listeners | **ignored** (reporte nativo) | `lib.rs:92-93, 322-323` |

### Nivel 2 — Migración asistida (debía iniciar en MVP)

| Elemento | Estado | Evidencia |
|---|---|---|
| JSR223 Sampler/Pre/PostProcessor, BeanShell | assisted con análisis heurístico y sugerencia | `lib.rs:324-327, 465-468`, `analyze_script` :702-745 |
| Funciones `__groovy`/`__jexl3`/`__javaScript`, correlación custom, manipulación compleja de vars | detectadas por heurística de scripts (vars.put, prev., crypto, Thread.sleep…) | `analyze_script` :702-745 |

### Nivel 3 — Compatibilidad JVM opt-in (sidecar)

⛔ **No implementado, ni siquiera el spike.** ADR-003: "No entra en el MVP salvo spike." No hay sidecar JVM. Es la brecha más grande de migración para clientes con scripting Groovy pesado.

### Nivel 4 — No soportado (reportado explícitamente)

| Elemento | Estado | Evidencia |
|---|---|---|
| JDBC, JMS avanzado | unsupported con razón | `classify_unknown` :680-695 |
| Plugins `.jar` de terceros (genéricos) | unsupported con razón | `classify_unknown` :688 |
| Remote testing legacy | unsupported (cae en Fase 6) | — |
| Sampler Kafka (plugin) | **assisted → `Step::Kafka`** (discrepancia con la matriz, ver abajo) | `lib.rs:328-358`, export comentado `export.rs:114-119` |

> **Discrepancia doc↔código:** la matriz (`docs/migration/jmeter-support-matrix.md`) lista Kafka como "Fase 7 / no soportado", pero el código ya lo parsea como `assisted` y lo mapea a `Step::Kafka` (IR 0.3.0). Es deuda de documentación, no bug. Conviene actualizar la matriz.

**Export inverso IR→JMX** (`crates/jmx-importer/src/export.rs`): real y *round-trippable* para Nivel 1 (test :413-483). Pasos sin equivalente JMeter nativo (p.ej. Kafka) se emiten como comentario XML, no se pierden silenciosamente.

---

## 5. Definition of Done — criterios de aceptación globales del MVP (§11)

| # | Criterio | ¿Cumplido? | Evidencia / matiz |
|---|---|---|---|
| 1 | Un QA puede importar un JMX HTTP real | ✅ | `cli import jmx` (`main.rs:545`), `import_jmx`/`import_jmx_content` en Tauri; 14 fixtures importan sin fallo silencioso (`tests/fixtures.rs`). |
| 2 | Genera YAML/IR y reporte de fidelidad | ✅ | `main.rs:559-565` escribe `.yaml` + `.fidelity.json`; `FidelityPanel.tsx` en UI. |
| 3 | Ejecutar localmente desde CLI | ✅ | `cli run` → `engine::run` (`main.rs:570-614`). |
| 4 | Ejecutar localmente desde UI | ✅ | comando `run_scenario` → `engine::run`, eventos live (`ui/src-tauri/src/lib.rs`). |
| 5 | Reporte con percentiles, throughput, errores y series temporales | ✅ | `crates/metrics/src/lib.rs` (p50..p99.9, throughput, error kinds, series, heatmap); `ReportView.tsx` (7 pestañas). |
| 6 | Salida usable en CI con exit codes | ✅ | `cli gate` exit 1 si falla (`main.rs:706-730`); `run`/`compare` con códigos. |
| 7 | Reporte HTML abre offline | ✅ | `crates/reports/src/html.rs` autocontenido (sin CDN); ADR-005. |
| 8 | El importador no falla en silencio | ✅ | `classify_unknown` clasifica todo con razón (`lib.rs:680-695`). |
| 9 | El engine demuestra mejora de eficiencia vs JMeter en benchmark HTTP | 🟡 | **Benchmark real ejecutado** (ver §6): throughput a la par (target es el cuello de botella), pero **~46x menos memoria** (20 MB vs 906 MB). El "2x VUs/core" textual no se demuestra por throughput; se reencuadra honestamente como ventaja de memoria. |
| 10 | La documentación permite completar el flujo en <30 min | ✅ (plausible) | `docs/migration/migrar-tu-primer-jmx.md`, `README.md`, `HelpView`. No cronometrado formalmente. |

**Veredicto DoD MVP: 9/10 plenos + 1 parcial honesto (criterio 9).** El MVP del plan está sustancialmente cumplido.

---

## 6. Requisitos no funcionales (rendimiento, memoria)

**Sí se midió, con metodología real y honesta** (`docs/benchmarks/perfkit-vs-jmeter.md`, `tools/benchmark.sh`):

- Mismo plan (`examples/jmx/bench-http.jmx`), mismo target (Node.js single-CPU, `tools/bench-target.js`), misma carga (50 VUs, 20 s, keep-alive). JMeter ejecutado de verdad (`jmeter -n -t … ` con JDK 21), memoria capturada con `/usr/bin/time -l`.

| Métrica | perfkit | JMeter | Ratio |
|---|---|---|---|
| Throughput (req/s) | ~116.6k | ~123.9k | 0.94x |
| Requests totales | 2.33M | 2.48M | 0.94x |
| Errores | 0 | 0 | — |
| p95 (ms) | 0.7 | 1.0 | comparable |
| **RSS pico (MB)** | **~20** | **~906** | **~46x menos** |

**Honestidad sobre lo no medido / lo no demostrado:**
- El criterio **"≥2x VUs/core" (§6.5) NO se demuestra por throughput** porque el cuello de botella es el target (un Node satura a ~120k req/s), no el generador. El propio doc lo dice: *"el criterio '2x VUs/core' del plan (§6.5) no se demuestra por throughput en este escenario"*. La ventaja real medida es **memoria (~46x)**, que es la traducción correcta de "más VUs por core/RAM".
- Faltaría, para cerrar el §6.5 rigurosamente: target multi-worker que no sature, o una medición de **VUs sostenibles por GB de RAM**.
- Los artefactos crudos del benchmark (`/tmp/*.jtl`, `summary.json`) **no están versionados** (viven en `/tmp/`); solo el `.md` con la tabla está en repo.
- **Sin benchmarks de overhead de observabilidad** (no aplica: no hay export Prometheus/OTLP).

---

## 7. Lo que NO está implementado / diferido (lista priorizada y honesta)

Ordenado por impacto para el objetivo "reemplazar JMeter de verdad":

1. **~~Suite de tests compila en rojo~~ → RESUELTO en esta sesión.** Se corrigieron los inicializadores de test desincronizados (`Ctx.interleave` en `engine/src/lib.rs`; `LabelStats`/`RunSummary` en `history/src/lib.rs` y `ai-assist/src/suggest.rs`, vía `..Default::default()`). `cargo test --workspace` ahora compila y pasa: **28 suites OK, 79 tests, 0 fallos**.
2. **`tests/golden|integration|benchmarks` vacíos (deuda real pendiente).** No hay golden snapshots ni suite de compatibilidad/paridad semántica formal vs JMeter (el plan §6.5 la pedía). Solo existe un smoke test de importación (`jmx-importer/tests/fixtures.rs`) y tests de semántica internos (`engine/tests/semantics.rs`).
3. **Modo compatibilidad JVM sidecar para JSR223/Groovy (Nivel 3).** ⛔ Ni spike. Es la brecha funcional más grande para adopción de clientes con scripting pesado. Documentado como diferido en ADR-003.
4. **Export Prometheus / OTLP (observabilidad, §6.10).** ⛔ No existe (solo un doc-comment en `crates/reports/src/lib.rs:3`). No hay crate `observability`. Diferido en ADR-005 como "capa opcional posterior".
5. **Cluster productivo: gRPC + mTLS + operator K8s binario.** Hoy HTTP/JSON sin TLS mutuo, sin barrier start, operator solo como manifiestos+README. Diferido explícito en ADR-010 / `cluster/src/lib.rs:4-5`. Riesgo: el transporte HTTP/JSON sin mTLS no es apto para producción distribuida.
6. **Broker Kafka real + SASL/SSL + Schema Registry + consumer validation + métricas por partición.** rskafka está cableado pero sin auth ni validación contra broker real. Diferido en ADR-009 / `kafka-adapter/src/lib.rs:6`.
7. **Registry de plugins de terceros + permisos WASM efectivos.** Firma/sandbox de primera parte reales, pero solo plugins puros (sin imports); `allow_net/fs/env` declarados y NO forzados; registry de terceros diferido. ADR-010 / `plugin-host/src/lib.rs`.
8. **IA SaaS real / proveedor LLM real.** Solo proveedor heurístico local; BYOK/SaaS son traits sin red. Gobernanza (off por defecto, preview, confirmación) sí real. Diferido en ADR-006/010.
9. **SSO / RBAC enterprise forzado / multi-tenant / almacenamiento centralizado.** RBAC es solo lógica `can()` no aplicada en el `Store`; sin SSO ni cifrado en reposo; SQLite local únicamente. Diferido en ADR-010.
10. **DSL TypeScript.** ⛔ Diferido por diseño (ADR-002): la UI edita el IR; el DSL compilaría al IR si existiera.
11. **SBOM y signing de releases multi-arch.** Documentado (Fase 5) pero sin evidencia en repo (el signing presente es de *plugins*, no de releases).
12. **Brechas menores de paridad/UX:** XPath (sin nodo IR), condiciones if/while solo `==/!=/true/false`, Constant Throughput Timer por-VU en vez de global, TTFB aproximado, sin proxy HTTP, edición de IR en UI sin auto-save, CSV no editable inline, sharing mode de CSV único compartido (no por-hilo).

---

## 8. Riesgos / deuda técnica

- **~~Falsa sensación de verde~~ → corregido.** El suite de tests no compilaba (lo enmascaraba un `grep` de verificación que daba falso positivo). Ya está en verde (79 tests). Lección: la verificación de CI debe fallar si *no hay* resultados de test, no solo si hay "FAILED". **Pendiente**: conectar `cargo test --workspace` como gate bloqueante en CI.
- **Sin red de seguridad de paridad semántica.** Sin suite de compatibilidad vs JMeter, los matices ya conocidos (Constant Throughput Timer global vs por-VU, Once Only por-iteración, condiciones limitadas) pueden sorprender al QA que migra y erosionar la confianza — justo el riesgo que el §6.5 buscaba mitigar.
- **Distribuido no apto para producción.** HTTP/JSON sin mTLS y sin barrier start: además del riesgo de seguridad, sin arranque sincronizado la fase de ramp-up no es coherente entre workers (mide distinto que JMeter remote bien configurado).
- **Permisos WASM declarativos pero no forzados.** Si en el futuro se habilitan imports sin implementar el enforcement de `allow_net/fs/env`, se abre superficie de ejecución insegura. Hoy mitigado porque solo se permiten plugins puros.
- **Deriva doc↔código.** La matriz JMX no refleja que Kafka ya migra a IR; el README sobre-vende el estado de tests. Señal de que la documentación va por delante de la verificación.
- **Artefactos de benchmark efímeros (`/tmp/`).** No reproducibles desde el repo; un revisor no puede auditar los números sin re-ejecutar.

---

## 9. Recomendación de siguientes pasos (orden sugerido)

1. **~~Arreglar la compilación de tests~~ → HECHO en esta sesión.** Inicializadores actualizados; `cargo test --workspace` en verde (79 tests). **Pendiente**: conectarlo al CI como gate bloqueante (que falle si no hay resultados, no solo si hay "FAILED").
2. **Poblar `tests/compatibility` + golden snapshots**: empezar por la semántica que ya sabemos divergente (Constant Throughput Timer, Once Only, condiciones if/while) con comparación numérica vs JMeter en un par de escenarios. Cierra el §6.5 y protege la confianza de migración.
3. **Cerrar honestamente el §6.5 de memoria/VUs**: añadir un benchmark de "VUs sostenibles por GB" o target multi-worker, y **versionar los artefactos** del benchmark (no `/tmp/`).
4. **Sincronizar documentación con código**: corregir la matriz JMX (Kafka), y ajustar el README para distinguir "compila" de "tests verdes" hasta que el paso 1 esté hecho.
5. **Decidir la apuesta de migración profunda**: o se aborda el **spike JVM sidecar (Nivel 3)**, o se invierte en madurar el análisis asistido de Groovy (hoy heurístico) — es lo que más mueve la aguja de "reemplazar JMeter" para clientes con scripting.
6. **Si se prioriza distribuido productivo**: migrar el control plane a gRPC+mTLS y añadir barrier start, antes que el operador K8s.
7. **Diferir conscientemente** (no son bloqueantes del MVP QA): Prometheus/OTLP, registry de terceros, IA LLM/SaaS, SSO/multi-tenant, DSL TypeScript — todos ya documentados como Fase posterior en los ADRs.

---

*Fin del análisis. Toda afirmación de estado está anclada a rutas de archivo verificadas el 2026-06-21. Donde el código difiere de la documentación o del plan, se señaló explícitamente.*
