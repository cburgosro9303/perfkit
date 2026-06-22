# Claude Opus Implementation Plan

## 1. Proposito

Este documento es la guia operativa para que Claude Opus coordine e implemente por fases una nueva suite moderna de performance testing capaz de reemplazar Apache JMeter, con foco inicial en usuarios QA que ya tienen scripts JMX existentes.

La implementacion debe priorizar adopcion real, calidad tecnica y compatibilidad de migracion. La tesis principal no es "crear otro motor de carga", sino construir una transicion creible desde JMeter hacia una plataforma moderna, eficiente, versionable, observable y preparada para escalar.

## 2. Decisiones Arquitectonicas No Negociables

1. El primer publico objetivo es QA tradicional que ejecuta pruebas de carga.
2. La migracion profunda de JMX es una prioridad de producto, no una herramienta secundaria.
3. El MVP debe permitir importar un JMX real, ejecutarlo localmente y entregar un reporte familiar.
4. El motor de ejecucion se construye en Rust 1.96.0 edicion 2024 con Tokio para alta concurrencia, baja memoria y latencia predecible.
5. El formato canonico de escenarios es un IR estructurado, serializado inicialmente como YAML.
6. La UI edita el IR canonico; el usuario QA no debe depender de TypeScript para empezar.
7. Un DSL TypeScript puede existir despues como capa opcional para usuarios avanzados, compilando siempre al IR.
8. OpenTelemetry y Prometheus son exports de observabilidad, no sustitutos del reporte nativo.
9. Kafka entra en el primer anio, pero no bloquea el MVP.
10. Kubernetes y ejecucion distribuida entran despues del modo local estable.
11. La IA debe soportar tres modos: local, bring-your-own-key y SaaS opt-in. SaaS debe venir apagado por defecto.
12. El registry de plugins inicia curado, con plugins de primera parte firmados. Terceros entran despues de estabilizar seguridad.
13. Plugins seguros se disenan con WASM/WASI y permisos declarativos.
14. Para compatibilidad legacy JMeter, la estrategia debe reconocer niveles de fidelidad, no prometer 100% automatico sin evidencia.

## 3. Modelo De Producto Recomendado

Recomendacion: open-core.

Core abierto:

- Engine local.
- CLI.
- UI local basica.
- IR/YAML.
- Importador JMX.
- Reporte de fidelidad de migracion.
- Ejecucion HTTP/HTTPS.
- Reportes HTML/JSON/JUnit basicos.
- Export Prometheus/OpenTelemetry.
- Plugins primera parte basicos.

Enterprise/comercial:

- Colaboracion multiusuario.
- RBAC/SSO/SAML/OIDC.
- Auditoria.
- Historico centralizado de ejecuciones.
- Comparacion historica avanzada.
- Baselines por branch/build/environment.
- Registry privado de plugins.
- Politicas de seguridad.
- IA enterprise local/BYOK/SaaS gobernada.
- Ejecucion distribuida gestionada.
- SaaS opcional.

Razon: si el importador JMX profundo se vuelve comercial desde el principio, se reduce la adopcion. Debe ser parte de la cuña de entrada del proyecto.

## 4. Arquitectura Objetivo

### 4.1 Componentes Principales

- CLI: valida, importa, ejecuta, depura, genera reportes, corre quality gates.
- UI Studio: importa JMX, muestra arbol de plan, edita escenarios, ejecuta local, muestra resultados.
- Scenario IR: modelo canonico versionado.
- YAML serializer: representacion humana y versionable del IR.
- JMX importer: parser, normalizador, mapper y reporte de fidelidad.
- Core engine: scheduler, usuarios virtuales, timers, pacing, variables, datasets, assertions.
- HTTP adapter: HTTP/1.1, HTTPS, keep-alive, cookies, headers, redirects, TLS config.
- Metrics engine: histogramas, contadores, series temporales, agregaciones por sampler/transaction.
- Report generator: HTML, JSON, JUnit, Markdown opcional.
- Observability exporter: Prometheus, OTLP metrics/logs/traces.
- Plugin host: WASM/WASI con permisos declarativos.
- Security module: secretos, redaccion, firmas, policies.
- Coordinator: control plane para ejecucion distribuida futura.
- Worker agent: data plane para generacion distribuida futura.
- Storage local: SQLite y archivos Parquet/JSON para runs locales.
- Storage enterprise futuro: Postgres + ClickHouse/Timescale + object storage.
- AI assistant futuro: migracion asistida, correlacion, analisis de resultados, recomendaciones.

### 4.2 Estructura Sugerida Del Repositorio

```text
.
├── crates/
│   ├── cli/
│   ├── engine/
│   ├── scenario-ir/
│   ├── jmx-importer/
│   ├── http-adapter/
│   ├── metrics/
│   ├── reports/
│   ├── observability/
│   ├── plugin-host/
│   └── security/
├── ui/
│   ├── app/
│   ├── components/
│   ├── features/
│   └── test-fixtures/
├── schemas/
│   ├── scenario-ir.schema.json
│   └── migration-report.schema.json
├── examples/
│   ├── jmx/
│   ├── yaml/
│   └── reports/
├── docs/
│   ├── adr/
│   ├── architecture/
│   ├── migration/
│   ├── qa-guides/
│   └── developer-guides/
├── tests/
│   ├── golden/
│   ├── integration/
│   ├── compatibility/
│   └── benchmarks/
└── tools/
    ├── fixtures/
    └── scripts/
```

