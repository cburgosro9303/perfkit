# syntax=docker/dockerfile:1
#
# Imagen de perfkit en dos etapas (multi-stage).
#
#   Etapa 1 (builder): compila el binario `perfkit` del crate `cli` en modo
#                      release usando la toolchain de Rust 1.95.
#   Etapa 2 (runtime): imagen mínima de Debian que solo contiene el binario y
#                      los certificados CA. Sin Rust, sin cargo, sin código fuente.
#
# La app de UI (Tauri, en ui/) NO forma parte de esta imagen: el core se compila
# con `cargo build --release -p cli`, que no requiere las dependencias de sistema
# de Tauri (ver Cargo.toml [workspace].exclude = ["ui/src-tauri"]).
#
# Construir:   docker build -t perfkit:dev .
# Ejecutar:    docker run --rm perfkit:dev --help
#
# La imagen es autónoma: no necesita dependencias externas en tiempo de ejecución.

# ----------------------------------------------------------------------------
# Etapa 1 — compilación
# ----------------------------------------------------------------------------
FROM rust:1.95-slim AS builder

# Directorio de trabajo del workspace.
WORKDIR /usr/src/perfkit

# Copiamos todo el workspace. El .dockerignore evita arrastrar target/,
# ui/node_modules, ui/dist, .git, reports/, etc.
COPY . .

# Compilamos SOLO el binario de la CLI en modo release.
# `-p cli` produce el binario `perfkit` (ver crates/cli/Cargo.toml [[bin]]).
# No compilamos ui/src-tauri (está excluido del workspace) ni el resto de
# binarios, para una imagen pequeña y un build reproducible.
RUN cargo build --release -p cli

# ----------------------------------------------------------------------------
# Etapa 2 — runtime mínimo
# ----------------------------------------------------------------------------
FROM debian:stable-slim AS runtime

# Certificados CA: perfkit hace peticiones HTTPS (reqwest + rustls), así que
# necesita el almacén de confianza del sistema. Limpiamos las listas de apt
# para no inflar la capa.
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Usuario sin privilegios: el contenedor NO corre como root.
RUN groupadd --system perfkit \
    && useradd --system --gid perfkit --no-create-home --shell /usr/sbin/nologin perfkit

# Copiamos únicamente el binario compilado desde la etapa builder.
COPY --from=builder /usr/src/perfkit/target/release/perfkit /usr/local/bin/perfkit

# A partir de aquí, todo se ejecuta como usuario no-root.
USER perfkit
WORKDIR /home/perfkit

# `perfkit` es el punto de entrada: los argumentos de `docker run` se le pasan
# directamente (p. ej. `docker run perfkit:dev --help`).
ENTRYPOINT ["perfkit"]
CMD ["--help"]
