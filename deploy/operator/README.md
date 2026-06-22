# perfkit operator (diseño / esqueleto — Fase 6)

> Estado: **diseño**. Este directorio describe el operator objetivo que reconcilia
> recursos `LoadTest` (`perfkit.dev/v1alpha1`). Todavía **no** existe el binario;
> aquí se documenta el bucle de reconciliación, el modelo de estados y los
> objetivos de producción (barrier start, health, mTLS). Sin builds.

## Qué hace

El operator observa objetos `LoadTest` en el clúster y los materializa como una
ejecución distribuida de perfkit: un **coordinator** (plano de control) y N
**worker** (plano de datos). Cuando la prueba termina, escribe el resultado del
**reporte consolidado** en `.status` del propio `LoadTest`.

Es el "cerebro" que conecta la API declarativa (el CRD) con los workloads
imperativos (Jobs/Pods), de forma análoga a cómo un Deployment gestiona Pods pero
para pruebas de carga efímeras.

```
                 watch / reconcile
   LoadTest CR  ───────────────────▶  perfkit-operator
       ▲                                   │ crea
       │ status (reporte consolidado)      ▼
       └──────────────────────  Job coordinator  ◀── métricas ── Pods worker (N)
                                      │ agrega                       │ ejecutan
                                      └─────────── reporte ──────────┘ su cuota de VUs
```

## Bucle de reconciliación (reconcile loop)

El operator es **nivel-disparado** (level-triggered): ante cualquier cambio
(creación/edición del CR, cambio en Pods/Job hijos, o re-encolado periódico)
recalcula el estado deseado y lo compara con el real. Es idempotente: ejecutar
`reconcile` varias veces sobre el mismo estado no produce efectos adicionales.

Pasos de un `reconcile(req)`:

1. **Cargar el CR.** `Get LoadTest`. Si no existe (fue borrado), no-op.
2. **Finalizer.** Si el CR no tiene el finalizer `perfkit.dev/finalizer`, añadirlo
   y volver a encolar. Si está marcado para borrado (`deletionTimestamp != nil`),
   ejecutar la limpieza (borrar Job/Pods/Service del test), quitar el finalizer y
   terminar.
3. **Validar `.spec`.** Resolver el escenario: `scenario.inline` o
   `scenario.configMapRef` (uno de los dos). Validar `vus >= 1`, `workers >= 1`,
   `durationSecs >= 1`. Ante spec inválida → `phase=Failed` con mensaje y `return`.
4. **Materializar dependencias (idempotente, con `ownerReferences` al CR):**
   - `Service` headless `lt-<name>-worker` para el descubrimiento DNS 1:1.
   - `ConfigMap` con el plan (si viene inline) y los umbrales.
   - `workers` Pods worker (`replicas = .spec.workers`), cada uno apuntando al
     coordinator por `COORDINATOR_URL`.
   - `Job` coordinator con `PERFKIT_VUS = .spec.vus`,
     `PERFKIT_DURATION_SECS = .spec.durationSecs`,
     `PERFKIT_EXPECTED_WORKERS = .spec.workers` y `PERFKIT_BARRIER_START=true`.

   Para cada objeto: **server-side apply** (o create-or-update). Si ya existe y
   coincide, no se toca.
5. **Calcular la fase y actualizar `.status`** (subrecurso `status`):
   - Sin Pods/Job aún listos → `Provisioning`.
   - Coordinator arrancado y workers registrándose → `Running`; reflejar
     `status.readyWorkers` (para visualizar el barrier start).
   - Job coordinator `Complete` → leer el reporte consolidado y poblar
     `status.throughput` y `status.errorRate`; aplicar `.spec.thresholds`:
     - dentro de umbrales → `phase=Succeeded`.
     - fuera de umbrales o Job `Failed` → `phase=Failed` con `status.message`.
   - Fijar `status.startTime` / `status.completionTime`, `observedGeneration` y
     `conditions` estándar (`Progressing`, `Succeeded`).
6. **Re-encolar** mientras la prueba siga `Running` (p. ej. cada 10 s) para
   refrescar el progreso; en estados terminales no re-encolar salvo cambio del CR.

