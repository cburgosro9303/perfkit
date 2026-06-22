import React, { useRef, useState } from "react";
import { api } from "../api";
import type { ImportResult } from "../types";
import { Button, Spinner } from "../components/ui";

interface ImportViewProps {
  onImported: (result: ImportResult) => void;
  onNew: () => void;
  onLoadResults: () => void;
}

export const ImportView: React.FC<ImportViewProps> = ({ onImported, onNew, onLoadResults }) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const handleImport = async () => {
    setError(null);
    try {
      setLoading(true);
      if (api.isTauri) {
        const path = await api.pickJmx();
        if (!path) return;
        const result = await api.importJmxPath(path);
        onImported(result);
      } else {
        fileRef.current?.click();
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Error al importar el archivo");
    } finally {
      setLoading(false);
    }
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setError(null);
    setLoading(true);
    try {
      const text = await file.text();
      const result = await api.importJmxContent(file.name, text);
      onImported(result);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Error al leer el archivo");
    } finally {
      setLoading(false);
      // Reset input so the same file can be chosen again
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const handleExample = async () => {
    setError(null);
    setLoading(true);
    try {
      const result = await api.exampleImport();
      onImported(result);
    } catch (err: unknown) {
      setError(
        `Error al cargar el ejemplo: ${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setLoading(false);
    }
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    const file = e.dataTransfer.files[0];
    if (!file) return;
    if (!file.name.endsWith(".jmx")) {
      setError("Solo se aceptan archivos .jmx");
      return;
    }
    setError(null);
    setLoading(true);
    try {
      const text = await file.text();
      const result = await api.importJmxContent(file.name, text);
      onImported(result);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Error al leer el archivo");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col items-center justify-center h-full gap-8 px-8">
      {/* Primary: create a plan from scratch */}
      <button
        onClick={onNew}
        className="w-full max-w-lg flex items-center gap-4 rounded-2xl border border-indigo-200 bg-indigo-50/70 hover:bg-indigo-50 hover:border-indigo-300 transition-colors p-5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
      >
        <div className="w-12 h-12 rounded-xl bg-indigo-600 flex items-center justify-center shrink-0 shadow-sm">
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </div>
        <div className="min-w-0">
          <p className="text-base font-semibold text-slate-900">Crear plan nuevo</p>
          <p className="text-sm text-slate-500 mt-0.5">
            Empezar de cero con la estructura nativa de perfkit
          </p>
        </div>
      </button>

      {/* Separator */}
      <div className="flex items-center gap-3 w-full max-w-lg">
        <div className="h-px flex-1 bg-slate-200" />
        <span className="text-xs text-slate-400">o importa un plan existente</span>
        <div className="h-px flex-1 bg-slate-200" />
      </div>

      {/* Drop zone */}
      <div
        className={`w-full max-w-lg flex flex-col items-center justify-center gap-5 rounded-2xl border-2 border-dashed transition-all p-14 cursor-pointer select-none ${
          dragging
            ? "border-indigo-500 bg-indigo-50 scale-[1.01]"
            : "border-slate-200 bg-white hover:border-indigo-400 hover:bg-indigo-50/50"
        }`}
        onClick={() => !loading && handleImport()}
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
      >
        {/* Upload icon */}
        <div className={`w-16 h-16 rounded-2xl flex items-center justify-center transition-colors ${
          dragging ? "bg-indigo-100" : "bg-slate-100"
        }`}>
          {loading ? (
            <Spinner size={28} />
          ) : (
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke={dragging ? "#6366f1" : "#94a3b8"} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
              <polyline points="17 8 12 3 7 8"/>
              <line x1="12" y1="3" x2="12" y2="15"/>
            </svg>
          )}
        </div>

        <div className="text-center">
          <p className="text-base font-semibold text-slate-800">
            {loading ? "Importando…" : dragging ? "Suelta aquí el archivo .jmx" : "Importar archivo .jmx"}
          </p>
          <p className="text-sm text-slate-500 mt-1">
            {api.isTauri
              ? "Haz clic para abrir el explorador de archivos"
              : "Haz clic para seleccionar o arrastra un archivo"}
          </p>
          {!loading && (
            <p className="text-xs text-slate-400 mt-2">Solo archivos .jmx de Apache JMeter</p>
          )}
        </div>
      </div>

      {/* Hidden file input for browser */}
      {!api.isTauri && (
        <input
          ref={fileRef}
          type="file"
          accept=".jmx"
          className="hidden"
          onChange={handleFileChange}
        />
      )}

      {/* Error */}
      {error && (
        <div className="flex items-center gap-2 px-4 py-3 bg-red-50 border border-red-200 rounded-xl text-red-700 text-sm max-w-lg w-full">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
            <circle cx="12" cy="12" r="10"/>
            <line x1="15" y1="9" x2="9" y2="15"/>
            <line x1="9" y1="9" x2="15" y2="15"/>
          </svg>
          {error}
        </div>
      )}

      {/* Secondary: open example */}
      <div className="flex items-center gap-3">
        <div className="h-px w-16 bg-slate-200" />
        <span className="text-xs text-slate-400">o</span>
        <div className="h-px w-16 bg-slate-200" />
      </div>

      <Button
        variant="ghost"
        onClick={handleExample}
        disabled={loading}
        className="text-slate-500"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10"/>
          <line x1="12" y1="8" x2="12" y2="12"/>
          <line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        {api.isTauri
          ? "Abrir un ejemplo ejecutable (httpbin.org)"
          : "Abrir un ejemplo (demo simulado)"}
      </Button>

      {/* Cargar resultados de una corrida hecha por terminal */}
      <div className="flex flex-col items-center gap-1.5">
        <Button
          variant="ghost"
          onClick={onLoadResults}
          disabled={loading}
          className="text-slate-500"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
          </svg>
          Cargar resultados de una corrida (summary.json)
        </Button>
        <p className="text-xs text-slate-400 text-center max-w-sm">
          Visualiza resultados de una corrida hecha por terminal (perfkit run --out).
        </p>
      </div>
    </div>
  );
};
