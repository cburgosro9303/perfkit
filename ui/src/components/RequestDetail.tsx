import React, { useState } from "react";
import { Badge, IconChevronDown, IconChevronRight } from "./ui";
import type { BadgeColor } from "./ui";
import { fmtBytes, fmtMs } from "../lib/format";
import type { SampleDetail } from "../types";

// ─── Status helpers ────────────────────────────────────────────────────────────

/** Color del badge de estado: 2xx verde, 3xx slate, 4xx ámbar, 5xx/err rojo. */
export function statusColor(d: SampleDetail): BadgeColor {
  if (!d.success || d.error) return "red";
  const s = d.status;
  if (s === undefined) return "slate";
  if (s >= 500) return "red";
  if (s >= 400) return "amber";
  if (s >= 300) return "slate";
  if (s >= 200) return "green";
  return "slate";
}

export function statusLabel(d: SampleDetail): string {
  if (d.status !== undefined) return String(d.status);
  return d.error ? "ERR" : "—";
}

const methodChipCls: Record<string, string> = {
  GET: "bg-blue-50 text-blue-700 border-blue-200",
  POST: "bg-emerald-50 text-emerald-700 border-emerald-200",
  PUT: "bg-amber-50 text-amber-700 border-amber-200",
  PATCH: "bg-amber-50 text-amber-700 border-amber-200",
  DELETE: "bg-red-50 text-red-700 border-red-200",
};

export const MethodChip: React.FC<{ method: string }> = ({ method }) => {
  const m = method.toUpperCase();
  const cls = methodChipCls[m] ?? "bg-slate-100 text-slate-600 border-slate-200";
  return (
    <span className={`inline-flex items-center px-1.5 py-0.5 text-[10px] font-bold rounded border tabular-nums ${cls}`}>
      {m}
    </span>
  );
};

/** Tabla clave/valor (headers, variables) en monospace. */
const KvTable: React.FC<{ rows: [string, string][] }> = ({ rows }) => {
  if (rows.length === 0) {
    return <p className="text-xs text-slate-400 italic">— ninguno —</p>;
  }
  return (
    <div className="rounded-lg border border-slate-200 overflow-hidden">
      <table className="w-full text-xs font-mono">
        <tbody>
          {rows.map(([k, v], i) => (
            <tr key={`${k}-${i}`} className={`border-b border-slate-100 last:border-0 ${i % 2 === 1 ? "bg-slate-50/50" : "bg-white"}`}>
              <td className="px-2.5 py-1.5 align-top text-slate-500 font-semibold whitespace-nowrap w-1 max-w-[40%]">
                <span className="break-all">{k}</span>
              </td>
              <td className="px-2.5 py-1.5 align-top text-slate-700 break-all">{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

/** Bloque <pre> monospace y scrollable para cuerpos (request/response). */
const BodyBlock: React.FC<{ body: string }> = ({ body }) => (
  <pre className="text-xs font-mono text-slate-700 bg-slate-50 border border-slate-200 rounded-lg p-3 overflow-auto max-h-72 whitespace-pre-wrap break-all">
    {body}
  </pre>
);

const SectionLabel: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <p className="text-[11px] font-semibold text-slate-500 uppercase tracking-wide mb-1.5">{children}</p>
);

// ─── RequestDetail (request/response/headers/body/vars/extracted/error) ─────────

/** Detalle completo de una petición capturada. Se usa en el panel de
 *  Peticiones del reporte y en el resultado de "Probar petición" del plan. */
export const RequestDetail: React.FC<{ detail: SampleDetail }> = ({ detail }) => {
  const [showRespHeaders, setShowRespHeaders] = useState(false);
  const color = statusColor(detail);
  const vars = detail.vars ?? [];

  return (
    <div className="flex flex-col gap-4">
      {/* Request */}
      <div>
        <SectionLabel>Request</SectionLabel>
        <div className="flex items-center gap-2 mb-2 text-xs font-mono text-slate-700 break-all">
          <MethodChip method={detail.method} />
          <span className="break-all">{detail.url}</span>
        </div>
        <p className="text-[11px] font-medium text-slate-400 mb-1">Headers</p>
        <KvTable rows={detail.req_headers} />
        {detail.req_body !== undefined && detail.req_body !== "" && (
          <div className="mt-2">
            <p className="text-[11px] font-medium text-slate-400 mb-1">Body</p>
            <BodyBlock body={detail.req_body} />
          </div>
        )}
      </div>

      {/* Response */}
      <div>
        <SectionLabel>Response</SectionLabel>
        <div className="flex items-center gap-2 mb-2">
          <Badge color={color}>{statusLabel(detail)}</Badge>
          <span className="text-xs text-slate-500 tabular-nums">
            {fmtMs(detail.latency_ms)} · {fmtBytes(detail.bytes)}
          </span>
        </div>
        <button
          type="button"
          onClick={() => setShowRespHeaders((s) => !s)}
          className="flex items-center gap-1 text-[11px] font-medium text-slate-500 hover:text-slate-700 mb-1 focus-visible:outline-none"
        >
          <span className="text-slate-400">{showRespHeaders ? <IconChevronDown /> : <IconChevronRight />}</span>
          Headers ({detail.resp_headers.length})
        </button>
        {showRespHeaders && <KvTable rows={detail.resp_headers} />}
        <div className="mt-2">
          <p className="text-[11px] font-medium text-slate-400 mb-1">Body</p>
          <BodyBlock body={detail.resp_body} />
        </div>
      </div>

      {/* Variables en este momento */}
      {vars.length > 0 && (
        <div>
          <SectionLabel>Variables en este momento</SectionLabel>
          <KvTable rows={vars} />
        </div>
      )}

      {/* Variables extraídas */}
      {detail.extracted.length > 0 && (
        <div>
          <SectionLabel>Variables extraídas</SectionLabel>
          <KvTable rows={detail.extracted} />
        </div>
      )}

      {/* Error */}
      {detail.error && (
        <div>
          <SectionLabel>Error</SectionLabel>
          <p className="text-xs font-mono text-red-600 bg-red-50 border border-red-100 rounded-lg p-2.5 break-all">
            {detail.error}
          </p>
        </div>
      )}
    </div>
  );
};
