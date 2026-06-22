#!/usr/bin/env bash
# Benchmark reproducible: perfkit vs Apache JMeter sobre el mismo plan HTTP y target.
# Uso: bash tools/benchmark.sh [threads] [duration_secs]
set -uo pipefail
ROOT="/Users/cburgosro/Projects/jmeter"
cd "$ROOT"
THREADS="${1:-50}"
DURATION="${2:-20}"
PORT=8899
JDK21="/Users/cburgosro/Library/Java/JavaVirtualMachines/ms-21.0.9/Contents/Home"
PK="$ROOT/target/release/perfkit"

echo "==> compilando perfkit (release)"
cargo build --release -p cli 2>&1 | tail -1

echo "==> levantando target (node) en :$PORT"
node tools/bench-target.js "$PORT" >/tmp/target.log 2>&1 &
TARGET=$!
trap 'kill $TARGET 2>/dev/null' EXIT
curl -s --retry 60 --retry-connrefused --retry-delay 1 -o /dev/null "http://127.0.0.1:$PORT/" \
  && echo "    target listo" || { echo "target no respondió"; exit 1; }

echo "==> importando el plan a IR de perfkit"
"$PK" import jmx examples/jmx/bench-http.jmx -o /tmp/bench.yaml >/dev/null

echo "==> warmup (3s)"
"$PK" run /tmp/bench.yaml --base-url "http://127.0.0.1:$PORT" --vus "$THREADS" --duration 3 --out /tmp/warm >/dev/null 2>&1

echo "==> perfkit: $THREADS VUs durante ${DURATION}s"
/usr/bin/time -l "$PK" run /tmp/bench.yaml --base-url "http://127.0.0.1:$PORT" \
  --vus "$THREADS" --duration "$DURATION" --out /tmp/bench-pk >/tmp/pk.out 2>/tmp/pk-time.txt
echo "    perfkit listo"

echo "==> JMeter: $THREADS threads durante ${DURATION}s (JDK 21, no-GUI)"
rm -f /tmp/jm.jtl /tmp/jm.log
JAVA_HOME="$JDK21" /usr/bin/time -l jmeter -n -t examples/jmx/bench-http.jmx \
  -Jthreads="$THREADS" -Jduration="$DURATION" -l /tmp/jm.jtl -j /tmp/jm.log \
  >/tmp/jm.out 2>/tmp/jm-time.txt
echo "    JMeter listo"

echo
python3 tools/bench-report.py /tmp/bench-pk/summary.json /tmp/jm.jtl /tmp/pk-time.txt /tmp/jm-time.txt "$DURATION"
