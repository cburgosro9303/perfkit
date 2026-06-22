import React, { useState } from "react";
import { api } from "./api";
import {
  Button,
  IconHelp,
  IconHistory,
  IconHome,
  IconPlan,
  IconPlus,
  IconReport,
  IconRun,
} from "./components/ui";
import { blankScenario } from "./lib/scaffold";
import type {
  ImportResult,
  MigrationReport,
  RunSummary,
  Scenario,
} from "./types";
import { HelpView } from "./views/HelpView";
import { HistoryView } from "./views/HistoryView";
import { ImportView } from "./views/ImportView";
import { PlanView } from "./views/PlanView";
import { ReportView } from "./views/ReportView";
import { RunView } from "./views/RunView";

// ─── View types ───────────────────────────────────────────────────────────────

type View = "import" | "plan" | "run" | "report" | "history" | "help";

interface NavItem {
  id: View;
  label: string;
  icon: React.ReactNode;
  requiresScenario: boolean;
}

// Flujo lineal del plan (también se muestra como pasos en la barra superior).
const STEP_ITEMS: NavItem[] = [
  { id: "import", label: "Inicio", icon: <IconHome />, requiresScenario: false },
  { id: "plan", label: "Plan", icon: <IconPlan />, requiresScenario: true },
  { id: "run", label: "Ejecutar", icon: <IconRun />, requiresScenario: true },
  { id: "report", label: "Reporte", icon: <IconReport />, requiresScenario: true },
];

// Vistas siempre disponibles (no dependen de un escenario, fuera del flujo lineal).
const UTILITY_ITEMS: NavItem[] = [
  { id: "history", label: "Histórico", icon: <IconHistory />, requiresScenario: false },
  { id: "help", label: "Ayuda", icon: <IconHelp />, requiresScenario: false },
];

// ─── Fidelity banner ─────────────────────────────────────────────────────────

interface FidelityBannerProps {
  report: MigrationReport;
  onViewDetail: () => void;
  onDismiss: () => void;
}

const FidelityBanner: React.FC<FidelityBannerProps> = ({
  report,
  onViewDetail,
  onDismiss,
}) => {
  const s = report.summary;
  const hasIssues = s.assisted > 0 || s.unsupported > 0;

  return (
    <div
      className={`flex items-center gap-3 px-4 py-2.5 text-sm border-b transition-colors ${
        hasIssues
          ? "bg-amber-50 border-amber-200 text-amber-800"
          : "bg-emerald-50 border-emerald-200 text-emerald-800"
      }`}
    >
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className="shrink-0"
      >
        {hasIssues ? (
          <>
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </>
        ) : (
          <polyline points="20 6 9 17 4 12" />
        )}
      </svg>

      <span className="flex-1">
        <span className="font-medium">{s.migrated} migrados</span>
        {s.assisted > 0 && (
          <span className="text-amber-700"> · {s.assisted} asistidos</span>
        )}
        {s.unsupported > 0 && (
          <span className="text-red-700"> · {s.unsupported} no soportados</span>
        )}
        {s.ignored > 0 && <span className="text-slate-500"> · {s.ignored} ignorados</span>}
        <span className="ml-1 font-semibold">
          · {s.fidelity_pct.toFixed(1)}% fidelidad
        </span>
      </span>

      {hasIssues && (
        <button
          onClick={onViewDetail}
          className="shrink-0 text-xs font-semibold underline underline-offset-2 hover:no-underline transition-all"
        >
          ver detalle
        </button>
      )}

      <button
        onClick={onDismiss}
        className="shrink-0 p-0.5 rounded hover:bg-black/10 transition-colors"
        aria-label="Cerrar"
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
        >
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>
  );
};

// ─── Left nav rail ────────────────────────────────────────────────────────────

const NavRailButton: React.FC<{
  item: NavItem;
  isActive: boolean;
  disabled: boolean;
  onNavigate: (v: View) => void;
}> = ({ item, isActive, disabled, onNavigate }) => (
  <button
    onClick={() => !disabled && onNavigate(item.id)}
    disabled={disabled}
    title={item.label}
    className={`group flex flex-col items-center gap-1 py-2.5 px-1 rounded-lg transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 ${
      isActive
        ? "bg-indigo-600 text-white"
        : disabled
        ? "text-slate-700 cursor-not-allowed"
        : "text-slate-400 hover:text-white hover:bg-slate-800"
    }`}
  >
    <span className="shrink-0">{item.icon}</span>
    <span className="text-[9px] font-medium uppercase tracking-wide leading-none">
      {item.label}
    </span>
  </button>
);

