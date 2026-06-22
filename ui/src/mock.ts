// Datos de muestra y simulación para correr la UI en un navegador (sin Tauri).
// En la app nativa, api.ts usa los comandos Tauri reales en su lugar.

import type {
  Comparison,
  GateResult,
  GateThresholds,
  HttpRequest,
  ImportResult,
  LiveSnapshot,
  MigrationReport,
  RunOptions,
  RunRecord,
  RunSummary,
  SampleDetail,
  Scenario,
  Step,
  TimePoint,
  TrendMetric,
  TrendPoint,
  ValidationReport,
} from "./types";

export function sampleScenario(): Scenario {
  // Transcripción fiel de /tmp/checkout.scenario.json (el IR REAL que produce el
  // importador del ejemplo nativo "checkout-demo.jmx"). El JSON exportado omite
  // `metadata`; lo añadimos porque la UI lo usa (cabecera/origen del plan).
  return {
    version: "0.3.0",
    name: "Checkout de e-commerce (demo)",
    variables: { username: "ana.perez" },
    thread_groups: [
      {
        name: "Compradores",
        load: {
          virtual_users: 5,
          ramp_up_secs: 2,
          hold_secs: 0,
          ramp_down_secs: 0,
          iterations: 5,
        },
        on_error: "continue",
        steps: [
          {
            type: "transaction",
            name: "Login",
            steps: [
              {
                type: "http",
                name: "POST /login",
                method: "POST",
                url: "https://httpbin.org:443/post",
                headers: {
                  Accept: "application/json",
                  "Content-Type": "application/json",
                },
                body: { kind: "raw", data: '{"user":"${username}","pass":"s3cr3t"}' },
                follow_redirects: true,
                timers: [{ timer: "constant", delay_ms: 200 }],
                assertions: [{ assert: "status_code", codes: [200] }],
              },
              {
                type: "http",
                name: "GET token",
                method: "GET",
                url: "https://httpbin.org:443/uuid",
                follow_redirects: true,
                assertions: [{ assert: "status_code", codes: [200] }],
                extractors: [{ extract: "json_path", var: "token", path: "$.uuid", default: "NONE" }],
              },
            ],
          },
          {
            type: "http",
            name: "GET /catalog",
            method: "GET",
            url: "https://httpbin.org:443/get",
            headers: { Authorization: "Bearer ${token}" },
            query: { page: "1" },
            follow_redirects: true,
            timers: [{ timer: "uniform_random", base_ms: 100, range_ms: 200 }],
            assertions: [{ assert: "status_code", codes: [200] }],
          },
          {
            type: "loop",
            name: "Agregar 2 productos",
            count: 2,
            steps: [
              {
                type: "http",
                name: "POST /cart",
                method: "POST",
                url: "https://httpbin.org:443/post",
                headers: {
                  Authorization: "Bearer ${token}",
                  "Content-Type": "application/json",
                },
                body: { kind: "raw", data: '{"sku":"SKU-001","qty":1,"token":"${token}"}' },
                follow_redirects: true,
                assertions: [{ assert: "status_code", codes: [200] }],
              },
            ],
          },
          {
            type: "http",
            name: "POST /checkout",
            method: "POST",
            url: "https://httpbin.org:443/post",
            headers: {
              Authorization: "Bearer ${token}",
              "Content-Type": "application/json",
            },
            body: { kind: "raw", data: '{"order":"confirm","user":"${username}","token":"${token}"}' },
            follow_redirects: true,
            assertions: [{ assert: "status_code", codes: [200] }],
          },
        ],
      },
    ],
    metadata: {
      generator: "perfkit-jmx-importer 0.1.0",
      source: "checkout-demo.jmx",
    },
  };
}

