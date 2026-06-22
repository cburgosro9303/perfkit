# ADR-003: Estrategia de migración JMX por niveles

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** jmx-migration-lead, qa-performance-semantics

## Contexto

La migración profunda de JMX es **prioridad de producto**, no una herramienta
secundaria (plan §2.2). La promesa correcta no es "100% automático" sino **alta
fidelidad declarativa con niveles explícitos** y un reporte que el QA entienda
(plan §5). El riesgo a evitar es una migración que falla en silencio: un elemento
JMeter que desaparece sin que nadie lo note produce pruebas inválidas.

## Decisión

### Los cuatro niveles de fidelidad (§5)

- **Nivel 1 — Migración nativa 1:1 (en MVP):** el elemento se traduce a un
  equivalente nativo del IR.
- **Nivel 2 — Migración asistida:** scripting (JSR223/Groovy/BeanShell), funciones
  `__groovy`/`__jexl3`/`__javaScript`, correlación o expresiones complejas. Se
  detecta, se ubica en el árbol y se propone equivalencia o acción manual.
- **Nivel 3 — Compatibilidad JVM opt-in (post-MVP):** ejecutar lógica Groovy en un
  sidecar JVM aislado y degradado. No entra en el MVP salvo spike.
- **Nivel 4 — No soportado:** plugins `.jar` de terceros, JDBC/JMS avanzado, remote
  testing legacy, samplers propietarios. Se **reporta explícitamente**.

### Contrato del reporte de fidelidad

El importador (`jmx-importer`) produce, junto al escenario, un `MigrationReport`
(definido en `scenario-ir::migration`). Cada elemento del JMX se clasifica con un
`MappingStatus`:

- `migrated` — traducido 1:1 a IR (Nivel 1).
- `assisted` — requiere revisión/acción manual (Nivel 2).
- `unsupported` — reportado, no ejecutado (Nivel 4).
- `ignored` — ignorado a propósito con razón (p.ej. listeners como metadata).

Cada `MappedElement` lleva `jmx_type`, `jmx_name`, `path` (ruta en el árbol),
`status`, `ir_ref` opcional y, **obligatoriamente para assisted/unsupported/ignored,
una `reason`**, además de una `suggestion` para el QA. El `FidelitySummary` agrega
totales y un `fidelity_pct` calculado como `migrated / (total − ignored)` (la
fidelidad mide migración declarativa exitosa sobre lo migrable).

### Regla de oro: nunca fallar en silencio

Ningún elemento del árbol JMX puede desaparecer sin quedar registrado en el
reporte. Un elemento inesperado se marca `unsupported` con razón, nunca se omite.
`MigrationReport::needs_attention()` indica si hay `assisted` o `unsupported` para
que la CLI/UI lo destaquen.

### Catálogo Nivel 1 del MVP

Objetivo declarativo del MVP (subconjunto del §5 priorizado por el backlog):
Test Plan, Thread Group básico, HTTP Request Defaults, HTTP Samplers, Header
Manager, Cookie Manager, Cache Manager, User Defined Variables, CSV Data Set
Config, Constant/Uniform Random/Gaussian Random/Constant Throughput Timers,
Response/Duration/Size/JSON Assertions, Regular Expression / JSON / Boundary
Extractors, y controllers Loop / Simple / Once Only / Transaction / If-While
(expresiones simples). Los **Listeners** se tratan como metadata de reporte
(`ignored` con razón), no como ejecución literal.

## Consecuencias

**Positivas**

- El QA siempre sabe qué migró, qué necesita acción y qué no se soporta.
- El `fidelity_pct` da una métrica objetiva de calidad de migración por fixture.
- La clasificación por niveles fija expectativas realistas y evita prometer de más.

**Negativas / costos**

- Mantener el catálogo y las razones/sugerencias por elemento es trabajo continuo.
- El reporte añade superficie a mantener junto al IR (su schema se versiona igual).
- Nivel 3 (JVM) queda como deuda explícita para clientes que no pueden reescribir.

## Alternativas consideradas

- **Prometer 100% automático:** descartado por §5; sin evidencia es falso y daña la
  confianza al primer JMX que no migra limpio.
- **Fallar al primer elemento no soportado:** descartado; bloquea migraciones que
  son válidas en su mayor parte. Mejor migrar lo posible y reportar el resto.
- **Omitir silenciosamente lo desconocido:** descartado explícitamente; es el peor
  resultado posible (pruebas inválidas sin aviso).
- **Mapear todo a un único "modo compatibilidad" JVM:** descartado para el MVP;
  el modo nativo es superior y la JVM es Nivel 3 opt-in.
