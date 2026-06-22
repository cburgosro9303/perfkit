import React, { useMemo, useState } from "react";
import { api } from "../api";
import { Chart } from "../components/Chart";
import { Bars, GroupedBars } from "../components/Bars";
import type { BarDatum, GroupedDatum } from "../components/Bars";
import { Heatmap } from "../components/Heatmap";
import { Badge, Button, Card, Field, IconCheck, IconChevronDown, IconChevronRight, IconX, Input, Stat, Tabs } from "../components/ui";
import { MethodChip, RequestDetail, statusColor, statusLabel } from "../components/RequestDetail";
import { fmtBytes, fmtDuration, fmtMs, fmtNum, fmtPct, fmtThroughput } from "../lib/format";
import {
  apdex,
  apdexColor,
  bucketLabel,
  HISTOGRAM_BOUNDS_MS,
  percentileCurve,
  statusClassColor,
} from "../lib/metrics";
import type { LabelStats, RunSummary, SampleDetail, TimePoint } from "../types";

interface ReportViewProps {
  summary: RunSummary;
  /** Permite cargar otro summary.json mientras se ve un reporte. */
  onLoadResults?: () => void;
}

type TabId = "resumen" | "latencia" | "capacidad" | "errores" | "sla" | "heatmap" | "peticiones";

// ─── KPI card ────────────────────────────────────────────────────────────────

const KpiCard: React.FC<{ label: string; value: string; accent?: string; sub?: string }> = ({
  label,
  value,
  accent,
  sub,
}) => (
  <Card>
    <Stat value={value} label={label} sub={sub} accent={accent} />
  </Card>
);

// ─── Section header (título + descripción) ─────────────────────────────────────

const SectionHeader: React.FC<{ title: string; desc?: string }> = ({ title, desc }) => (
  <div className="px-4 pt-4 pb-1">
    <p className="text-xs font-semibold text-slate-600 uppercase tracking-wide">{title}</p>
    {desc && <p className="text-xs text-slate-400 mt-0.5 normal-case font-normal tracking-normal">{desc}</p>}
  </div>
);

// ─── Label row ───────────────────────────────────────────────────────────────

const LabelRow: React.FC<{ stat: LabelStats; isOdd: boolean }> = ({ stat, isOdd }) => {
  const errPct = stat.error_rate * 100;
  const isTx = stat.kind === "transaction";

  return (
    <tr
      className={`border-b border-slate-100 last:border-0 text-sm ${
        isOdd ? "bg-slate-50/50" : "bg-white"
      }`}
    >
      <td className="px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span className="font-medium text-slate-800">{stat.label}</span>
          {isTx && (
            <Badge color="indigo">TX</Badge>
          )}
        </div>
      </td>
      <td className="px-3 py-2.5 tabular-nums text-right">{fmtNum(stat.count)}</td>
      <td className={`px-3 py-2.5 tabular-nums text-right ${errPct > 1 ? "text-red-600 font-semibold" : "text-slate-600"}`}>
        {stat.errors} ({fmtPct(errPct, true)})
      </td>
      <td className="px-3 py-2.5 tabular-nums text-right text-slate-600">{fmtThroughput(stat.throughput_per_sec)}</td>
      <td className="px-3 py-2.5 tabular-nums text-right text-slate-600">{fmtMs(stat.p50_ms)}</td>
      <td className="px-3 py-2.5 tabular-nums text-right text-slate-600">{fmtMs(stat.p90_ms)}</td>
      <td className={`px-3 py-2.5 tabular-nums text-right font-medium ${
        stat.p95_ms > 500 ? "text-red-600" : stat.p95_ms > 200 ? "text-amber-600" : "text-slate-800"
      }`}>{fmtMs(stat.p95_ms)}</td>
      <td className="px-3 py-2.5 tabular-nums text-right text-slate-600">{fmtMs(stat.p99_ms)}</td>
      <td className="px-3 py-2.5 tabular-nums text-right text-slate-500">{fmtBytes(stat.bytes_total)}</td>
    </tr>
  );
};

// ─── Request inspection (Peticiones) ──────────────────────────────────────────

