# Matriz de soporte JMeter → perfkit

Catálogo de elementos JMeter y su nivel de fidelidad en el importador (a IR v0.2.0).
Niveles (ver `docs/adr/ADR-003`): **Nivel 1** nativo 1:1 · **Nivel 2** asistido (revisión
manual) · **Nivel 4** no soportado (reportado, nunca silencioso). El importador clasifica
cada elemento como `migrated | assisted | unsupported | ignored`.

## Nivel 1 — Migración nativa (migrated)

| Elemento JMeter | Notas |
|---|---|
| Test Plan | nombre + User Defined Variables |
| Thread Group / setUp / tearDown | threads, ramp-up, loops, scheduler (duration) |
| HTTP Request Defaults | base_url, timeouts, follow redirects |
| HTTP Request (HTTPSamplerProxy) | método, URL, query, body raw/form, headers |
| HTTP Header Manager | a nivel de sampler, grupo o plan |
| HTTP Cookie Manager | cookies habilitadas por VU en el engine |
| User Defined Variables (Arguments) | → `scenario.variables` |
| CSV Data Set Config | filename, variableNames, delimiter, recycle, ignoreFirstLine |
| Constant / Uniform / Gaussian / Constant Throughput Timer | think-time / pacing |
| Response Assertion | response_code (→ status), response_data contains/matches (regex) |
| Duration Assertion · Size Assertion | umbral de latencia / tamaño |
| JSON Assertion (JSONPathAssertion) | JSONPath + valor esperado |
| Regular Expression Extractor | refname, regex, template ($n$), default |
| JSON Extractor (JSONPostProcessor) | referenceNames, jsonPathExprs, defaults |
| Boundary Extractor | left/right boundary |
| Loop / If / While Controller | If/While: condiciones simples (`==`, `!=`, `true/false`) |
| Transaction Controller | mide la transacción agregada |
| Simple Controller (GenericController) | se aplana en el padre |
| **Throughput Controller (percent)** | porcentaje de ejecuciones (v0.2) |
| **Interleave Controller** | un hijo por pasada, rotatorio (v0.2) |
| **Random Controller** | un hijo al azar por pasada (v0.2) |

## Nivel 2 — Migración asistida (assisted)

| Elemento | Razón / acción |
|---|---|
| JSR223 Sampler/Pre/PostProcessor (Groovy) | sin equivalente declarativo; portar a extractores/variables |
| BeanShell Sampler/Pre/PostProcessor | idem, legacy |
| Once Only Controller | en perfkit se ejecuta cada iteración; envolver en condición |
| Throughput Controller "total executions" | no soportado el modo total; se ejecuta siempre, usar porcentaje/loop |
| If/While con expresiones complejas | el engine solo evalúa `==`, `!=`, `true/false` |

## Ignorados con razón (ignored)

| Elemento | Razón |
|---|---|
| Listeners (View Results Tree, Aggregate Report, ...) | en perfkit el reporte es nativo |
| Cache Manager | no relevante para generación de carga en el MVP |

## Nivel 4 — No soportado (unsupported, reportado)

| Elemento | Estado |
|---|---|
| JDBC Request / config | fuera del MVP HTTP |
| JMS / MQ | futuro |
| XPath Extractor / XPath Assertion | requiere motor XML; pendiente |
| Plugins de terceros (.jar) | no se ejecutan; se reportan |
| Remote testing (distribuido legacy) | Fase 6 (modelo propio) |

> **Kafka samplers:** ya **no** son "no soportado". Desde IR `0.3.0` se migran como
> `assisted` a `Step::Kafka` (el engine los ejecuta con `rskafka`; SASL/SSL y broker
> real validado siguen diferidos — ver ADR-009). El export inverso IR→JMX los emite
> como comentario XML cuando JMeter no tiene un sampler nativo equivalente.

> Cualquier elemento no listado se reporta como `unsupported` con su tipo y ruta — **nunca
> se ignora en silencio**. La cobertura se mide en el reporte de fidelidad (`perfkit import
> jmx ... --fidelity-report`) y en los fixtures de `examples/jmx/`.
