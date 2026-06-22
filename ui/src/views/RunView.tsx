import React, { useRef, useState } from "react";
import { api } from "../api";
import { Chart } from "../components/Chart";
import { Button, Card, Field, Input, Spinner, Stat, Toggle } from "../components/ui";
import { fmtMs, fmtNum, fmtPct, fmtThroughput } from "../lib/format";
import type { LiveSnapshot, RunOptions, RunSummary, Scenario, TimePoint } from "../types";

interface RunViewProps {
  scenario: Scenario;
  onFinished: (summary: RunSummary) => void;
}

type RunState = "idle" | "running" | "stopping";

// Tope de captura: por defecto 5000; el usuario puede subirlo hasta 50.000
// (tope duro en el input para no volver a congelar la webview en cargas altas).
const CAPTURE_LIMIT_DEFAULT = 5000;
const CAPTURE_LIMIT_MIN = 100;
const CAPTURE_LIMIT_MAX = 50000;

export const RunView: React.FC<RunViewProps> = ({ scenario, onFinished }) => {
  // Overrides opcionales para ESTA corrida (vacío = usar lo del plan).
  const [baseUrl, setBaseUrl] = useState("");
  const [vusOverride, setVusOverride] = useState("");
  const [durationOverride, setDurationOverride] = useState("");
  const [capture, setCapture] = useState(false);
  const [plaintext, setPlaintext] = useState(false);
  const [captureLimit, setCaptureLimit] = useState(CAPTURE_LIMIT_DEFAULT);
  const [runState, setRunState] = useState<RunState>("idle");
  const [snapshots, setSnapshots] = useState<LiveSnapshot[]>([]);
  const [latest, setLatest] = useState<LiveSnapshot | null>(null);

  const cancelRef = useRef<(() => void) | null>(null);

  const handleRun = async () => {
    setSnapshots([]);
    setLatest(null);
    setRunState("running");

    const opts: RunOptions = {
      base_url_override: baseUrl || undefined,
      vus: vusOverride ? Number(vusOverride) : undefined,
      duration_secs: durationOverride ? Number(durationOverride) : undefined,
      capture: capture || undefined,
      capture_plaintext: (capture && plaintext) || undefined,
      capture_limit: capture ? captureLimit : undefined,
    };

    try {
      const cancel = await api.run(
        scenario,
        opts,
        (snap) => {
          setLatest(snap);
          setSnapshots((prev) => [...prev, snap]);
        },
        (summary) => {
          setRunState("idle");
          cancelRef.current = null;
          onFinished(summary);
        },
      );
      cancelRef.current = cancel;
    } catch (err) {
      setRunState("idle");
      console.error("Run failed:", err);
    }
  };

  const handleStop = async () => {
    setRunState("stopping");
    if (cancelRef.current) {
      cancelRef.current();
      cancelRef.current = null;
    }
    await api.cancel();
    setRunState("idle");
  };

  const isRunning = runState === "running";
  const isStopping = runState === "stopping";

  // Build time series for charts
  const throughputSeries: { x: number; y: number }[] = snapshots.map((s) => ({
    x: s.elapsed_secs,
    y: s.throughput_per_sec,
  }));
  const p95Series: { x: number; y: number }[] = snapshots.map((s) => ({
    x: s.elapsed_secs,
    y: s.p95_ms,
  }));
  const vusSeries: { x: number; y: number }[] = snapshots.map((s) => ({
    x: s.elapsed_secs,
    y: s.active_vus,
  }));

  const errorRate = latest ? latest.error_rate * 100 : 0;

  return (
    <div className="flex flex-col gap-6 p-6 max-w-4xl mx-auto w-full">
      {/* Config bar */}
      <Card>
        <div className="mb-3">
          <p className="text-sm font-semibold text-slate-700">Ejecutar «{scenario.name}»</p>
          <p className="text-xs text-slate-500 mt-0.5">
            Overrides <span className="font-medium">solo para esta corrida</span> (opcionales).
            Si los dejas vacíos se usa lo definido en el plan — no editan el plan.
          </p>
        </div>
        <div className="flex items-end gap-4 flex-wrap">
          <Field
            label="URL base (override)"
            help="Apunta el mismo plan a otro entorno (staging, local…). Vacío = la del plan."
            className="flex-1 min-w-48"
          >
            <Input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder={scenario.defaults?.base_url || "usar la del plan"}
              className="font-mono text-xs"
              disabled={isRunning}
            />
          </Field>
          <Field label="VUs (override)" help="Vacío = los del plan. Aplica a todos los grupos." className="w-32">
            <Input
              type="number"
              min={1}
              value={vusOverride}
              onChange={(e) => setVusOverride(e.target.value)}
              placeholder={`plan: ${scenario.thread_groups.reduce((a, g) => a + (g.load?.virtual_users ?? 0), 0)}`}
              className="tabular-nums"
              disabled={isRunning}
            />
          </Field>
          <Field label="Duración s (override)" help="Vacío = iteraciones/duración del plan." className="w-32">
            <Input
              type="number"
              min={1}
              value={durationOverride}
              onChange={(e) => setDurationOverride(e.target.value)}
              placeholder="plan"
              className="tabular-nums"
              disabled={isRunning}
            />
          </Field>

          <div className="flex gap-2 pb-0.5">
            {!isRunning && !isStopping && (
              <Button
                variant="primary"
                size="lg"
                onClick={handleRun}
                icon={
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                    <polygon points="5 3 19 12 5 21 5 3"/>
                  </svg>
                }
              >
                Ejecutar
              </Button>
            )}
            {isRunning && (
              <Button
                variant="danger"
                size="lg"
                onClick={handleStop}
                icon={
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <rect x="3" y="3" width="18" height="18" rx="2"/>
                  </svg>
                }
              >
                Detener
              </Button>
            )}
            {isStopping && (
              <Button variant="danger" size="lg" disabled icon={<Spinner size={14} />}>
                Deteniendo…
              </Button>
            )}
          </div>
        </div>

        {/* Captura de peticiones (depuración) */}
        <div className="mt-4 pt-4 border-t border-slate-100">
          <Toggle
            checked={capture}
            onChange={(v) => {
              if (isRunning) return;
              setCapture(v);
              // Al desactivar la captura, también se apaga "mostrar secretos".
              if (!v) setPlaintext(false);
              // Al activar la captura, prellena overrides pequeños si están vacíos
              // (corridas cortas para depurar). No es obligatorio: el usuario puede cambiarlo.
              if (v && !vusOverride) setVusOverride("1");
            }}
            label="Capturar peticiones (depuración)"
          />
          <p className="text-xs text-slate-400 mt-1.5">
            Guarda el detalle de cada petición (request/response) para inspeccionarlo, hasta
            el límite de captura (salvaguarda contra OOM). Úsalo en corridas cortas; no para carga real.
          </p>

          {/* Límite de captura (solo con captura activa) */}
          {capture && (
            <div className="mt-4">
              <Field label="Límite de captura" className="w-40">
                <Input
                  type="number"
                  min={CAPTURE_LIMIT_MIN}
                  max={CAPTURE_LIMIT_MAX}
                  step={100}
                  value={captureLimit}
                  onChange={(e) => {
                    const n = Number(e.target.value);
                    if (!Number.isFinite(n)) return;
                    setCaptureLimit(
                      Math.min(CAPTURE_LIMIT_MAX, Math.max(CAPTURE_LIMIT_MIN, Math.round(n))),
                    );
                  }}
                  className="tabular-nums"
                  disabled={isRunning}
                />
              </Field>
              <p className="text-xs text-slate-400 mt-1.5">
                Máximo de peticiones a guardar para inspección (tope 50.000; valores altos usan
                más memoria). Para capturar todo a alto volumen, usa la CLI a archivo.
              </p>
            </div>
          )}

          {/* Mostrar secretos sin redactar (solo con captura activa) */}
          <div className={`mt-4 ${capture ? "" : "opacity-50"}`}>
            <Toggle
              checked={plaintext}
              onChange={(v) => {
                if (isRunning || !capture) return;
                setPlaintext(v);
              }}
              label="Mostrar secretos sin redactar"
            />
            <p className="text-xs text-slate-400 mt-1.5">
              Registra cabeceras y variables en texto plano. Úsalo solo en entornos de prueba.
            </p>
          </div>
        </div>
      </Card>

      {/* Idle state */}
      {!isRunning && snapshots.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-4 py-20 text-center">
          <div className="w-16 h-16 rounded-full bg-slate-100 flex items-center justify-center">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#94a3b8" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <polygon points="5 3 19 12 5 21 5 3"/>
            </svg>
          </div>
          <div>
            <p className="text-sm font-semibold text-slate-700">
              Listo para ejecutar "{scenario.name}"
            </p>
            <p className="text-xs text-slate-400 mt-1">
              Configura los parámetros arriba y pulsa Ejecutar
            </p>
          </div>
        </div>
      )}

      {/* Live dashboard */}
      {(isRunning || snapshots.length > 0) && (
        <div className="flex flex-col gap-5">
          {/* KPI row */}
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
            <Card className="col-span-1">
              <div className="flex items-center gap-2 mb-1">
                <span className={`w-2 h-2 rounded-full ${isRunning ? "bg-emerald-500 animate-pulse" : "bg-slate-300"}`} />
                <span className="text-xs text-slate-500 font-medium">VUs activos</span>
              </div>
              <Stat value={fmtNum(latest?.active_vus ?? 0)} label="" accent="text-indigo-600" />
            </Card>
            <Card>
              <span className="text-xs text-slate-500 font-medium block mb-1">Requests</span>
              <Stat value={fmtNum(latest?.total_requests ?? 0)} label="" />
            </Card>
            <Card>
              <span className="text-xs text-slate-500 font-medium block mb-1">Throughput</span>
              <Stat
                value={fmtThroughput(latest?.throughput_per_sec ?? 0)}
                label=""
                accent="text-emerald-600"
              />
            </Card>
            <Card>
              <span className="text-xs text-slate-500 font-medium block mb-1">P95</span>
              <Stat
                value={fmtMs(latest?.p95_ms ?? 0)}
                label=""
                accent={
                  (latest?.p95_ms ?? 0) > 500
                    ? "text-red-600"
                    : (latest?.p95_ms ?? 0) > 200
                    ? "text-amber-600"
                    : "text-slate-900"
                }
              />
            </Card>
            <Card>
              <span className="text-xs text-slate-500 font-medium block mb-1">Error rate</span>
              <Stat
                value={fmtPct(errorRate, true)}
                label=""
                accent={errorRate > 1 ? "text-red-600" : "text-slate-900"}
              />
            </Card>
          </div>

          {/* Charts */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Card padding={false}>
              <div className="px-4 pt-4 pb-2">
                <p className="text-xs font-semibold text-slate-600 uppercase tracking-wide">Throughput</p>
              </div>
              <Chart
                series={throughputSeries}
                color="#6366f1"
                height={130}
                valueFormat={fmtThroughput}
              />
            </Card>
            <Card padding={false}>
              <div className="px-4 pt-4 pb-2">
                <p className="text-xs font-semibold text-slate-600 uppercase tracking-wide">Latencia P95</p>
              </div>
              <Chart
                series={p95Series}
                color="#f59e0b"
                height={130}
                valueFormat={fmtMs}
              />
            </Card>
          </div>

          <Card padding={false}>
            <div className="px-4 pt-4 pb-2">
              <p className="text-xs font-semibold text-slate-600 uppercase tracking-wide">Usuarios Virtuales</p>
            </div>
            <Chart
              series={vusSeries}
              color="#10b981"
              height={90}
              valueFormat={(v) => String(Math.round(v))}
            />
          </Card>
        </div>
      )}
    </div>
  );
};
