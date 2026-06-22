// Tipos del frontend, alineados con la salida serde de los crates Rust
// (scenario-ir, metrics). Los enums van internamente etiquetados (campo "type",
// "timer", "assert", "extract", "kind"), tal como los serializa el backend.

export interface Scenario {
  version: string;
  name: string;
  variables?: Record<string, string>;
  datasets?: Dataset[];
  defaults?: HttpDefaults;
  thread_groups: ThreadGroup[];
  metadata?: Metadata;
}

export interface Metadata {
  generator?: string;
  source?: string;
  notes?: string[];
}

export interface HttpDefaults {
  base_url?: string;
  headers?: Record<string, string>;
  connect_timeout_ms?: number;
  response_timeout_ms?: number;
  follow_redirects?: boolean;
}

export interface Dataset {
  name: string;
  path: string;
  delimiter: string;
  variable_names: string[];
  recycle: boolean;
  first_line_is_header: boolean;
}

export interface ThreadGroup {
  name: string;
  load: LoadProfile;
  on_error: "continue" | "stop_thread" | "stop_test";
  steps: Step[];
}

export interface LoadProfile {
  virtual_users: number;
  ramp_up_secs: number;
  hold_secs: number;
  ramp_down_secs: number;
  iterations?: number | null;
  duration_secs?: number | null;
}

export type StepType = "http" | "transaction" | "loop" | "if" | "while" | "timer";

export interface HttpRequest {
  type: "http";
  name: string;
  method: string;
  url: string;
  headers?: Record<string, string>;
  query?: Record<string, string>;
  body?: Body;
  follow_redirects?: boolean;
  timeout_ms?: number;
  timers?: Timer[];
  assertions?: Assertion[];
  extractors?: Extractor[];
}

export interface Transaction {
  type: "transaction";
  name: string;
  steps: Step[];
}
export interface LoopController {
  type: "loop";
  name: string;
  count: number;
  steps: Step[];
}
export interface IfController {
  type: "if";
  name: string;
  condition: string;
  steps: Step[];
}
export interface WhileController {
  type: "while";
  name: string;
  condition: string;
  steps: Step[];
  max_iterations: number;
}
export interface ThroughputController {
  type: "throughput";
  name: string;
  percent: number;
  steps: Step[];
}
export interface InterleaveController {
  type: "interleave";
  name: string;
  steps: Step[];
}
export interface RandomController {
  type: "random";
  name: string;
  steps: Step[];
}
export interface KafkaRequest {
  type: "kafka";
  name: string;
  brokers: string[];
  topic: string;
  key?: string;
  payload: string;
  partition?: number;
  headers?: Record<string, string>;
}
export type TimerStep = Timer & { type: "timer" };

export type Step =
  | HttpRequest
  | Transaction
  | LoopController
  | IfController
  | WhileController
  | ThroughputController
  | InterleaveController
  | RandomController
  | KafkaRequest
  | TimerStep;

export type Body =
  | { kind: "raw"; content_type?: string; data: string }
  | { kind: "form"; fields: Record<string, string> };

export type Timer =
  | { timer: "constant"; delay_ms: number }
  | { timer: "uniform_random"; base_ms: number; range_ms: number }
  | { timer: "gaussian"; offset_ms: number; deviation_ms: number }
  | { timer: "constant_throughput"; target_per_minute: number };

export type Assertion =
  | { assert: "status_code"; codes: number[] }
  | { assert: "body_contains"; substring: string; negate: boolean }
  | { assert: "body_matches"; pattern: string; negate: boolean }
  | { assert: "json_path"; path: string; equals?: string; exists?: boolean }
  | { assert: "duration_below_ms"; max_ms: number }
  | { assert: "size_below_bytes"; max_bytes: number };

export type Extractor =
  | { extract: "regex"; var: string; pattern: string; group: number; default?: string }
  | { extract: "json_path"; var: string; path: string; default?: string }
  | { extract: "boundary"; var: string; left: string; right: string; default?: string };

// --- Migración / fidelidad ---

export type MappingStatus = "migrated" | "assisted" | "unsupported" | "ignored";

export interface MappedElement {
  jmx_type: string;
  jmx_name: string;
  path: string;
  status: MappingStatus;
  ir_ref?: string;
  reason?: string;
  suggestion?: string;
}

