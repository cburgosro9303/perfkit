# CI/CD de perfkit

Dos workflows de GitHub Actions cubren integración continua y publicación:

- **`.github/workflows/ci.yml`** — en cada `push` y `pull_request`.
- **`.github/workflows/release.yml`** — al empujar un tag `v*`.

## Integración continua (`ci.yml`)

### Trabajo `test`

Garantiza la calidad básica del código en cada cambio:

1. `actions/checkout@v4`.
2. Instala la toolchain **Rust 1.95.0** con `dtolnay/rust-toolchain` y los
   componentes `rustfmt` y `clippy` (la versión coincide con
   `rust-toolchain.toml`).
3. `Swatinem/rust-cache@v2` para cachear `~/.cargo` y `target/`.
4. `cargo fmt --all -- --check` — falla si el formato no es canónico.
5. `cargo clippy --workspace --all-targets -- -D warnings` — **cualquier warning
   de clippy hace fallar el build**.
6. `cargo test --workspace` — corre todos los tests del workspace.

### Trabajo `quality-gate`

Ejercita el **flujo real de perfkit** de extremo a extremo, no solo unit tests.
Depende de `test` (`needs: test`) y reproduce el ciclo del §16 del plan:
*importar → validar → ejecutar → gate en CI*.

1. Compila el binario release: `cargo build --release -p cli`.
2. Añade `target/release` al `PATH`.
3. **Import**: `perfkit import jmx examples/jmx/http-get-simple.jmx -o /tmp/s.yaml`.
4. **Validate**: `perfkit validate /tmp/s.yaml` (sale con código `1` si hay
   errores → el job falla).
5. Levanta un **servidor HTTP local mínimo** (`tools/test-server.py`, incluido en
   el repo; responde `200` a cualquier ruta) en el puerto `8787` y espera a que
   acepte conexiones.
6. **Run**: ejecuta una carga corta contra ese servidor
   (`--vus 5 --duration 5`) y escribe `reports/ci/summary.json`.
7. **Gate**: `perfkit gate reports/ci/summary.json --thresholds examples/yaml/thresholds.yaml`.
8. Sube `summary.json` como artefacto (con `if: always()`).
9. Detiene el servidor (`if: always()`).

### El patrón quality-gate-en-CI (códigos de salida)

El comando `perfkit gate` es la pieza clave: compara un `summary.json` (resultado
machine-readable de un run) contra umbrales versionados y comunica el veredicto
**por código de salida**, que es lo que GitHub Actions usa para marcar el job
como verde o rojo.

| Comando             | Éxito       | Fallo                          | Código de fallo |
| ------------------- | ----------- | ------------------------------ | --------------- |
| `perfkit validate`  | exit `0`    | escenario inválido             | `1`             |
| `perfkit gate`      | exit `0`    | umbrales incumplidos           | `1`             |
| cualquier comando   | —           | error de E/S, parseo, etc.     | `2`             |

Como un código de salida distinto de `0` aborta el step (y por tanto el job), no
hace falta lógica extra en el YAML: si el run incumple los umbrales, el pipeline
**falla automáticamente**.

Los umbrales viven en `examples/yaml/thresholds.yaml` y son explícitos:

```yaml
max_error_rate: 0.05          # 5% de errores como máximo
max_p95_ms: 1500              # p95 <= 1500 ms
max_p99_ms: 3000              # p99 <= 3000 ms
min_throughput_per_sec: 1     # al menos 1 req/s
```

Cada umbral es opcional: solo se evalúan los presentes en el archivo. Para tu
propio pipeline, copia este archivo y ajusta los valores a tu SLA.

> Reproducir localmente lo que hace el job:
>
> ```bash
> make build
> python3 tools/test-server.py 8787 &
> ./target/release/perfkit import jmx examples/jmx/http-get-simple.jmx -o /tmp/s.yaml
> ./target/release/perfkit validate /tmp/s.yaml
> ./target/release/perfkit run /tmp/s.yaml --base-url http://127.0.0.1:8787 \
>     --vus 5 --duration 5 --out reports/ci
> ./target/release/perfkit gate reports/ci/summary.json \
>     --thresholds examples/yaml/thresholds.yaml
> echo "exit code del gate: $?"
> ```

## Release (`release.yml`)

Se dispara al empujar un tag que empiece por `v`:

```bash
git tag v0.1.0
git push origin v0.1.0
```

### Binarios por plataforma

El trabajo `build` usa una **matriz** sobre dos pares runner/target:

| Runner          | Target                         |
| --------------- | ------------------------------ |
| `ubuntu-latest` | `x86_64-unknown-linux-gnu`     |
| `macos-latest`  | `aarch64-apple-darwin`         |

Para cada combinación:

1. Instala Rust 1.95 con el `target` correspondiente.
2. `cargo build --release -p cli --target <target>`.
3. Empaqueta el binario como `perfkit-<target>.tar.gz` y genera su `.sha256`.
4. Adjunta el `.tar.gz` y el checksum al release de GitHub con
   `softprops/action-gh-release@v2`.

### Release multi-arquitectura

El trabajo `docker` (depende de `build`) publica la imagen de contenedor en
**GHCR** para `linux/amd64` y `linux/arm64`:

1. `docker/setup-qemu-action` + `docker/setup-buildx-action` habilitan la
   construcción multi-arch.
2. `docker/login-action` inicia sesión en `ghcr.io` con el `GITHUB_TOKEN`
   (requiere `permissions: packages: write`, ya declarado en el workflow).
3. `docker/metadata-action` deriva las etiquetas a partir del tag semver
   (`{{version}}`, `{{major}}.{{minor}}` y `latest`).
4. `docker/build-push-action` construye con el `Dockerfile` y empuja las dos
   arquitecturas a `ghcr.io/<owner>/<repo>`.

El job está **guardado** (`if: github.event_name == 'push'`) para ejecutarse solo
en el flujo de tag del repositorio original y no en contextos sin permisos de
packages (p. ej. forks). Así el paso queda documentado y activo sin riesgo de
fallos en bifurcaciones.

## SBOM

El objetivo `make sbom` genera un *Software Bill of Materials*:

- Si `cargo-cyclonedx` está instalado, produce `sbom.json` en formato
  **CycloneDX** (`cargo cyclonedx --format json -o sbom.json`), consumible por
  escáneres de vulnerabilidades (Grype, Trivy, Dependency-Track, etc.).
- Si no lo está, hace **fallback** a `cargo tree --workspace > sbom.txt` y avisa
  cómo instalarlo (`cargo install cargo-cyclonedx`).

Instalación recomendada para CI:

```bash
cargo install cargo-cyclonedx
make sbom        # genera sbom.json
```
