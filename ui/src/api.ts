// Capa de API: usa los comandos Tauri reales dentro de la app nativa, y un mock
// con datos de muestra cuando corre en un navegador (para demo y screenshots).

import {
  mockEvaluateGate,
  mockHistoryCompare,
  mockHistoryList,
  mockHistoryRecord,
  mockHistorySetBaseline,
  mockHistoryTrend,
  mockImport,
  mockValidate,
  sampleScenario,
  simulateRun,
} from "./mock";
import type {
  Comparison,
  GateResult,
  GateThresholds,
  ImportResult,
  LiveSnapshot,
  RunOptions,
  RunRecord,
  RunSummary,
  Scenario,
  TrendMetric,
  TrendPoint,
  ValidationReport,
} from "./types";

export const isTauri =
  typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

function downloadBlob(content: string, name: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

export const api = {
  isTauri,

  /** Abre el diálogo nativo para elegir un .jmx (solo Tauri). */
  async pickJmx(): Promise<string | null> {
    if (!isTauri) return null;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({ multiple: false, filters: [{ name: "JMeter", extensions: ["jmx"] }] });
    return typeof res === "string" ? res : null;
  },

  /** Importa un .jmx desde una ruta del sistema (Tauri) o devuelve el mock (navegador). */
  async importJmxPath(path: string): Promise<ImportResult> {
    return isTauri ? invoke<ImportResult>("import_jmx", { path }) : mockImport(path);
  },

  /**
   * Carga un ejemplo. En la app nativa importa un JMX empaquetado **ejecutable**
   * (apunta a httpbin.org → se puede correr de verdad). En el navegador devuelve el
   * escenario de muestra simulado.
   */
  async exampleImport(): Promise<ImportResult> {
    return isTauri ? invoke<ImportResult>("example_import") : mockImport("checkout.jmx");
  },

  /** Importa un .jmx a partir de su contenido XML (drag & drop / file input). */
  async importJmxContent(name: string, xml: string): Promise<ImportResult> {
    return isTauri ? invoke<ImportResult>("import_jmx_content", { name, xml }) : mockImport(name);
  },

  async validate(scenario: Scenario): Promise<ValidationReport> {
    return isTauri ? invoke<ValidationReport>("validate_scenario", { scenario }) : mockValidate(scenario);
  },

  async newScenario(): Promise<Scenario> {
    if (isTauri) return invoke<Scenario>("new_scenario");
    return sampleScenario();
  },

  /**
   * Ejecuta un escenario. Llama a onMetrics con cada snapshot en vivo y a
   * onFinished con el resumen. Devuelve una función para cancelar/desuscribir.
   */
  async run(
    scenario: Scenario,
    opts: RunOptions,
    onMetrics: (s: LiveSnapshot) => void,
    onFinished: (s: RunSummary) => void,
  ): Promise<() => void> {
    if (isTauri) {
      const { listen } = await import("@tauri-apps/api/event");
      const un1 = await listen<LiveSnapshot>("run-metrics", (e) => onMetrics(e.payload));
      const un2 = await listen<RunSummary>("run-finished", (e) => {
        onFinished(e.payload);
      });
      await invoke("run_scenario", {
        scenario,
        baseUrl: opts.base_url_override ?? null,
        vus: opts.vus ?? null,
        durationSecs: opts.duration_secs ?? null,
        capture: opts.capture ?? false,
        capturePlaintext: opts.capture_plaintext ?? false,
        captureLimit: opts.capture_limit ?? 0,
      });
      return () => {
        un1();
        un2();
      };
    }
    return simulateRun(scenario, opts, onMetrics, onFinished);
  },

  async cancel(): Promise<void> {
    if (isTauri) await invoke("cancel_run");
  },

  /**
   * Exporta el escenario. En la app nativa abre un diálogo de guardado y escribe el
   * archivo (yaml/json/jmx/pkb). En navegador (demo) solo descarga JSON client-side.
   * Devuelve la ruta/resultado, null si se canceló, o "__native_only__" si el formato
   * requiere la app nativa.
   */
  async exportScenario(
    scenario: Scenario,
    format: "yaml" | "json" | "jmx" | "pkb",
  ): Promise<string | null> {
    const base = (scenario.name || "escenario").replace(/\s+/g, "_");
    if (isTauri) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const path = await save({
        defaultPath: `${base}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return null;
      await invoke("export_scenario", { scenario, format, path });
      return path as string;
    }
    if (format === "json") {
      downloadBlob(JSON.stringify(scenario, null, 2), `${base}.json`, "application/json");
      return "(descargado)";
    }
    return "__native_only__";
  },

  /** Exporta el reporte (HTML/JSON/JUnit) a un directorio (solo Tauri). */
  async exportReport(summary: RunSummary): Promise<string | null> {
    if (!isTauri) return null;
    return invoke<string>("export_report", { summary });
  },

  /**
   * Carga un `summary.json` producido por la CLI (`perfkit run --out <dir>`) o el
   * benchmark, y devuelve el RunSummary para visualizarlo en el Reporte. En la app
   * nativa abre el diálogo nativo y valida vía el comando `load_summary`; en el
   * navegador usa un file input y parsea el JSON en el cliente. Devuelve null si se
   * canceló o si el archivo no es un JSON válido.
   */
  async loadSummary(): Promise<RunSummary | null> {
    if (isTauri) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const res = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (typeof res !== "string") return null;
      return invoke<RunSummary>("load_summary", { path: res });
    }
    // browser: native file input → parse JSON client-side
    return new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = "application/json,.json";
      input.onchange = async () => {
        const f = input.files?.[0];
        if (!f) return resolve(null);
        try {
          resolve(JSON.parse(await f.text()) as RunSummary);
        } catch {
          resolve(null);
        }
      };
      input.click();
    });
  },

  /**
   * Histórico de corridas y comparación contra baseline. En la app nativa usa los
   * comandos `history_*`; en el navegador usa un store en memoria (sembrado) de mock.ts.
   */
  history: {
    /** Guarda el resumen actual como una corrida histórica; devuelve su id. */
    async record(args: {
      summary: RunSummary;
      branch?: string;
      build?: string;
      environment?: string;
      commit?: string;
    }): Promise<number> {
      if (isTauri) {
        return invoke<number>("history_record", {
          summary: args.summary,
          branch: args.branch ?? null,
          build: args.build ?? null,
          environment: args.environment ?? null,
          commit: args.commit ?? null,
        });
      }
      return mockHistoryRecord(args);
    },

    /** Lista corridas históricas (filtros opcionales). */
    async list(args: {
      scenario?: string;
      environment?: string;
      limit?: number;
    } = {}): Promise<RunRecord[]> {
      if (isTauri) {
        return invoke<RunRecord[]>("history_list", {
          scenario: args.scenario ?? null,
          environment: args.environment ?? null,
          limit: args.limit ?? null,
        });
      }
      return mockHistoryList(args);
    },

    /** Fija una corrida como baseline para (branch, entorno, escenario). */
    async setBaseline(args: {
      branch: string;
      environment: string;
      scenario: string;
      runId: number;
    }): Promise<void> {
      if (isTauri) {
        await invoke("history_set_baseline", {
          branch: args.branch,
          environment: args.environment,
          scenario: args.scenario,
          runId: args.runId,
        });
        return;
      }
      mockHistorySetBaseline(args);
    },

    /** Compara una corrida contra el baseline fijado; null si no hay baseline. */
    async compare(args: {
      runId: number;
      branch: string;
      environment: string;
      scenario: string;
    }): Promise<Comparison | null> {
      if (isTauri) {
        return invoke<Comparison | null>("history_compare", {
          runId: args.runId,
          branch: args.branch,
          environment: args.environment,
          scenario: args.scenario,
        });
      }
      return mockHistoryCompare(args);
    },

    /** Serie de tendencia de una métrica a lo largo de las corridas históricas. */
    async trend(args: {
      scenario: string;
      environment: string;
      metric: TrendMetric;
      limit?: number;
    }): Promise<TrendPoint[]> {
      if (isTauri) {
        return invoke<TrendPoint[]>("history_trend", {
          scenario: args.scenario,
          environment: args.environment,
          metric: args.metric,
          limit: args.limit ?? null,
        });
      }
      return mockHistoryTrend(args);
    },
  },

  /** Evalúa el quality gate (umbrales SLA) sobre un resumen. */
  async evaluateGate(args: {
    summary: RunSummary;
    thresholds: GateThresholds;
  }): Promise<GateResult> {
    if (isTauri) {
      return invoke<GateResult>("evaluate_gate", {
        summary: args.summary,
        thresholds: args.thresholds,
      });
    }
    return mockEvaluateGate(args);
  },
};
