# ADR-008: Throughput / Interleave / Random controllers — IR v0.2.0

## Estado
Aceptado (Fase 4).

## Contexto
La Fase 4 amplía la paridad con JMeter ("más controllers"). El MVP (IR v0.1.0) cubría
Loop, If, While, Transaction y Simple/Once-Only. Faltaban tres controladores de uso
común que cambian el flujo de ejecución por pasada:
- **Throughput Controller** (porcentaje de ejecuciones),
- **Interleave Controller** (un hijo por pasada, rotatorio),
- **Random Controller** (un hijo al azar por pasada).

Añadirlos modifica un **contrato compartido** (el IR), lo que por las reglas del
`delivery-coordinator` (§8) exige ADR + bump de versión + schema + fixtures + docs.

## Decisión
- Subir **`IR_VERSION` a `0.2.0`** (cambio aditivo: nuevas variantes de `Step`).
- Añadir al IR: `Step::Throughput(ThroughputController{name, percent, steps})`,
  `Step::Interleave(InterleaveController{name, steps})`,
  `Step::Random(RandomController{name, steps})`.
- **Semántica en el engine** (verificada con tests contra un servidor real):
  - Throughput: ejecuta sus hijos con probabilidad `percent`% por pasada
    (0 → nunca, 100 → siempre, intermedio → aleatorio por pasada). Aproxima el modo
    "percent executions" de JMeter; el modo "total executions" se importa como
    **assisted** (se ejecuta siempre) con sugerencia.
  - Interleave: contador rotatorio por VU y por nombre de controlador → ejecuta el
    siguiente hijo en cada pasada.
  - Random: elige un hijo al azar por pasada.
- **Importer**: mapea `ThroughputController` (lee `percentThroughput` tanto de
  `stringProp` como de `<FloatProperty>`), `InterleaveControl` y `RandomController`.
- Regenerar `schemas/scenario-ir.schema.json` con `perfkit schema`.
- Fixtures: `examples/jmx/throughput-controller.jmx`, `interleave-controller.jmx`.

## Consecuencias
- Escenarios v0.1.0 siguen siendo válidos (los campos no cambiaron); los nuevos
  escenarios declaran `version: 0.2.0`.
- La UI (`types.ts`, `PlanTree`) reconoce las nuevas variantes.
- Tests de semántica (`crates/engine/tests/semantics.rs`) fijan el comportamiento
  esperado (Loop = count×iteraciones, Interleave equireparte, Throughput 0/100).

## Alternativas consideradas
- **No versionar / cambio silencioso**: rechazado por las reglas de contrato (§8).
- **Modelar Throughput como total executions exacto**: requiere estado por-thread más
  complejo; se difiere. El modo porcentaje cubre el caso más común y es determinista
  en los extremos.
