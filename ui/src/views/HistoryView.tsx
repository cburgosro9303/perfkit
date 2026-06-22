import React, { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Chart } from "../components/Chart";
import {
  Badge,
  Button,
  Card,
  EmptyState,
  Field,
  IconCheck,
  IconReport,
  IconX,
  Input,
  Select,
} from "../components/ui";
import { fmtMs, fmtPct, fmtThroughput } from "../lib/format";
import type {
  Comparison,
  RunRecord,
  RunSummary,
  TrendMetric,
  TrendPoint,
} from "../types";

interface HistoryViewProps {
  currentSummary: RunSummary | null;
}

// ─── Helpers ────────────────────────────────────────────────────────────────

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-ES", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Formato de un Δ con signo y color (mejor/peor según `lowerIsBetter`). */
function deltaTone(delta: number, lowerIsBetter: boolean): "green" | "red" | "slate" {
  if (Math.abs(delta) < 1e-9) return "slate";
  const improved = lowerIsBetter ? delta < 0 : delta > 0;
  return improved ? "green" : "red";
}

const METRIC_LABELS: Record<TrendMetric, string> = {
  p95: "Latencia P95",
  throughput: "Throughput",
  error_rate: "Tasa de error",
};

// ─── Comparison card ──────────────────────────────────────────────────────────

const DeltaStat: React.FC<{
  label: string;
  value: string;
  tone: "green" | "red" | "slate";
}> = ({ label, value, tone }) => {
  const color =
    tone === "green" ? "text-emerald-600" : tone === "red" ? "text-red-600" : "text-slate-700";
  return (
    <div className="flex flex-col gap-0.5">
      <span className={`text-2xl font-bold tabular-nums tracking-tight ${color}`}>{value}</span>
      <span className="text-xs font-medium text-slate-500 uppercase tracking-wide">{label}</span>
    </div>
  );
};

const signed = (n: number, fmt: (v: number) => string): string =>
  `${n > 0 ? "+" : ""}${fmt(n)}`;

// ─── View ─────────────────────────────────────────────────────────────────────