export function sampleReport(): MigrationReport {
  // Transcripción fiel de /tmp/checkout.fidelity.json (el informe REAL de importar
  // "checkout-demo.jmx"): 21 elementos, todos `migrated` → fidelidad 100%.
  return {
    source: "examples/jmx/checkout-demo.jmx",
    generated_by: "perfkit-jmx-importer 0.1.0",
    summary: { total: 21, migrated: 21, assisted: 0, unsupported: 0, ignored: 0, fidelity_pct: 100.0 },
    elements: [
      { jmx_type: "TestPlan", jmx_name: "Checkout de e-commerce (demo)", path: "Test Plan", status: "migrated", ir_ref: "scenario" },
      { jmx_type: "ThreadGroup", jmx_name: "Compradores", path: "Test Plan > Compradores", status: "migrated", ir_ref: "thread_group" },
      { jmx_type: "HTTPSamplerProxy", jmx_name: "POST /login", path: "Test Plan > Compradores > Login > POST /login", status: "migrated", ir_ref: "http" },
      { jmx_type: "HeaderManager", jmx_name: "Headers", path: "Test Plan > Compradores > Login > POST /login > Headers", status: "migrated", ir_ref: "http.headers" },
      { jmx_type: "ResponseAssertion", jmx_name: "Status 200", path: "Test Plan > Compradores > Login > POST /login > Status 200", status: "migrated", ir_ref: "http.assertions" },
      { jmx_type: "ConstantTimer", jmx_name: "Pausa", path: "Test Plan > Compradores > Login > POST /login > Pausa", status: "migrated", ir_ref: "http.timers" },
      { jmx_type: "HTTPSamplerProxy", jmx_name: "GET token", path: "Test Plan > Compradores > Login > GET token", status: "migrated", ir_ref: "http" },
      { jmx_type: "JSONPostProcessor", jmx_name: "Extraer token", path: "Test Plan > Compradores > Login > GET token > Extraer token", status: "migrated", ir_ref: "http.extractors" },
      { jmx_type: "ResponseAssertion", jmx_name: "Status 200", path: "Test Plan > Compradores > Login > GET token > Status 200", status: "migrated", ir_ref: "http.assertions" },
      { jmx_type: "TransactionController", jmx_name: "Login", path: "Test Plan > Compradores", status: "migrated", ir_ref: "transaction" },
      { jmx_type: "HTTPSamplerProxy", jmx_name: "GET /catalog", path: "Test Plan > Compradores > GET /catalog", status: "migrated", ir_ref: "http" },
      { jmx_type: "HeaderManager", jmx_name: "Auth", path: "Test Plan > Compradores > GET /catalog > Auth", status: "migrated", ir_ref: "http.headers" },
      { jmx_type: "ResponseAssertion", jmx_name: "Status 200", path: "Test Plan > Compradores > GET /catalog > Status 200", status: "migrated", ir_ref: "http.assertions" },
      { jmx_type: "UniformRandomTimer", jmx_name: "Pausa aleatoria", path: "Test Plan > Compradores > GET /catalog > Pausa aleatoria", status: "migrated", ir_ref: "http.timers" },
      { jmx_type: "HTTPSamplerProxy", jmx_name: "POST /cart", path: "Test Plan > Compradores > Agregar 2 productos > POST /cart", status: "migrated", ir_ref: "http" },
      { jmx_type: "HeaderManager", jmx_name: "Auth", path: "Test Plan > Compradores > Agregar 2 productos > POST /cart > Auth", status: "migrated", ir_ref: "http.headers" },
      { jmx_type: "ResponseAssertion", jmx_name: "Status 200", path: "Test Plan > Compradores > Agregar 2 productos > POST /cart > Status 200", status: "migrated", ir_ref: "http.assertions" },
      { jmx_type: "LoopController", jmx_name: "Agregar 2 productos", path: "Test Plan > Compradores", status: "migrated", ir_ref: "loop" },
      { jmx_type: "HTTPSamplerProxy", jmx_name: "POST /checkout", path: "Test Plan > Compradores > POST /checkout", status: "migrated", ir_ref: "http" },
      { jmx_type: "HeaderManager", jmx_name: "Auth", path: "Test Plan > Compradores > POST /checkout > Auth", status: "migrated", ir_ref: "http.headers" },
      { jmx_type: "ResponseAssertion", jmx_name: "Status 200", path: "Test Plan > Compradores > POST /checkout > Status 200", status: "migrated", ir_ref: "http.assertions" },
    ],
    notes: [],
  };
}

export function sampleYaml(): string {
  // Mismo IR que sampleScenario(), como YAML — copia fiel de /tmp/checkout.yaml.
  // Los `${...}` van escapados como \${...} porque esto es un template literal.
  return `version: 0.3.0
name: Checkout de e-commerce (demo)
variables:
  username: ana.perez
thread_groups:
- name: Compradores
  load:
    virtual_users: 5
    ramp_up_secs: 2
    hold_secs: 0
    ramp_down_secs: 0
    iterations: 5
  on_error: continue
  steps:
  - type: transaction
    name: Login
    steps:
    - type: http
      name: POST /login
      method: POST
      url: https://httpbin.org:443/post
      headers:
        Accept: application/json
        Content-Type: application/json
      body:
        kind: raw
        data: '{"user":"\${username}","pass":"s3cr3t"}'
      follow_redirects: true
      timers:
      - timer: constant
        delay_ms: 200
      assertions:
      - assert: status_code
        codes:
        - 200
    - type: http
      name: GET token
      method: GET
      url: https://httpbin.org:443/uuid
      follow_redirects: true
      assertions:
      - assert: status_code
        codes:
        - 200
      extractors:
      - extract: json_path
        var: token
        path: $.uuid
        default: NONE
  - type: http
    name: GET /catalog
    method: GET
    url: https://httpbin.org:443/get
    headers:
      Authorization: Bearer \${token}
    query:
      page: '1'
    follow_redirects: true
    timers:
    - timer: uniform_random
      base_ms: 100
      range_ms: 200
    assertions:
    - assert: status_code
      codes:
      - 200
  - type: loop
    name: Agregar 2 productos
    count: 2
    steps:
    - type: http
      name: POST /cart
      method: POST
      url: https://httpbin.org:443/post
      headers:
        Authorization: Bearer \${token}
        Content-Type: application/json
      body:
        kind: raw
        data: '{"sku":"SKU-001","qty":1,"token":"\${token}"}'
      follow_redirects: true
      assertions:
      - assert: status_code
        codes:
        - 200
  - type: http
    name: POST /checkout
    method: POST
    url: https://httpbin.org:443/post
    headers:
      Authorization: Bearer \${token}
      Content-Type: application/json
    body:
      kind: raw
      data: '{"order":"confirm","user":"\${username}","token":"\${token}"}'
    follow_redirects: true
    assertions:
    - assert: status_code
      codes:
      - 200
metadata:
  generator: perfkit-jmx-importer 0.1.0
  source: examples/jmx/checkout-demo.jmx
`;
}