La estructura exacta puede ajustarse durante el bootstrap, pero debe preservar separacion entre IR, importer, engine, adapters, reports y UI.

## 5. Estrategia JMX Por Niveles

La compatibilidad JMX se debe implementar con reporte explicito de fidelidad. La promesa correcta es: alta fidelidad declarativa, migracion asistida para scripting y modo compatibilidad opt-in para casos legacy.

### Nivel 1: Migracion Nativa 1:1

Debe entrar en MVP.

Elementos objetivo:

- Test Plan.
- Thread Group basico.
- Setup/teardown thread groups si son simples.
- HTTP Request Defaults.
- HTTP Samplers.
- Header Manager.
- Cookie Manager.
- Cache Manager.
- User Defined Variables.
- CSV Data Set Config.
- Constant Timer.
- Uniform Random Timer.
- Gaussian Random Timer.
- Constant Throughput Timer.
- Response Assertion.
- Duration Assertion.
- Size Assertion.
- JSON Assertion.
- Regular Expression Extractor.
- JSON Extractor.
- Boundary Extractor.
- XPath Extractor si el parser XML queda dentro del MVP.
- Loop Controller.
- If Controller para expresiones simples.
- While Controller para expresiones simples.
- Transaction Controller.
- Once Only Controller.
- Throughput Controller si se puede validar semantica.
- Simple Controller.
- Listeners como metadata de reporte, no como ejecucion literal.

### Nivel 2: Migracion Asistida

Debe iniciar en MVP y madurar en fase 2.

Elementos:

- JSR223 Sampler/PostProcessor/PreProcessor con Groovy.
- BeanShell legacy.
- Funciones `__groovy`, `__jexl3`, `__javaScript`.
- Expresiones complejas en controllers.
- Correlacion custom.
- Firma de payloads.
- Manipulacion compleja de variables.

Salida esperada:

- Detectar y clasificar scripts.
- Mostrar ubicacion exacta en el arbol JMX.
- Proponer equivalencia en IR cuando sea posible.
- Marcar accion manual requerida cuando no sea posible.
- Generar recomendaciones de migracion.

### Nivel 3: Modo Compatibilidad JVM Opt-In

No entra en MVP salvo spike tecnico. Entra en fase 2 si hay demanda.

Objetivo:

- Ejecutar logica Groovy/JSR223 en un sidecar JVM aislado.
- Mantener compatibilidad para clientes que no pueden reescribir scripts.
- Marcar claramente que el sandbox es degradado frente al modo nativo.
- Limitar red, filesystem, CPU, memoria y secretos.

### Nivel 4: No Soportado Inicialmente

Debe reportarse explicitamente, no fallar silenciosamente.

Elementos:

- Plugins JMeter `.jar` de terceros.
- JDBC avanzado.
- JMS avanzado.
- Remote testing legacy.
- Include/Module controllers complejos.
- Custom samplers propietarios.

## 6. Roles De Agentes Para Claude Opus

Claude Opus debe actuar como coordinador general y desplegar agentes especializados. Cada agente debe producir entregables verificables, no solo analisis.

### 6.1 Coordinador Principal De Implementacion

Nombre sugerido: `delivery-coordinator`

Modelo/esfuerzo recomendado: Opus, `max`.

Mision:

- Mantener vision, alcance, secuencia y calidad.
- Descomponer fases en tareas verificables.
- Evitar scope creep.
- Resolver conflictos entre agentes.
- Revisar entregables antes de merge.
- Mantener decisiones en ADRs.
- Asegurar que el foco QA/JMX no se pierda.

Responsabilidades:

- Crear roadmap ejecutable.
- Abrir issues o tareas por fase.
- Definir Definition of Done por modulo.
- Coordinar contratos entre IR, importer, engine, CLI y UI.
- Revisar resultados de benchmarks y pruebas de compatibilidad.
- Exigir evidencia: tests, fixtures, reportes, capturas, comandos.

Reglas:

- Ningun agente debe modificar contratos compartidos sin ADR.
- Ningun modulo puede depender directamente de detalles internos de otro si existe una interfaz.
- Cada fase debe cerrar con demo reproducible.
- No aceptar "funciona en teoria"; requerir prueba local.

### 6.2 Arquitecto De Plataforma

Nombre sugerido: `platform-architect`

Modelo/esfuerzo recomendado: Opus, `xhigh` o `max` para ADRs principales.

Mision:

- Definir boundaries, contratos, schemas y estructura de paquetes.
- Gobernar la evolucion del IR.
- Asegurar mantenibilidad a largo plazo.

Entregables:

- ADRs iniciales.
- Diagrama C4.
- Schema del IR.
- Politica de versionado del IR.
- Matriz de compatibilidad JMX -> IR.
- Contratos entre CLI/UI/engine/importer.

