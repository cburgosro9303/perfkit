import React, { useMemo } from "react";

// Gráfica de barras dependency-free (SVG). Soporta barras simples (una serie,
// color por barra opcional) y barras agrupadas (dos valores por categoría).
// Estética slate/indigo, coherente con el resto de la UI.

export interface BarDatum {
  /** Etiqueta del eje X (rango del bucket, código de estado, label…). */
  label: string;
  /** Altura de la barra. */
  value: number;
  /** Color de la barra (si se omite, usa `color` global). */
  color?: string;
  /** Texto opcional bajo la etiqueta (p.ej. conteo formateado). */
  sub?: string;
}

interface BarsProps {
  data: BarDatum[];
  color?: string;
  height?: number;
  /** Formatea el valor mostrado encima de la barra y en el tooltip. */
  valueFormat?: (v: number) => string;
  /** Inclina las etiquetas del eje X (útil con rangos largos). */
  tiltLabels?: boolean;
}

const defaultFmt = (v: number) => String(Math.round(v));

/** Barras verticales simples, una por dato, etiquetadas en el eje X. */
export const Bars: React.FC<BarsProps> = ({
  data,
  color = "#6366f1",
  height = 180,
  valueFormat = defaultFmt,
  tiltLabels = false,
}) => {
  const maxV = useMemo(
    () => Math.max(1, ...data.map((d) => d.value)),
    [data],
  );

  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center text-slate-400 text-xs" style={{ height }}>
        Sin datos
      </div>
    );
  }

  const plotH = height;
  const labelH = tiltLabels ? 44 : 26;

  return (
    <div className="w-full">
      <div className="flex items-end gap-1" style={{ height: plotH }}>
        {data.map((d, i) => {
          const frac = d.value / maxV;
          const barH = Math.max(d.value > 0 ? 2 : 0, frac * (plotH - 18));
          return (
            <div
              key={`${d.label}-${i}`}
              className="flex-1 min-w-0 flex flex-col items-center justify-end h-full group"
              title={`${d.label}: ${valueFormat(d.value)}`}
            >
              <span className="text-[9px] tabular-nums text-slate-400 mb-0.5 opacity-0 group-hover:opacity-100 transition-opacity leading-none">
                {valueFormat(d.value)}
              </span>
              <div
                className="w-full rounded-t-sm transition-colors"
                style={{ height: barH, backgroundColor: d.color ?? color, minWidth: 4 }}
              />
            </div>
          );
        })}
      </div>
      <div className="flex gap-1" style={{ height: labelH }}>
        {data.map((d, i) => (
          <div key={`${d.label}-lbl-${i}`} className="flex-1 min-w-0 flex flex-col items-center pt-1">
            <span
              className={`text-[9px] text-slate-500 leading-tight text-center break-words ${
                tiltLabels ? "rotate-[-30deg] origin-top whitespace-nowrap" : ""
              }`}
            >
              {d.label}
            </span>
            {d.sub && !tiltLabels && (
              <span className="text-[9px] tabular-nums text-slate-400 leading-tight">{d.sub}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

// ─── Barras agrupadas (dos series por categoría) ───────────────────────────────

export interface GroupedDatum {
  label: string;
  a: number;
  b: number;
}

interface GroupedBarsProps {
  data: GroupedDatum[];
  /** Nombres de las dos series (leyenda). */
  seriesNames: [string, string];
  colors?: [string, string];
  height?: number;
  valueFormat?: (v: number) => string;
}

/** Dos barras por categoría (p.ej. TTFB p95 vs Total p95), con leyenda. */
export const GroupedBars: React.FC<GroupedBarsProps> = ({
  data,
  seriesNames,
  colors = ["#818cf8", "#4f46e5"],
  height = 200,
  valueFormat = defaultFmt,
}) => {
  const maxV = useMemo(
    () => Math.max(1, ...data.flatMap((d) => [d.a, d.b])),
    [data],
  );

  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center text-slate-400 text-xs" style={{ height }}>
        Sin datos
      </div>
    );
  }

  const plotH = height;

  return (
    <div className="w-full">
      {/* Leyenda */}
      <div className="flex items-center gap-4 mb-2 px-1">
        {([0, 1] as const).map((idx) => (
          <span key={idx} className="flex items-center gap-1.5 text-[11px] text-slate-500">
            <span className="inline-block w-2.5 h-2.5 rounded-sm" style={{ backgroundColor: colors[idx] }} />
            {seriesNames[idx]}
          </span>
        ))}
      </div>
      <div className="flex items-end gap-3" style={{ height: plotH }}>
        {data.map((d, i) => (
          <div key={`${d.label}-${i}`} className="flex-1 min-w-0 flex flex-col items-center justify-end h-full">
            <div className="w-full flex items-end justify-center gap-1 h-full">
              {([["a", d.a], ["b", d.b]] as const).map(([k, v], idx) => (
                <div
                  key={k}
                  className="flex-1 max-w-[40%] rounded-t-sm transition-colors"
                  style={{
                    height: Math.max(v > 0 ? 2 : 0, (v / maxV) * (plotH - 16)),
                    backgroundColor: colors[idx],
                    minWidth: 6,
                  }}
                  title={`${d.label} · ${seriesNames[idx]}: ${valueFormat(v)}`}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
      <div className="flex gap-3 pt-1">
        {data.map((d, i) => (
          <div key={`${d.label}-lbl-${i}`} className="flex-1 min-w-0 text-center">
            <span className="text-[9px] text-slate-500 leading-tight break-words block">{d.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
};