export async function mockImport(_path: string): Promise<ImportResult> {
  await delay(250);
  return { scenario: sampleScenario(), report: sampleReport(), yaml: sampleYaml() };
}

export async function mockValidate(_s: Scenario): Promise<ValidationReport> {
  await delay(120);
  return { issues: [] };
}

/** Claves de header cuyo valor se enmascara al capturar (case-insensitive). */
const SENSITIVE_HEADER_RE = /authorization|cookie|token|password/i;

const REDACTED = "***REDACTED***";

/** JWT de muestra (con pinta creíble) que se muestra sin redactar cuando el
 *  usuario activa "Mostrar secretos sin redactar" (capture_plaintext). */
const SAMPLE_JWT =
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI0MiIsIm5hbWUiOiJBbmEgUMOpcmV6Iiwicm9sZSI6ImN1c3RvbWVyIiwiaWF0IjoxNzE4ODAwMDAwLCJleHAiOjE3MTg4MDM2MDB9.s5Q1m2bq3F0pVx-K8nQh7wZ2tJ4cR6yA9dL0eU1gP3";

/** Contraseña real de muestra (se muestra solo con capture_plaintext). */
const SAMPLE_PASSWORD = "Hunter2!";

/** UUID de muestra: es lo que httpbin GET /uuid devuelve y el extractor json_path
 *  ($.uuid) guarda en la variable `token`. El Authorization del flujo es
 *  `Bearer <este uuid>`. Se muestra sin redactar solo con capture_plaintext. */
const SAMPLE_UUID = "3b1f9c2a-7d84-4e16-9a0b-2f5c8e6d1a47";

/** Authorization header sin redactar (Bearer + JWT) para el modo plaintext. */
const SAMPLE_AUTH = `Bearer ${SAMPLE_JWT}`;

/** Devuelve el valor a mostrar para una clave sensible según el flag plaintext.
 *  Con `plaintext` muestra un valor real creíble; sin él, el marcador redactado. */
function reveal(key: string, real: string, plaintext: boolean): string {
  if (!SENSITIVE_HEADER_RE.test(key)) return real;
  if (!plaintext) return REDACTED;
  if (/authorization/i.test(key)) return SAMPLE_AUTH;
  if (/password/i.test(key)) return SAMPLE_PASSWORD;
  // token, cookie, … → el JWT de muestra.
  return SAMPLE_JWT;
}

/** Construye un SampleDetail a partir del paso HTTP realmente probado, para que
 *  la prueba en el navegador refleje la petición real (no un login enlatado). */
function probeDetail(scenario: Scenario, plaintext: boolean): SampleDetail {
  const step: Step | undefined = scenario.thread_groups[0]?.steps[0];
  const vars: [string, string][] = Object.entries(scenario.variables ?? {}).map(
    ([k, v]) => [k, reveal(k, v, plaintext)],
  );

  if (!step || step.type !== "http") {
    // Sin paso HTTP probable: detalle mínimo y honesto.
    const label = step && "name" in step ? step.name : "Prueba";
    return {
      seq: 1,
      label,
      kind: "http",
      method: "GET",
      url: "",
      req_headers: [],
      status: undefined,
      resp_headers: [],
      resp_body: "",
      latency_ms: 0,
      bytes: 0,
      success: false,
      error: "El paso seleccionado no es una petición HTTP.",
      extracted: [],
      vars,
    };
  }

  const http: HttpRequest = step;
  const req_headers: [string, string][] = Object.entries(http.headers ?? {}).map(
    ([k, v]) => [k, reveal(k, v, plaintext)],
  );
  const req_body =
    http.body?.kind === "raw"
      ? http.body.data
      : http.body?.kind === "form"
        ? JSON.stringify(http.body.fields)
        : undefined;
  const resp_body = JSON.stringify(
    { ok: true, probe: true, label: http.name, method: http.method },
    null,
    2,
  );

  return {
    seq: 1,
    label: http.name,
    kind: "http",
    method: http.method,
    url: http.url,
    req_headers,
    ...(req_body !== undefined && req_body !== "" ? { req_body } : {}),
    status: 200,
    resp_headers: [
      ["content-type", "application/json"],
      ["x-probe", "1"],
    ],
    resp_body,
    latency_ms: 40 + Math.round(Math.random() * 80),
    bytes: resp_body.length,
    success: true,
    extracted: [],
    vars,
  };
}

/** LabelStats sintético para el resumen de una prueba de 1 petición. */
function probeOverall(detail: SampleDetail): RunSummary["overall"] {
  const ms = detail.latency_ms;
  const errors = detail.success ? 0 : 1;
  return {
    label: detail.label,
    kind: "http",
    count: 1,
    errors,
    error_rate: errors,
    throughput_per_sec: 0,
    min_ms: ms,
    mean_ms: ms,
    max_ms: ms,
    p50_ms: ms,
    p90_ms: ms,
    p95_ms: ms,
    p99_ms: ms,
    p999_ms: ms,
    bytes_total: detail.bytes,
  };
}

