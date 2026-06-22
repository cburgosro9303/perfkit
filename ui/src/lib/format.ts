/** Format milliseconds as a readable duration string */
export function fmtMs(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
}

/** Format a percentage (0-1 float or 0-100) as "12.3%" */
export function fmtPct(value: number, isAlreadyPct = false): string {
  const pct = isAlreadyPct ? value : value * 100;
  return `${pct.toFixed(1)}%`;
}

/** Format a number with thousands separators */
export function fmtNum(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 10_000) return `${(n / 1_000).toFixed(1)}k`;
  return n.toLocaleString("en-US");
}

/** Format throughput as "req/s" */
export function fmtThroughput(rps: number): string {
  if (rps >= 1000) return `${(rps / 1000).toFixed(1)}k/s`;
  return `${rps.toFixed(1)}/s`;
}

/** Format bytes */
export function fmtBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(2)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

/** Format seconds as "mm:ss" */
export function fmtDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}
