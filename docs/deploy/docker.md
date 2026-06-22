# Despliegue con Docker

perfkit se distribuye como una imagen de contenedor autónoma: contiene solo el
binario `perfkit` y los certificados CA, sin Rust, sin `cargo` y sin código
fuente. Esto la hace pequeña, reproducible y segura para correr en CI o en
cualquier host con Docker.

## Construir la imagen

Desde la raíz del repositorio:

```bash
docker build -t perfkit:dev .
# o, con el atajo del Makefile:
make docker
```

El `Dockerfile` usa una construcción en **dos etapas** (multi-stage):

1. **Etapa `builder`** (`rust:1.95-slim`): copia el workspace completo y ejecuta
   `cargo build --release -p cli`, que produce el binario `perfkit`. La app de UI
   (`ui/`, basada en Tauri) está **excluida** del workspace del core
   (`exclude = ["ui/src-tauri"]` en `Cargo.toml`), de modo que esta compilación
   no requiere las dependencias de sistema de Tauri.
2. **Etapa `runtime`** (`debian:stable-slim`): instala `ca-certificates`
   (perfkit hace peticiones HTTPS con reqwest + rustls), crea un usuario sin
   privilegios `perfkit` y copia únicamente el binario a `/usr/local/bin/perfkit`.

El `ENTRYPOINT` es `perfkit`, así que los argumentos de `docker run` se le pasan
directamente.

## Ejecutar

La imagen es **autónoma**: no necesita dependencias externas.

```bash
# Ayuda (verifica que la imagen funciona):
docker run --rm perfkit:dev --help

# Crear un escenario de ejemplo en el directorio actual:
docker run --rm -v "$PWD:/work" -w /work perfkit:dev init demo -o demo.yaml

# Importar un JMX montando el repositorio:
docker run --rm -v "$PWD:/work" -w /work perfkit:dev \
  import jmx examples/jmx/http-get-simple.jmx -o /work/s.yaml

# Ejecutar una carga (montando un volumen para los reportes):
docker run --rm -v "$PWD:/work" -w /work perfkit:dev \
  run /work/s.yaml --vus 10 --duration 30 --out /work/reports/run-docker
```

> Nota: el contenedor corre como usuario **no-root** (`perfkit`). Si montas un
> volumen para escribir reportes, asegúrate de que el directorio del host tenga
> permisos de escritura adecuados.

## Imagen `.dockerignore`

El archivo `.dockerignore` evita copiar al contexto de build artefactos pesados o
irrelevantes: `target/`, `ui/node_modules/`, `ui/dist/`, `.git/`, `reports/` y el
scratch de Playwright (`.playwright-mcp/`). Esto acelera el build y mantiene la
imagen limpia.

## Publicación multi-arquitectura (GHCR)

El workflow de release (`.github/workflows/release.yml`) construye y publica la
imagen en GitHub Container Registry (`ghcr.io/<owner>/<repo>`) para
`linux/amd64` y `linux/arm64` usando Buildx + QEMU. Ver
[ci-cd.md](./ci-cd.md#release-multi-arquitectura) para el detalle.

Una vez publicada:

```bash
docker pull ghcr.io/<owner>/<repo>:0.1.0
docker run --rm ghcr.io/<owner>/<repo>:0.1.0 --help
```