### 6.3 Ingeniero Rust Engine

Nombre sugerido: `rust-engine-lead`

Modelo/esfuerzo recomendado: Opus o Sonnet fuerte, `xhigh`.

Mision:

- Implementar el hot path del motor.
- Garantizar scheduler preciso, baja memoria y alta concurrencia.

Entregables:

- Runtime de VUs.
- Scheduler de ramp-up/hold/ramp-down.
- Timers.
- Pacing.
- Variables por VU y por test.
- Ejecucion de samplers HTTP.
- Assertions en runtime.
- Backpressure y cancelacion ordenada.
- Benchmarks comparativos.

Criterios:

- Medicion con reloj monotonic.
- Separacion entre medicion de latencia y overhead de reporte.
- Tests de semantica de timers.
- Uso controlado de memoria por VU.

### 6.4 Especialista De Migracion JMX

Nombre sugerido: `jmx-migration-lead`

Modelo/esfuerzo recomendado: Opus, `xhigh`.

Mision:

- Construir el importador JMX profundo.
- Mapear elementos JMeter a IR.
- Generar reporte de fidelidad.

Entregables:

- Parser JMX robusto.
- Normalizador de arbol JMeter.
- Mapper JMX -> IR.
- Catalogo de elementos soportados.
- Reporte de fidelidad por elemento.
- Golden fixtures con JMX reales/sinteticos.
- Pruebas de roundtrip conceptual.

Criterios:

- No fallar silenciosamente.
- Cada elemento debe quedar como migrated, assisted, unsupported o ignored-with-reason.
- El reporte debe ser entendible por QA.

### 6.5 QA Performance Semantics Engineer

Nombre sugerido: `qa-performance-semantics`

Modelo/esfuerzo recomendado: Opus, `xhigh`.

Mision:

- Validar que la herramienta se comporte como QA espera.
- Comparar semantica contra JMeter.

Entregables:

- Suite de compatibilidad con JMeter.
- Fixtures de test plans.
- Matriz de equivalencia de timers/controllers/assertions.
- Benchmarks de VUs/core y memoria.
- Reportes comparativos.

Criterios:

- El MVP debe demostrar al menos 2x VUs/core frente a JMeter en escenario HTTP de referencia, o explicar tecnicamente por que no.
- El resultado de percentiles debe ser confiable y reproducible.

### 6.6 Arquitecto Frontend/UX

Nombre sugerido: `frontend-ux-lead`

Modelo/esfuerzo recomendado: Opus para UX inicial `xhigh`, Sonnet/implementador `high`.

Mision:

- Crear una UI moderna pero familiar para QA.
- Priorizar importacion, arbol de plan, edicion esencial, ejecucion y reporte.

Entregables MVP:

- Shell de UI.
- Vista de proyectos/runs.
- Import JMX.
- Arbol de plan.
- Editor de elementos HTTP/timers/assertions/datasets.
- Run console.
- Dashboard live.
- Reporte post-run.
- Vista de reporte de fidelidad.

Reglas UX:

- No hacer landing page.
- Primera pantalla debe ser operativa.
- El usuario QA debe poder trabajar sin escribir TypeScript.
- La vista YAML es util, pero no debe ser obligatoria.

### 6.7 Ingeniero De Reportes Y Analitica

Nombre sugerido: `reporting-analytics-lead`

Modelo/esfuerzo recomendado: `high` o `xhigh`.

Mision:

- Entregar reportes nativos familiares, comparables y utiles.

Entregables:

- Aggregate report.
- Percentiles p50/p90/p95/p99/p99.9.
- Throughput.
- Error rate.
- Response time over time.
- Latency distribution.
- Top slow samplers.
- Error summary.
- HTML report standalone.
- JSON machine-readable.
- JUnit XML para CI.

Criterios:

- OTel/Prometheus no reemplazan el reporte.
- El reporte debe funcionar offline.
- El HTML debe ser compartible como artefacto de pipeline.

### 6.8 Ingeniero CLI/Developer Experience

Nombre sugerido: `cli-dx-lead`

Modelo/esfuerzo recomendado: `high`.

Mision:

- Hacer que el flujo local y CI sea simple.

Comandos objetivo:

```text
perfkit init
perfkit import jmx input.jmx -o scenario.yaml
perfkit validate scenario.yaml
perfkit run scenario.yaml
perfkit run scenario.yaml --report html --out reports/run-001
perfkit debug scenario.yaml --once
perfkit gate reports/run-001/summary.json --thresholds thresholds.yaml
perfkit convert jmx input.jmx --fidelity-report
```

Nota: `perfkit` es nombre temporal. El coordinador debe permitir renombrarlo.

Entregables:

- UX de comandos.
- Help claro.
- Codigos de salida para CI.
- Logs estructurados.
- Modo verbose/debug.
- Config por environment.

### 6.9 Security/Governance Engineer

Nombre sugerido: `security-governance-lead`

Modelo/esfuerzo recomendado: Opus, `xhigh`.

Mision:

