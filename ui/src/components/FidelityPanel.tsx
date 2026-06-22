import React, { useState } from "react";
import type { MappedElement, MappingStatus, MigrationReport } from "../types";
import { Badge } from "./ui";

// ─── Status config ────────────────────────────────────────────────────────────

const STATUS_CONFIG: Record<
  MappingStatus,
  { label: string; color: "green" | "amber" | "red" | "slate"; dot: string }
> = {
  migrated: { label: "Migrado", color: "green", dot: "bg-emerald-500" },
  assisted: { label: "Asistido", color: "amber", dot: "bg-amber-500" },
  unsupported: { label: "No soportado", color: "red", dot: "bg-red-500" },
  ignored: { label: "Ignorado", color: "slate", dot: "bg-slate-400" },
};

// ─── Donut chart ─────────────────────────────────────────────────────────────

interface DonutProps {
  migrated: number;
  assisted: number;
  unsupported: number;
  ignored: number;
  total: number;
  pct: number;
}

const DonutChart: React.FC<DonutProps> = ({
  migrated,
  assisted,
  unsupported,
  ignored,
  total,
  pct,
}) => {
  const size = 80;
  const r = 30;
  const cx = size / 2;
  const cy = size / 2;
  const strokeW = 10;

  const segments = [
    { value: migrated, color: "#10b981" },  // emerald
    { value: assisted, color: "#f59e0b" },  // amber
    { value: unsupported, color: "#ef4444" }, // red
    { value: ignored, color: "#94a3b8" },    // slate
  ];

  const circumference = 2 * Math.PI * r;
  let offset = 0;

  const arcs = segments.map((seg) => {
    const frac = total > 0 ? seg.value / total : 0;
    const len = frac * circumference;
    const arc = {
      ...seg,
      dasharray: `${len} ${circumference - len}`,
      dashoffset: -offset,
    };
    offset += len;
    return arc;
  });

  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="shrink-0">
      {/* Background ring */}
      <circle
        cx={cx} cy={cy} r={r}
        fill="none"
        stroke="#e2e8f0"
        strokeWidth={strokeW}
      />
      {/* Segments */}
      {arcs.map((arc, i) => (
        <circle
          key={i}
          cx={cx} cy={cy} r={r}
          fill="none"
          stroke={arc.color}
          strokeWidth={strokeW}
          strokeDasharray={arc.dasharray}
          strokeDashoffset={arc.dashoffset}
          transform={`rotate(-90 ${cx} ${cy})`}
          style={{ transition: "stroke-dasharray 0.5s ease" }}
        />
      ))}
      {/* Center label */}
      <text
        x={cx} y={cy - 3}
        textAnchor="middle"
        fontSize="13"
        fontWeight="700"
        fill="#0f172a"
        fontFamily="ui-monospace, monospace"
      >
        {Math.round(pct)}%
      </text>
      <text
        x={cx} y={cy + 10}
        textAnchor="middle"
        fontSize="7"
        fill="#64748b"
        fontFamily="system-ui"
      >
        FIDELIDAD
      </text>
    </svg>
  );
};

// ─── Element row ─────────────────────────────────────────────────────────────

const ElementRow: React.FC<{ el: MappedElement; isOdd: boolean }> = ({ el, isOdd }) => {
  const cfg = STATUS_CONFIG[el.status];
  const needsAttention = el.status === "assisted" || el.status === "unsupported";

  return (
    <tr
      className={`border-b border-slate-100 last:border-0 transition-colors ${
        isOdd ? "bg-slate-50/50" : "bg-white"
      } ${needsAttention ? "hover:bg-amber-50/60" : "hover:bg-slate-50"}`}
    >
      <td className="px-3 py-2.5 w-8">
        <span
          className={`inline-block w-2 h-2 rounded-full ${cfg.dot}`}
          title={cfg.label}
        />
      </td>
      <td className="px-2 py-2.5">
        <Badge color={cfg.color}>{cfg.label}</Badge>
      </td>
      <td className="px-3 py-2.5">
        <span className="text-xs font-mono text-slate-600 bg-slate-100 px-1.5 py-0.5 rounded">
          {el.jmx_type}
        </span>
      </td>
      <td className="px-3 py-2.5">
        <span className="text-sm font-medium text-slate-800">{el.jmx_name}</span>
      </td>
      <td className="px-3 py-2.5 hidden md:table-cell">
        <span className="text-xs text-slate-400 font-mono truncate max-w-[200px] block">
          {el.path}
        </span>
      </td>
      <td className="px-3 py-2.5">
        {(el.reason || el.suggestion) && (
          <div className="flex flex-col gap-0.5">
            {el.reason && (
              <p className="text-xs text-slate-600 leading-snug">{el.reason}</p>
            )}
            {el.suggestion && (
              <p className="text-xs text-amber-700 leading-snug italic">{el.suggestion}</p>
            )}
          </div>
        )}
      </td>
    </tr>
  );
};

