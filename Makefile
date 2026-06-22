# Makefile de perfkit — atajos para desarrollo, calidad, empaquetado y release.
#
# Uso:  make <objetivo>     (p. ej. `make test`, `make lint`, `make docker`)
#       make help           para ver el listado de objetivos.

# Binario y crate de la CLI.
BIN          := perfkit
CLI_CRATE    := cli
DOCKER_IMAGE := perfkit:dev
SCHEMA_DIR   := schemas

# Objetivos que no producen un archivo con su nombre.
.PHONY: help build test lint fmt clippy run docker bench sbom schemas ui clean

# Objetivo por defecto: ayuda.
.DEFAULT_GOAL := help

help: ## Muestra esta ayuda
	@echo "perfkit — objetivos disponibles:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "} {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

build: ## Compila el binario perfkit en modo release
	cargo build --release -p $(CLI_CRATE)

test: ## Ejecuta los tests del workspace
	cargo test --workspace

lint: fmt clippy ## Comprueba formato y ejecuta clippy (warnings = error)

fmt: ## Comprueba el formato del código (no lo modifica)
	cargo fmt --all -- --check

clippy: ## Ejecuta clippy tratando los warnings como errores
	cargo clippy --workspace --all-targets -- -D warnings

run: ## Ejecuta la CLI; pasa argumentos con ARGS="..."
	cargo run -p $(CLI_CRATE) -- $(ARGS)

docker: ## Construye la imagen de contenedor (perfkit:dev)
	docker build -t $(DOCKER_IMAGE) .

bench: ## Benchmark perfkit vs JMeter (tools/benchmark.sh)
	bash tools/benchmark.sh

schemas: ## Regenera los JSON Schema en schemas/
	cargo run -p $(CLI_CRATE) -- schema --out $(SCHEMA_DIR)

# SBOM (Software Bill of Materials).
# Si `cargo-cyclonedx` está instalado, genera sbom.json en formato CycloneDX
# (estándar de la industria, consumible por escáneres de vulnerabilidades).
# Si no, hace fallback a `cargo tree` -> sbom.txt y avisa cómo instalarlo.
sbom: ## Genera el SBOM (CycloneDX si está disponible, si no cargo tree)
	@if command -v cargo-cyclonedx >/dev/null 2>&1; then \
		echo "==> generando SBOM CycloneDX en sbom.json"; \
		cargo cyclonedx --format json -o sbom.json; \
	else \
		echo "==> cargo-cyclonedx no instalado; fallback a 'cargo tree' -> sbom.txt"; \
		echo "    (instala con: cargo install cargo-cyclonedx)"; \
		cargo tree --workspace > sbom.txt; \
		echo "==> SBOM (árbol de dependencias) escrito en sbom.txt"; \
	fi

ui: ## Arranca la app de UI (Tauri) en modo desarrollo
	cd ui && pnpm install && pnpm tauri dev

clean: ## Limpia artefactos de compilación
	cargo clean