export const HistoryView: React.FC<HistoryViewProps> = ({ currentSummary }) => {
  const isDemo = !api.isTauri;

  // Filtro/identidad de la corrida actual.
  const [branch, setBranch] = useState("main");
  const [environment, setEnvironment] = useState("staging");
  const [build, setBuild] = useState("");
  const [commit, setCommit] = useState("");

  const [runs, setRuns] = useState<RunRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  // Estado de comparación: corrida recién guardada (o seleccionada) vs baseline.
  const [recordedId, setRecordedId] = useState<number | null>(null);
  const [baselineId, setBaselineId] = useState<number | null>(null);
  const [comparison, setComparison] = useState<Comparison | null>(null);

  // Tendencia.
  const [metric, setMetric] = useState<TrendMetric>("p95");
  const [trend, setTrend] = useState<TrendPoint[]>([]);

  const scenarioName = currentSummary?.scenario_name ?? "Checkout de e-commerce (demo)";

  const flash = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3500);
  }, []);

  const refreshList = useCallback(async () => {
    setLoading(true);
    try {
      const rows = await api.history.list({ limit: 50 });
      setRuns(rows);
    } catch (e) {
      flash(`No se pudo cargar el histórico: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [flash]);

  const refreshTrend = useCallback(async () => {
    try {
      const pts = await api.history.trend({
        scenario: scenarioName,
        environment,
        metric,
        limit: 30,
      });
      setTrend(pts);
    } catch {
      setTrend([]);
    }
  }, [scenarioName, environment, metric]);

  const refreshComparison = useCallback(
    async (runId: number | null) => {
      if (runId === null) {
        setComparison(null);
        return;
      }
      try {
        const cmp = await api.history.compare({
          runId,
          branch,
          environment,
          scenario: scenarioName,
        });
        setComparison(cmp);
      } catch {
        setComparison(null);
      }
    },
    [branch, environment, scenarioName],
  );

  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  useEffect(() => {
    void refreshTrend();
  }, [refreshTrend]);

  // Recalcula la comparación cuando cambian baseline/corrida o la identidad.
  useEffect(() => {
    void refreshComparison(recordedId);
  }, [recordedId, baselineId, refreshComparison]);

  const handleSave = async () => {
    if (!currentSummary) return;
    setSaving(true);
    try {
      const id = await api.history.record({
        summary: currentSummary,
        branch: branch || undefined,
        environment: environment || undefined,
        build: build || undefined,
        commit: commit || undefined,
      });
      setRecordedId(id);
      flash(`Corrida #${id} guardada en el histórico.`);
      await refreshList();
      await refreshTrend();
    } catch (e) {
      flash(`Error al guardar: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const handleSetBaseline = async (run: RunRecord) => {
    try {
      await api.history.setBaseline({
        branch: run.branch || branch,
        environment: run.environment || environment,
        scenario: run.scenario,
        runId: run.id,
      });
      setBaselineId(run.id);
      flash(`Corrida #${run.id} fijada como baseline.`);
      // Si la identidad del baseline coincide con la del filtro, recompara.
      void refreshComparison(recordedId ?? run.id);
      if (recordedId === null) setRecordedId(run.id);
    } catch (e) {
      flash(`Error al fijar baseline: ${e}`);
    }
  };

  const trendSeries = useMemo(
    () =>
      trend.map((p, i) => ({
        x: i,
        y: metric === "error_rate" ? p.value * 100 : p.value,
      })),
    [trend, metric],
  );

  const trendFormat = useMemo(() => {
    if (metric === "p95") return fmtMs;
    if (metric === "throughput") return fmtThroughput;
    return (v: number) => `${v.toFixed(2)}%`;
  }, [metric]);

  const trendColor =
    metric === "p95" ? "#f59e0b" : metric === "throughput" ? "#6366f1" : "#ef4444";

  return (
    <div className="flex flex-col gap-6 p-6 max-w-5xl mx-auto w-full">
      {/* Encabezado */}
      <div>
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-semibold text-slate-900">Histórico y comparación</h1>
          {isDemo && <Badge color="slate">demo</Badge>}
        </div>
        <p className="text-sm text-slate-500 mt-0.5">
          Guarda corridas, fija un <span className="font-medium">baseline</span> y detecta
          regresiones de rendimiento entre ejecuciones.
          {isDemo && (
            <span className="text-slate-400">
              {" "}
              En el navegador los datos son de muestra y viven solo en esta pestaña.
            </span>
          )}
        </p>
      </div>

      {/* 1) Guardar run actual */}
      <Card>
        <div className="mb-4">
          <p className="text-sm font-semibold text-slate-700">Guardar run actual</p>
          <p className="text-xs text-slate-500 mt-0.5">
            {currentSummary
              ? `Resumen disponible: «${currentSummary.scenario_name}». Etiqueta la corrida y guárdala.`
              : "No hay un reporte cargado. Ejecuta un escenario para poder guardarlo."}
          </p>
        </div>
        <div className="flex items-end gap-3 flex-wrap">
          <Field label="Branch" className="w-36">
            <Input
              value={branch}
              onChange={(e) => setBranch(e.target.value)}
              placeholder="main"
              disabled={!currentSummary || saving}
            />
          </Field>
          <Field label="Entorno" className="w-40">
            <Input
              value={environment}
              onChange={(e) => setEnvironment(e.target.value)}
              placeholder="staging"
              disabled={!currentSummary || saving}
            />
          </Field>
          <Field label="Build" className="w-36">
            <Input
              value={build}
              onChange={(e) => setBuild(e.target.value)}
              placeholder="ci-1234"
              disabled={!currentSummary || saving}
            />
          </Field>
          <Field label="Commit (opcional)" className="w-40">
            <Input
              value={commit}
              onChange={(e) => setCommit(e.target.value)}
              placeholder="a1b2c3d"
              className="font-mono text-xs"
              disabled={!currentSummary || saving}
            />
          </Field>
          <div className="pb-0.5">
            <Button
              variant="primary"
              onClick={handleSave}
              disabled={!currentSummary || saving}
              icon={<IconReport />}
            >
              {saving ? "Guardando…" : "Guardar en histórico"}
            </Button>
          </div>
        </div>
      </Card>

      {/* 3) Comparación vs baseline */}
      <Card>
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <p className="text-sm font-semibold text-slate-700">Comparación vs baseline</p>
            <p className="text-xs text-slate-500 mt-0.5">
              Δ de la corrida {recordedId !== null ? `#${recordedId}` : "actual"} contra el
              baseline de <code className="text-slate-600">{branch}</code> ·{" "}
              <code className="text-slate-600">{environment}</code>.
            </p>
          </div>
          {comparison && (
            <Badge color={comparison.is_regression ? "red" : "green"}>
              {comparison.is_regression ? (
                <>
                  <IconX /> REGRESIÓN
                </>
              ) : (
                <>
                  <IconCheck /> OK
                </>
              )}
            </Badge>
          )}
        </div>

        {comparison ? (
          <div
            className={`rounded-xl border p-5 ${
              comparison.is_regression
                ? "bg-red-50 border-red-200"
                : "bg-emerald-50 border-emerald-200"
            }`}
          >
            <div className="grid grid-cols-3 gap-4">
              <DeltaStat
                label="P95 Δ"
                value={signed(comparison.p95_delta_pct, (v) => `${v.toFixed(1)}%`)}
                tone={deltaTone(comparison.p95_delta_pct, true)}
              />
              <DeltaStat
                label="Throughput Δ"
                value={signed(comparison.throughput_delta_pct, (v) => `${v.toFixed(1)}%`)}
                tone={deltaTone(comparison.throughput_delta_pct, false)}
              />
              <DeltaStat
                label="Error rate Δ"
                value={signed(comparison.error_rate_delta * 100, (v) => `${v.toFixed(2)} pp`)}
                tone={deltaTone(comparison.error_rate_delta, true)}
              />
            </div>
          </div>
        ) : (
          <EmptyState
            icon={<IconReport />}
            title="Sin comparación todavía"
            description="Fija una corrida como baseline (abajo) y guarda o selecciona una corrida actual para ver el delta."
          />
        )}
      </Card>

      {/* 4) Tendencia */}
      <Card padding={false}>
        <div className="flex items-center justify-between gap-3 px-5 pt-5 pb-2">
          <div>
            <p className="text-sm font-semibold text-slate-700">Tendencia</p>
            <p className="text-xs text-slate-500 mt-0.5">
              {METRIC_LABELS[metric]} a lo largo de las corridas de{" "}
              <code className="text-slate-600">{environment}</code>.
            </p>
          </div>
          <div className="w-44">
            <Select
              value={metric}
              onChange={(e) => setMetric(e.target.value as TrendMetric)}
              aria-label="Métrica de tendencia"
            >
              <option value="p95">Latencia P95</option>
              <option value="throughput">Throughput</option>
              <option value="error_rate">Tasa de error</option>
            </Select>
          </div>
        </div>
        <div className="px-2 pb-4">
          {trendSeries.length >= 2 ? (
            <Chart
              series={trendSeries}
              color={trendColor}
              height={160}
              valueFormat={trendFormat}
            />
          ) : (
            <div className="px-3">
              <EmptyState
                title="No hay suficientes corridas"
                description="Guarda al menos dos corridas del mismo escenario y entorno para dibujar una tendencia."
              />
            </div>
          )}
        </div>
      </Card>

      {/* 2) Runs históricos */}
      <Card padding={false}>
        <div className="flex items-center justify-between gap-3 px-5 pt-5 pb-3">
          <div>
            <p className="text-sm font-semibold text-slate-700">Runs históricos</p>
            <p className="text-xs text-slate-500 mt-0.5">
              {loading ? "Cargando…" : `${runs.length} corrida${runs.length !== 1 ? "s" : ""}`}
            </p>
          </div>
          <Button variant="ghost" size="sm" onClick={() => void refreshList()} disabled={loading}>
            Refrescar
          </Button>
        </div>
        {runs.length === 0 ? (
          <div className="px-3 pb-4">
            <EmptyState
              icon={<IconReport />}
              title="Sin corridas guardadas"
              description="Guarda el resumen de una ejecución para empezar a construir el histórico."
            />
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="text-left text-xs font-semibold text-slate-500 uppercase tracking-wide border-b border-slate-200">
                  <th className="px-3 py-2.5">ID</th>
                  <th className="px-3 py-2.5">Escenario</th>
                  <th className="px-3 py-2.5">Env</th>
                  <th className="px-3 py-2.5">Branch</th>
                  <th className="px-3 py-2.5">Fecha</th>
                  <th className="px-3 py-2.5 text-right">Throughput</th>
                  <th className="px-3 py-2.5 text-right">P95</th>
                  <th className="px-3 py-2.5 text-right">Error %</th>
                  <th className="px-3 py-2.5 text-right">Acción</th>
                </tr>
              </thead>
              <tbody>
                {runs.map((r, i) => {
                  const isBaseline = r.id === baselineId;
                  const isCurrent = r.id === recordedId;
                  return (
                    <tr
                      key={r.id}
                      className={`border-b border-slate-100 last:border-0 text-sm ${
                        i % 2 ? "bg-slate-50/50" : "bg-white"
                      }`}
                    >
                      <td className="px-3 py-2.5 tabular-nums text-slate-500">
                        <div className="flex items-center gap-1.5">
                          #{r.id}
                          {isBaseline && <Badge color="indigo">baseline</Badge>}
                          {isCurrent && !isBaseline && <Badge color="blue">actual</Badge>}
                        </div>
                      </td>
                      <td className="px-3 py-2.5 font-medium text-slate-800 truncate max-w-[14rem]">
                        {r.scenario}
                      </td>
                      <td className="px-3 py-2.5 text-slate-600">{r.environment ?? "—"}</td>
                      <td className="px-3 py-2.5 text-slate-600 font-mono text-xs">
                        {r.branch ?? "—"}
                      </td>
                      <td className="px-3 py-2.5 text-slate-500 whitespace-nowrap">
                        {fmtDate(r.started_at)}
                      </td>
                      <td className="px-3 py-2.5 tabular-nums text-right text-slate-600">
                        {fmtThroughput(r.throughput)}
                      </td>
                      <td
                        className={`px-3 py-2.5 tabular-nums text-right font-medium ${
                          r.p95_ms > 500
                            ? "text-red-600"
                            : r.p95_ms > 200
                              ? "text-amber-600"
                              : "text-slate-800"
                        }`}
                      >
                        {fmtMs(r.p95_ms)}
                      </td>
                      <td
                        className={`px-3 py-2.5 tabular-nums text-right ${
                          r.error_rate > 0.01 ? "text-red-600 font-semibold" : "text-slate-600"
                        }`}
                      >
                        {fmtPct(r.error_rate * 100, true)}
                      </td>
                      <td className="px-3 py-2.5 text-right">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => void handleSetBaseline(r)}
                          disabled={isBaseline}
                        >
                          {isBaseline ? "Baseline" : "Fijar como baseline"}
                        </Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {/* Toast */}
      {toast && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 px-4 py-2.5 rounded-lg bg-slate-900 text-white text-sm shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
};