/** Simula una ejecución: emite snapshots ~cada 250ms y al final un RunSummary.
 *  Para una *prueba* (capture && vus<=1) no corre la línea de tiempo larga: tras
 *  un pequeño retardo devuelve un resumen mínimo con el detalle del paso probado. */
export function simulateRun(
  scenario: Scenario,
  opts: RunOptions,
  onMetrics: (s: LiveSnapshot) => void,
  onFinished: (s: RunSummary) => void,
): () => void {
  // ── Prueba de 1 petición ("Probar petición"): respuesta casi instantánea ──
  const isProbe = (opts.capture ?? false) && (opts.vus ?? 99) <= 1;
  if (isProbe) {
    let cancelled = false;
    const detail = probeDetail(scenario, opts.capture_plaintext ?? false);
    const handle = setTimeout(() => {
      if (cancelled) return;
      onFinished({
        run_id: "run-probe",
        scenario_name: scenario.name,
        started_at: new Date().toISOString(),
        duration_secs: 0,
        config: { virtual_users: 1, thread_groups: 1 },
        overall: probeOverall(detail),
        labels: [probeOverall(detail)],
        timeseries: [],
        errors: detail.error ? [{ message: detail.error, count: 1 }] : [],
        details: [detail],
      });
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(handle);
    };
  }

  const vusTarget = opts.vus ?? 50;
  const duration = opts.duration_secs ?? 12;
  const ramp = Math.min(10, duration / 2);
  let t = 0;
  let total = 0;
  let errors = 0;
  const ts: TimePoint[] = [];
  let cancelled = false;

  const tick = () => {
    if (cancelled) return;
    t += 0.25;
    const vus = Math.round(Math.min(vusTarget, (t / ramp) * vusTarget));
    const baseP95 = 60 + Math.random() * 30 + Math.max(0, vus - 30) * 1.2;
    const tp = vus * (1000 / (baseP95 + 40));
    const reqThisTick = Math.round(tp * 0.25);
    total += reqThisTick;
    const errThisTick = Math.random() < 0.2 ? Math.round(reqThisTick * 0.01) : 0;
    errors += errThisTick;
    if (Number.isInteger(Math.round(t)) && Math.abs(t - Math.round(t)) < 0.13) {
      ts.push({
        t_secs: Math.round(t),
        throughput: tp,
        error_rate: errThisTick / Math.max(1, reqThisTick),
        avg_ms: baseP95 * 0.6,
        p95_ms: baseP95,
        active_vus: vus,
        // Bytes/s ≈ throughput × tamaño medio recibido (~820 B/req).
        bytes: Math.round(tp * 820),
      });
    }
    onMetrics({
      elapsed_secs: t,
      active_vus: vus,
      total_requests: total,
      total_errors: errors,
      throughput_per_sec: total / t,
      error_rate: errors / Math.max(1, total),
      p50_ms: baseP95 * 0.45,
      p95_ms: baseP95,
      p99_ms: baseP95 * 1.6,
    });
    if (t >= duration) {
      clearInterval(handle);
      onFinished(buildSummary(opts, total, errors, duration, vusTarget, ts));
    }
  };
  const handle = setInterval(tick, 250);
  return () => {
    cancelled = true;
    clearInterval(handle);
  };
}

/** Detalle de muestra (request/response) para la vista de inspección de peticiones.
 *  Refleja el flujo "checkout de e-commerce" contra httpbin.org (el mismo ejemplo
 *  que la app nativa). Con `plaintext` los secretos (Authorization, token, pass) se
 *  muestran sin redactar, igual que hace el motor nativo con capture_plaintext. */
function sampleDetails(plaintext = false): SampleDetail[] {
  // El token es el uuid que httpbin /uuid devuelve y el extractor guarda en `token`.
  const tokenVal = plaintext ? SAMPLE_UUID : "ey...redacted-sample";
  // Authorization = "Bearer ${token}" → coherente con el token (uuid) extraído.
  const auth = plaintext ? `Bearer ${SAMPLE_UUID}` : REDACTED;
  const pass = plaintext ? SAMPLE_PASSWORD : "********";
  const loginBody = `{"user":"ana.perez","pass":"${pass}"}`;
  // Variables vigentes tras el login (httpbin no devuelve token; lo da GET /uuid).
  const varsAfterLogin: [string, string][] = [
    ["username", "ana.perez"],
    ["token", tokenVal],
  ];

  return [
    {
      // POST /login → httpbin /post (echo del body enviado).
      seq: 1,
      label: "POST /login",
      kind: "http",
      method: "POST",
      url: "https://httpbin.org/post",
      req_headers: [
        ["Accept", "application/json"],
        ["Content-Type", "application/json"],
      ],
      req_body: loginBody,
      status: 200,
      resp_headers: [
        ["content-type", "application/json"],
        ["server", "gunicorn/19.9.0"],
      ],
      resp_body: `{"json":${loginBody},"url":"https://httpbin.org/post"}`,
      latency_ms: 168,
      bytes: 412,
      success: true,
      extracted: [],
      vars: [
        ["username", "ana.perez"],
        ["password", plaintext ? SAMPLE_PASSWORD : REDACTED],
      ],
    },
    {
      // GET token → httpbin /uuid; el extractor json_path $.uuid guarda `token`.
      seq: 2,
      label: "GET token",
      kind: "http",
      method: "GET",
      url: "https://httpbin.org/uuid",
      req_headers: [["Accept", "application/json"]],
      status: 200,
      resp_headers: [
        ["content-type", "application/json"],
        ["server", "gunicorn/19.9.0"],
      ],
      resp_body: `{"uuid":"${tokenVal}"}`,
      latency_ms: 73,
      bytes: 53,
      success: true,
      extracted: [["token", tokenVal]],
      vars: [["username", "ana.perez"]],
    },
    {
      // GET /catalog → httpbin /get?page=1; el header Authorization se hace eco.
      seq: 3,
      label: "GET /catalog",
      kind: "http",
      method: "GET",
      url: "https://httpbin.org/get?page=1",
      req_headers: [["Authorization", auth]],
      status: 200,
      resp_headers: [
        ["content-type", "application/json"],
        ["server", "gunicorn/19.9.0"],
      ],
      resp_body: `{"args":{"page":"1"},"headers":{"Authorization":"${auth}","Host":"httpbin.org"},"url":"https://httpbin.org/get?page=1"}`,
      latency_ms: 94,
      bytes: 540,
      success: true,
      extracted: [],
      vars: [...varsAfterLogin, ["page", "1"]],
    },
    {
      // POST /cart (1ª de 2, loop x2) → httpbin /post; body con ${token}.
      seq: 4,
      label: "POST /cart",
      kind: "http",
      method: "POST",
      url: "https://httpbin.org/post",
      req_headers: [
        ["Authorization", auth],
        ["Content-Type", "application/json"],
      ],
      req_body: `{"sku":"SKU-001","qty":1,"token":"${tokenVal}"}`,
      status: 200,
      resp_headers: [
        ["content-type", "application/json"],
        ["server", "gunicorn/19.9.0"],
      ],
      resp_body: `{"json":{"sku":"SKU-001","qty":1,"token":"${tokenVal}"},"url":"https://httpbin.org/post"}`,
      latency_ms: 121,
      bytes: 360,
      success: true,
      extracted: [],
      vars: [...varsAfterLogin, ["__jm__Agregar 2 productos__idx", "0"]],
    },
    {
      // POST /cart (2ª de 2, loop x2) → httpbin /post; body con ${token}.
      seq: 5,
      label: "POST /cart",
      kind: "http",
      method: "POST",
      url: "https://httpbin.org/post",
      req_headers: [
        ["Authorization", auth],
        ["Content-Type", "application/json"],
      ],
      req_body: `{"sku":"SKU-001","qty":1,"token":"${tokenVal}"}`,
      status: 200,
      resp_headers: [
        ["content-type", "application/json"],
        ["server", "gunicorn/19.9.0"],
      ],
      resp_body: `{"json":{"sku":"SKU-001","qty":1,"token":"${tokenVal}"},"url":"https://httpbin.org/post"}`,
      latency_ms: 133,
      bytes: 360,
      success: true,
      extracted: [],
      vars: [...varsAfterLogin, ["__jm__Agregar 2 productos__idx", "1"]],
    },
    {
      // POST /checkout → httpbin /post; body con ${username} y ${token}.
      seq: 6,
      label: "POST /checkout",
      kind: "http",
      method: "POST",
      url: "https://httpbin.org/post",
      req_headers: [
        ["Authorization", auth],
        ["Content-Type", "application/json"],
      ],
      req_body: `{"order":"confirm","user":"ana.perez","token":"${tokenVal}"}`,
      status: 200,
      resp_headers: [
        ["content-type", "application/json"],
        ["server", "gunicorn/19.9.0"],
      ],
      resp_body: `{"json":{"order":"confirm","user":"ana.perez","token":"${tokenVal}"},"url":"https://httpbin.org/post"}`,
      latency_ms: 152,
      bytes: 380,
      success: true,
      extracted: [],
      vars: varsAfterLogin,
    },
  ];
}

/** Tope de detalles generados en la demo del navegador (rendimiento). El backend
 *  nativo captura todas las peticiones; aquí generamos solo una MUESTRA acotada
 *  (la lista está paginada, así que ~1000 filas son seguras y navegables). */
const DETAILS_CAP = 1000;

/** Genera una muestra de hasta DETAILS_CAP detalles ciclando los endpoints de
 *  muestra, con `seq` creciente. Es una MUESTRA del demo: NO refleja el total real
 *  de peticiones (la app nativa sí captura todas, en orden). `plaintext` controla
 *  si los secretos se muestran sin redactar. */
function buildDetails(total: number, plaintext: boolean): SampleDetail[] {
  const template = sampleDetails(plaintext);
  const n = Math.min(Math.max(0, total), DETAILS_CAP);
  const out: SampleDetail[] = [];
  for (let i = 0; i < n; i++) {
    const base = template[i % template.length];
    out.push({ ...base, seq: i + 1 });
  }
  return out;
}

/** Límites de buckets del histograma (mismos que el backend). */
const HISTOGRAM_BOUNDS_MS = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000];

