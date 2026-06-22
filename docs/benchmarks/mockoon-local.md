# Benchmark local con Mockoon (perfkit vs JMeter)

Comparativa **reproducible y offline**: perfkit y Apache JMeter ejecutan el **mismo
plan** contra el **mismo target** (un mock local de Mockoon). Así la comparación es
justa (ambos pegan al mismo servidor) y no depende de internet ni de límites de tasa.

## 1. Levantar el mock

El API de mock está en [`tools/mockoon/perfkit-bench.json`](../../tools/mockoon/perfkit-bench.json)
(puerto **3001**, respuestas fijas sin plantillas → rápidas y deterministas). Endpoints:
`GET /` (throughput), `GET /get`, `POST /post`, `GET /uuid`, `GET /headers`.

- **App Mockoon:** ábrela → *Import/Open environment* → elige `tools/mockoon/perfkit-bench.json`
  → botón **Play** (queda escuchando en `http://localhost:3001`).
- **CLI (opcional, headless):**
  ```bash
  npm i -g @mockoon/cli
  mockoon-cli start --data tools/mockoon/perfkit-bench.json --port 3001
  ```

## 2. Correr el benchmark

```bash
bash tools/benchmark-mockoon.sh [threads] [duration_secs] [port]
# por defecto: 50 threads, 20 s, puerto 3001
```

El script:
1. Verifica que el mock responde en `http://127.0.0.1:<port>` (si tienes `mockoon-cli`
   instalado y el mock no está arriba, **lo arranca solo**).
2. Compila `perfkit --release` e importa `examples/jmx/bench-mockoon.jmx` a IR. El plan
   recorre **los 5 endpoints** del mock por iteración (GET `/`, GET `/get?page=1`,
   GET `/uuid` → extrae `token`, POST `/post` con `${token}`, GET `/headers` con
   `Authorization: Bearer ${token}`) → mezcla GET/POST + correlación, más representativo.
3. Hace warmup y mide **perfkit** y **JMeter** con la misma carga, capturando memoria
   pico con `/usr/bin/time -l`.
4. Imprime una tabla comparativa y escribe `docs/benchmarks/mockoon-results.md`.

> Requisitos para el lado JMeter: `jmeter` en el `PATH` y un JDK 21 (la ruta se puede
> sobreescribir con `JDK21=/ruta/al/jdk bash tools/benchmark-mockoon.sh`). Si no hay
> `jmeter`, el script mide solo perfkit.

## 3. Cómo leer los resultados

Resultado típico (validado localmente; los números exactos dependen de tu máquina y de
que Mockoon —Node— suele ser el cuello de botella):

| Métrica | perfkit | JMeter | Ventaja |
|---|---:|---:|---:|
| Throughput (req/s) | ~114k | ~114k | ~1.0x (paridad) |
| Errores | 0 | 0 | — |
| Memoria pico RSS (MB) | ~17 | ~900 | **~50x menos** |

**Interpretación honesta:** como Mockoon (Node) satura antes que el generador, el
*throughput* sale **a la par** entre ambos (los dos chocan con el mismo techo del mock).
La ventaja real y medible del motor Rust/Tokio es la **memoria** (~50x menos) y el
overhead por VU. Para estresar el generador y no el mock, usa un target multi-worker o
sube los VUs hasta que el mock deje de ser el límite.

## Extra: correr el checkout completo contra el mock

El mock también sirve los endpoints del ejemplo de checkout (`/post`, `/uuid`, `/get`),
así que puedes ejecutar el flujo rico contra él sin internet:

```bash
perfkit run /tmp/checkout.yaml --base-url http://127.0.0.1:3001 --vus 20 --duration 15
# (o importa examples/jmx/checkout-demo.jmx y apunta --base-url al mock)
```