### Máquina de estados de `.status.phase`

```
Pending ─▶ Provisioning ─▶ Running ─▶ Succeeded
                                  └──▶ Failed
```

- **Pending**: CR aceptado, aún sin reconciliar.
- **Provisioning**: creando Service/ConfigMap/Pods/Job.
- **Running**: coordinator activo; workers ejecutando su cuota.
- **Succeeded**: reporte consolidado dentro de los umbrales.
- **Failed**: spec inválida, fallo de worker/coordinator, o umbrales superados.

## Reparto de carga (no duplicación)

Punto crítico de la Fase 6 (DoD: *"la carga se reparte, no se duplica"*). El
coordinator divide `.spec.vus` entre los workers READY antes de arrancar
(p. ej. 300 VUs / 3 workers = 100 VUs por worker, repartiendo el resto). El
operator NO ejecuta carga; solo orquesta y observa. El Service de workers es
**headless** precisamente para que el coordinator vea una IP por worker y asigne
cuotas 1:1, en lugar de hablar con una VIP que reparte por round-robin.

## Objetivos de producción (diferidos)

Estos puntos forman parte del objetivo de Fase 6 pero se implementan en el binario
real, no en este esqueleto:

- **Barrier start.** El coordinator espera a que los `PERFKIT_EXPECTED_WORKERS`
  estén READY (registrados y con su cuota asignada) antes de soltar la carga, para
  que la rampa empiece coordinada. El operator refleja el progreso en
  `status.readyWorkers`. Timeout configurable → `Failed` si no se alcanza el quórum.
- **Health reporting.** Workers exponen `/healthz` y `/readyz`; el coordinator
  hace seguimiento de latido (heartbeat). Un worker caído se reporta con claridad
  (DoD: *"fallos de worker se reportan claramente"*) en `status.message`/eventos, y
  el operator decide reintentar o marcar `Failed` según política.
- **mTLS y secretos.** Comunicación coordinator↔worker autenticada y cifrada con
  mTLS (DoD: *"mTLS y secretos funcionan"*). Objetivo de producción: emisión de
  certificados (cert-manager o un CA propio) montados como Secrets, y rotación.
  Los secretos del SUT (tokens, credenciales) se inyectan vía Secret, nunca en el CR.
- **Protocolo gRPC.** El plano de datos objetivo es gRPC con streaming de métricas
  (este despliegue documenta puertos HTTP :7700/:7711 como contrato simplificado).
- **Autoscaling.** Escalado del pool de workers (HPA o lógica del operator) en
  función de la carga objetivo. Diferido.

## Esqueleto sugerido (no implementado)

Implementación objetivo en Rust con [`kube-rs`](https://kube.rs) (`Controller`,
`watcher`, `finalizer`, `Api::patch_status`). Estructura propuesta:

```
deploy/operator/
  README.md            <- este documento (diseño)
  (futuro) src/
    main.rs            <- arranque del Controller + leader election (Lease)
    reconcile.rs       <- fn reconcile(LoadTest) -> Action (el bucle de arriba)
    resources.rs       <- builders de Job coordinator / Pods worker / Service / ConfigMap
    status.rs          <- mapeo reporte consolidado -> .status + evaluación de thresholds
    crd.rs             <- tipos Rust del CRD (derive CustomResource)
```

Pseudo-firma del reconcile (orientativa):

```rust
async fn reconcile(lt: Arc<LoadTest>, ctx: Arc<Ctx>) -> Result<Action> {
    // 1. finalizer / borrado
    // 2. validar spec + resolver scenario (inline | configMapRef)
    // 3. apply: Service headless, ConfigMap, N Pods worker, Job coordinator
    // 4. calcular phase desde el estado de Job/Pods + reporte consolidado
    // 5. patch .status (throughput, errorRate, phase, conditions)
    // 6. Ok(Action::requeue(Duration::from_secs(10))) mientras Running
}
```

Permisos RBAC necesarios: ver `deploy/kubernetes/rbac.yaml`
(`loadtests` + `loadtests/status`, `jobs`, `pods`, `services`, `configmaps`,
`events`, `leases`).
