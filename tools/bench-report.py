#!/usr/bin/env python3
"""Compara los resultados de perfkit vs JMeter y emite una tabla + markdown."""
import csv
import json
import math
import os
import sys


def maxrss_mb(path):
    try:
        for line in open(path, errors="ignore"):
            if "maximum resident set size" in line:
                return int(line.strip().split()[0]) / 1e6  # macOS reporta bytes
    except FileNotFoundError:
        pass
    return float("nan")


def pct(sorted_vals, p):
    if not sorted_vals:
        return 0.0
    i = max(0, min(len(sorted_vals) - 1, int(math.ceil(p / 100 * len(sorted_vals)) - 1)))
    return float(sorted_vals[i])


def main():
    pk_summary, jtl, pk_time, jm_time, dur = sys.argv[1:6]
    dur = float(dur)
    out = sys.argv[6] if len(sys.argv) > 6 else "docs/benchmarks/perfkit-vs-jmeter.md"
    target = sys.argv[7] if len(sys.argv) > 7 else "mismo target local (`examples/jmx/bench-http.jmx`)"
    reproduce = sys.argv[8] if len(sys.argv) > 8 else "bash tools/benchmark.sh [threads] [duration]"

    pk = json.load(open(pk_summary))["overall"]
    pk_tp, pk_n, pk_err = pk["throughput_per_sec"], pk["count"], pk["errors"]
    pk_p50, pk_p95, pk_p99 = pk["p50_ms"], pk["p95_ms"], pk["p99_ms"]
    pk_rss = maxrss_mb(pk_time)

    if not os.path.isfile(jtl) or os.path.getsize(jtl) == 0:
        print(
            f"⚠ No hay resultados de JMeter en '{jtl}' (JMeter falló o no generó el .jtl).\n"
            f"  perfkit → throughput={pk_tp:,.0f} req/s · requests={pk_n:,} · errores={pk_err} · "
            f"p50={pk_p50:.1f}ms · p95={pk_p95:.1f}ms · p99={pk_p99:.1f}ms · RSS={pk_rss:.0f}MB\n"
            "  Comparación omitida. Revisa el log de JMeter (-j) para el motivo."
        )
        return

    elapsed, n, ok = [], 0, 0
    with open(jtl) as f:
        for row in csv.DictReader(f):
            try:
                elapsed.append(int(row["elapsed"]))
            except (KeyError, ValueError):
                continue
            n += 1
            if row.get("success", "true") == "true":
                ok += 1
    elapsed.sort()
    jm_tp = n / dur if dur else 0.0
    jm_p50, jm_p95, jm_p99 = pct(elapsed, 50), pct(elapsed, 95), pct(elapsed, 99)
    jm_rss = maxrss_mb(jm_time)

    def ratio(a, b):
        return f"{a / b:.2f}x" if b else "—"

    rows = [
        ("Throughput (req/s)", f"{pk_tp:,.0f}", f"{jm_tp:,.0f}", ratio(pk_tp, jm_tp)),
        ("Requests totales", f"{pk_n:,}", f"{n:,}", ratio(pk_n, n)),
        ("Errores", f"{pk_err:,}", f"{n - ok:,}", ""),
        ("Latencia p50 (ms)", f"{pk_p50:.1f}", f"{jm_p50:.1f}", ""),
        ("Latencia p95 (ms)", f"{pk_p95:.1f}", f"{jm_p95:.1f}", ""),
        ("Latencia p99 (ms)", f"{pk_p99:.1f}", f"{jm_p99:.1f}", ""),
        ("Memoria pico RSS (MB)", f"{pk_rss:.0f}", f"{jm_rss:.0f}", ratio(jm_rss, pk_rss) + " menos"),
    ]

    w = max(len(r[0]) for r in rows)
    line = "-" * (w + 38)
    print(line)
    print(f"{'Métrica':<{w}}  {'perfkit':>12}  {'JMeter':>12}  {'ventaja':>10}")
    print(line)
    for name, a, b, r in rows:
        print(f"{name:<{w}}  {a:>12}  {b:>12}  {r:>10}")
    print(line)
    tp_ratio = pk_tp / jm_tp if jm_tp else float("nan")
    mem_ratio = jm_rss / pk_rss if pk_rss else float("nan")
    print(f"\nperfkit hizo {tp_ratio:.2f}x el throughput de JMeter y usó {mem_ratio:.1f}x menos memoria.")

    # markdown
    md = ["# Benchmark perfkit vs Apache JMeter", "",
          f"Escenario HTTP de referencia (GET), {target}, "
          f"{int(dur)}s, igual concurrencia. perfkit en `--release`; JMeter `-n` (no-GUI) sobre JDK 21.", "",
          "| Métrica | perfkit | JMeter | Ventaja |", "|---|---:|---:|---:|"]
    for name, a, b, r in rows:
        md.append(f"| {name} | {a} | {b} | {r} |")
    md += ["", f"**Resumen:** perfkit ≈ {tp_ratio:.2f}x throughput y {mem_ratio:.1f}x menos memoria pico que JMeter "
           "en este escenario. La latencia debe ser comparable (ambos saturan el mismo target); la ventaja real "
           "del motor Rust/Tokio está en memoria por VU y overhead de reporte.", "",
           f"> Reproducir: `{reproduce}`"]
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
    open(out, "w").write("\n".join(md) + "\n")
    print(f"\n✔ reporte markdown: {out}")


if __name__ == "__main__":
    main()
