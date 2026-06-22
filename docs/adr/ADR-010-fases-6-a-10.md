# ADR-010: Arquitectura de Fases 6–10 (distribuido, plugins, IA, enterprise)

## Estado
Aceptado (slices MVP; producción parcialmente diferida).

## Contexto
Tras el MVP (Fases 0–3) y la profundización JMX (Fase 4), las Fases 5–10 añaden
capacidades de plataforma. Varias requieren infraestructura externa real (clúster K8s,
broker Kafka, SaaS de IA, SSO). Se decide entregar **slices MVP funcionales, compilando y
con tests**, marcando lo que necesita infra como diferido.

## Decisiones por fase
- **Fase 5 — Release/CI/Docker:** `Dockerfile` multi-stage (binario `perfkit` standalone),
  GitHub Actions (test/lint/clippy + quality-gate en CI con `perfkit gate`), release
  multi-arch, Makefile, SBOM (cyclonedx/`cargo tree`). Ver `docs/deploy/`.
- **Fase 6 — Distribuido:** crate `cluster`. Control plane **HTTP/JSON** (axum worker +
  coordinator con reqwest). El coordinator **reparte** los VUs (no los duplica), ejecuta
  en paralelo, **consolida** los `RunSummary` (merge ponderado de percentiles — aproximado)
  y **reporta fallos de worker**. Manifiestos K8s (CRD `LoadTest`), Helm y diseño del
  operator en `deploy/`. **gRPC + mTLS + operator binario + autoscaling: diferidos** a
  producción (HTTP/JSON es el MVP demostrable localmente, ver tests `cluster`).
- **Fase 7 — Kafka:** ver `ADR-009`. Broker real diferido.
- **Fase 8 — Plugins WASM:** crate `plugin-host` (**wasmi**, intérprete puro). ABI v1,
  permisos **deny-by-default**, firma **ed25519** obligatoria + verificación de SHA-256,
  límite de **fuel** (anti bucle infinito), registry curado con revocación y version
  pinning. Un plugin sin firmar o manipulado **no carga**. Registry de terceros: diferido.
- **Fase 9 — IA gobernada:** crate `ai-assist`. **SaaS apagado por defecto; ningún dato
  sale por defecto**; `preview_payload` muestra exactamente qué se enviaría (redactado);
  toda sugerencia es revisable (`requires_confirmation`). Proveedor incluido = local
  heurístico (sin red); BYOK/SaaS son traits del caller. Sin llamadas de red en el crate.
- **Fase 10 — Enterprise:** crate `history` (**SQLite** vía rusqlite bundled). Histórico de
  runs, baselines por branch/environment/scenario, tendencias, **detección de regresión**,
  anotaciones, retención, **RBAC deny-by-default** (Viewer/Operator/Admin) y **auditoría**.
  SSO/multi-tenant/almacenamiento centralizado gestionado: diferidos.

## Consecuencias
- Todo lo anterior está cableado en el binario `perfkit` (`cluster`, `history`, `ai`,
  `plugin`) y verificado con tests + demos locales reproducibles.
- Lo diferido (infra real) queda documentado aquí y en `deploy/`/`docs/`, sin stubs que
  simulen funcionalidad inexistente.

## Alternativas consideradas
- **gRPC (tonic) para Fase 6:** descartado por ahora por la dependencia de `protoc`;
  HTTP/JSON cumple la DoD local. gRPC/mTLS quedan como objetivo de producción.
- **wasmtime para Fase 8:** descartado a favor de **wasmi** (más liviano, compila rápido,
  suficiente para plugins puros del MVP).
