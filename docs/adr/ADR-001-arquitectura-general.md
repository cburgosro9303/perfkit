# ADR-001: Arquitectura general (open-core, workspace de crates)

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** delivery-coordinator, platform-architect

## Contexto

`perfkit` es una suite moderna de performance testing escrita en Rust + Tokio
cuyo objetivo es ofrecer una transición creíble desde Apache JMeter para equipos
de QA tradicional. El plan (§2, §3, §4) fija varias restricciones no negociables:
motor en Rust/Tokio, formato canónico IR serializado como YAML, migración JMX
profunda como cuña de adopción, y un modelo de producto **open-core** donde el
importador JMX y el reporte de fidelidad viven en el core abierto (si se vuelven
comerciales, se reduce la adopción).

Necesitamos una arquitectura que: (1) mantenga separación estricta entre el IR,
el importador, el motor, los adaptadores, los reportes y la UI; (2) permita
construir el MVP local sin servicios externos; y (3) preserve la posibilidad de
añadir ejecución distribuida, Kafka, plugins WASM e IA gobernada sin reescribir
el núcleo.

## Decisión

Adoptamos un **workspace Cargo de Rust (edición 2024)** con un crate por
responsabilidad y límites explícitos entre módulos. Ningún crate depende de los
detalles internos de otro: la comunicación pasa por el IR y por interfaces
públicas.

Crates del workspace (`crates/`):

- `scenario-ir`: IR canónico + serde + JSON Schema (schemars) + validador.
- `jmx-importer`: parser JMX (roxmltree), mapeo Nivel 1, reporte de fidelidad.
- `http-adapter`: ejecución HTTP/HTTPS (reqwest/rustls), cookie store por VU.
- `engine`: scheduler, VUs async, timers, assertions, extractores, datasets.
- `metrics`: histogramas (hdrhistogram), percentiles, throughput, series.
- `reports`: HTML offline / JSON / JUnit + quality gate.
- `security`: secretos por entorno + redacción (stub para la política completa).
- `cli`: binario `perfkit` (orquesta el resto).

La **app de UI** (`ui/src-tauri`) es una app nativa Tauri 2 + React/TS/Tailwind
y se mantiene **excluida del workspace** (`exclude = ["ui/src-tauri"]`) para que
`cargo build --workspace` del core no exija las dependencias de sistema de Tauri.
La UI consume los crates del engine por path (ver ADR-007).

### Toolchain: Rust 1.95.0 (desviación del 1.96.0 pedido)

El plan (§2.4) pedía **Rust 1.96.0 edición 2024**. El entorno de desarrollo tiene
**1.95.0**, que **ya soporta edición 2024**. Para garantizar reproducibilidad y no
bloquear el bootstrap se fija la toolchain a `1.95.0` en `rust-toolchain.toml`
(`channel = "1.95.0"`, componentes `rustfmt` y `clippy`), con `rust-version = "1.95"`
en el `Cargo.toml` del workspace. **Esta es una desviación consciente y documentada**:
no cambia ninguna decisión arquitectónica (edición 2024 se mantiene) y puede
revisarse al subir a 1.96.0 cuando esté disponible en el entorno.

## Consecuencias

**Positivas**

- Compilación independiente y tests por crate; el core compila sin Tauri.
- Límites claros: cambiar el motor no toca el importador ni los reportes.
- El IR como contrato único reduce el acoplamiento (ver ADR-002).
- El modelo open-core mantiene el importador JMX como parte abierta de la cuña.

**Negativas / costos**

- Más ceremonia de cargo (varios `Cargo.toml`, deps de workspace).
- La UI fuera del workspace exige cuidado para que sus deps por path no se
  desincronicen de las versiones del core.
- La desviación de toolchain debe re-evaluarse explícitamente más adelante.

## Alternativas consideradas

- **Crate monolítico único:** descartado; mezcla hot path del motor con parsing y
  reportes, dificulta tests y viola la separación exigida por el plan (§4.2).
- **Multi-repo (un repo por componente):** descartado para el MVP; añade fricción
  de versionado y CI sin beneficio mientras el equipo es pequeño.
- **Incluir la UI Tauri dentro del workspace:** descartado; obligaría a instalar
  dependencias de sistema de Tauri para cualquier `cargo build --workspace`.
- **Fijar Rust 1.96.0 estricto:** descartado por no estar disponible en el entorno;
  habría bloqueado el bootstrap sin aportar nada (1.95.0 ya da edición 2024).