- Disenar seguridad desde el inicio.
- Evitar fugas de secretos, ejecucion insegura de plugins y riesgos de IA.

Entregables:

- Threat model.
- Manejo de secretos.
- Redaccion de logs/reportes.
- Politica de plugins firmados.
- Modelo de permisos WASM.
- Politica IA: local, BYOK, SaaS opt-in.
- Disclaimer y controles tecnicos.
- Reglas para modo compatibilidad JVM.

Criterios:

- SaaS IA apagado por defecto.
- No enviar endpoints, tokens, payloads o CSVs a terceros sin opt-in explicito.
- Registry curado con firma y version pinning.

### 6.10 Observability Engineer

Nombre sugerido: `observability-lead`

Modelo/esfuerzo recomendado: `high`.

Mision:

- Exponer telemetria util sin contaminar el hot path.

Entregables:

- Export Prometheus.
- Export OTLP metrics.
- Structured logs.
- Trace context injection para requests generadas.
- Configuracion de sampling.
- Benchmarks de overhead.

Criterios:

- Exporters asincronos.
- Backpressure controlado.
- OTel nunca debe ser requisito para ver resultados.

### 6.11 DevOps/SRE/Kubernetes Engineer

Nombre sugerido: `devops-sre-lead`

Modelo/esfuerzo recomendado: `high`, `xhigh` para fase distribuida.

Mision:

- Preparar empaquetado local, Docker, CI y luego Kubernetes.

Entregables MVP:

- Dockerfile.
- Release multi-arch.
- CI basico.
- Artefactos de reportes.
- Scripts reproducibles.

Entregables fase 2:

- Coordinator.
- Worker agent.
- gRPC/mTLS.
- Docker Compose distribuido.
- Kubernetes CRD `LoadTest`.
- Operator.
- Helm chart.

### 6.12 Plugin/WASM Engineer

Nombre sugerido: `plugin-wasm-lead`

Modelo/esfuerzo recomendado: `high` fase 2, `xhigh` fase 3.

Mision:

- Crear extensibilidad segura y gobernada.

Entregables:

- Plugin ABI.
- WASM host.
- Permisos declarativos.
- Firma y verificacion.
- SDK de plugin primera parte.
- Plugin examples.

No MVP salvo spike arquitectonico.

### 6.13 Kafka/Event Protocol Engineer

Nombre sugerido: `kafka-protocol-lead`

Modelo/esfuerzo recomendado: `xhigh`.

Mision:

- Implementar Kafka en el primer anio sin contaminar el MVP HTTP.

Entregables fase 2:

- Kafka producer sampler.
- Kafka consumer validation mode.
- SASL/SSL config.
- Schema Registry integration.
- Metrics por topic/partition.
- Assertions para eventos.
- Data-driven event payloads.

### 6.14 AI Migration Assistant Engineer

Nombre sugerido: `ai-migration-lead`

Modelo/esfuerzo recomendado: Opus, `xhigh`.

Mision:

- Usar IA para acelerar migracion, no como gimmick.

Entregables fase 2:

- Analisis de Groovy/BeanShell.
- Sugerencias de porte a IR o script target.
- Auto-correlation suggestions.
- Threshold suggestions.
- Modo local.
- Modo BYOK.
- Modo SaaS opt-in.
- Redaccion previa de datos.

Reglas:

- No enviar datos sensibles por defecto.
- Toda sugerencia debe mostrarse como propuesta revisable.
- No modificar escenarios automaticamente sin confirmacion.

### 6.15 Technical Writer / QA Enablement

Nombre sugerido: `docs-qa-enablement`

Modelo/esfuerzo recomendado: `medium` a `high`.

Mision:

- Hacer que QA pueda adoptar la herramienta.

Entregables:

- Guia "Migrar desde JMeter".
- Tabla JMeter -> Nueva herramienta.
- Tutorial importar JMX.
- Tutorial correr local.
- Tutorial leer reporte.
- Guia CI.
- Guia troubleshooting.
- Ejemplos reales.

## 7. Matriz De Esfuerzo Por Tipo De Tarea

| Tipo de tarea | Agente recomendado | Esfuerzo |
|---|---|---|
| ADR arquitectonico fundamental | delivery-coordinator + platform-architect | max |
| Diseno IR/schema | platform-architect + jmx-migration-lead | xhigh |
| Parser JMX y mapping complejo | jmx-migration-lead | xhigh |
| Semantica de timers/controllers | qa-performance-semantics + rust-engine-lead | xhigh |
| Hot path del engine | rust-engine-lead | xhigh |
| CLI commands | cli-dx-lead | high |
| UI MVP | frontend-ux-lead | high |
| Reportes nativos | reporting-analytics-lead | high |
| Seguridad/secretos/plugins | security-governance-lead | xhigh |
| Observabilidad | observability-lead | high |
| Docker/CI | devops-sre-lead | high |
| Kubernetes/operator | devops-sre-lead | xhigh |
| Kafka | kafka-protocol-lead | xhigh |
| IA local/BYOK/SaaS | ai-migration-lead + security-governance-lead | xhigh |
| Documentacion QA | docs-qa-enablement | medium/high |
| Benchmarks comparativos | qa-performance-semantics | xhigh |

