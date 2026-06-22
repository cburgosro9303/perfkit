# ADR-006: Seguridad, secretos e IA

- **Estado:** Aceptado
- **Fecha:** 2026-06-19
- **Decisores:** security-governance-lead, platform-architect

## Contexto

Una herramienta de carga maneja endpoints, tokens, payloads y datasets que suelen
contener datos sensibles. El plan (§2.11–§2.13, §6.9, §9) exige seguridad desde el
inicio: no filtrar secretos, no ejecutar plugins inseguros y no enviar datos a
terceros sin consentimiento. En particular la IA debe ser **local / BYOK / SaaS
opt-in con SaaS apagado por defecto**, y los plugins deben empezar curados y
firmados.

## Decisión

### Manejo de secretos por entorno (no versionados)

El crate `security` carga variables/secretos desde **variables de entorno con
prefijo** (`vars_from_env`, p.ej. prefijo `PERFKIT_VAR_`: `PERFKIT_VAR_TOKEN=abc`
→ `{ TOKEN: "abc" }`). Esto permite inyectar credenciales en el escenario **sin
escribirlas en el YAML**, de modo que los secretos **no se versionan** en el IR ni
en control de versiones.

### Redacción de logs y reportes

`security::redact` redacta valores sensibles comunes (tokens, claves, valores de
headers/params marcados) de texto multilínea antes de que llegue a logs o
reportes, para que un HTML compartido o un log de CI no filtre credenciales. El
crate es hoy un **stub** que cubre el alcance MVP (carga por entorno + redacción
básica); la política completa (threat model, firmas, permisos) se desarrolla en
fases posteriores.

### TLS sin dependencias de sistema

El adaptador HTTP usa **rustls** (no OpenSSL del sistema), reduciendo superficie de
ataque por dependencias nativas y haciendo los builds reproducibles (ver ADR-004).

### IA gobernada: local / BYOK / SaaS opt-in

Tres modos, con **SaaS apagado por defecto**:

- **Local:** modelo en la máquina del usuario; los datos no salen.
- **BYOK (bring-your-own-key):** el usuario aporta su propia clave/endpoint.
- **SaaS opt-in:** requiere activación explícita; antes de enviar nada, los datos
  se redactan/anonimizan y el usuario ve exactamente qué se enviaría. Ninguna
  sugerencia modifica el escenario sin confirmación.

Regla: **por defecto no se envían endpoints, tokens, payloads ni CSVs a terceros.**

### Plugins firmados / WASM (futuro)

El registry inicia **curado**, con plugins de primera parte **firmados** y version
pinning. Los plugins de terceros entran después de estabilizar seguridad y se
diseñan con **WASM/WASI y permisos declarativos** (red/filesystem/CPU/memoria/
secretos limitados). Un plugin no firmado no carga. Esto no entra en el MVP salvo
spike arquitectónico.

## Consecuencias

**Positivas**

- Los secretos viven en el entorno, nunca en el YAML versionado.
- La redacción reduce el riesgo de filtración en logs y HTML compartidos.
- "SaaS off por defecto" da una postura de privacidad defendible desde el día uno.
- El modelo WASM/firmas prepara extensibilidad segura sin abrir ejecución arbitraria.

**Negativas / costos**

- El `security` actual es un stub: redacción y manejo de secretos son básicos y
  deben endurecerse (threat model completo pendiente).
- BYOK/SaaS exigen UX clara de consentimiento y previsualización de datos.
- WASM/firmas/permisos son trabajo significativo diferido a fases posteriores.

## Alternativas consideradas

- **Secretos en el YAML del escenario:** descartado; se versionarían y filtrarían.
- **IA SaaS encendida por defecto:** descartado explícitamente por §2.11; viola la
  expectativa de privacidad y el control de datos.
- **Plugins nativos sin sandbox:** descartado; ejecución arbitraria sin permisos es
  un riesgo inaceptable. WASM/WASI con permisos declarativos es el camino.
- **OpenSSL del sistema para TLS:** descartado a favor de rustls (menor superficie,
  builds reproducibles).
