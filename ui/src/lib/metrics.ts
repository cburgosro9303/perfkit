// Cálculos derivados de las métricas de un RunSummary (histograma, percentiles,
// Apdex). Funciones puras y dependency-free; las vistas solo presentan.

import { fmtMs } from "./format";

/** Límites de buckets del histograma (deben coincidir con el backend). */
export const HISTOGRAM_BOUNDS_MS = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000];

/** Etiqueta legible del bucket `i` de un histograma con `bounds` límites.
 *  i=0 → `<1ms`; i=N (último) → `≥10s`; resto → `a–bms`. */
export function bucketLabel(i: number, bounds: number[]): string {
  if (bounds.length === 0) return "";
  if (i === 0) return `<${bounds[0]}ms`;
  if (i >= bounds.length) {
    const last = bounds[bounds.length - 1];
    return last >= 1000 ? `≥${last / 1000}s` : `≥${last}ms`;
  }
  return `${bounds[i - 1]}–${bounds[i]}ms`;
}

/** Valor representativo (límite superior) del bucket `i`, para curvas/percentiles.
 *  El último bucket (abierto) usa el último bound. */
export function bucketUpper(i: number, bounds: number[]): number {
  if (bounds.length === 0) return 0;
  if (i >= bounds.length) return bounds[bounds.length - 1];
  return bounds[i];
}

export interface PercentilePoint {
  x: number; // percentil 0..100
  y: number; // latencia (ms)
}

/** Curva de percentiles a partir de counts/bounds: acumula counts y mapea cada
 *  bucket a (percentil, límite superior). Empieza en (0, primer límite). */
export function percentileCurve(counts: number[], bounds: number[]): PercentilePoint[] {
  const total = counts.reduce((a, b) => a + b, 0);
  if (total === 0 || bounds.length === 0) return [];
  const pts: PercentilePoint[] = [{ x: 0, y: bucketUpper(0, bounds) }];
  let acc = 0;
  for (let i = 0; i < counts.length; i++) {
    acc += counts[i];
    const pct = (acc / total) * 100;
    pts.push({ x: pct, y: bucketUpper(i, bounds) });
  }
  return pts;
}

export interface ApdexResult {
  score: number;
  satisfied: number;
  tolerating: number;
  frustrated: number;
  total: number;
}

/** Apdex desde el histograma: satisfechas ≤ T, tolerando ≤ 4T, resto frustradas.
 *  Apdex = (satisfechas + tolerando/2) / total.
 *  Un bucket cuenta como satisfecho/tolerado si su límite superior ≤ umbral
 *  (criterio conservador con datos agregados). */
export function apdex(counts: number[], bounds: number[], t: number): ApdexResult {
  const total = counts.reduce((a, b) => a + b, 0);
  if (total === 0) return { score: 1, satisfied: 0, tolerating: 0, frustrated: 0, total: 0 };
  let satisfied = 0;
  let tolerating = 0;
  let frustrated = 0;
  for (let i = 0; i < counts.length; i++) {
    const upper = bucketUpper(i, bounds);
    // El último bucket es abierto (≥ último bound): siempre frustrado salvo que T lo cubra.
    const isOpen = i >= bounds.length;
    if (!isOpen && upper <= t) satisfied += counts[i];
    else if (!isOpen && upper <= 4 * t) tolerating += counts[i];
    else frustrated += counts[i];
  }
  const score = (satisfied + tolerating / 2) / total;
  return { score, satisfied, tolerating, frustrated, total };
}

/** Color (clase Tailwind text-*) del score Apdex: ≥0.94 verde, ≥0.85 ámbar, resto rojo. */
export function apdexColor(score: number): string {
  if (score >= 0.94) return "text-emerald-600";
  if (score >= 0.85) return "text-amber-600";
  return "text-red-600";
}

/** Color por clase de código de estado HTTP. */
export function statusClassColor(code: number): string {
  if (code >= 500) return "#ef4444"; // red
  if (code >= 400) return "#f59e0b"; // amber
  if (code >= 300) return "#94a3b8"; // slate
  if (code >= 200) return "#10b981"; // emerald
  return "#94a3b8";
}

/** Reexport por comodidad en las vistas. */
export { fmtMs };