## 8. Protocolo De Coordinacion Entre Agentes

Cada agente debe trabajar con este contrato:

1. Declarar objetivo concreto.
2. Listar archivos o modulos que tocara.
3. Confirmar dependencias con otros agentes.
4. Implementar una unidad verificable.
5. Agregar pruebas o evidencia.
6. Documentar decisiones si cambia un contrato.
7. Entregar resumen con comandos ejecutados y resultados.

El coordinador debe bloquear merges si:

- No hay prueba o evidencia.
- Cambia el IR sin actualizar schema, fixtures y documentacion.
- Cambia el engine sin benchmark o test de regresion.
- Cambia UI sin validacion visual minima.
- Cambia seguridad sin threat model o justificacion.
- Agrega dependencia pesada sin ADR.
- Rompe compatibilidad de escenarios existentes sin version bump.

## 9. Fases De Implementacion

### Fase 0: Inception Tecnica

Objetivo:

Definir contratos antes de escribir demasiado codigo.

Duracion sugerida: 2 a 3 semanas.

Agentes:

- delivery-coordinator: max.
- platform-architect: max.
- jmx-migration-lead: xhigh.
- qa-performance-semantics: xhigh.
- security-governance-lead: high.
- frontend-ux-lead: high.

Entregables:

- ADR-001: Arquitectura general.
- ADR-002: IR canonico y versionado.
- ADR-003: Estrategia de migracion JMX por niveles.
- ADR-004: Modelo de ejecucion local.
- ADR-005: Reportes nativos vs observability export.
- ADR-006: Seguridad, secretos e IA.
- Schema inicial del IR.
- Schema del reporte de fidelidad.
- 10 JMX fixtures representativos.
- Matriz JMeter element -> support level.
- UX wireframes MVP.
- Benchmark plan contra JMeter.

Definition of Done:

- El repo compila con scaffold minimo.
- Hay schemas versionados.
- Hay fixtures JMX.
- Hay ADRs aceptados.
- Hay plan de benchmark.

### Fase 1: Bootstrap Del Core Y CLI

Objetivo:

Crear base ejecutable local con IR/YAML, CLI y parser JMX inicial.

Duracion sugerida: 4 a 6 semanas.

Agentes:

- rust-engine-lead: xhigh.
- cli-dx-lead: high.
- jmx-migration-lead: xhigh.
- platform-architect: high.
- qa-performance-semantics: high.

Entregables:

- Workspace Rust.
- Crate scenario-ir.
- Parser YAML -> IR.
- Validador IR.
- CLI `validate`.
- CLI `import jmx`.
- Parser XML/JMX inicial.
- Mapping de Test Plan, Thread Group, HTTP Sampler, Header Manager, CSV Data Set.
- Reporte de fidelidad JSON.
- Fixtures golden.

Definition of Done:

- `validate` detecta errores de schema.
- `import jmx` produce YAML e informe de fidelidad.
- Golden tests cubren fixtures base.
- No hay ejecucion de carga aun obligatoria.

### Fase 2: MVP Engine HTTP Local

Objetivo:

Ejecutar escenarios HTTP/HTTPS localmente con metricas confiables.

Duracion sugerida: 6 a 8 semanas.

Agentes:

- rust-engine-lead: xhigh.
- qa-performance-semantics: xhigh.
- reporting-analytics-lead: high.
- cli-dx-lead: high.
- observability-lead: medium/high.

Entregables:

- Scheduler ramp-up/hold/ramp-down.
- Usuarios virtuales asincronos.
- HTTP/HTTPS adapter.
- Variables y datasets CSV.
- Cookies y headers.
- Timers MVP.
- Assertions MVP.
- Extractores regex/JSONPath.
- CLI `run`.
- Summary JSON.
- Reporte HTML inicial.
- Export Prometheus opcional.

Definition of Done:

- Un YAML importado desde JMX ejecuta carga HTTP local.
- Se generan metricas p50/p90/p95/p99, throughput y error rate.
- Reporte HTML abre offline.
- Benchmarks iniciales comparan con JMeter en escenario HTTP simple.

### Fase 3: UI MVP Para QA

Objetivo:

Permitir flujo QA: importar, inspeccionar, editar lo esencial, ejecutar y leer reporte.

Duracion sugerida: 6 a 8 semanas.

Agentes:

- frontend-ux-lead: xhigh.
- reporting-analytics-lead: high.
- cli-dx-lead: high.
- platform-architect: medium/high.

Entregables:

- UI app.
- Import JMX desde UI.
- Vista arbol del plan.
- Vista de reporte de fidelidad.
- Editor de HTTP Sampler.
- Editor de variables/datasets.
- Editor de assertions/timers basicos.
- Ejecucion local desde UI.
- Dashboard live inicial.
- Reporte post-run.

Definition of Done:

- Un QA puede importar un JMX y ejecutar sin tocar CLI.
- La UI no requiere TypeScript.
- El reporte de fidelidad es claro.
- Capturas o pruebas visuales validan layout desktop.

