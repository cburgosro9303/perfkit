import React, { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { FidelityPanel } from "../components/FidelityPanel";
import { PlanTree, resolveNode, type NodeId } from "../components/PlanTree";
import { RequestDetail } from "../components/RequestDetail";
import { StepEditor } from "../components/StepEditor";
import {
  Button,
  EmptyState,
  IconPlay,
  IconPlus,
  IconTrash,
  IconX,
  Spinner,
  Tabs,
} from "../components/ui";
import {
  addStep,
  addThreadGroup,
  canMove,
  deleteNode,
  duplicateNode,
  moveNode,
} from "../lib/mutate";
import { STEP_KIND_OPTIONS } from "../lib/scaffold";
import type {
  HttpRequest,
  MigrationReport,
  SampleDetail,
  Scenario,
  ThreadGroup,
  ValidationReport,
} from "../types";

interface PlanViewProps {
  scenario: Scenario;
  report: MigrationReport | null;
  yaml: string;
  onScenarioChange: (s: Scenario) => void;
}

type RightTab = "detail" | "fidelity" | "yaml";

const RIGHT_TABS = [
  { id: "detail", label: "Detalle" },
  { id: "fidelity", label: "Fidelidad" },
  { id: "yaml", label: "YAML" },
];

// ─── Validation badge ─────────────────────────────────────────────────────────

const ValidationBadge: React.FC<{ validation: ValidationReport | null; loading: boolean }> = ({
  validation,
  loading,
}) => {
  if (loading) {
    return (
      <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-slate-100 text-slate-500">
        <span className="w-2 h-2 rounded-full bg-slate-300 animate-pulse" />
        Validando…
      </span>
    );
  }
  if (!validation) return null;
  const errors = validation.issues.filter((i) => i.severity === "error");
  const warnings = validation.issues.filter((i) => i.severity === "warning");

  if (errors.length === 0 && warnings.length === 0) {
    return (
      <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-emerald-50 text-emerald-700 border border-emerald-200">
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12"/>
        </svg>
        Válido
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-red-50 text-red-700 border border-red-200">
      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/>
        <line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
      {errors.length} error{errors.length !== 1 ? "es" : ""}
      {warnings.length > 0 && `, ${warnings.length} aviso${warnings.length !== 1 ? "s" : ""}`}
    </span>
  );
};

// ─── Add-step dropdown menu ────────────────────────────────────────────────────

const AddStepMenu: React.FC<{
  disabled: boolean;
  onAdd: (kind: (typeof STEP_KIND_OPTIONS)[number]) => void;
}> = ({ disabled, onAdd }) => {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  return (
    <div className="relative" ref={ref}>
      <Button
        variant="primary"
        size="sm"
        icon={<IconPlus />}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        Añadir
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" className="ml-0.5">
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </Button>
      {open && (
        <div className="absolute left-0 top-full mt-1 z-20 w-44 rounded-lg border border-slate-200 bg-white shadow-lg py-1">
          {STEP_KIND_OPTIONS.map((opt) => (
            <button
              key={opt.kind}
              onClick={() => {
                onAdd(opt);
                setOpen(false);
              }}
              className="w-full text-left px-3 py-1.5 text-sm text-slate-700 hover:bg-indigo-50 hover:text-indigo-700 transition-colors"
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

// ─── Authoring toolbar ─────────────────────────────────────────────────────────

const AuthoringToolbar: React.FC<{
  scenario: Scenario;
  selectedId: NodeId;
  onScenarioChange: (s: Scenario) => void;
  onSelect: (id: NodeId) => void;
  httpStep: HttpRequest | null;
  probing: boolean;
  onProbe: (step: HttpRequest) => void;
}> = ({ scenario, selectedId, onScenarioChange, onSelect, httpStep, probing, onProbe }) => {
  const isRoot = selectedId.kind === "root";

  const apply = (res: { scenario: Scenario; select: NodeId }) => {
    onScenarioChange(res.scenario);
    onSelect(res.select);
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5 px-3 py-2 border-b border-slate-200 bg-white">
      <AddStepMenu
        disabled={false}
        onAdd={(opt) => apply(addStep(scenario, selectedId, opt.factory()))}
      />
      <Button
        variant="secondary"
        size="sm"
        onClick={() => apply(addThreadGroup(scenario))}
        title="Añadir un grupo de hilos al plan"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
          <circle cx="9" cy="7" r="4"/>
          <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
          <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
        </svg>
        Grupo de hilos
      </Button>

      <div className="w-px h-5 bg-slate-200 mx-0.5" />

      <Button
        variant="ghost"
        size="sm"
        disabled={isRoot}
        onClick={() => apply(duplicateNode(scenario, selectedId))}
        title="Duplicar"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
        </svg>
        Duplicar
      </Button>
      <Button
        variant="ghost"
        size="sm"
        disabled={isRoot || !canMove(scenario, selectedId, "up")}
        onClick={() => apply(moveNode(scenario, selectedId, "up"))}
        title="Subir"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <line x1="12" y1="19" x2="12" y2="5"/>
          <polyline points="5 12 12 5 19 12"/>
        </svg>
        Subir
      </Button>
      <Button
        variant="ghost"
        size="sm"
        disabled={isRoot || !canMove(scenario, selectedId, "down")}
        onClick={() => apply(moveNode(scenario, selectedId, "down"))}
        title="Bajar"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <polyline points="19 12 12 19 5 12"/>
        </svg>
        Bajar
      </Button>
      <Button
        variant="ghost"
        size="sm"
        disabled={isRoot}
        onClick={() => apply(deleteNode(scenario, selectedId))}
        title="Eliminar"
        className="text-slate-500 hover:text-red-600"
      >
        <IconTrash />
        Eliminar
      </Button>

      {httpStep && (
        <>
          <div className="w-px h-5 bg-slate-200 mx-0.5" />
          <Button
            variant="primary"
            size="sm"
            icon={probing ? <Spinner size={13} /> : <IconPlay />}
            disabled={probing}
            onClick={() => onProbe(httpStep)}
            title="Ejecutar esta petición de forma aislada (1 VU)"
          >
            Probar petición
          </Button>
        </>
      )}
    </div>
  );
};

// ─── Probe result modal (resultado de "Probar petición") ────────────────────────

const ProbeModal: React.FC<{
  probing: boolean;
  result: SampleDetail | null;
  requestName: string;
  onClose: () => void;
}> = ({ probing, result, requestName, onClose }) => (
  <div
    className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
    onClick={onClose}
    role="dialog"
    aria-modal="true"
    aria-label={`Resultado de la prueba — ${requestName}`}
  >
    <div
      className="bg-white rounded-xl border border-slate-200 shadow-xl max-w-2xl w-full max-h-[85vh] flex flex-col"
      onClick={(e) => e.stopPropagation()}
    >
      {/* Header (sticky dentro de la tarjeta) */}
      <div className="sticky top-0 z-10 flex items-center justify-between gap-3 px-4 py-3 border-b border-slate-200 bg-white rounded-t-xl">
        <span className="text-sm font-semibold text-slate-800 truncate">
          Resultado de la prueba — <span className="font-mono text-slate-600">{requestName}</span>
        </span>
        <button
          type="button"
          onClick={onClose}
          className="shrink-0 p-1 rounded text-slate-400 hover:text-slate-700 hover:bg-slate-100 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
          aria-label="Cerrar"
        >
          <IconX />
        </button>
      </div>

      {/* Body (scrollable) */}
      <div className="flex-1 overflow-y-auto p-4">
        <p className="text-[11px] text-slate-400 mb-3">
          Prueba aislada de 1 petición — las variables de pasos anteriores no están disponibles.
        </p>
        {probing ? (
          <div className="flex items-center gap-2 text-sm text-slate-500 py-10 justify-center">
            <Spinner size={16} />
            Ejecutando…
          </div>
        ) : result ? (
          <RequestDetail detail={result} />
        ) : (
          <p className="text-sm text-slate-400 italic py-10 text-center">Sin respuesta capturada</p>
        )}
      </div>
    </div>
  </div>
);

// ─── PlanView ─────────────────────────────────────────────────────────────────

export const PlanView: React.FC<PlanViewProps> = ({
  scenario,
  report,
  yaml,
  onScenarioChange,
}) => {
  const [selectedId, setSelectedId] = useState<NodeId>({ kind: "root" });
  const [rightTab, setRightTab] = useState<RightTab>("detail");
  const [validation, setValidation] = useState<ValidationReport | null>(null);
  const [validating, setValidating] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [probing, setProbing] = useState(false);
  const [probeResult, setProbeResult] = useState<SampleDetail | null>(null);
  const [probeOpen, setProbeOpen] = useState(false);
  const [probeName, setProbeName] = useState("");

  const doExport = useCallback(
    async (format: "yaml" | "json" | "jmx" | "pkb") => {
      try {
        const res = await api.exportScenario(scenario, format);
        if (res === "__native_only__") {
          setToast(`Exportar ${format.toUpperCase()} está disponible en la app nativa (perfkit tauri).`);
        } else if (res) {
          setToast(`Exportado a ${res}`);
        }
      } catch (e) {
        setToast(`Error al exportar: ${e}`);
      }
      setTimeout(() => setToast(null), 4000);
    },
    [scenario],
  );

  const validate = useCallback(async (s: Scenario) => {
    setValidating(true);
    try {
      const v = await api.validate(s);
      setValidation(v);
    } finally {
      setValidating(false);
    }
  }, []);

  useEffect(() => {
    void validate(scenario);
  }, [scenario, validate]);

  const resolved = resolveNode(scenario, selectedId);
  // Si la selección quedó "huérfana" tras un borrado, vuelve a root.
  useEffect(() => {
    if (!resolved) setSelectedId({ kind: "root" });
  }, [resolved]);

  // Cambiar de nodo cierra el resultado de "Probar petición": no debe persistir
  // entre selecciones. La clave serializada es estable (orden de campos fijo).
  const selectedKey = JSON.stringify(selectedId);
  useEffect(() => {
    setProbeOpen(false);
    setProbeResult(null);
    setProbing(false);
  }, [selectedKey]);

  // Solo los pasos HTTP son "probables" de forma aislada.
  const httpStep: HttpRequest | null =
    resolved?.kind === "step" && resolved.step.type === "http" ? resolved.step : null;

  const handleProbe = useCallback(
    async (step: HttpRequest) => {
      setProbeName(step.name);
      setProbing(true);
      setProbeOpen(true);
      setProbeResult(null);

      const probeGroup: ThreadGroup = {
        name: "Prueba",
        load: {
          virtual_users: 1,
          ramp_up_secs: 0,
          hold_secs: 0,
          ramp_down_secs: 0,
          iterations: 1,
          duration_secs: null,
        },
        on_error: "continue",
        steps: [step],
      };
      const temp: Scenario = { ...scenario, thread_groups: [probeGroup] };

      try {
        await api.run(
          temp,
          { capture: true, vus: 1 },
          () => {},
          (summary) => {
            setProbeResult(summary.details?.[0] ?? null);
            setProbing(false);
          },
        );
      } catch (e) {
        setProbeResult(null);
        setProbing(false);
        setToast(`Error al probar la petición: ${e}`);
        setTimeout(() => setToast(null), 4000);
      }
    },
    [scenario],
  );

  return (
    <div className="flex h-full overflow-hidden">
      {toast && (
        <div className="fixed bottom-4 right-4 z-50 max-w-sm px-4 py-2.5 rounded-lg bg-slate-900 text-white text-sm shadow-lg">
          {toast}
        </div>
      )}
      {probeOpen && (
        <ProbeModal
          probing={probing}
          result={probeResult}
          requestName={probeName}
          onClose={() => setProbeOpen(false)}
        />
      )}
      {/* Left: Plan tree */}
      <div className="w-60 shrink-0 border-r border-slate-200 bg-slate-50/60 flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-3 py-2.5 border-b border-slate-200 shrink-0">
          <span className="text-xs font-semibold text-slate-500 uppercase tracking-wide">Plan</span>
          <ValidationBadge validation={validation} loading={validating} />
        </div>
        <div className="flex items-center gap-1 px-3 py-1.5 border-b border-slate-200 bg-white">
          <span className="text-[11px] text-slate-400 mr-0.5">Exportar:</span>
          {(["jmx", "yaml", "pkb", "json"] as const).map((f) => (
            <button
              key={f}
              onClick={() => doExport(f)}
              title={f === "jmx" ? "Apache JMeter" : f === "pkb" ? "binario compacto perfkit" : f}
              className="text-[10px] font-semibold uppercase px-1.5 py-0.5 rounded border border-slate-200 text-slate-600 hover:bg-indigo-50 hover:text-indigo-700 hover:border-indigo-200 transition-colors"
            >
              {f}
            </button>
          ))}
        </div>
        <div className="flex-1 overflow-y-auto">
          <PlanTree
            scenario={scenario}
            selectedId={selectedId}
            onSelect={setSelectedId}
          />
        </div>
      </div>

      {/* Right: tabbed panels */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Authoring toolbar (only meaningful in the Detail tab) */}
        {rightTab === "detail" && (
          <AuthoringToolbar
            scenario={scenario}
            selectedId={selectedId}
            onScenarioChange={onScenarioChange}
            onSelect={setSelectedId}
            httpStep={httpStep}
            probing={probing}
            onProbe={handleProbe}
          />
        )}

        <div className="shrink-0 border-b border-slate-200 bg-white">
          <Tabs
            tabs={RIGHT_TABS}
            active={rightTab}
            onChange={(id) => setRightTab(id as RightTab)}
          />
        </div>

        <div className="flex-1 overflow-y-auto bg-white">
          {rightTab === "detail" && (
            <StepEditor
              selected={resolved}
              scenario={scenario}
              onScenarioChange={onScenarioChange}
            />
          )}

          {rightTab === "fidelity" && (
            <div className="p-5">
              {report ? (
                <FidelityPanel report={report} />
              ) : (
                <EmptyState
                  icon={
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                      <polyline points="14 2 14 8 20 8"/>
                    </svg>
                  }
                  title="Plan nativo (creado en perfkit)"
                  description="Sin reporte de fidelidad — eso es solo para importaciones JMX."
                />
              )}
            </div>
          )}

          {rightTab === "yaml" && (
            <div className="p-5">
              <div className="rounded-xl border border-slate-200 bg-slate-950 overflow-auto">
                <div className="flex items-center justify-between px-4 py-2 border-b border-slate-800">
                  <span className="text-xs font-medium text-slate-400 font-mono">
                    {scenario.name}.yaml
                  </span>
                  <span className="text-xs text-slate-600">Solo lectura</span>
                </div>
                {yaml ? (
                  <pre className="p-4 text-xs font-mono text-slate-200 leading-relaxed whitespace-pre-wrap overflow-x-auto">
                    {yaml}
                  </pre>
                ) : (
                  <pre className="p-4 text-xs font-mono text-slate-500 leading-relaxed whitespace-pre-wrap">
                    # La vista previa de YAML está disponible al importar un JMX o
                    # exportando el plan. Usa "Exportar: yaml" para generar el archivo.
                  </pre>
                )}
              </div>

              {validation && validation.issues.length > 0 && (
                <div className="mt-4 flex flex-col gap-2">
                  {validation.issues.map((issue, i) => (
                    <div
                      key={i}
                      className={`flex items-start gap-2 px-3 py-2.5 rounded-lg text-sm ${
                        issue.severity === "error"
                          ? "bg-red-50 border border-red-200 text-red-700"
                          : "bg-amber-50 border border-amber-200 text-amber-700"
                      }`}
                    >
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="mt-0.5 shrink-0">
                        <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                        <line x1="12" y1="9" x2="12" y2="13"/>
                        <line x1="12" y1="17" x2="12.01" y2="17"/>
                      </svg>
                      <div>
                        <code className="text-[10px] font-mono bg-black/10 px-1 rounded">{issue.path}</code>
                        <p className="mt-0.5">{issue.message}</p>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