// ─── Filter bar ──────────────────────────────────────────────────────────────

type FilterStatus = MappingStatus | "all" | "attention";

const FILTER_OPTIONS: { id: FilterStatus; label: string }[] = [
  { id: "all", label: "Todos" },
  { id: "attention", label: "Requieren atención" },
  { id: "migrated", label: "Migrado" },
  { id: "assisted", label: "Asistido" },
  { id: "unsupported", label: "No soportado" },
  { id: "ignored", label: "Ignorado" },
];

// ─── FidelityPanel (main export) ─────────────────────────────────────────────

interface FidelityPanelProps {
  report: MigrationReport;
}

export const FidelityPanel: React.FC<FidelityPanelProps> = ({ report }) => {
  const [filter, setFilter] = useState<FilterStatus>("all");

  const { summary, elements } = report;

  const filtered = elements.filter((el) => {
    if (filter === "all") return true;
    if (filter === "attention") return el.status === "assisted" || el.status === "unsupported";
    return el.status === filter;
  });

  const attentionCount =
    elements.filter((e) => e.status === "assisted" || e.status === "unsupported").length;

  return (
    <div className="flex flex-col gap-5">
      {/* Summary header */}
      <div className="flex items-center gap-5 p-4 rounded-xl border border-slate-200 bg-white">
        <DonutChart
          migrated={summary.migrated}
          assisted={summary.assisted}
          unsupported={summary.unsupported}
          ignored={summary.ignored}
          total={summary.total}
          pct={summary.fidelity_pct}
        />

        <div className="flex flex-wrap gap-x-6 gap-y-3 flex-1">
          <Stat value={summary.total} label="Total" />
          <Stat value={summary.migrated} label="Migrados" color="text-emerald-600" />
          {summary.assisted > 0 && (
            <Stat value={summary.assisted} label="Asistidos" color="text-amber-600" />
          )}
          {summary.unsupported > 0 && (
            <Stat value={summary.unsupported} label="No soportados" color="text-red-600" />
          )}
          {summary.ignored > 0 && (
            <Stat value={summary.ignored} label="Ignorados" color="text-slate-500" />
          )}
        </div>

        {attentionCount > 0 && (
          <div className="shrink-0 flex items-center gap-1.5 px-3 py-2 rounded-lg bg-amber-50 border border-amber-200 text-amber-700 text-sm font-medium">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
              <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
            </svg>
            {attentionCount} requieren revisión
          </div>
        )}
      </div>

      {report.notes && report.notes.length > 0 && (
        <div className="flex flex-col gap-1 px-3 py-2.5 rounded-lg bg-blue-50 border border-blue-100">
          {report.notes.map((n, i) => (
            <p key={i} className="text-xs text-blue-700">{n}</p>
          ))}
        </div>
      )}

      {/* Filter tabs */}
      <div className="flex gap-1 flex-wrap">
        {FILTER_OPTIONS.map((opt) => (
          <button
            key={opt.id}
            onClick={() => setFilter(opt.id)}
            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors ${
              filter === opt.id
                ? "bg-indigo-600 text-white"
                : "bg-white border border-slate-200 text-slate-600 hover:bg-slate-50"
            }`}
          >
            {opt.label}
            {opt.id === "attention" && attentionCount > 0 && (
              <span className={`ml-1.5 text-[10px] font-bold px-1 rounded ${
                filter === "attention" ? "bg-white/20 text-white" : "bg-amber-100 text-amber-700"
              }`}>
                {attentionCount}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Elements table */}
      <div className="rounded-xl border border-slate-200 overflow-hidden bg-white">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-slate-50 border-b border-slate-200">
              <th className="px-3 py-2.5 w-8" />
              <th className="px-2 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">Estado</th>
              <th className="px-3 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">Tipo JMX</th>
              <th className="px-3 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">Nombre</th>
              <th className="px-3 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide hidden md:table-cell">Ruta</th>
              <th className="px-3 py-2.5 text-left text-xs font-semibold text-slate-500 uppercase tracking-wide">Razón / Sugerencia</th>
            </tr>
          </thead>
          <tbody>
            {filtered.length === 0 ? (
              <tr>
                <td colSpan={6} className="text-center py-10 text-slate-400 text-sm">
                  No hay elementos en esta categoría.
                </td>
              </tr>
            ) : (
              filtered.map((el, i) => (
                <ElementRow key={i} el={el} isOdd={i % 2 === 1} />
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
};

// ─── Small Stat for fidelity summary ────────────────────────────────────────

const Stat: React.FC<{ value: number; label: string; color?: string }> = ({
  value,
  label,
  color = "text-slate-800",
}) => (
  <div className="flex flex-col">
    <span className={`text-xl font-bold tabular-nums ${color}`}>{value}</span>
    <span className="text-[10px] text-slate-500 uppercase tracking-wide">{label}</span>
  </div>
);