interface NavRailProps {
  activeView: View;
  onNavigate: (v: View) => void;
  hasScenario: boolean;
  hasReport: boolean;
  scenarioName?: string;
}

// Un ítem está bloqueado si requiere escenario y no hay; el Reporte se desbloquea
// además cuando hay un resumen cargado (p.ej. vía "Cargar resultados").
const isItemDisabled = (item: NavItem, hasScenario: boolean, hasReport: boolean) =>
  item.requiresScenario && !hasScenario && !(item.id === "report" && hasReport);

const NavRail: React.FC<NavRailProps> = ({
  activeView,
  onNavigate,
  hasScenario,
  hasReport,
  scenarioName,
}) => (
  <nav className="flex flex-col w-[72px] shrink-0 bg-slate-900 border-r border-slate-800 h-full">
    {/* Logo mark */}
    <div className="flex items-center justify-center h-14 border-b border-slate-800 shrink-0">
      <div className="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="white"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
        </svg>
      </div>
    </div>

    {/* Nav items */}
    <div className="flex flex-col gap-1 p-2 flex-1">
      {STEP_ITEMS.map((item) => (
        <NavRailButton
          key={item.id}
          item={item}
          isActive={activeView === item.id}
          disabled={isItemDisabled(item, hasScenario, hasReport)}
          onNavigate={onNavigate}
        />
      ))}

      {/* Separador hacia las vistas siempre disponibles */}
      <div className="my-1.5 mx-2 h-px bg-slate-800" />

      {UTILITY_ITEMS.map((item) => (
        <NavRailButton
          key={item.id}
          item={item}
          isActive={activeView === item.id}
          disabled={isItemDisabled(item, hasScenario, hasReport)}
          onNavigate={onNavigate}
        />
      ))}
    </div>

    {/* Bottom: env badge */}
    <div className="p-2 border-t border-slate-800 shrink-0">
      <div
        className={`text-[8px] font-medium uppercase tracking-wide text-center px-1 py-1.5 rounded ${
          api.isTauri
            ? "bg-emerald-900/50 text-emerald-400"
            : "bg-slate-800 text-slate-500"
        }`}
      >
        {api.isTauri ? "Nativa" : "Demo"}
      </div>
    </div>
  </nav>
);

// ─── App ──────────────────────────────────────────────────────────────────────

interface AppState {
  scenario: Scenario | null;
  report: MigrationReport | null;
  yaml: string;
  runSummary: RunSummary | null;
  view: View;
  showBanner: boolean;
}