### Fase 4: Migracion JMX Profunda Y Semantica JMeter

Objetivo:

Ampliar paridad con JMeter y reducir friccion de migracion.

Duracion sugerida: 8 a 12 semanas.

Agentes:

- jmx-migration-lead: xhigh.
- qa-performance-semantics: xhigh.
- rust-engine-lead: high/xhigh.
- ai-migration-lead: high para spikes.
- security-governance-lead: high.

Entregables:

- Mas controllers.
- Mas timers.
- Transaction Controller.
- Throughput Controller.
- Extractores avanzados.
- Assertions avanzadas.
- Deteccion de JSR223/Groovy/BeanShell.
- Migracion asistida inicial.
- Reporte de fidelidad enriquecido.
- Comparativas JMeter automatizadas.
- Spike de modo compatibilidad JVM aislado.

Definition of Done:

- Al menos 85% de elementos declarativos en fixtures representativos migran Nivel 1.
- El resto queda clasificado con accion sugerida.
- Existen tests comparativos contra JMeter para semantica critica.

### Fase 5: CI, Docker, Quality Gates Y Release

Objetivo:

Hacer el MVP usable en pipelines y equipos.

Duracion sugerida: 4 a 6 semanas.

Agentes:

- cli-dx-lead: high.
- devops-sre-lead: high.
- reporting-analytics-lead: high.
- security-governance-lead: high.

Entregables:

- Docker image.
- GitHub Actions examples.
- JUnit XML.
- Quality gates.
- Thresholds YAML.
- Exit codes confiables.
- Artefactos de reporte.
- Release multi-arch.
- SBOM.
- Signing basico.

Definition of Done:

- Un pipeline puede ejecutar prueba y fallar por thresholds.
- Se publica reporte como artefacto.
- Imagen Docker corre sin dependencias externas.

### Fase 6: Ejecucion Distribuida Y Kubernetes

Objetivo:

Escalar mas alla de un nodo sin repetir los problemas de JMeter remote testing.

Duracion sugerida: 8 a 12 semanas.

Agentes:

- devops-sre-lead: xhigh.
- rust-engine-lead: xhigh.
- observability-lead: high.
- security-governance-lead: xhigh.
- platform-architect: high.

Entregables:

- Coordinator.
- Worker agent.
- Protocolo gRPC.
- mTLS.
- Distribucion real de carga.
- Barrier start.
- Health reporting.
- Aggregation de metricas.
- Docker Compose distribuido.
- Kubernetes CRD `LoadTest`.
- Operator.
- Helm chart.

Definition of Done:

- Un test se distribuye entre N workers y reporta resultado consolidado.
- La carga se reparte, no se duplica.
- Fallos de worker se reportan claramente.
- mTLS y secretos funcionan.

### Fase 7: Kafka Y Eventos

Objetivo:

Cumplir compromiso de soporte Kafka durante el primer anio.

Duracion sugerida: 6 a 10 semanas.

Agentes:

- kafka-protocol-lead: xhigh.
- rust-engine-lead: high.
- reporting-analytics-lead: high.
- security-governance-lead: high.

Entregables:

- Kafka producer sampler.
- Config SSL/SASL.
- Payload templating.
- CSV/data-driven events.
- Assertions sobre publish result.
- Consumer validation opcional.
- Schema Registry spike/integracion.
- Metrics por topic/partition.

Definition of Done:

- Un escenario produce carga Kafka con metricas y errores claros.
- Credenciales se manejan como secretos.
- Reportes distinguen HTTP vs Kafka.

### Fase 8: Plugins WASM Y Registry Curado

Objetivo:

Crear extensibilidad segura sin abrir la puerta a ejecucion arbitraria.

Duracion sugerida: 8 a 12 semanas.

Agentes:

- plugin-wasm-lead: xhigh.
- security-governance-lead: xhigh.
- platform-architect: high.
- docs-qa-enablement: medium.

Entregables:

- WASM host.
- Plugin ABI.
- Manifest de plugin.
- Permisos declarativos.
- Firma/verificacion.
- Version pinning.
- Revocacion.
- Registry curado primera parte.
- SDK inicial.

Definition of Done:

- Un plugin primera parte corre con permisos limitados.
- Un plugin no firmado no carga.
- Documentacion explica riesgos y modelo de seguridad.

### Fase 9: IA Gobernada

Objetivo:

Usar IA para migracion, correlacion y analisis con controles de datos.

Duracion sugerida: 6 a 10 semanas.

Agentes:

- ai-migration-lead: xhigh.
- security-governance-lead: xhigh.
- product/ux via frontend-ux-lead: high.

Entregables:

- IA local.
- BYOK.
- SaaS opt-in.
- Redaccion/anonimizacion.
- Allowlist de datos.
- Analisis de scripts Groovy.
- Sugerencias de correlacion.
- Sugerencias de thresholds.
- Explicacion de resultados.

Definition of Done:

- Ningun dato sale a SaaS por defecto.
- El usuario ve exactamente que se enviaria.
- Toda sugerencia es revisable antes de aplicar.

