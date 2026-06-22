import React, { useMemo } from "react";
import type { HeatmapRow } from "../types";

interface HeatmapProps {
  /** Una fila por segundo, cada una con un vector de counts por bucket. */
  rows: HeatmapRow[];
  /** Límites de los buckets de latencia (ms). Define las etiquetas del eje Y. */
  bounds: number[];
  /** Formatea la etiqueta de cada bucket (eje Y). */
  bucketLabel?: (i: number, bounds: number[]) => string;
}

// Interpola blanco → indigo (#6366f1) según intensidad 0..1.
function intensityColor(t: number): string {
  if (t <= 0) return "#f8fafc"; // slate-50 para celdas vacías
  // blanco (255,255,255) → indigo (99,102,241)
  const r = Math.round(255 + (99 - 255) * t);
  const g = Math.round(255 + (102 - 255) * t);
  const b = Math.round(255 + (241 - 255) * t);
  return `rgb(${r},${g},${b})`;
}

const defaultBucketLabel = (i: number, bounds: number[]): string => {
  if (i === 0) return `<${bounds[0]}ms`;
  if (i >= bounds.length) {
    const last = bounds[bounds.length - 1];
    return last >= 1000 ? `≥${last / 1000}s` : `≥${last}ms`;
  }
  return `${bounds[i - 1]}–${bounds[i]}ms`;
};

/** Heatmap latencia×tiempo: filas = buckets de latencia (de mayor a menor,
 *  eje Y arriba=lento), columnas = segundos. Color = intensidad del count. */
export const Heatmap: React.FC<HeatmapProps> = ({ rows, bounds, bucketLabel = defaultBucketLabel }) => {
  const nBuckets = bounds.length + 1; // un bucket extra para "≥ último bound"

  const { maxCount, grid } = useMemo(() => {
    let max = 0;
    const g = rows.map((row) => {
      const counts = row.counts ?? [];
      for (const c of counts) if (c > max) max = c;
      return counts;
    });
    return { maxCount: max, grid: g };
  }, [rows]);

  if (rows.length === 0 || bounds.length === 0) {
    return (
      <div className="flex items-center justify-center text-slate-400 text-xs py-12">
        Sin datos de heatmap
      </div>
    );
  }

  // Filas del eje Y: de arriba (más lento) a abajo (más rápido).
  const bucketIdx = Array.from({ length: nBuckets }, (_, i) => nBuckets - 1 - i);

  return (
    <div className="w-full overflow-x-auto">
      <div className="flex flex-col gap-px min-w-fit">
        {bucketIdx.map((bi) => (
          <div key={bi} className="flex items-center gap-2">
            <span className="text-[9px] tabular-nums text-slate-400 w-16 text-right shrink-0 leading-none">
              {bucketLabel(bi, bounds)}
            </span>
            <div className="flex gap-px">
              {grid.map((counts, ti) => {
                const c = counts[bi] ?? 0;
                const t = maxCount > 0 ? c / maxCount : 0;
                return (
                  <div
                    key={ti}
                    className="shrink-0"
                    style={{
                      width: 10,
                      height: 12,
                      backgroundColor: intensityColor(t),
                      borderRadius: 1,
                    }}
                    title={`t=${rows[ti].t_secs}s · ${bucketLabel(bi, bounds)} · ${c} muestras`}
                  />
                );
              })}
            </div>
          </div>
        ))}
        {/* Eje X (tiempo): primer y último segundo */}
        <div className="flex items-center gap-2 mt-1">
          <span className="w-16 shrink-0" />
          <div className="flex justify-between" style={{ width: grid.length * 11 - 1 }}>
            <span className="text-[9px] tabular-nums text-slate-400">{rows[0].t_secs}s</span>
            <span className="text-[9px] tabular-nums text-slate-400">{rows[rows.length - 1].t_secs}s</span>
          </div>
        </div>
      </div>

      {/* Leyenda de intensidad */}
      <div className="flex items-center gap-2 mt-4 px-1">
        <span className="text-[10px] text-slate-400">0</span>
        <div
          className="h-2.5 w-32 rounded-sm"
          style={{ background: `linear-gradient(to right, ${intensityColor(0.001)}, ${intensityColor(1)})` }}
        />
        <span className="text-[10px] tabular-nums text-slate-400">{maxCount} muestras</span>
      </div>
    </div>
  );
};
