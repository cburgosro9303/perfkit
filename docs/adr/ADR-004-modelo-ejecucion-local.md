# ADR-004: Modelo de ejecución local

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** rust-engine-lead, qa-performance-semantics

## Contexto

El MVP debe ejecutar escenarios HTTP/HTTPS localmente con métricas confiables y
demostrar mejor eficiencia que JMeter (plan §2.4, §6.3, §11.9). JMeter usa un hilo
por usuario virtual (modelo thread-per-VU), lo que limita la concurrencia y el uso
de memoria. Necesitamos un modelo de ejecución con alta concurrencia, baja
memoria, latencia predecible y medición que no se contamine con el overhead de
reporte.

## Decisión

El motor (`engine`) se construye sobre **Tokio** con VUs **asíncronos** (no
thread-per-VU) y el adaptador HTTP usa **reqwest con rustls**.

### Scheduler ramp-up / hold / ramp-down

Por `ThreadGroup` el scheduler arranca los VUs escalonadamente durante el
`ramp_up`, los mantiene durante `hold` (o por `duration`), y los retira en
`ramp_down`. El arranque de cada VU se reparte en el tiempo de ramp-up
(`ramp_up_ms * vu / virtual_users`) para evitar un pico inicial. La CLI permite
sobrescribir `--vus` y `--duration`.

### Aislamiento por VU

Cada VU mantiene su propio **cookie store** (vía el adaptador HTTP), de modo que
las sesiones no se mezclan entre usuarios virtuales, replicando el Cookie Manager
de JMeter por hilo. Las variables y el cursor de datasets se manejan por VU.

### Medición con reloj monotónico

La latencia de cada sample se mide con `std::time::Instant` (reloj **monotónico**),
no con el reloj de pared, para que ajustes de hora del sistema no corrompan las
mediciones. El `start` del run y los deadlines de grupo se derivan del mismo reloj.

### Agregación fuera del hot path

Los VUs **no agregan métricas**: emiten cada `Sample` por un canal `mpsc`
(`tokio::sync::mpsc::unbounded_channel`) hacia una **tarea agregadora** dedicada
que alimenta los histogramas (hdrhistogram) y construye las series por segundo.
Así la ruta caliente (generar carga) queda separada de la estadística, cumpliendo
el criterio del plan de no mezclar medición de latencia con overhead de reporte.
Opcionalmente se emite un `LiveSnapshot` para dashboards en vivo.

### Cancelación cooperativa

El run recibe una bandera de cancelación compartida; los VUs y el scheduler la
revisan en sus puntos de espera y terminan de forma ordenada (cooperativa), sin
matar tareas a la fuerza, garantizando que la agregadora drene los samples
pendientes antes de cerrar.

## Consecuencias

**Positivas**

- Miles de VUs por core con memoria acotada (async > thread-per-VU).
- Percentiles confiables: medición monotónica + agregación aislada del hot path.
- rustls evita depender de OpenSSL del sistema (builds reproducibles, ver ADR-006).
- Cancelación ordenada ⇒ resultados consistentes incluso al abortar un run.

**Negativas / costos**

- Código async es más difícil de razonar (backpressure, cancelación, deadlines).
- El canal mpsc unbounded puede crecer si la agregadora se atrasa; hay que vigilar
  su comportamiento bajo carga extrema.
- La semántica exacta de timers/controllers debe validarse contra JMeter (suite de
  compatibilidad), no se asume equivalente.

## Alternativas consideradas

- **Thread-per-VU (como JMeter):** descartado; es justamente el límite de eficiencia
  que buscamos superar (objetivo ≥2x VUs/core).
- **Agregar métricas dentro de cada VU:** descartado; contamina el hot path y sesga
  la latencia medida.
- **Reloj de pared (`SystemTime`) para latencias:** descartado; sensible a saltos de
  hora/NTP y produce mediciones no confiables.
- **HTTP con OpenSSL nativo:** descartado a favor de rustls por portabilidad y para
  no arrastrar dependencias de sistema.
- **Cancelación por abort de tareas:** descartado; deja métricas a medias y estado
  inconsistente. Se prefiere cancelación cooperativa.