### Fase 10: Enterprise Historico Y Colaboracion

Objetivo:

Agregar valor enterprise sin dañar el core OSS.

Duracion sugerida: 10 a 16 semanas.

Agentes:

- platform-architect: xhigh.
- reporting-analytics-lead: xhigh.
- security-governance-lead: xhigh.
- frontend-ux-lead: high.
- devops-sre-lead: high.

Entregables:

- Historico centralizado.
- Comparacion contra baseline.
- Trends.
- Regression detection.
- RBAC.
- SSO.
- Auditoria.
- Projects/teams.
- Run annotations.
- Retention policies.

Definition of Done:

- Runs quedan vinculados a commit/build/environment.
- Se puede comparar contra baseline.
- Hay controles enterprise de acceso y auditoria.

## 10. Backlog MVP Priorizado

Prioridad P0:

- Repo scaffold.
- IR schema.
- YAML parser.
- JMX parser.
- Mapping JMX Nivel 1 basico.
- Reporte de fidelidad.
- Engine HTTP local.
- CLI validate/import/run.
- CSV datasets.
- Variables.
- Timers basicos.
- Assertions basicas.
- Extractores regex/JSONPath.
- Reporte HTML.
- UI import/arbol/run/report.

Prioridad P1:

- More JMeter controllers.
- More assertions.
- Reporte live.
- Prometheus export.
- JUnit XML.
- Docker.
- Threshold gates.
- Comparativa JMeter automatizada.

Prioridad P2:

- Migracion asistida Groovy.
- Modo debug paso a paso.
- UI editor YAML.
- OTel export.
- SBOM/signing.
- Docker Compose.

Fuera del MVP:

- Kubernetes operator.
- Kafka.
- TS DSL.
- Marketplace terceros.
- IA SaaS.
- Multi-tenant enterprise.

## 11. Criterios De Aceptacion Globales Del MVP

1. Un QA puede importar un JMX HTTP real.
2. La herramienta genera YAML/IR y reporte de fidelidad.
3. El QA puede ejecutar localmente desde CLI.
4. El QA puede ejecutar localmente desde UI.
5. El reporte incluye percentiles, throughput, errores y series temporales.
6. La salida puede usarse en CI con exit codes.
7. El reporte HTML se abre offline.
8. El importador no falla silenciosamente ante elementos no soportados.
9. El engine demuestra mejora de eficiencia frente a JMeter en benchmark HTTP.
10. La documentacion permite completar el flujo en menos de 30 minutos.

## 12. Prompt Inicial Para Lanzar La Implementacion Con Claude Opus

Usar este prompt como primer mensaje para Claude Opus:

```text
Asume el rol de Coordinador Principal de Implementacion de una nueva suite moderna de performance testing que reemplazara Apache JMeter.

Tu objetivo no es solo escribir codigo: debes coordinar una implementacion por fases con agentes especializados, manteniendo calidad arquitectonica, compatibilidad de migracion JMX y foco en QA tradicional.

Contexto no negociable:
- Primer publico: QA tradicional que hoy usa JMeter para pruebas de carga.
- MVP: importar JMX, ejecutar localmente, mostrar UI familiar y generar reportes reconocibles.
- Motor: Rust/Tokio.
- Formato canonico: IR estructurado serializado como YAML.
- UI: web moderna, orientada a QA, sin requerir TypeScript al inicio.
- DSL TypeScript: diferido y opcional, compila al IR.
- Observabilidad: Prometheus/OpenTelemetry como exports, no como reemplazo del reporte nativo.
- Migracion JMX: profunda, por niveles, con reporte de fidelidad.
- Kafka: fase posterior dentro del primer anio, no MVP.
- IA: local, BYOK o SaaS opt-in, con redaccion y controles; SaaS apagado por defecto.
- Plugins: registry curado, primera parte al inicio, firma y version pinning.

Primero debes:
1. Leer CLAUDE_OPUS_IMPLEMENTATION_PLAN.md completo.
2. Crear un plan de ejecucion para Fase 0 y Fase 1.
3. Definir agentes especializados a desplegar y sus responsabilidades.
4. Crear ADRs iniciales antes de implementar logica compleja.
5. Crear el scaffold del repo con separacion entre scenario-ir, jmx-importer, engine, cli, reports y ui.
6. Implementar solo lo necesario para cerrar Fase 0/Fase 1 con pruebas y fixtures.

Reglas:
- No pierdas el foco en QA/JMX.
- No priorices Kubernetes, Kafka, IA producto o marketplace antes del MVP.
- No cambies el IR sin actualizar schema, fixtures y documentacion.
- No aceptes migraciones silenciosas: todo elemento JMX debe clasificarse como migrated, assisted, unsupported o ignored-with-reason.
- Cada fase debe terminar con comandos reproducibles y evidencia.

Entrega inicial esperada:
- Plan de trabajo de Fase 0/Fase 1.
- Lista de agentes a usar.
- ADRs a crear.
- Estructura inicial del repo.
- Primer set de tareas implementables.
- Criterios de aceptacion.

Empieza inspeccionando el workspace actual y proponiendo el primer commit funcional.
```

