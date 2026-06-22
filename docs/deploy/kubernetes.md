# Despliegue distribuido de perfkit (Docker Compose · Kubernetes · Helm)

Guía para ejecutar perfkit en **modo distribuido** (Fase 6): un **coordinator**
(plano de control, API HTTP/JSON en `:7700`) reparte la carga entre N **workers**
(plano de datos, API HTTP en `:7711`), cada uno ejecuta su cuota de usuarios
virtuales (VUs) y transmite métricas; el coordinator emite un **reporte
consolidado**.

> **Estado.** Los manifiestos describen el contrato de despliegue objetivo. Las
> imágenes `perfkit/coordinator:0.1` y `perfkit/worker:0.1`, el binario del
> operator y el plano gRPC/mTLS son **objetivo de producción** (ver la sección
> [Qué queda diferido](#qué-queda-diferido-para-producción)). Hoy `perfkit` corre
> en local con `cargo run` (ver `README.md`).

## Arquitectura

```
                 API control :7700
   coordinator ───────────────────────────────┐
       │  reparte VUs (barrier start)          │ agrega métricas → reporte consolidado
       ▼                                        ▲
   worker-0   worker-1   worker-2  ...  worker-N
       │ ejecutan su cuota de VUs y stream de métricas (:7711)
       ▼
     target (SUT)
```

- **Reparto, no duplicación.** `vus` es la carga **total**: 300 VUs con 3 workers
  son 100 VUs por worker, no 300 en cada uno.
- **Service headless** para los workers: el coordinator ve una IP por Pod y asigna
  cuotas 1:1, en vez de una VIP que balancea por round-robin.

Ficheros relevantes:

| Fichero | Rol |
|---|---|
| `deploy/docker-compose.distributed.yml` | Demo local autocontenida (coordinator + workers + target). |
| `deploy/examples/scenario.yaml` | Plan de demo (IR canónico) usado por el compose. |
| `deploy/kubernetes/namespace.yaml` | Namespace `perfkit`. |
| `deploy/kubernetes/coordinator.yaml` | Deployment (1) + Service ClusterIP `:7700`. |
| `deploy/kubernetes/worker.yaml` | Deployment (3) + Service headless `:7711`. |
| `deploy/kubernetes/loadtest-crd.yaml` | CRD `loadtests.perfkit.dev` (`v1alpha1`). |
| `deploy/kubernetes/loadtest-sample.yaml` | CR `LoadTest` de ejemplo + ConfigMap. |
| `deploy/kubernetes/rbac.yaml` | ServiceAccount + ClusterRole(+Binding) del operator. |
| `deploy/helm/perfkit/` | Chart Helm parametrizable. |
| `deploy/operator/README.md` | Diseño del operator (reconcile loop). |

## 1. Demo local con Docker Compose

Levanta coordinator, 3 workers y un SUT de juguete (`python -m http.server`) en una
red de Docker, sin dependencias externas.

```bash
# Provee un plan: usa el de ejemplo o genera el tuyo.
#   ./target/debug/perfkit init -o deploy/examples/scenario.yaml   # opcional

docker compose -f deploy/docker-compose.distributed.yml up --scale worker=3
```

- El coordinator descubre a los workers por DNS del servicio `worker`.
- API de control: <http://localhost:7700> · salud: `/healthz` · reporte: `/report`.
- Los workers exponen `:7711` solo dentro de la red (no se publican al host).

Parar y limpiar:

```bash
docker compose -f deploy/docker-compose.distributed.yml down -v
```

> Si aún no tienes las imágenes `perfkit/*:0.1`, descomenta los bloques `build:`
> del compose para construirlas desde un Dockerfile local (cuando exista), o ajusta
> el tag a tu registry.

## 2. Kubernetes (manifiestos planos)

```bash
# Namespace.
kubectl apply -f deploy/kubernetes/namespace.yaml

# Plano de control y de datos.
kubectl apply -f deploy/kubernetes/coordinator.yaml
kubectl apply -f deploy/kubernetes/worker.yaml

# Comprobar.
kubectl -n perfkit get pods,svc
kubectl -n perfkit logs deploy/perfkit-coordinator -f
```

El coordinator resuelve a los workers vía el Service headless
`perfkit-worker.perfkit.svc.cluster.local:7711`.

Para alimentar el plan a los manifiestos planos, crea el ConfigMap que monta el
coordinator (clave esperada `scenario.yaml`, opcionalmente `thresholds.yaml`):

```bash
kubectl -n perfkit create configmap perfkit-scenario \
  --from-file=scenario.yaml=deploy/examples/scenario.yaml \
  --from-file=thresholds.yaml=examples/yaml/thresholds.yaml
```

### Vía declarativa con el CRD `LoadTest`

En lugar de gestionar Deployments a mano, declara la prueba como un recurso y deja
que el **operator** la materialice (coordinator Job + Pods worker) y rellene el
estado.

```bash
# 1. Instala el CRD y el RBAC del operator.
kubectl apply -f deploy/kubernetes/loadtest-crd.yaml
kubectl apply -f deploy/kubernetes/rbac.yaml
# 2. (Producción) despliega el binario del operator con la ServiceAccount
#    perfkit-operator. Hoy es diseño: ver deploy/operator/README.md.

# 3. Declara una prueba.
kubectl apply -f deploy/kubernetes/loadtest-sample.yaml

# 4. Observa el estado (subrecurso status: phase / throughput / errorRate).
kubectl -n perfkit get loadtests
kubectl -n perfkit describe loadtest demo-distribuida
```

Esquema de `LoadTest` (`perfkit.dev/v1alpha1`), campos de `.spec`:

| Campo | Tipo | Descripción |
|---|---|---|
| `scenario.inline` / `scenario.configMapRef` | string / ref | Plan (IR canónico): embebido o por ConfigMap (uno de los dos). |
| `vus` | int | VUs **totales** a repartir entre workers. |
| `durationSecs` | int | Duración en segundos. |
| `workers` | int | Número de workers. |
| `thresholds` | object | `maxErrorRate`, `maxP95Ms`, `maxP99Ms`, `minThroughputPerSec`. |

`.status` (lo escribe el operator): `phase`
(`Pending`/`Provisioning`/`Running`/`Succeeded`/`Failed`), `throughput`,
`errorRate`, `readyWorkers`, `startTime`, `completionTime`, `message`, `conditions`.

> **Importante.** El CRD y el RBAC se pueden aplicar ya, pero hasta que exista el
> binario del operator nadie reconcilia los `LoadTest`: el objeto se crea y queda
> en `Pending`. El reconcile loop está especificado en `deploy/operator/README.md`.

## 3. Helm

Mismo despliegue, parametrizado por `values.yaml` (imágenes, réplicas, recursos,
`target.enabled`, instalación del CRD).

```bash
helm install perfkit deploy/helm/perfkit \
  --namespace perfkit --create-namespace

# Ajustes habituales.
helm upgrade perfkit deploy/helm/perfkit -n perfkit \
  --set worker.replicas=5 \
  --set coordinator.vus=1000 \
  --set coordinator.durationSecs=120 \
  --set target.enabled=false      # desactiva el SUT de juguete en entornos reales

# Renderizar sin aplicar (para revisar el YAML generado).
helm template perfkit deploy/helm/perfkit -n perfkit | less

# Desinstalar (el CRD se conserva por la anotación helm.sh/resource-policy: keep).
helm uninstall perfkit -n perfkit
```

Valores principales (`deploy/helm/perfkit/values.yaml`):

| Clave | Por defecto | Descripción |
|---|---|---|
| `coordinator.replicas` | `1` | Réplicas del coordinator. |
| `coordinator.vus` | `300` | Carga total a repartir. |
| `coordinator.durationSecs` | `60` | Duración de la prueba. |
| `coordinator.barrierStart` | `true` | Esperar a todos los workers antes de arrancar. |
| `worker.replicas` | `3` | Número de workers. |
| `coordinator.resources` / `worker.resources` | requests/limits | Recursos. |
| `scenario.create` / `scenario.inline` / `scenario.thresholds` | — | Plan + umbrales como ConfigMap. |
| `target.enabled` | `true` | Despliega el SUT de juguete. |
| `crd.install` | `true` | Instala el CRD `LoadTest`. |

## Verificación rápida

```bash
kubectl -n perfkit get pods                      # coordinator 1/1, workers N/N, target 1/1
kubectl -n perfkit get endpoints perfkit-worker  # una IP por Pod worker (headless)
kubectl -n perfkit logs deploy/perfkit-coordinator | grep -i "report\|workers"
```

## Qué queda diferido para producción

Estos elementos están **diseñados** (ver Fase 6 del plan y
`deploy/operator/README.md`) pero **no incluidos** como binario funcional aquí:

- **Binario real del operator.** Hoy hay RBAC + CRD + diseño del reconcile loop,
  no un controlador que reconcilie. Sin él, los `LoadTest` quedan en `Pending`.
- **Imágenes `perfkit/coordinator` y `perfkit/worker`.** Son conceptuales; falta
  publicar los Dockerfiles/imágenes (el `cli` actual no expone subcomandos
  `coordinator`/`worker`).
- **mTLS y gestión de certificados.** Comunicación coordinator↔worker autenticada
  y cifrada (cert-manager / CA propio + Secrets + rotación). Aquí el contrato es
  HTTP plano dentro del clúster.
- **Plano gRPC con streaming de métricas.** El objetivo de Fase 6 es gRPC; estos
  manifiestos usan HTTP `:7700`/`:7711` como contrato simplificado.
- **Autoscaling** del pool de workers (HPA o lógica del operator).
- **Barrier start, health reporting y agregación** robustos viven en el binario,
  no en el YAML: aquí se exponen como variables de entorno y probes
  (`PERFKIT_BARRIER_START`, `PERFKIT_EXPECTED_WORKERS`, `/healthz`, `/readyz`).

## Referencias

- Plan: `CLAUDE_OPUS_IMPLEMENTATION_PLAN.md` → *Fase 6: Ejecución Distribuida y Kubernetes*.
- Diseño del operator: `deploy/operator/README.md`.
- Umbrales del quality gate: `examples/yaml/thresholds.yaml`.