/** Genera un histograma realista (long-tail, pico en 50–250ms) cuyos counts
 *  suman exactamente `total`. Hay 14 buckets: 13 bounds + 1 abierto (≥10s). */
function buildHistogram(total: number): number[] {
  // Pesos relativos por bucket: <1,1-2,2-5,5-10,10-25,25-50,50-100,100-250,
  // 250-500,500-1k,1-2.5k,2.5-5k,5-10k,≥10k
  const weights = [1, 2, 5, 12, 30, 70, 140, 120, 55, 22, 9, 4, 2, 1];
  const sumW = weights.reduce((a, b) => a + b, 0);
  const counts = weights.map((w) => Math.floor((w / sumW) * total));
  // Ajusta el redondeo asignando el resto al bucket modal (100–250ms, idx 7).
  let assigned = counts.reduce((a, b) => a + b, 0);
  let rem = total - assigned;
  let i = 7;
  while (rem > 0) {
    counts[i % counts.length]++;
    rem--;
    i++;
  }
  return counts;
}

/** Códigos de estado coherentes con `errors`: en el flujo httpbin todo éxito es
 *  200 (/post, /get y /uuid devuelven 200; las aserciones piden 200). Los errores
 *  se reparten entre 4xx y 5xx. Las cuentas suman `total`. */