const App: React.FC = () => {
  const [state, setState] = useState<AppState>({
    scenario: null,
    report: null,
    yaml: "",
    runSummary: null,
    view: "import",
    showBanner: false,
  });

  const handleImported = (result: ImportResult) => {
    setState((prev) => ({
      ...prev,
      scenario: result.scenario,
      report: result.report,
      yaml: result.yaml,
      runSummary: null,
      view: "plan",
      showBanner: true,
    }));
  };

  const handleNewScenario = () => {
    setState((prev) => ({
      ...prev,
      scenario: blankScenario(),
      report: null,
      yaml: "",
      runSummary: null,
      view: "plan",
      showBanner: false,
    }));
  };

  const handleScenarioChange = (scenario: Scenario) => {
    setState((prev) => ({ ...prev, scenario }));
  };

  const handleRunFinished = (summary: RunSummary) => {
    setState((prev) => ({
      ...prev,
      runSummary: summary,
      view: "report",
    }));
  };

  // Carga un summary.json hecho por terminal (perfkit run --out) y lo muestra en
  // el Reporte. Cancelar (null) se ignora en silencio; un error se notifica.
  const handleLoadSummary = async () => {
    try {
      const summary = await api.loadSummary();
      if (!summary) return;
      setState((prev) => ({
        ...prev,
        runSummary: summary,
        view: "report",
      }));
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("No se pudo cargar el resumen:", msg);
      alert(`No se pudo cargar el resumen:\n${msg}`);
    }
  };

  const handleNavigate = (view: View) => {
    setState((prev) => ({ ...prev, view }));
  };

  const handleViewFidelityDetail = () => {
    setState((prev) => ({ ...prev, view: "plan", showBanner: false }));
    // Give PlanView a moment to render before selecting fidelity tab
    // (handled via prop when needed — here we just navigate)
  };

  const { scenario, report, yaml, runSummary, view, showBanner } = state;

  return (
    <div className="flex h-screen overflow-hidden bg-slate-100">
      {/* Left nav rail */}
      <NavRail
        activeView={view}
        onNavigate={handleNavigate}
        hasScenario={scenario !== null}
        hasReport={runSummary !== null}
        scenarioName={scenario?.name}
      />

      {/* Main content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top bar */}
        <div className="flex items-center gap-3 px-5 h-14 border-b border-slate-200 bg-white shrink-0">
          <div className="flex-1 min-w-0">
            {scenario ? (
              <div className="flex items-center gap-2">
                <span className="text-sm font-semibold text-slate-900 truncate">
                  {scenario.name}
                </span>
                <span className="text-slate-300">·</span>
                <span className="text-xs text-slate-500">
                  {scenario.thread_groups.length} grupo
                  {scenario.thread_groups.length !== 1 ? "s" : ""}
                  {" · "}
                  {scenario.thread_groups.reduce((acc, g) => acc + g.steps.length, 0)} pasos
                </span>
              </div>
            ) : (
              <span className="text-sm text-slate-400 font-medium">perfkit Studio</span>
            )}
          </div>

          {/* Step indicators */}
          {scenario && (
            <div className="flex items-center gap-1.5">
              {STEP_ITEMS.map((item, i) => (
                <React.Fragment key={item.id}>
                  {i > 0 && (
                    <div className="w-8 h-px bg-slate-200" />
                  )}
                  <button
                    onClick={() => !item.requiresScenario || scenario ? handleNavigate(item.id) : undefined}
                    className={`flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-colors ${
                      view === item.id
                        ? "bg-indigo-50 text-indigo-700"
                        : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    <span>{item.icon}</span>
                    <span className="hidden sm:inline">{item.label}</span>
                  </button>
                </React.Fragment>
              ))}
            </div>
          )}

          {/* Always-reachable: new plan */}
          <button
            onClick={handleNewScenario}
            title="Crear un plan nuevo desde cero"
            className="shrink-0 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold bg-indigo-600 text-white hover:bg-indigo-700 active:bg-indigo-800 shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1"
          >
            <IconPlus />
            Nuevo plan
          </button>
        </div>

        {/* Fidelity banner */}
        {showBanner && report && (
          <FidelityBanner
            report={report}
            onViewDetail={handleViewFidelityDetail}
            onDismiss={() => setState((prev) => ({ ...prev, showBanner: false }))}
          />
        )}

        {/* View content */}
        <div className="flex-1 overflow-hidden">
          {view === "import" && (
            <ImportView
              onImported={handleImported}
              onNew={handleNewScenario}
              onLoadResults={handleLoadSummary}
            />
          )}

          {view === "plan" && scenario && (
            <PlanView
              scenario={scenario}
              report={report}
              yaml={yaml}
              onScenarioChange={handleScenarioChange}
            />
          )}

          {view === "run" && scenario && (
            <div className="h-full overflow-y-auto">
              <RunView scenario={scenario} onFinished={handleRunFinished} />
            </div>
          )}

          {view === "report" && runSummary && (
            <div className="h-full overflow-y-auto">
              <ReportView summary={runSummary} onLoadResults={handleLoadSummary} />
            </div>
          )}

          {view === "report" && !runSummary && (
            <div className="flex flex-col items-center justify-center h-full gap-4 text-center">
              <div className="w-14 h-14 rounded-full bg-slate-100 flex items-center justify-center">
                <IconReport />
              </div>
              <div>
                <p className="text-sm font-semibold text-slate-700">Sin reporte todavía</p>
                <p className="text-xs text-slate-400 mt-1">
                  Ejecuta el escenario o carga un summary.json de una corrida
                </p>
              </div>
              <div className="flex items-center gap-2">
                {scenario && (
                  <button
                    onClick={() => handleNavigate("run")}
                    className="text-sm text-indigo-600 font-medium hover:underline"
                  >
                    Ir a Ejecutar
                  </button>
                )}
                <Button variant="secondary" size="sm" onClick={handleLoadSummary}>
                  Cargar resultados (summary.json)
                </Button>
              </div>
            </div>
          )}

          {view === "history" && (
            <div className="h-full overflow-y-auto">
              <HistoryView currentSummary={runSummary} />
            </div>
          )}

          {view === "help" && <HelpView />}
        </div>
      </div>
    </div>
  );
};

export default App;
