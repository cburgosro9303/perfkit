# Benchmark perfkit vs Apache JMeter

Comparativa reproducible del generador de carga, no del target.

## Metodología

- **Mismo plan** para ambos: `examples/jmx/bench-http.jmx` (1 Thread Group, GET, sin think-time, keep-alive activado). perfkit lo **importa** a IR; JMeter lo corre tal cual. Así ambos ejecutan la misma lógica.
- **Mismo target local**: `tools/bench-target.js` (servidor HTTP Node de una sola CPU que responde un JSON pequeño).
- **Misma carga**: 50 VUs/threads, 20 s, sin ramp.
- perfkit compilado en `--release`; JMeter `-n` (no-GUI) sobre **JDK 21**.
- **Memoria** = pico RSS vía `/usr/bin/time -l` (en JMeter mide el JVM, que `exec`-uta `java`).
- Reproducir: `bash tools/benchmark.sh [threads] [duration]`.

## Resultado (50 VUs, 20 s, M-series)

| Métrica | perfkit | JMeter | Ventaja |
|---|---:|---:|---:|
| Throughput | 116,587 req/s | 123,863 req/s | 0.94x |
| Requests | 2,331,797 | 2,477,253 | — |
| Errores | 0 | 0 | — |
| p50 / p95 / p99 (ms) | 0.4 / 0.7 / 0.9 | ~0 / 1.0 / 1.0 | comparable |
| **Pico RSS** | **20 MB** | **906 MB** | **46x menos** |

## Interpretación (honesta)

- **Throughput a la par**, porque **el cuello de botella es el target** (un solo proceso Node satura a ~120k req/s); ninguno de los dos generadores es el límite. Por eso el criterio "2x VUs/core" del plan (§6.5) no se demuestra por throughput en este escenario: habría que usar un target multi-worker o medir VUs sostenibles por unidad de memoria.
- **La ventaja real y medible es la memoria: ~46x menos** (20 MB vs 906 MB). Eso es exactamente "más VUs por core/RAM": perfkit puede sostener muchísimos más usuarios virtuales en la misma máquina. El modelo async (Tokio, tareas) pesa ~KB por VU; el modelo de JMeter (un hilo JVM por VU) pesa ~MB por VU.
- **Latencia comparable**, con perfkit ligeramente mejor en colas (p95 0.7 vs 1.0 ms) por menor overhead de reporte (agregación en histograma fuera del hot path vs escritura de JTL por muestra).

## Aprendizaje de compatibilidad

La primera corrida dio **40% de errores en JMeter** (`java.net.BindException`): el plan no traía `HTTPSampler.use_keepalive`, así que JMeter abría una conexión TCP por request y agotaba puertos efímeros. perfkit reutiliza conexiones por VU (pool keep-alive) y no sufrió esto. Tras activar keep-alive en el JMX, ambos quedaron en 0 errores. Es justo el tipo de detalle de semántica JMeter que el importador y la suite de compatibilidad deben cuidar (Fase 4).

## Próximos pasos del benchmark (Fase 5/§6.5)

- Target multi-worker para sacar al target del camino y medir el techo real de cada generador.
- Curva VUs↔memoria (VUs sostenibles por GB).
- Escenarios con think-time, datasets y assertions para medir overhead de cada feature.