export interface FidelitySummary {
  total: number;
  migrated: number;
  assisted: number;
  unsupported: number;
  ignored: number;
  fidelity_pct: number;
}

export interface MigrationReport {
  source: string;
  generated_by: string;
  summary: FidelitySummary;
  elements: MappedElement[];
  notes?: string[];
}

// --- Validación ---

export interface ValidationIssue {
  severity: "error" | "warning";
  path: string;
  message: string;
}
export interface ValidationReport {
  issues: ValidationIssue[];
}

// --- Métricas / resultados ---

export interface LabelStats {
  label: string;
  kind: "http" | "kafka" | "transaction";
  count: number;
  errors: number;
  error_rate: number;
  throughput_per_sec: number;
  min_ms: number;
  mean_ms: number;
  max_ms: number;
  p50_ms: number;
  p90_ms: number;
  p95_ms: number;
  p99_ms: number;
  p999_ms: number;
  bytes_total: number;
  ttfb_mean_ms?: number;
  ttfb_p95_ms?: number;
  sent_bytes?: number;
}

export interface TimePoint {
  t_secs: number;
  throughput: number;
  error_rate: number;
  avg_ms: number;
  p95_ms: number;
  active_vus: number;
  bytes?: number;
}

/** Una fila del heatmap latencia×tiempo: un vector de counts por bucket en `t_secs`. */
export interface HeatmapRow {
  t_secs: number;
  counts: number[];
}

export interface ErrorBucket {
  message: string;
  count: number;
}

export interface RunConfig {
  virtual_users: number;
  thread_groups: number;
}

export interface SampleDetail {
  seq: number;
  label: string;
  kind: "http" | "kafka" | "transaction";
  method: string;
  url: string;
  req_headers: [string, string][];
  req_body?: string;
  status?: number;
  resp_headers: [string, string][];
  resp_body: string;
  latency_ms: number;
  bytes: number;
  success: boolean;
  error?: string;
  extracted: [string, string][];
  vars?: [string, string][];
}

export interface RunSummary {
  run_id: string;
  scenario_name: string;
  started_at: string;
  duration_secs: number;
  config: RunConfig;
  overall: LabelStats;
  labels: LabelStats[];
  timeseries: TimePoint[];
  errors: ErrorBucket[];
  details?: SampleDetail[];
  histogram_bounds_ms?: number[];
  histogram_counts?: number[];
  status_codes?: [number, number][];
  error_kinds?: [string, number][];
  bytes_received?: number;
  bytes_sent?: number;
  latency_heatmap?: HeatmapRow[];
}

export interface LiveSnapshot {
  elapsed_secs: number;
  active_vus: number;
  total_requests: number;
  total_errors: number;
  throughput_per_sec: number;
  error_rate: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
}

export interface ImportResult {
  scenario: Scenario;
  report: MigrationReport;
  yaml: string;
}

// --- Histórico / comparación (history_* commands) ---

/** Una corrida guardada en el histórico (serde de Rust: campos snake_case). */
export interface RunRecord {
  id: number;
  scenario: string;
  branch?: string;
  build?: string;
  environment?: string;
  started_at: string;
  duration_secs: number;
  throughput: number;
  error_rate: number;
  p95_ms: number;
  p99_ms: number;
  requests: number;
}

/** Resultado de comparar una corrida contra su baseline. */
export interface Comparison {
  p95_delta_pct: number;
  throughput_delta_pct: number;
  error_rate_delta: number;
  is_regression: boolean;
}

/** Un punto de una serie de tendencia (una métrica a lo largo del tiempo). */
export interface TrendPoint {
  started_at: string;
  value: number;
}

/** Métrica seleccionable para tendencia/comparación histórica. */
export type TrendMetric = "p95" | "throughput" | "error_rate";

// --- Quality gate (evaluate_gate) ---

export interface GateThresholds {
  max_error_rate: number;
  max_p95_ms: number;
  max_p99_ms: number;
  min_throughput_per_sec: number;
}

export interface GateCheckResult {
  label: string;
  pass: boolean;
  actual: number;
  threshold: number;
}

export interface GateResult {
  pass: boolean;
  checks: GateCheckResult[];
}

export interface RunOptions {
  base_url_override?: string;
  vus?: number;
  duration_secs?: number;
  capture?: boolean;
  capture_plaintext?: boolean;
  capture_limit?: number;
}
