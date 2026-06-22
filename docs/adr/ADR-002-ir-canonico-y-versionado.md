# ADR-002: IR canónico y versionado

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** platform-architect, jmx-migration-lead

## Contexto

El plan (§2.5, §2.6, §4.1) establece que el formato canónico de escenarios es un
**IR estructurado serializado como YAML**, que la UI edita el IR (el QA no escribe
TypeScript para empezar) y que un eventual DSL TypeScript compilaría siempre al
IR. El IR es, por tanto, el **contrato central** que comparten importador, engine,
reports, CLI y UI. Necesitamos: (1) una representación humana y versionable;
(2) un schema verificable por máquina; y (3) una política de versionado que evite
romper escenarios existentes en silencio.

## Decisión

El IR vive en el crate `scenario-ir` (`Scenario`, `ThreadGroup`, `HttpDefaults`,
`Dataset`, samplers, timers, assertions, extractores, `Metadata`).

1. **Serialización YAML** con `serde` + `serde_yaml_ng` (`0.10`). YAML es el formato
   humano/versionable; la UI lee y escribe este YAML.
2. **JSON Schema** generado con `schemars` (`1.2`) vía `derive(JsonSchema)` en los
   tipos del IR. El comando `perfkit schema` materializa los schemas en `schemas/`
   (escenario + reporte de fidelidad). El schema es la fuente de verdad para
   validación y para herramientas externas.
3. **Validador del IR** en `scenario-ir`: deserializar YAML al IR ya valida la
   estructura; reglas semánticas adicionales se aplican sobre el modelo cargado.
4. **Versionado semver del IR.** El `Scenario` tiene un campo `version` (string
   semver) que por defecto toma `IR_VERSION`. Hoy `IR_VERSION = "0.1.0"`. Un cambio
   incompatible debe **subir** esta versión; los lectores deben rechazar (o migrar)
   versiones mayores desconocidas en lugar de adivinar.

### Regla de gobernanza (no negociable)

**Cambiar el IR ⇒ actualizar, en el mismo cambio: (a) los tipos en `scenario-ir`,
(b) el JSON Schema regenerado, (c) los fixtures/golden afectados y (d) la
documentación.** Esto refleja el bloqueo de merges del plan (§8): "no cambies el
IR sin actualizar schema, fixtures y documentación" y "no rompas compatibilidad de
escenarios existentes sin version bump". Ningún módulo modifica este contrato sin
un ADR.

## Consecuencias

**Positivas**

- Un único contrato desacopla los crates: el engine ejecuta el IR, no el JMX.
- El JSON Schema habilita validación temprana (CLI, UI, CI) y editores externos.
- El campo `version` permite evolucionar el formato sin sorpresas silenciosas.
- YAML es revisable en code review y diffeable en control de versiones.

**Negativas / costos**

- Disciplina obligatoria: todo cambio del IR toca 4 lugares (tipos/schema/fixtures/docs).
- `serde_yaml_ng` es un fork mantenido de `serde_yaml`; hay que seguir su evolución.
- Mantener semver del IR exige criterio para distinguir cambios compatibles de los
  que requieren bump mayor.

## Alternativas consideradas

- **JSON como formato canónico:** descartado para edición humana; YAML es más legible
  para QA y mejor para diffs. (JSON sigue disponible como salida de reportes.)
- **TypeScript/DSL como formato primario:** descartado por §2.6; obligaría a QA a
  programar. El DSL queda diferido y, si existe, compila al IR.
- **Schema escrito a mano (JSON Schema manual):** descartado; se desincroniza de los
  tipos. Generarlo desde el código (schemars) mantiene una sola fuente de verdad.
- **Sin campo de versión:** descartado; imposibilita detectar incompatibilidades y
  contradice la regla de no romper escenarios en silencio.
