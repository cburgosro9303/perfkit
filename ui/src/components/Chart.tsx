import React, { useMemo } from "react";

interface DataPoint {
  x: number;
  y: number;
}

interface ChartProps {
  series: DataPoint[];
  color?: string;
  fillColor?: string;
  height?: number;
  valueFormat?: (v: number) => string;
  label?: string;
  animated?: boolean;
  /** Línea horizontal punteada de referencia (p.ej. umbral SLA), en unidades de y. */
  refLine?: number;
  /** Color de la línea de referencia. */
  refLineColor?: string;
}

const defaultFmt = (v: number) => v.toFixed(1);

export const Chart: React.FC<ChartProps> = ({
  series,
  color = "#6366f1",
  fillColor,
  height = 120,
  valueFormat = defaultFmt,
  label,
  animated = false,
  refLine,
  refLineColor = "#ef4444",
}) => {
  const w = 400;
  const h = height;
  const pad = { top: 12, right: 12, bottom: 24, left: 8 };

  const data = useMemo(() => {
    if (series.length < 2) return null;

    const xs = series.map((d) => d.x);
    const ys = series.map((d) => d.y);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = 0;
    // La línea de referencia entra en la escala para que siempre quede visible.
    const dataMax = Math.max(...ys, refLine ?? 0);
    const maxY = dataMax * 1.1 || 1;

    const innerW = w - pad.left - pad.right;
    const innerH = h - pad.top - pad.bottom;

    const px = (x: number) => pad.left + ((x - minX) / (maxX - minX || 1)) * innerW;
    const py = (y: number) => pad.top + innerH - ((y - minY) / (maxY - minY || 1)) * innerH;

    const pts = series.map((d) => ({ sx: px(d.x), sy: py(d.y), ...d }));

    // Smooth path using cubic bezier
    const pathD = pts.reduce((acc, pt, i) => {
      if (i === 0) return `M ${pt.sx},${pt.sy}`;
      const prev = pts[i - 1];
      const cpx = (prev.sx + pt.sx) / 2;
      return `${acc} C ${cpx},${prev.sy} ${cpx},${pt.sy} ${pt.sx},${pt.sy}`;
    }, "");

    // Area path
    const first = pts[0];
    const last = pts[pts.length - 1];
    const areaD = `${pathD} L ${last.sx},${pad.top + innerH} L ${first.sx},${pad.top + innerH} Z`;

    const maxPt = pts.reduce((a, b) => (a.y > b.y ? a : b));
    const minPt = pts.reduce((a, b) => (a.y < b.y ? a : b));

    const refY = refLine !== undefined ? py(refLine) : null;

    return { pts, pathD, areaD, maxPt, minPt, maxY, minY, innerH, innerW, refY };
  }, [series, h, refLine]);

  const resolvedFill = fillColor ?? `${color}18`;

  if (!data || series.length < 2) {
    return (
      <div
        className="flex items-center justify-center text-slate-400 text-xs"
        style={{ height }}
      >
        {label && <span className="mr-2 font-medium">{label}</span>}
        Sin datos aún…
      </div>
    );
  }

  return (
    <div className="relative select-none">
      {label && (
        <span className="absolute top-0 left-2 text-xs font-semibold text-slate-500">{label}</span>
      )}
      <svg
        width="100%"
        viewBox={`0 0 ${w} ${h}`}
        preserveAspectRatio="none"
        style={{ height, display: "block" }}
      >
        <defs>
          <linearGradient id={`fill-${color.replace("#", "")}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity="0.3" />
            <stop offset="100%" stopColor={color} stopOpacity="0.02" />
          </linearGradient>
        </defs>

        {/* Grid lines */}
        {[0.25, 0.5, 0.75].map((r) => (
          <line
            key={r}
            x1={pad.left}
            y1={pad.top + (1 - r) * (h - pad.top - pad.bottom)}
            x2={w - pad.right}
            y2={pad.top + (1 - r) * (h - pad.top - pad.bottom)}
            stroke="#e2e8f0"
            strokeWidth="1"
          />
        ))}

        {/* Reference line (umbral SLA) */}
        {data.refY !== null && (
          <>
            <line
              x1={pad.left}
              y1={data.refY}
              x2={w - pad.right}
              y2={data.refY}
              stroke={refLineColor}
              strokeWidth="1.5"
              strokeDasharray="4 3"
              opacity="0.8"
            />
            <text
              x={pad.left + 2}
              y={Math.max(data.refY - 3, pad.top + 8)}
              fontSize="9"
              fill={refLineColor}
              fontWeight="600"
              fontFamily="ui-monospace, monospace"
            >
              {valueFormat(refLine as number)}
            </text>
          </>
        )}

        {/* Area fill */}
        <path
          d={data.areaD}
          fill={`url(#fill-${color.replace("#", "")})`}
        />

        {/* Line */}
        <path
          d={data.pathD}
          fill="none"
          stroke={color}
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {/* Max label */}
        <text
          x={Math.min(data.maxPt.sx + 4, w - pad.right - 30)}
          y={Math.max(data.maxPt.sy - 4, pad.top + 10)}
          fontSize="9"
          fill={color}
          fontWeight="600"
          fontFamily="ui-monospace, monospace"
        >
          {valueFormat(data.maxPt.y)}
        </text>

        {/* Min label (only if noticeably different) */}
        {data.maxPt.y > 0 && data.minPt.y < data.maxPt.y * 0.7 && (
          <text
            x={Math.min(data.minPt.sx + 4, w - pad.right - 30)}
            y={Math.min(data.minPt.sy + 12, h - pad.bottom + 2)}
            fontSize="9"
            fill="#94a3b8"
            fontFamily="ui-monospace, monospace"
          >
            {valueFormat(data.minPt.y)}
          </text>
        )}

        {/* Last value dot */}
        {data.pts.length > 0 && (
          <circle
            cx={data.pts[data.pts.length - 1].sx}
            cy={data.pts[data.pts.length - 1].sy}
            r="3"
            fill={color}
            stroke="white"
            strokeWidth="1.5"
          />
        )}
      </svg>
    </div>
  );
};