function buildStatusCodes(total: number, errors: number): [number, number][] {
  const c200 = total - errors; // todos los éxitos: 200 OK
  const c503 = Math.ceil(errors * 0.6);
  const c500 = Math.round(errors * 0.2);
  const c429 = Math.round(errors * 0.12);
  const c404 = errors - c503 - c500 - c429;
  const out: [number, number][] = [
    [200, c200],
    [404, Math.max(0, c404)],
    [429, c429],
    [500, c500],
    [503, c503],
  ];
  return out.filter(([, n]) => n > 0);
}

/** Tipos de error coherentes con `errors` (suman `errors`). */
function buildErrorKinds(errors: number): [string, number][] {
  if (errors <= 0) return [];
  const status = Math.ceil(errors * 0.55);
  const timeout = Math.round(errors * 0.2);
  const connection = Math.round(errors * 0.15);
  const assertion = errors - status - timeout - connection;
  const out: [string, number][] = [
    ["status", status],
    ["timeout", timeout],
    ["connection", connection],
    ["assertion", Math.max(0, assertion)],
  ];
  return out.filter(([, n]) => n > 0);
}

/** Heatmap latencia×tiempo: reparte el histograma global por segundo, con una
 *  pequeña "ventana caliente" (picos de latencia) hacia la mitad de la corrida. */
function buildHeatmap(ts: TimePoint[], hist: number[], duration: number): { t_secs: number; counts: number[] }[] {
  const secs = ts.length > 0 ? ts.map((p) => p.t_secs) : Array.from({ length: Math.max(1, Math.round(duration)) }, (_, i) => i + 1);
  const n = secs.length;
  const total = hist.reduce((a, b) => a + b, 0);
  const perSec = total / Math.max(1, n);
  const mid = n / 2;
  return secs.map((t, si) => {
    // Sesgo hacia buckets lentos cerca de la mitad (simula degradación pasajera).
    const heat = Math.exp(-((si - mid) ** 2) / (2 * (n / 6 || 1) ** 2)); // 0..1
    const row = hist.map((c, bi) => {
      const base = (c / Math.max(1, total)) * perSec;
      // Buckets lentos (idx>=8) amplificados por `heat`.
      const factor = bi >= 8 ? 1 + heat * 2.5 : 1 - heat * 0.15;
      return Math.max(0, Math.round(base * factor));
    });
    return { t_secs: t, counts: row };
  });
}

