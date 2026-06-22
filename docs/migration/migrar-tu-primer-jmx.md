# Migrar tu primer JMX (en menos de 30 minutos)

Esta guía lleva a un QA desde un archivo `.jmx` existente hasta un reporte
ejecutado localmente y un quality gate en CI, usando la CLI `perfkit`. No requiere
escribir TypeScript ni levantar ninguna infraestructura.

> Objetivo (criterio de aceptación del MVP): "importé mi JMX, entendí qué migró y
> qué no, ejecuté la prueba localmente, obtuve un reporte que reconozco y lo llevé
> a CI sin reescribir todo".

## 0. Requisitos e instalación

- Rust **1.95.0** (lo fija `rust-toolchain.toml`; al entrar al repo se selecciona solo).
- Tu archivo `.jmx` a mano.

Compila el binario:

```bash
cargo build --release
# el binario queda en target/release/perfkit
```

Verifica que responde:

```bash
./target/release/perfkit --help
```

## 1. Importar el JMX al IR

```bash
perfkit import jmx tu.jmx -o escenario.yaml --fidelity-report
```

Esto produce dos cosas:

- `escenario.yaml`: tu plan en el **IR canónico** (YAML legible y versionable).
- el **reporte de fidelidad** (JSON) con la clasificación de cada elemento.

## 2. Leer el reporte de fidelidad (paso clave)

El importador **nunca falla en silencio**: cada elemento del JMX queda clasificado.

| Estado        | Qué significa                                              | Qué hacer                                   |
|---------------|-----------------------------------------------------------|---------------------------------------------|
| `migrated`    | Migrado 1:1 a IR nativo (Nivel 1)                         | Nada. Listo.                                |
| `assisted`    | Necesita revisión/acción manual (Nivel 2, p.ej. Groovy)  | Revisa la `reason` y aplica la `suggestion`.|
| `unsupported` | No soportado por ahora (Nivel 4)                         | Decide reemplazo o quítalo del alcance.     |
| `ignored`     | Ignorado a propósito con razón (p.ej. listeners)        | Normalmente nada; verifica la razón.        |

Cada entrada incluye `jmx_type`, `jmx_name`, la `path` en el árbol y, para
`assisted`/`unsupported`/`ignored`, una `reason` y una `suggestion`. El resumen
trae el `fidelity_pct` (porcentaje migrado 1:1 sobre lo migrable).

### Qué hacer con `assisted` y `unsupported`

- **`assisted` (scripting/correlación):** abre el `escenario.yaml` en la ubicación
  indicada por `path`. Si la `suggestion` propone un equivalente nativo
  (extractor regex/JSONPath, variable, timer), aplícalo. Si la lógica Groovy es
  imprescindible y no tiene equivalente, queda como deuda para el modo
  compatibilidad JVM (post-MVP); márcalo y continúa con el resto.
- **`unsupported` (plugins `.jar`, JDBC/JMS avanzado, etc.):** no se ejecuta. Elige
  reemplazo declarativo si existe, o acota el escenario para el MVP. El elemento ya
  quedó registrado, así que tu prueba no lo perderá en silencio.

> ¿Solo quieres el reporte sin ejecutar nada más? `perfkit convert jmx tu.jmx --fidelity-report`.

## 3. Validar el escenario

```bash
perfkit validate escenario.yaml
```

Comprueba que el YAML cumple el schema del IR. Si editaste el YAML a mano, este
paso atrapa errores de estructura antes de ejecutar.

## 4. Ejecutar la prueba localmente

```bash
perfkit run escenario.yaml --report html --out reports/run-001
```

Opciones útiles:

- `--report html json junit` para generar varios formatos (por defecto, todos).
- `--base-url https://staging.tu-app.com` para redirigir todas las peticiones a otro origen.
- `--vus 50` y `--duration 120` para sobrescribir usuarios virtuales y duración (segundos).

Para depurar una sola iteración con 1 VU antes de cargar:

```bash
perfkit debug escenario.yaml --once
```

## 5. Abrir el reporte HTML

```bash
open reports/run-001/index.html   # macOS (o ábrelo con doble clic)
```

El HTML es **autocontenido y abre offline** (sin servidor ni red). Verás
percentiles **p50/p90/p95/p99/p99.9**, throughput, error rate y series de tiempo,
con un formato reconocible para quien viene de JMeter.

## 6. Quality gate en CI

Define umbrales en `thresholds.yaml`:

```yaml
max_error_rate: 0.01        # 1% de error como máximo
max_p95_ms: 800             # p95 ≤ 800 ms
max_p99_ms: 1500            # p99 ≤ 1500 ms
min_throughput_per_sec: 50  # al menos 50 req/s
```

Ejecuta el gate sobre el `summary.json` del run:

```bash
perfkit gate reports/run-001/summary.json --thresholds thresholds.yaml
```

Si algún umbral no se cumple, el comando devuelve un **exit code distinto de cero**,
de modo que el pipeline falla. En CI, además, publica `reports/run-001/index.html`
como artefacto y usa `junit.xml` para que la pestaña de tests del pipeline muestre
el resultado.

### Ejemplo en CI (concepto)

```bash
perfkit import jmx tu.jmx -o escenario.yaml --fidelity-report
perfkit validate escenario.yaml
perfkit run escenario.yaml --report html json junit --out reports/run-001
perfkit gate reports/run-001/summary.json --thresholds thresholds.yaml
# exit code != 0 ⇒ el pipeline falla por umbrales
```

## Resumen del flujo

```text
tu.jmx → import jmx → escenario.yaml + reporte de fidelidad
       → validate → run --report html → abrir HTML → gate (CI)
```

Manejo de secretos: no escribas tokens en el YAML. Pásalos por variables de entorno
con prefijo (p.ej. `PERFKIT_VAR_TOKEN=...`) para que no se versionen; los logs y
reportes redactan valores sensibles comunes (ver ADR-006).
