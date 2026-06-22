#!/usr/bin/env bash
# Benchmark local reproducible perfkit vs Apache JMeter contra un mock de Mockoon.
# El mismo plan y el mismo target (Mockoon) para ambos → comparación justa.
#
# Uso:  bash tools/benchmark-mockoon.sh [threads] [duration_secs] [port]
# Antes: levanta el mock — en la app Mockoon importa tools/mockoon/perfkit-bench.json
#        y dale Play; o por CLI:  mockoon-cli start --data tools/mockoon/perfkit-bench.json --port 3001
set -uo pipefail
ROOT="/Users/cburgosro/Projects/jmeter"
cd "$ROOT"
THREADS="${1:-100}"
DURATION="${2:-60}"
PORT="${3:-3001}"
HOST="127.0.0.1"
BASE="http://$HOST:$PORT"
ENV_FILE="$ROOT/tools/mockoon/perfkit-bench.json"
# JDK 21 para JMeter (sobreescribible: JDK21=/ruta bash tools/benchmark-mockoon.sh)
JDK21="${JDK21:-/Users/cburgosro/Library/Java/JavaVirtualMachines/ms-21.0.9/Contents/Home}"
PK="$ROOT/target/release/perfkit"

MOCK_PID=""
cleanup() { [ -n "$MOCK_PID" ] && kill "$MOCK_PID" 2>/dev/null; }
trap cleanup EXIT

# 1) Asegurar que el mock está arriba (auto-arranque con mockoon-cli si existe).
if ! curl -s -o /dev/null --max-time 3 "$BASE/"; then
  if command -v mockoon-cli >/dev/null 2>&1; then
    echo "==> arrancando Mockoon (mockoon-cli) en :$PORT"
    mockoon-cli start --data "$ENV_FILE" --port "$PORT" >/tmp/mockoon.log 2>&1 &
    MOCK_PID=$!
    curl -s --retry 40 --retry-connrefused --retry-delay 1 -o /dev/null "$BASE/" \
      || { echo "Mockoon no respondió en $BASE"; exit 1; }
  else
    echo "✘ No hay nada escuchando en $BASE y no se encontró 'mockoon-cli'."
    echo "  Opción A (app):  abre Mockoon, importa $ENV_FILE y dale Play (puerto $PORT)."
    echo "  Opción B (cli):  npm i -g @mockoon/cli && mockoon-cli start --data $ENV_FILE --port $PORT"
    exit 1
  fi
fi
echo "==> mock Mockoon respondiendo en $BASE  ✓"

# 2) Compilar perfkit (release) e importar el plan.
echo "==> compilando perfkit (release)"
cargo build --release -p cli 2>&1 | tail -1
echo "==> importando el plan a IR de perfkit"
"$PK" import jmx examples/jmx/bench-mockoon.jmx -o /tmp/bench-mk.yaml >/dev/null

# 3) Warmup.
echo "==> warmup (3s)"
"$PK" run /tmp/bench-mk.yaml --base-url "$BASE" --vus "$THREADS" --duration 3 --out /tmp/warm-mk >/dev/null 2>&1

# 4) perfkit.
echo "==> perfkit: $THREADS VUs durante ${DURATION}s contra Mockoon"
/usr/bin/time -l "$PK" run /tmp/bench-mk.yaml --base-url "$BASE" \
  --vus "$THREADS" --duration "$DURATION" --out /tmp/bench-mk-pk >/tmp/pk.out 2>/tmp/pk-time.txt
echo "    perfkit listo"

# Resumen solo-perfkit (cuando JMeter no está disponible o falla).
pk_only() {
  python3 - <<'PY'
import json
s = json.load(open('/tmp/bench-mk-pk/summary.json'))["overall"]
print(f"  perfkit → throughput={s['throughput_per_sec']:,.0f} req/s · requests={s['count']:,} · "
      f"errores={s['errors']} · p50={s['p50_ms']:.1f}ms · p95={s['p95_ms']:.1f}ms · p99={s['p99_ms']:.1f}ms")
PY
}

# 5) JMeter (mismo plan, mismo target).
if command -v jmeter >/dev/null 2>&1; then
  echo "==> JMeter: $THREADS threads durante ${DURATION}s contra Mockoon (JDK 21)"
  rm -f /tmp/jm-mk.jtl /tmp/jm-mk.log
  JAVA_HOME="$JDK21" /usr/bin/time -l jmeter -n -t examples/jmx/bench-mockoon.jmx \
    -Jhost="$HOST" -Jport="$PORT" -Jthreads="$THREADS" -Jduration="$DURATION" \
    -l /tmp/jm-mk.jtl -j /tmp/jm-mk.log >/tmp/jm.out 2>/tmp/jm-time.txt
  jm_rc=$?
  if [ "$jm_rc" -ne 0 ] || [ ! -s /tmp/jm-mk.jtl ]; then
    echo "⚠ JMeter no produjo resultados (exit $jm_rc, sin .jtl). Últimas líneas de su log:"
    tail -n 6 /tmp/jm-mk.log /tmp/jm.out 2>/dev/null | sed 's/^/    /'
    echo "→ Comparación omitida; muestro solo perfkit:"
    pk_only
  else
    echo "    JMeter listo"
    echo
    python3 tools/bench-report.py /tmp/bench-mk-pk/summary.json /tmp/jm-mk.jtl /tmp/pk-time.txt /tmp/jm-time.txt "$DURATION" \
      "docs/benchmarks/mockoon-results.md" \
      "mismo plan (\`examples/jmx/bench-mockoon.jmx\`) contra un mock local de Mockoon (puerto $PORT)" \
      "bash tools/benchmark-mockoon.sh [threads] [duration] [port]"
  fi
else
  echo "⚠ 'jmeter' no está en el PATH — solo se midió perfkit:"
  pk_only
fi