const RequestRow: React.FC<{ detail: SampleDetail; isOdd: boolean }> = ({ detail, isOdd }) => {
  const [open, setOpen] = useState(false);
  const color = statusColor(detail);

  return (
    <div className={`border-b border-slate-100 last:border-0 ${isOdd ? "bg-slate-50/40" : "bg-white"}`}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-slate-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-indigo-500"
      >
        <span className="shrink-0 text-slate-400">{open ? <IconChevronDown /> : <IconChevronRight />}</span>
        <span className="shrink-0 text-xs tabular-nums text-slate-400 w-8">#{detail.seq}</span>
        <span className="shrink-0 w-12 flex justify-center">
          <Badge color={color}>{statusLabel(detail)}</Badge>
        </span>
        <span className="shrink-0 w-14 flex justify-start">
          <MethodChip method={detail.method} />
        </span>
        <span className="shrink-0 text-sm font-medium text-slate-800 truncate max-w-[28%]">{detail.label}</span>
        <span className="flex-1 min-w-0 text-xs font-mono text-slate-500 truncate" title={detail.url}>
          {detail.url}
        </span>
        <span
          className={`shrink-0 text-xs tabular-nums text-right w-16 ${
            detail.latency_ms > 1000 ? "text-red-600 font-semibold" : "text-slate-500"
          }`}
        >
          {fmtMs(detail.latency_ms)}
        </span>
      </button>

      {open && (
        <div className="px-3 pb-4 pt-1 bg-slate-50/30">
          <RequestDetail detail={detail} />
        </div>
      )}
    </div>
  );
};

/** Filas por página en la lista de Peticiones (evita renderizar miles de filas
 *  expandibles a la vez y congelar la webview). */
const REQUESTS_PER_PAGE = 100;

const RequestsPanel: React.FC<{ details: SampleDetail[]; total: number }> = ({ details, total }) => {
  // Muestra acotada: el tope de captura aplica tanto a la app nativa como al demo
  // del navegador, así que el mensaje es agnóstico al modo.
  const isSample = details.length < total;
  const pageCount = Math.max(1, Math.ceil(details.length / REQUESTS_PER_PAGE));
  const [page, setPage] = useState(0);

  // Reinicia a la página 1 cuando cambia el resumen (nueva corrida).
  React.useEffect(() => {
    setPage(0);
  }, [details]);

  // Acota la página por si details encogió respecto a un render anterior.
  const safePage = Math.min(page, pageCount - 1);
  const start = safePage * REQUESTS_PER_PAGE;
  const pageItems = details.slice(start, start + REQUESTS_PER_PAGE);

  return (
  <Card padding={false}>
    <div className="px-4 py-3 border-b border-slate-200">
      <p className="text-sm font-semibold text-slate-700">
        {isSample
          ? `Peticiones · ${fmtNum(details.length)} de ${fmtNum(total)} (muestra)`
          : `Peticiones (${fmtNum(details.length)}) · en orden de ejecución`}
      </p>
      <p className="text-xs text-slate-400 mt-0.5">
        {isSample
          ? "Se capturó una muestra acotada (tope de captura) para proteger la memoria — igual que conviene hacer con el View Results Tree de JMeter en cargas altas. Para guardar más, sube el «Límite de captura» en Ejecutar; para capturar todo a alto volumen, exporta por CLI a un archivo."
          : "Detalle de cada petición en orden de ejecución."}
      </p>
    </div>
    <div className="max-h-[70vh] overflow-y-auto">
      {pageItems.map((d, i) => (
        <RequestRow key={d.seq} detail={d} isOdd={(start + i) % 2 === 1} />
      ))}
    </div>
    {pageCount > 1 && (
      <div className="flex items-center justify-between gap-3 px-4 py-3 border-t border-slate-200">
        <p className="text-xs text-slate-500 tabular-nums">
          Página {safePage + 1} de {pageCount} · {fmtNum(details.length)} peticiones
        </p>
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => setPage((p) => Math.max(0, p - 1))}
            disabled={safePage <= 0}
          >
            Anterior
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => setPage((p) => Math.min(pageCount - 1, p + 1))}
            disabled={safePage >= pageCount - 1}
          >
            Siguiente
          </Button>
        </div>
      </div>
    )}
  </Card>
  );
};

// ─── Resumen tab ───────────────────────────────────────────────────────────────

