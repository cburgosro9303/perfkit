//! `kafka-adapter` — productor Kafka para el sampler Kafka de perfkit (Fase 7).
//!
//! Hace templating de `${var}` en topic/key/payload/headers y publica con `rskafka`
//! (cliente puro en Rust, sin librdkafka). La **ejecución contra un broker real** se
//! valida fuera de los tests unitarios (estos cubren el templating y el armado del
//! registro). Credenciales SASL/SSL: ver §6.13 (manejarse como secretos).

use scenario_ir::model::KafkaRequest;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum KafkaError {
    #[error("conexión a brokers fallida: {0}")]
    Connect(String),
    #[error("error al publicar en Kafka: {0}")]
    Produce(String),
}

/// Registro listo para publicar (ya interpolado).
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRecord {
    pub brokers: Vec<String>,
    pub topic: String,
    pub partition: i32,
    pub key: Option<String>,
    pub value: String,
    pub headers: BTreeMap<String, String>,
}

/// Interpola `${var}` con las variables del VU (deja intactas las desconocidas).
pub fn interpolate(s: &str, vars: &HashMap<String, String>) -> String {
    if !s.contains("${") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let name = &after[..end];
            match vars.get(name) {
                Some(v) => out.push_str(v),
                None => {
                    out.push_str("${");
                    out.push_str(name);
                    out.push('}');
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Construye el registro a publicar a partir del IR + variables.
pub fn prepare(req: &KafkaRequest, vars: &HashMap<String, String>) -> PreparedRecord {
    PreparedRecord {
        brokers: req.brokers.iter().map(|b| interpolate(b, vars)).collect(),
        topic: interpolate(&req.topic, vars),
        partition: req.partition.unwrap_or(0),
        key: req.key.as_ref().map(|k| interpolate(k, vars)),
        value: interpolate(&req.payload, vars),
        headers: req
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), interpolate(v, vars)))
            .collect(),
    }
}

/// Publica un registro en Kafka y devuelve la latencia en microsegundos.
///
/// Requiere un broker accesible; sin broker devuelve `Err(Connect(..))`, que el
/// engine reporta como muestra fallida (errores claros).
pub async fn produce(rec: &PreparedRecord) -> Result<u64, KafkaError> {
    use rskafka::client::ClientBuilder;
    use rskafka::client::partition::{Compression, UnknownTopicHandling};
    use rskafka::record::Record;

    let start = Instant::now();
    let client = ClientBuilder::new(rec.brokers.clone())
        .build()
        .await
        .map_err(|e| KafkaError::Connect(e.to_string()))?;
    let partition = client
        .partition_client(
            rec.topic.clone(),
            rec.partition,
            UnknownTopicHandling::Error,
        )
        .await
        .map_err(|e| KafkaError::Connect(e.to_string()))?;

    let record = Record {
        key: rec.key.clone().map(|k| k.into_bytes()),
        value: Some(rec.value.clone().into_bytes()),
        headers: rec
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone().into_bytes()))
            .collect(),
        timestamp: chrono::Utc::now(),
    };
    partition
        .produce(vec![record], Compression::NoCompression)
        .await
        .map_err(|e| KafkaError::Produce(e.to_string()))?;
    Ok(start.elapsed().as_micros() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scenario_ir::model::KafkaRequest;

    #[test]
    fn templating_and_prepare() {
        let mut vars = HashMap::new();
        vars.insert("env".to_string(), "prod".to_string());
        vars.insert("id".to_string(), "42".to_string());
        let req = KafkaRequest {
            name: "publish".into(),
            brokers: vec!["${env}-kafka:9092".into()],
            topic: "orders-${env}".into(),
            key: Some("k-${id}".into()),
            payload: "{\"id\":${id}}".into(),
            partition: Some(2),
            headers: BTreeMap::from([("source".to_string(), "perfkit-${env}".to_string())]),
        };
        let rec = prepare(&req, &vars);
        assert_eq!(rec.brokers, vec!["prod-kafka:9092"]);
        assert_eq!(rec.topic, "orders-prod");
        assert_eq!(rec.key.as_deref(), Some("k-42"));
        assert_eq!(rec.value, "{\"id\":42}");
        assert_eq!(rec.partition, 2);
        assert_eq!(
            rec.headers.get("source").map(String::as_str),
            Some("perfkit-prod")
        );
    }

    #[test]
    fn unknown_var_is_left_intact() {
        let vars = HashMap::new();
        assert_eq!(interpolate("a-${missing}-b", &vars), "a-${missing}-b");
    }
}