function buildSummary(
  opts: RunOptions,
  total: number,
  errors: number,
  duration: number,
  vus: number,
  ts: TimePoint[],
): RunSummary {
  // Con captura activada generamos una MUESTRA de detalles (tope DETAILS_CAP)
  // para no congelar la webview. La muestra NO determina los conteos: overall.count
  // y los conteos por label reflejan el total REAL simulado (coherente con el
  // gráfico de throughput por segundo). La app nativa sí captura todas.
  const details = opts.capture
    ? buildDetails(total, opts.capture_plaintext ?? false)
    : undefined;
  const overallCount = total;

  const mk = (label: string, kind: "http" | "transaction", count: number, p95: number) => {
    const errors = Math.round(count * 0.005);
    return {
    label,
    kind,
    count,
    errors,
    error_rate: count > 0 ? errors / count : 0,
    throughput_per_sec: count / duration,
    min_ms: 8,
    mean_ms: p95 * 0.6,
    max_ms: p95 * 2.4,
    p50_ms: p95 * 0.45,
    p90_ms: p95 * 0.85,
    p95_ms: p95,
    p99_ms: p95 * 1.6,
    p999_ms: p95 * 2.1,
    bytes_total: count * 820,
    // TTFB ≈ 60–70% del p95 (la red/servidor antes del primer byte).
    ttfb_mean_ms: Math.round(p95 * 0.6 * 0.65),
    ttfb_p95_ms: Math.round(p95 * 0.66),
    sent_bytes: count * 180,
    };
  };
  const overall = mk("ALL", "http", overallCount, 142);
  // Conteos por endpoint que suman EXACTAMENTE overall.count (sin deriva de
  // redondeo). El flujo ejecuta por iteración: 1×login, 1×token, 1×catalog,
  // 2×cart (loop x2) y 1×checkout = 6 peticiones. Repartimos 1/6 a cada uno de
  // los cuatro singleton y el resto (≈2/6) a POST /cart, así cart ≈ 2× el resto.
  const unit = Math.round(overallCount / 6);
  const loginCount = unit;
  const tokenCount = unit;
  const catalogCount = unit;
  const checkoutCount = unit;
  const cartCount = overallCount - loginCount - tokenCount - catalogCount - checkoutCount;
  // Datasets derivados, coherentes con overall.count / overall.errors.
  const histogram = buildHistogram(overall.count);
  const overallErrors = overall.errors;
  const statusCodes = buildStatusCodes(overall.count, overallErrors);
  const errorKinds = buildErrorKinds(overallErrors);
  const heatmap = buildHeatmap(ts, histogram, duration);
  // Bytes recibidos/enviados: suma de los por-label (recibidos 820 B, enviados 180 B).
  const bytesReceived = overall.count * 820;
  const bytesSent = overall.count * 180;

  // Tabla de mensajes de error: reparte los errores en dos mensajes plausibles.
  const errorTable =
    overallErrors > 0
      ? [
          { message: "HTTP 503 Service Unavailable", count: Math.ceil(overallErrors * 0.6) },
          { message: "Connection timed out", count: Math.floor(overallErrors * 0.4) },
        ].filter((e) => e.count > 0)
      : [];

  return {
    run_id: "run-mock",
    scenario_name: DEFAULT_SCENARIO,
    started_at: new Date().toISOString(),
    duration_secs: duration,
    config: { virtual_users: vus, thread_groups: 1 },
    overall,
    labels: [
      // Endpoints HTTP del flujo checkout: sus counts suman exactamente
      // overall.count. POST /cart ≈ 2× el resto (el loop "Agregar 2 productos").
      mk("POST /login", "http", loginCount, 168),
      mk("GET token", "http", tokenCount, 73),
      mk("GET /catalog", "http", catalogCount, 96),
      mk("POST /cart", "http", cartCount, 121),
      mk("POST /checkout", "http", checkoutCount, 152),
      // "Login" es el controlador de transacción que envuelve POST /login + GET
      // token (mismo nº de iteraciones); kind "transaction" no suma al total HTTP.
      mk("Login", "transaction", loginCount, 470),
    ],
    timeseries: ts,
    errors: errorTable,
    histogram_bounds_ms: HISTOGRAM_BOUNDS_MS,
    histogram_counts: histogram,
    status_codes: statusCodes,
    error_kinds: errorKinds,
    bytes_received: bytesReceived,
    bytes_sent: bytesSent,
    latency_heatmap: heatmap,
    ...(details ? { details } : {}),
  };
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ─── Mock del histórico (in-memory) ────────────────────────────────────────────
// Sustituye a los comandos Tauri history_* cuando la UI corre en el navegador.
// Es un store a nivel de módulo (persiste mientras viva la pestaña) sembrado con
// corridas históricas para que la demo muestre tendencias/comparación al instante.

const DEFAULT_SCENARIO = "Checkout de e-commerce (demo)";
const DEFAULT_ENV = "staging";

/** Construye un RunRecord histórico sembrado (id, fecha relativa en días). */
function seedRun(
  id: number,
  daysAgo: number,
  p95: number,
  throughput: number,
  errorRate: number,
  branch: string,
): RunRecord {
  const started = new Date(Date.now() - daysAgo * 24 * 3600 * 1000);
  return {
    id,
    scenario: DEFAULT_SCENARIO,
    branch,
    build: `ci-${1000 + id}`,
    environment: DEFAULT_ENV,
    started_at: started.toISOString(),
    duration_secs: 120,
    throughput,
    error_rate: errorRate,
    p95_ms: p95,
    p99_ms: Math.round(p95 * 1.6),
    requests: Math.round(throughput * 120),
  };
}

/** Store mutable a nivel de módulo. La semilla muestra una mejora de p95 con
 *  un repunte en medio (para que la tendencia y la comparación sean legibles). */
const historyStore: RunRecord[] = [
  seedRun(1, 28, 210, 38, 0.012, "main"),
  seedRun(2, 21, 198, 41, 0.009, "main"),
  seedRun(3, 14, 256, 36, 0.021, "main"), // repunte (peor)
  seedRun(4, 7, 172, 45, 0.006, "main"),
  seedRun(5, 2, 165, 47, 0.005, "main"),
];

let historySeq = historyStore.reduce((m, r) => Math.max(m, r.id), 0);

/** Baselines fijados, indexados por branch∥env∥scenario → runId. */
const baselineStore = new Map<string, number>();
function baselineKey(branch: string, environment: string, scenario: string): string {
  return `${branch} ${environment} ${scenario}`;
}
// Por defecto el baseline es la corrida estable más antigua, para que la demo
// muestre una comparación nada más guardar una corrida nueva.
baselineStore.set(baselineKey("main", DEFAULT_ENV, DEFAULT_SCENARIO), 2);

/** Deriva un RunRecord (sin id) desde el RunSummary actual. */
function recordFromSummary(
  summary: RunSummary,
  meta: { branch?: string; build?: string; environment?: string; commit?: string },
): Omit<RunRecord, "id"> {
  const o = summary.overall;
  return {
    scenario: summary.scenario_name,
    branch: meta.branch || undefined,
    build: meta.build || undefined,
    environment: meta.environment || undefined,
    started_at: summary.started_at,
    duration_secs: summary.duration_secs,
    throughput: o.throughput_per_sec,
    error_rate: o.error_rate,
    p95_ms: o.p95_ms,
    p99_ms: o.p99_ms,
    requests: o.count,
  };
}

export function mockHistoryRecord(args: {
  summary: RunSummary;
  branch?: string;
  build?: string;
  environment?: string;
  commit?: string;
}): number {
  const id = ++historySeq;
  const rec: RunRecord = { id, ...recordFromSummary(args.summary, args) };
  historyStore.push(rec);
  return id;
}

export function mockHistoryList(args: {
  scenario?: string;
  environment?: string;
  limit?: number;
}): RunRecord[] {
  let rows = historyStore.filter(
    (r) =>
      (!args.scenario || r.scenario === args.scenario) &&
      (!args.environment || r.environment === args.environment),
  );
  // Más recientes primero.
  rows = rows
    .slice()
    .sort((a, b) => Date.parse(b.started_at) - Date.parse(a.started_at));
  if (args.limit && args.limit > 0) rows = rows.slice(0, args.limit);
  return rows;
}

export function mockHistorySetBaseline(args: {
  branch: string;
  environment: string;
  scenario: string;
  runId: number;
}): void {
  baselineStore.set(baselineKey(args.branch, args.environment, args.scenario), args.runId);
}

export function mockHistoryCompare(args: {
  runId: number;
  branch: string;
  environment: string;
  scenario: string;
}): Comparison | null {
  const baseId = baselineStore.get(
    baselineKey(args.branch, args.environment, args.scenario),
  );
  if (baseId === undefined) return null;
  const current = historyStore.find((r) => r.id === args.runId);
  const base = historyStore.find((r) => r.id === baseId);
  if (!current || !base) return null;

  const pct = (cur: number, ref: number) =>
    ref === 0 ? 0 : ((cur - ref) / ref) * 100;
  const p95_delta_pct = pct(current.p95_ms, base.p95_ms);
  const throughput_delta_pct = pct(current.throughput, base.throughput);
  const error_rate_delta = current.error_rate - base.error_rate;
  // Regresión: p95 ≥ +10%, o throughput ≤ −10%, o +1 punto porcentual de error.
  const is_regression =
    p95_delta_pct >= 10 || throughput_delta_pct <= -10 || error_rate_delta >= 0.01;
  return { p95_delta_pct, throughput_delta_pct, error_rate_delta, is_regression };
}

export function mockHistoryTrend(args: {
  scenario: string;
  environment: string;
  metric: TrendMetric;
  limit?: number;
}): TrendPoint[] {
  let rows = historyStore.filter(
    (r) => r.scenario === args.scenario && r.environment === args.environment,
  );
  rows = rows
    .slice()
    .sort((a, b) => Date.parse(a.started_at) - Date.parse(b.started_at));
  if (args.limit && args.limit > 0) rows = rows.slice(-args.limit);
  const pick = (r: RunRecord): number =>
    args.metric === "p95"
      ? r.p95_ms
      : args.metric === "throughput"
        ? r.throughput
        : r.error_rate;
  return rows.map((r) => ({ started_at: r.started_at, value: pick(r) }));
}

export function mockEvaluateGate(args: {
  summary: RunSummary;
  thresholds: GateThresholds;
}): GateResult {
  const o = args.summary.overall;
  const t = args.thresholds;
  const checks = [
    {
      label: "Tasa de error",
      pass: o.error_rate <= t.max_error_rate,
      actual: o.error_rate,
      threshold: t.max_error_rate,
    },
    {
      label: "P95",
      pass: o.p95_ms <= t.max_p95_ms,
      actual: o.p95_ms,
      threshold: t.max_p95_ms,
    },
    {
      label: "P99",
      pass: o.p99_ms <= t.max_p99_ms,
      actual: o.p99_ms,
      threshold: t.max_p99_ms,
    },
    {
      label: "Throughput mínimo",
      pass: o.throughput_per_sec >= t.min_throughput_per_sec,
      actual: o.throughput_per_sec,
      threshold: t.min_throughput_per_sec,
    },
  ];
  return { pass: checks.every((c) => c.pass), checks };
}