const ResumenTab: React.FC<{ summary: RunSummary }> = ({ summary }) => {
  const { overall, labels, timeseries, errors } = summary;
  const overallErrorPct = overall.error_rate * 100;

  const throughputSeries = timeseries.map((p: TimePoint) => ({ x: p.t_secs, y: p.throughput }));
  const p95Series = timeseries.map((p: TimePoint) => ({ x: p.t_secs, y: p.p95_ms }));
  const errorSeries = timeseries.map((p: TimePoint) => ({ x: p.t_secs, y: p.error_rate * 100 }));
  const vusSeries = timeseries.map((p: TimePoint) => ({ x: p.t_secs, y: p.active_vus }));

  const sortedLabels = [...labels].sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "transaction" ? -1 : 1;
    return b.count - a.count;
  });

  return (
    <>
      {/* KPI cards */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <KpiCard
          label="Requests totales"
          value={fmtNum(overall.count)}
          sub={`${fmtThroughput(overall.throughput_per_sec)}`}
        />
        <KpiCard
          label="Throughput"
          value={fmtThroughput(overall.throughput_per_sec)}
          sub="peticiones por segundo"
        />
        <KpiCard
          label="Tasa de error"
          value={fmtPct(overallErrorPct, true)}
          accent={overallErrorPct > 1 ? "text-red-600" : "text-emerald-600"}
          sub={`${fmtNum(overall.errors)} errores`}
        />
        <KpiCard
          label="Latencia media"
          value={fmtMs(overall.mean_ms)}
          sub={`min ${fmtMs(overall.min_ms)} · max ${fmtMs(overall.max_ms)}`}
        />
      </div>

      {/* Percentile cards */}
      <div className="grid grid-cols-4 gap-3">
        {([50, 90, 95, 99] as const).map((p) => {
          const key = `p${p}_ms` as keyof LabelStats;
          const val = overall[key] as number;
          return (
            <KpiCard
              key={p}
              label={`P${p}`}
              value={fmtMs(val)}
              accent={p >= 95 && val > 500 ? "text-red-600" : undefined}
            />
          );
        })}
      </div>

      {/* Time series charts */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card padding={false}>
          <SectionHeader title="Throughput" />
          <Chart series={throughputSeries} color="#6366f1" height={130} valueFormat={fmtThroughput} />
        </Card>
        <Card padding={false}>
          <SectionHeader title="P95 Latencia" />
          <Chart series={p95Series} color="#f59e0b" height={130} valueFormat={fmtMs} />
        </Card>
        <Card padding={false}>
          <SectionHeader title="Tasa de error (%)" />
          <Chart series={errorSeries} color="#ef4444" height={100} valueFormat={(v) => `${v.toFixed(2)}%`} />
        </Card>
        <Card padding={false}>
          <SectionHeader title="Usuarios Virtuales" />
          <Chart series={vusSeries} color="#10b981" height={100} valueFormat={(v) => String(Math.round(v))} />
        </Card>
      </div>

      {/* Per-label table */}
      <Card padding={false}>
        <div className="px-4 py-3 border-b border-slate-200">
          <p className="text-sm font-semibold text-slate-700">Por endpoint</p>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200">
                <th className="px-3 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">Label</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Count</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Errores</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Throughput</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">P50</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">P90</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">P95</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">P99</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Bytes</th>
              </tr>
            </thead>
            <tbody>
              {sortedLabels.map((stat, i) => (
                <LabelRow key={stat.label} stat={stat} isOdd={i % 2 === 1} />
              ))}
            </tbody>
          </table>
        </div>
      </Card>

      {/* Errors table */}
      {errors.length > 0 && (
        <Card padding={false}>
          <div className="px-4 py-3 border-b border-slate-200 flex items-center gap-2">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="8" x2="12" y2="12"/>
              <line x1="12" y1="16" x2="12.01" y2="16"/>
            </svg>
            <p className="text-sm font-semibold text-slate-700">Errores</p>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-red-50 border-b border-red-100">
                  <th className="px-3 py-2.5 text-left text-xs font-semibold text-red-600 uppercase tracking-wide">Mensaje</th>
                  <th className="px-3 py-2.5 text-right text-xs font-semibold text-red-600 uppercase tracking-wide">Ocurrencias</th>
                </tr>
              </thead>
              <tbody>
                {errors.map((err, i) => (
                  <tr key={i} className="border-b border-slate-100 last:border-0">
                    <td className="px-3 py-2.5 font-mono text-xs text-slate-700">{err.message}</td>
                    <td className="px-3 py-2.5 text-right tabular-nums font-semibold text-red-600">
                      {fmtNum(err.count)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}
    </>
  );
};

// ─── Latencia tab ──────────────────────────────────────────────────────────────

const LatenciaTab: React.FC<{ summary: RunSummary }> = ({ summary }) => {
  const bounds = summary.histogram_bounds_ms ?? HISTOGRAM_BOUNDS_MS;
  const counts = summary.histogram_counts ?? [];
  const labels = summary.labels ?? [];

  const [apdexT, setApdexT] = useState(500);

  const percentileSeries = useMemo(() => percentileCurve(counts, bounds), [counts, bounds]);
  const apdexResult = useMemo(() => apdex(counts, bounds, apdexT), [counts, bounds, apdexT]);

  const histBars: BarDatum[] = counts.map((c, i) => ({
    label: bucketLabel(i, bounds),
    value: c,
    color: "#6366f1",
  }));

  // TTFB vs Total (p95) por endpoint — solo labels http con datos TTFB.
  const ttfbData: GroupedDatum[] = labels
    .filter((l) => l.kind !== "transaction")
    .map((l) => ({
      label: l.label,
      a: l.ttfb_p95_ms ?? 0,
      b: l.p95_ms ?? 0,
    }));

  const hasHistogram = counts.length > 0 && counts.some((c) => c > 0);

  return (
    <>
      {/* Curva de percentiles */}
      <Card padding={false}>
        <SectionHeader
          title="Curva de percentiles"
          desc="Latencia (ms) por percentil. La cola derecha revela el peor caso."
        />
        {percentileSeries.length >= 2 ? (
          <Chart
            series={percentileSeries}
            color="#6366f1"
            height={180}
            valueFormat={fmtMs}
          />
        ) : (
          <div className="px-4 pb-4 text-xs text-slate-400">Sin datos de histograma.</div>
        )}
      </Card>

      {/* Histograma de latencias */}
      <Card padding={false}>
        <SectionHeader
          title="Histograma de latencias"
          desc="Distribución de muestras por rango de latencia."
        />
        <div className="px-4 pb-4">
          {hasHistogram ? (
            <Bars data={histBars} color="#6366f1" height={200} valueFormat={fmtNum} tiltLabels />
          ) : (
            <div className="text-xs text-slate-400">Sin datos de histograma.</div>
          )}
        </div>
      </Card>

      {/* TTFB vs Total (p95) por endpoint */}
      <Card padding={false}>
        <SectionHeader
          title="TTFB vs Total (p95) por endpoint"
          desc="TTFB alto frente a Total bajo = latencia de red/servidor; ambos altos = procesamiento."
        />
        <div className="px-4 pb-4">
          {ttfbData.length > 0 ? (
            <GroupedBars
              data={ttfbData}
              seriesNames={["TTFB p95", "Total p95"]}
              colors={["#818cf8", "#4f46e5"]}
              height={200}
              valueFormat={fmtMs}
            />
          ) : (
            <div className="text-xs text-slate-400">Sin endpoints con datos de TTFB.</div>
          )}
        </div>
      </Card>

      {/* Apdex */}
      <Card>
        <div className="flex items-center justify-between flex-wrap gap-3 mb-4">
          <div>
            <p className="text-xs font-semibold text-slate-600 uppercase tracking-wide">Apdex</p>
            <p className="text-xs text-slate-400 mt-0.5">
              Satisfacción del usuario: satisfechas ≤ T, tolerando ≤ 4T, frustradas el resto.
            </p>
          </div>
          <div className="w-28">
            <Field label="T (ms)">
              <Input
                type="number"
                min={1}
                value={apdexT}
                onChange={(e) => setApdexT(Math.max(1, Number(e.target.value) || 1))}
              />
            </Field>
          </div>
        </div>
        {hasHistogram ? (
          <div className="flex items-center gap-8 flex-wrap">
            <div className="flex flex-col">
              <span className={`text-5xl font-bold tabular-nums tracking-tight ${apdexColor(apdexResult.score)}`}>
                {apdexResult.score.toFixed(2)}
              </span>
              <span className="text-xs font-medium text-slate-500 uppercase tracking-wide mt-1">
                Apdex (T={apdexT}ms)
              </span>
            </div>
            <div className="grid grid-cols-3 gap-4 text-sm">
              <div className="flex flex-col">
                <span className="text-lg font-semibold tabular-nums text-emerald-600">{fmtNum(apdexResult.satisfied)}</span>
                <span className="text-xs text-slate-500">Satisfechas (≤{apdexT}ms)</span>
              </div>
              <div className="flex flex-col">
                <span className="text-lg font-semibold tabular-nums text-amber-600">{fmtNum(apdexResult.tolerating)}</span>
                <span className="text-xs text-slate-500">Tolerando (≤{4 * apdexT}ms)</span>
              </div>
              <div className="flex flex-col">
                <span className="text-lg font-semibold tabular-nums text-red-600">{fmtNum(apdexResult.frustrated)}</span>
                <span className="text-xs text-slate-500">Frustradas</span>
              </div>
            </div>
          </div>
        ) : (
          <div className="text-xs text-slate-400">Sin datos de histograma para calcular Apdex.</div>
        )}
      </Card>
    </>
  );
};

// ─── Capacidad tab ─────────────────────────────────────────────────────────────

const CapacidadTab: React.FC<{ summary: RunSummary }> = ({ summary }) => {
  const timeseries = summary.timeseries ?? [];
  const labels = summary.labels ?? [];

  // Throughput / p95 vs VUs — ordenados por VUs ascendente para la curva.
  const byVus = [...timeseries].sort((a, b) => a.active_vus - b.active_vus);
  const tpVsVus = byVus.map((p) => ({ x: p.active_vus, y: p.throughput }));
  const p95VsVus = byVus.map((p) => ({ x: p.active_vus, y: p.p95_ms }));
  const bytesSeries = timeseries.map((p) => ({ x: p.t_secs, y: p.bytes ?? 0 }));

  const hasBytes = timeseries.some((p) => (p.bytes ?? 0) > 0);

  // Top endpoints por tiempo total (count * mean_ms).
  const topByTime = [...labels]
    .filter((l) => l.kind !== "transaction")
    .map((l) => ({ ...l, totalMs: l.count * l.mean_ms }))
    .sort((a, b) => b.totalMs - a.totalMs);

  return (
    <>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card padding={false}>
          <SectionHeader
            title="Throughput vs VUs"
            desc="El punto donde deja de subir es la saturación (knee)."
          />
          <Chart series={tpVsVus} color="#6366f1" height={180} valueFormat={fmtThroughput} />
        </Card>
        <Card padding={false}>
          <SectionHeader
            title="P95 vs VUs"
            desc="Si dispara al subir VUs, el sistema está saturado."
          />
          <Chart series={p95VsVus} color="#f59e0b" height={180} valueFormat={fmtMs} />
        </Card>
      </div>

      {/* Bytes/s + KPIs de datos */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <KpiCard label="Datos recibidos" value={fmtBytes(summary.bytes_received ?? 0)} />
        <KpiCard label="Datos enviados" value={fmtBytes(summary.bytes_sent ?? 0)} />
      </div>
      <Card padding={false}>
        <SectionHeader title="Bytes/s en el tiempo" desc="Throughput de datos recibidos por segundo." />
        {hasBytes ? (
          <Chart series={bytesSeries} color="#0ea5e9" height={150} valueFormat={(v) => `${fmtBytes(v)}/s`} />
        ) : (
          <div className="px-4 pb-4 text-xs text-slate-400">Sin datos de bytes por segundo.</div>
        )}
      </Card>

      {/* Top endpoints por tiempo total */}
      <Card padding={false}>
        <div className="px-4 py-3 border-b border-slate-200">
          <p className="text-sm font-semibold text-slate-700">Top endpoints por tiempo total</p>
          <p className="text-xs text-slate-400 mt-0.5">Dónde se concentra el tiempo (count × media).</p>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-slate-50 border-b border-slate-200">
                <th className="px-3 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">Endpoint</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Count</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Media</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-slate-500 uppercase tracking-wide">Tiempo total</th>
              </tr>
            </thead>
            <tbody>
              {topByTime.map((l, i) => (
                <tr key={l.label} className={`border-b border-slate-100 last:border-0 ${i % 2 === 1 ? "bg-slate-50/50" : "bg-white"}`}>
                  <td className="px-3 py-2.5 font-medium text-slate-800">{l.label}</td>
                  <td className="px-3 py-2.5 text-right tabular-nums text-slate-600">{fmtNum(l.count)}</td>
                  <td className="px-3 py-2.5 text-right tabular-nums text-slate-600">{fmtMs(l.mean_ms)}</td>
                  <td className="px-3 py-2.5 text-right tabular-nums font-semibold text-slate-800">{fmtMs(l.totalMs)}</td>
                </tr>
              ))}
              {topByTime.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-3 py-4 text-xs text-slate-400">Sin endpoints.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </>
  );
};

// ─── Errores tab ───────────────────────────────────────────────────────────────

const ERROR_KIND_LABELS: Record<string, string> = {
  status: "Estado HTTP",
  assertion: "Aserción",
  timeout: "Timeout",
  connection: "Conexión",
  other: "Otro",
};

const ERROR_KIND_COLORS: Record<string, string> = {
  status: "#f59e0b",
  assertion: "#a855f7",
  timeout: "#ef4444",
  connection: "#dc2626",
  other: "#94a3b8",
};

const ErroresTab: React.FC<{ summary: RunSummary }> = ({ summary }) => {
  const statusCodes = summary.status_codes ?? [];
  const errorKinds = summary.error_kinds ?? [];
  const errors = summary.errors ?? [];

  const statusBars: BarDatum[] = [...statusCodes]
    .sort((a, b) => a[0] - b[0])
    .map(([code, count]) => ({
      label: String(code),
      value: count,
      color: statusClassColor(code),
    }));

  const kindBars: BarDatum[] = errorKinds.map(([kind, count]) => ({
    label: ERROR_KIND_LABELS[kind] ?? kind,
    value: count,
    color: ERROR_KIND_COLORS[kind] ?? "#94a3b8",
  }));

  return (
    <>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card padding={false}>
          <SectionHeader title="Distribución por código de estado" desc="2xx verde · 3xx slate · 4xx ámbar · 5xx rojo." />
          <div className="px-4 pb-4">
            {statusBars.length > 0 ? (
              <Bars data={statusBars} height={180} valueFormat={fmtNum} />
            ) : (
              <div className="text-xs text-slate-400">Sin códigos de estado.</div>
            )}
          </div>
        </Card>
        <Card padding={false}>
          <SectionHeader title="Errores por tipo" desc="Naturaleza de los fallos." />
          <div className="px-4 pb-4">
            {kindBars.length > 0 ? (
              <Bars data={kindBars} height={180} valueFormat={fmtNum} />
            ) : (
              <div className="text-xs text-slate-400">Sin errores clasificados.</div>
            )}
          </div>
        </Card>
      </div>

      {/* Tabla de errores existente (mensaje → conteo) */}
      <Card padding={false}>
        <div className="px-4 py-3 border-b border-slate-200 flex items-center gap-2">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
          <p className="text-sm font-semibold text-slate-700">Errores</p>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-red-50 border-b border-red-100">
                <th className="px-3 py-2.5 text-left text-xs font-semibold text-red-600 uppercase tracking-wide">Mensaje</th>
                <th className="px-3 py-2.5 text-right text-xs font-semibold text-red-600 uppercase tracking-wide">Ocurrencias</th>
              </tr>
            </thead>
            <tbody>
              {errors.map((err, i) => (
                <tr key={i} className="border-b border-slate-100 last:border-0">
                  <td className="px-3 py-2.5 font-mono text-xs text-slate-700">{err.message}</td>
                  <td className="px-3 py-2.5 text-right tabular-nums font-semibold text-red-600">{fmtNum(err.count)}</td>
                </tr>
              ))}
              {errors.length === 0 && (
                <tr>
                  <td colSpan={2} className="px-3 py-4 text-xs text-slate-400">Sin errores registrados.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </>
  );
};

// ─── SLA tab ───────────────────────────────────────────────────────────────────

interface SlaThresholds {
  max_error_rate: number;
  max_p95_ms: number;
  max_p99_ms: number;
  min_throughput_per_sec: number;
}

const SLA_DEFAULTS: SlaThresholds = {
  max_error_rate: 0.05,
  max_p95_ms: 1500,
  max_p99_ms: 3000,
  min_throughput_per_sec: 1,
};

interface SlaCheck {
  label: string;
  pass: boolean;
  detail: string;
}

const CheckRow: React.FC<{ check: SlaCheck }> = ({ check }) => (
  <div className="flex items-center gap-3 px-4 py-3 border-b border-slate-100 last:border-0">
    <span
      className={`shrink-0 inline-flex items-center justify-center w-6 h-6 rounded-full ${
        check.pass ? "bg-emerald-50 text-emerald-600" : "bg-red-50 text-red-600"
      }`}
    >
      {check.pass ? <IconCheck /> : <IconX />}
    </span>
    <span className="text-sm font-medium text-slate-700 flex-1">{check.label}</span>
    <span className={`text-sm tabular-nums ${check.pass ? "text-slate-500" : "text-red-600 font-semibold"}`}>
      {check.detail}
    </span>
  </div>
);

const SlaTab: React.FC<{ summary: RunSummary }> = ({ summary }) => {
  const { overall, timeseries } = summary;
  const [th, setTh] = useState<SlaThresholds>(SLA_DEFAULTS);

  const checks: SlaCheck[] = useMemo(() => {
    const errRate = overall.error_rate;
    const tp = overall.throughput_per_sec;
    return [
      {
        label: "Tasa de error",
        pass: errRate <= th.max_error_rate,
        detail: `${fmtPct(errRate * 100, true)} ≤ ${fmtPct(th.max_error_rate * 100, true)}`,
      },
      {
        label: "P95",
        pass: overall.p95_ms <= th.max_p95_ms,
        detail: `${fmtMs(overall.p95_ms)} ≤ ${fmtMs(th.max_p95_ms)}`,
      },
      {
        label: "P99",
        pass: overall.p99_ms <= th.max_p99_ms,
        detail: `${fmtMs(overall.p99_ms)} ≤ ${fmtMs(th.max_p99_ms)}`,
      },
      {
        label: "Throughput mínimo",
        pass: tp >= th.min_throughput_per_sec,
        detail: `${fmtThroughput(tp)} ≥ ${fmtThroughput(th.min_throughput_per_sec)}`,
      },
    ];
  }, [overall, th]);

  const allPass = checks.every((c) => c.pass);

  const throughputSeries = timeseries.map((p) => ({ x: p.t_secs, y: p.throughput }));
  const p95Series = timeseries.map((p) => ({ x: p.t_secs, y: p.p95_ms }));
  const errorSeries = timeseries.map((p) => ({ x: p.t_secs, y: p.error_rate * 100 }));

  const upd = (k: keyof SlaThresholds, v: number) =>
    setTh((prev) => ({ ...prev, [k]: Number.isFinite(v) ? v : prev[k] }));

  return (
    <>
      {/* Banner de veredicto */}
      <div
        className={`rounded-xl border p-5 flex items-center gap-4 ${
          allPass ? "bg-emerald-50 border-emerald-200" : "bg-red-50 border-red-200"
        }`}
      >
        <span
          className={`shrink-0 inline-flex items-center justify-center w-12 h-12 rounded-full ${
            allPass ? "bg-emerald-100 text-emerald-700" : "bg-red-100 text-red-700"
          }`}
        >
          {allPass ? <IconCheck /> : <IconX />}
        </span>
        <div>
          <p className={`text-2xl font-bold tracking-tight ${allPass ? "text-emerald-700" : "text-red-700"}`}>
            {allPass ? "PASA" : "FALLA"}
          </p>
          <p className={`text-sm ${allPass ? "text-emerald-600" : "text-red-600"}`}>
            {allPass
              ? "Todos los umbrales SLA se cumplen."
              : `${checks.filter((c) => !c.pass).length} de ${checks.length} umbrales incumplidos.`}
          </p>
        </div>
      </div>

      {/* Editor de umbrales */}
      <Card>
        <p className="text-xs font-semibold text-slate-600 uppercase tracking-wide mb-4">Umbrales</p>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <Field label="Max error rate" help="0–1 (p.ej. 0.05 = 5%)">
            <Input
              type="number" step="0.01" min={0} max={1}
              value={th.max_error_rate}
              onChange={(e) => upd("max_error_rate", Number(e.target.value))}
            />
          </Field>
          <Field label="Max P95 (ms)">
            <Input type="number" min={0} value={th.max_p95_ms} onChange={(e) => upd("max_p95_ms", Number(e.target.value))} />
          </Field>
          <Field label="Max P99 (ms)">
            <Input type="number" min={0} value={th.max_p99_ms} onChange={(e) => upd("max_p99_ms", Number(e.target.value))} />
          </Field>
          <Field label="Min throughput (req/s)">
            <Input type="number" step="0.1" min={0} value={th.min_throughput_per_sec} onChange={(e) => upd("min_throughput_per_sec", Number(e.target.value))} />
          </Field>
        </div>
      </Card>

      {/* Checks individuales */}
      <Card padding={false}>
        <div className="px-4 py-3 border-b border-slate-200">
          <p className="text-sm font-semibold text-slate-700">Verificación de umbrales</p>
        </div>
        {checks.map((c) => (
          <CheckRow key={c.label} check={c} />
        ))}
      </Card>

      {/* Time-charts con línea de umbral */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card padding={false}>
          <SectionHeader title="Throughput vs umbral mínimo" />
          <Chart
            series={throughputSeries}
            color="#6366f1"
            height={150}
            valueFormat={fmtThroughput}
            refLine={th.min_throughput_per_sec}
            refLineColor="#0ea5e9"
          />
        </Card>
        <Card padding={false}>
          <SectionHeader title="P95 vs umbral" />
          <Chart series={p95Series} color="#f59e0b" height={150} valueFormat={fmtMs} refLine={th.max_p95_ms} />
        </Card>
        <Card padding={false}>
          <SectionHeader title="Tasa de error vs umbral (%)" />
          <Chart
            series={errorSeries}
            color="#ef4444"
            height={150}
            valueFormat={(v) => `${v.toFixed(2)}%`}
            refLine={th.max_error_rate * 100}
          />
        </Card>
      </div>
    </>
  );
};

// ─── Heatmap tab ───────────────────────────────────────────────────────────────

const HeatmapTab: React.FC<{ summary: RunSummary }> = ({ summary }) => {
  const rows = summary.latency_heatmap ?? [];
  const bounds = summary.histogram_bounds_ms ?? HISTOGRAM_BOUNDS_MS;

  return (
    <Card padding={false}>
      <SectionHeader
        title="Heatmap latencia × tiempo"
        desc="Cada celda: nº de muestras en ese segundo y rango de latencia. Bandas oscuras altas = picos de latencia."
      />
      <div className="px-4 pb-5 pt-2">
        <Heatmap rows={rows} bounds={bounds} bucketLabel={bucketLabel} />
      </div>
    </Card>
  );
};

// ─── ReportView ───────────────────────────────────────────────────────────────

export const ReportView: React.FC<ReportViewProps> = ({ summary, onLoadResults }) => {
  const details = summary.details ?? [];
  const hasDetails = details.length > 0;
  const [tab, setTab] = useState<TabId>("resumen");

  const handleExport = async () => {
    const path = await api.exportReport(summary);
    if (path) {
      alert(`Reporte exportado a:\n${path}`);
    }
  };

  const tabs: { id: TabId; label: string }[] = [
    { id: "resumen", label: "Resumen" },
    { id: "latencia", label: "Latencia" },
    { id: "capacidad", label: "Capacidad" },
    { id: "errores", label: "Errores" },
    { id: "sla", label: "SLA" },
    { id: "heatmap", label: "Heatmap" },
    ...(hasDetails ? [{ id: "peticiones" as TabId, label: `Peticiones (${fmtNum(details.length)})` }] : []),
  ];

  return (
    <div className="flex flex-col gap-6 p-6 max-w-5xl mx-auto w-full">
      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-lg font-semibold text-slate-900">{summary.scenario_name}</h2>
          <p className="text-sm text-slate-500 mt-0.5">
            {new Date(summary.started_at).toLocaleString("es-ES")} ·{" "}
            {fmtDuration(summary.duration_secs)} ·{" "}
            {summary.config.virtual_users} VUs · {summary.config.thread_groups} grupo
            {summary.config.thread_groups !== 1 ? "s" : ""}
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {onLoadResults && (
            <Button
              variant="secondary"
              size="sm"
              icon={
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
                </svg>
              }
              onClick={onLoadResults}
              title="Cargar otro summary.json"
            >
              Cargar resultados
            </Button>
          )}
          <Button
            variant="secondary"
            icon={
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                <polyline points="17 8 12 3 7 8"/>
                <line x1="12" y1="3" x2="12" y2="15"/>
              </svg>
            }
            onClick={handleExport}
            disabled={!api.isTauri}
            title={!api.isTauri ? "Disponible en la app nativa" : "Exportar reporte"}
          >
            Exportar reporte
          </Button>
        </div>
      </div>

      <Tabs tabs={tabs} active={tab} onChange={(id) => setTab(id as TabId)} />

      {tab === "resumen" && <ResumenTab summary={summary} />}
      {tab === "latencia" && <LatenciaTab summary={summary} />}
      {tab === "capacidad" && <CapacidadTab summary={summary} />}
      {tab === "errores" && <ErroresTab summary={summary} />}
      {tab === "sla" && <SlaTab summary={summary} />}
      {tab === "heatmap" && <HeatmapTab summary={summary} />}
      {tab === "peticiones" && hasDetails && (
        <RequestsPanel details={details} total={summary.overall.count} />
      )}
    </div>
  );
};
