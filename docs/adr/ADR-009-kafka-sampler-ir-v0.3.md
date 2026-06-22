# ADR-009: Kafka producer sampler — IR v0.3.0

## Estado
Aceptado (Fase 7).

## Contexto
El plan compromete soporte Kafka en el primer año (§2.9), sin contaminar el MVP HTTP.
Se necesita expresar carga Kafka en el IR, ejecutarla y reportarla distinguiéndola de
HTTP. Es un cambio de contrato (IR + métricas) → ADR + bump de versión.

## Decisión
- **`IR_VERSION` → `0.3.0`**. Nueva variante `Step::Kafka(KafkaRequest{ name, brokers,
  topic, key?, payload, partition?, headers })` con templating `${var}`.
- **`metrics::SampleKind::Kafka`**: el reporte distingue HTTP vs Kafka (etiqueta `kafka`
  en el HTML); el total agrega muestras de protocolo (HTTP+Kafka), las transacciones
  aparte.
- **Crate `kafka-adapter`** (cliente **rskafka**, puro en Rust, sin librdkafka):
  `prepare()` interpola el registro y `produce()` publica (latencia en µs). Credenciales
  SASL/SSL se manejarán como **secretos** (no se versionan).
- **Engine**: ejecuta `Step::Kafka` vía el adapter; si no hay broker accesible registra
  una muestra fallida con error claro (no falla en silencio).
- **Importer**: los samplers Kafka de plugins de terceros (tags que contienen "kafka") se
  mapean *best-effort* a `Step::Kafka` con estado **assisted** (revisar brokers/topic).

## Consecuencias
- La **ejecución contra un broker real** queda fuera del alcance verificado aquí (no hay
  broker en el entorno); los tests unitarios cubren templating y armado del registro, y
  el engine produce errores claros sin broker. Con un broker provisto por el usuario,
  funciona.
- Consumer validation, Schema Registry y métricas por partición quedan diferidos (§6.13).

## Alternativas consideradas
- **librdkafka (rdkafka crate)**: descartado por la dependencia de la librería C de
  sistema; rskafka es puro Rust y simplifica el build/Docker.