## 13. Prompt Para Crear Agentes Especializados En Claude

Si la herramienta permite definir agentes via configuracion, usar estos perfiles conceptuales:

```json
{
  "delivery-coordinator": {
    "description": "Coordina la implementacion completa por fases, mantiene decisiones arquitectonicas y revisa calidad.",
    "prompt": "Eres el coordinador principal. Tu prioridad es entregar el MVP enfocado en QA y migracion JMX profunda. Divide trabajo en fases, exige evidencia, crea ADRs y evita scope creep."
  },
  "platform-architect": {
    "description": "Define arquitectura, boundaries, IR, schemas y contratos entre modulos.",
    "prompt": "Eres arquitecto de plataforma. Disena el IR canonico, versionado, contratos y ADRs. Ningun modulo debe acoplarse innecesariamente."
  },
  "jmx-migration-lead": {
    "description": "Construye parser JMX, mapping JMX->IR y reporte de fidelidad.",
    "prompt": "Eres especialista en migracion JMX. Tu objetivo es importar planes JMeter reales por niveles, sin fallos silenciosos y con reporte de fidelidad claro para QA."
  },
  "rust-engine-lead": {
    "description": "Implementa engine Rust/Tokio, scheduler, VUs, HTTP adapter, timers y assertions.",
    "prompt": "Eres lider de engine Rust. Prioriza precision de timing, bajo overhead, tests y benchmarks. Mantente dentro del IR definido."
  },
  "qa-performance-semantics": {
    "description": "Valida semantica frente a JMeter y define benchmarks.",
    "prompt": "Eres QA performance senior. Compara comportamiento contra JMeter, especialmente timers, controllers, throughput, percentiles y reportes."
  },
  "frontend-ux-lead": {
    "description": "Construye UI para QA: importar, arbol, editar, ejecutar y analizar.",
    "prompt": "Eres arquitecto Frontend/UX. Disena una UI operativa para QA tradicional. No obligues a usar codigo. Prioriza flujo JMX->run->report."
  },
  "reporting-analytics-lead": {
    "description": "Implementa reportes nativos y artefactos CI.",
    "prompt": "Eres especialista en reportes de performance. Entrega HTML offline, JSON, JUnit, percentiles, throughput, errores y series temporales."
  },
  "security-governance-lead": {
    "description": "Define seguridad, secretos, plugins, IA gobernada y compatibilidad JVM aislada.",
    "prompt": "Eres security/governance engineer. Controla secretos, redaccion, firma de plugins, permisos y riesgos de IA. SaaS off por defecto."
  },
  "cli-dx-lead": {
    "description": "Implementa CLI y experiencia local/CI.",
    "prompt": "Eres lider CLI/DX. Crea comandos claros, exit codes, logs, validacion, importacion, ejecucion y quality gates."
  },
  "devops-sre-lead": {
    "description": "Entrega Docker, CI y luego ejecucion distribuida/Kubernetes.",
    "prompt": "Eres DevOps/SRE. Prioriza reproducibilidad, artefactos, Docker y CI. Kubernetes entra despues del MVP local."
  }
}
```

## 14. Comando Sugerido Para Claude CLI

Ejemplo conceptual. Ajustar segun disponibilidad local de modelos y permisos:

```bash
claude --model opus --effort max --agent delivery-coordinator -p "$(cat <<'PROMPT'
Asume el rol de Coordinador Principal de Implementacion.
Lee CLAUDE_OPUS_IMPLEMENTATION_PLAN.md completo y comienza con Fase 0/Fase 1.
Debes definir agentes especializados, ADRs iniciales, scaffold del repo y primer set de tareas implementables.
No pierdas el foco en QA tradicional y migracion JMX profunda.
PROMPT
)"
```

Si el CLI soporta `--agents`, convertir los perfiles del apartado anterior a JSON y pasarlos en la configuracion de la sesion.

## 15. Primeras Tareas Concretas Para El Coordinador

1. Confirmar nombre temporal del binario.
2. Crear ADR-001 a ADR-006.
3. Crear estructura de repo.
4. Definir `scenario-ir.schema.json`.
5. Definir `migration-report.schema.json`.
6. Crear fixtures JMX minimos:
   - HTTP GET simple.
   - HTTP POST con headers.
   - CSV Data Set.
   - Regex extractor.
   - JSON extractor.
   - Constant Timer.
   - Loop Controller.
   - Transaction Controller.
   - JSR223 example.
   - Unsupported plugin example.
7. Implementar parser JMX inicial.
8. Implementar mapping Test Plan -> IR.
9. Implementar reporte de fidelidad.
10. Implementar CLI `validate`.
11. Implementar CLI `import jmx`.
12. Agregar golden tests.
13. Documentar flujo "Migrar primer JMX".

## 16. Principio Rector

La herramienta gana si un QA que hoy usa JMeter puede decir:

"Importe mi JMX, entendi que migro y que no, ejecute la prueba localmente, obtuve un reporte que reconozco y puedo llevar esto a CI sin reescribir todo."

Todo lo que no acerque el producto a esa frase debe esperar.
