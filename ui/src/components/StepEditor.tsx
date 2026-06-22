import React from "react";
import type {
  Assertion,
  Extractor,
  HttpRequest,
  KafkaRequest,
  Scenario,
  Step,
  ThreadGroup,
  ThroughputController,
  Timer,
  TimerStep,
} from "../types";
import { isContainerStep } from "../lib/mutate";
import { Button, Field, Input, Select, Textarea, IconPlus, IconTrash } from "./ui";

// ─── Small section header for list editors ──────────────────────────────────────

const SectionRow: React.FC<{
  children: React.ReactNode;
  onRemove: () => void;
}> = ({ children, onRemove }) => (
  <div className="flex items-start gap-2 rounded-lg border border-slate-200 bg-slate-50/60 p-2.5">
    <div className="flex-1 min-w-0 flex flex-col gap-2">{children}</div>
    <Button
      variant="ghost"
      size="sm"
      onClick={onRemove}
      className="text-slate-400 hover:text-red-500 px-1.5 shrink-0"
      title="Quitar"
    >
      <IconTrash />
    </Button>
  </div>
);

// ─── Assertions editor ──────────────────────────────────────────────────────────

const ASSERT_KINDS: { value: Assertion["assert"]; label: string }[] = [
  { value: "status_code", label: "Código de estado" },
  { value: "body_contains", label: "Cuerpo contiene" },
  { value: "body_matches", label: "Cuerpo coincide (regex)" },
  { value: "json_path", label: "JSONPath" },
  { value: "duration_below_ms", label: "Duración menor a (ms)" },
  { value: "size_below_bytes", label: "Tamaño menor a (bytes)" },
];

function defaultAssertion(kind: Assertion["assert"]): Assertion {
  switch (kind) {
    case "status_code":
      return { assert: "status_code", codes: [200] };
    case "body_contains":
      return { assert: "body_contains", substring: "", negate: false };
    case "body_matches":
      return { assert: "body_matches", pattern: "", negate: false };
    case "json_path":
      return { assert: "json_path", path: "$." };
    case "duration_below_ms":
      return { assert: "duration_below_ms", max_ms: 1000 };
    case "size_below_bytes":
      return { assert: "size_below_bytes", max_bytes: 1024 };
  }
}

const AssertionsEditor: React.FC<{
  value: Assertion[];
  onChange: (v: Assertion[]) => void;
}> = ({ value, onChange }) => {
  const update = (i: number, a: Assertion) =>
    onChange(value.map((x, idx) => (idx === i ? a : x)));
  const remove = (i: number) => onChange(value.filter((_, idx) => idx !== i));

  return (
    <div className="flex flex-col gap-2">
      {value.map((a, i) => (
        <SectionRow key={i} onRemove={() => remove(i)}>
          <Select
            value={a.assert}
            onChange={(e) => update(i, defaultAssertion(e.target.value as Assertion["assert"]))}
            className="text-xs"
          >
            {ASSERT_KINDS.map((k) => (
              <option key={k.value} value={k.value}>{k.label}</option>
            ))}
          </Select>

          {a.assert === "status_code" && (
            <Input
              value={a.codes.join(", ")}
              placeholder="200, 201"
              onChange={(e) =>
                update(i, {
                  assert: "status_code",
                  codes: e.target.value
                    .split(",")
                    .map((c) => Number(c.trim()))
                    .filter((n) => !Number.isNaN(n)),
                })
              }
              className="text-xs font-mono tabular-nums"
            />
          )}

          {(a.assert === "body_contains" || a.assert === "body_matches") && (
            <>
              <Input
                value={a.assert === "body_contains" ? a.substring : a.pattern}
                placeholder={a.assert === "body_contains" ? "texto a buscar" : "expresión regular"}
                onChange={(e) =>
                  update(
                    i,
                    a.assert === "body_contains"
                      ? { ...a, substring: e.target.value }
                      : { ...a, pattern: e.target.value },
                  )
                }
                className="text-xs font-mono"
              />
              <label className="flex items-center gap-1.5 text-xs text-slate-600">
                <input
                  type="checkbox"
                  checked={a.negate}
                  onChange={(e) => update(i, { ...a, negate: e.target.checked })}
                  className="rounded border-slate-300"
                />
                Negar (no debe cumplirse)
              </label>
            </>
          )}

          {a.assert === "json_path" && (
            <>
              <Input
                value={a.path}
                placeholder="$.token"
                onChange={(e) => update(i, { ...a, path: e.target.value })}
                className="text-xs font-mono"
              />
              <div className="flex gap-1.5">
                <Input
                  value={a.equals ?? ""}
                  placeholder="igual a (opcional)"
                  onChange={(e) =>
                    update(i, { ...a, equals: e.target.value || undefined })
                  }
                  className="text-xs font-mono flex-1"
                />
                <label className="flex items-center gap-1.5 text-xs text-slate-600 shrink-0 px-1">
                  <input
                    type="checkbox"
                    checked={a.exists ?? false}
                    onChange={(e) =>
                      update(i, { ...a, exists: e.target.checked || undefined })
                    }
                    className="rounded border-slate-300"
                  />
                  existe
                </label>
              </div>
            </>
          )}

          {a.assert === "duration_below_ms" && (
            <Input
              type="number"
              min={0}
              value={a.max_ms}
              onChange={(e) => update(i, { ...a, max_ms: Number(e.target.value) })}
              className="text-xs tabular-nums"
            />
          )}

          {a.assert === "size_below_bytes" && (
            <Input
              type="number"
              min={0}
              value={a.max_bytes}
              onChange={(e) => update(i, { ...a, max_bytes: Number(e.target.value) })}
              className="text-xs tabular-nums"
            />
          )}
        </SectionRow>
      ))}
      <Button
        variant="ghost"
        size="sm"
        icon={<IconPlus />}
        onClick={() => onChange([...value, defaultAssertion("status_code")])}
        className="self-start text-slate-500"
      >
        Agregar assertion
      </Button>
    </div>
  );
};

// ─── Extractors editor ──────────────────────────────────────────────────────────

const EXTRACT_KINDS: { value: Extractor["extract"]; label: string }[] = [
  { value: "regex", label: "Regex" },
  { value: "json_path", label: "JSONPath" },
  { value: "boundary", label: "Delimitadores" },
];

function defaultExtractor(kind: Extractor["extract"]): Extractor {
  switch (kind) {
    case "regex":
      return { extract: "regex", var: "var", pattern: "", group: 1 };
    case "json_path":
      return { extract: "json_path", var: "var", path: "$." };
    case "boundary":
      return { extract: "boundary", var: "var", left: "", right: "" };
  }
}

const ExtractorsEditor: React.FC<{
  value: Extractor[];
  onChange: (v: Extractor[]) => void;
}> = ({ value, onChange }) => {
  const update = (i: number, e: Extractor) =>
    onChange(value.map((x, idx) => (idx === i ? e : x)));
  const remove = (i: number) => onChange(value.filter((_, idx) => idx !== i));

  return (
    <div className="flex flex-col gap-2">
      {value.map((ex, i) => (
        <SectionRow key={i} onRemove={() => remove(i)}>
          <div className="flex gap-1.5">
            <Select
              value={ex.extract}
              onChange={(e) =>
                update(i, defaultExtractor(e.target.value as Extractor["extract"]))
              }
              className="text-xs w-32 shrink-0"
            >
              {EXTRACT_KINDS.map((k) => (
                <option key={k.value} value={k.value}>{k.label}</option>
              ))}
            </Select>
            <Input
              value={ex.var}
              placeholder="variable"
              onChange={(e) => update(i, { ...ex, var: e.target.value })}
              className="text-xs font-mono flex-1"
            />
          </div>

          {ex.extract === "regex" && (
            <>
              <Input
                value={ex.pattern}
                placeholder="expresión regular"
                onChange={(e) => update(i, { ...ex, pattern: e.target.value })}
                className="text-xs font-mono"
              />
              <div className="flex gap-1.5">
                <Input
                  type="number"
                  min={0}
                  value={ex.group}
                  onChange={(e) => update(i, { ...ex, group: Number(e.target.value) })}
                  className="text-xs tabular-nums w-20 shrink-0"
                  title="Grupo de captura"
                />
                <Input
                  value={ex.default ?? ""}
                  placeholder="valor por defecto (opcional)"
                  onChange={(e) =>
                    update(i, { ...ex, default: e.target.value || undefined })
                  }
                  className="text-xs font-mono flex-1"
                />
              </div>
            </>
          )}

          {ex.extract === "json_path" && (
            <div className="flex gap-1.5">
              <Input
                value={ex.path}
                placeholder="$.token"
                onChange={(e) => update(i, { ...ex, path: e.target.value })}
                className="text-xs font-mono flex-1"
              />
              <Input
                value={ex.default ?? ""}
                placeholder="por defecto"
                onChange={(e) =>
                  update(i, { ...ex, default: e.target.value || undefined })
                }
                className="text-xs font-mono w-28 shrink-0"
              />
            </div>
          )}

          {ex.extract === "boundary" && (
            <>
              <div className="flex gap-1.5">
                <Input
                  value={ex.left}
                  placeholder="izquierda"
                  onChange={(e) => update(i, { ...ex, left: e.target.value })}
                  className="text-xs font-mono flex-1"
                />
                <Input
                  value={ex.right}
                  placeholder="derecha"
                  onChange={(e) => update(i, { ...ex, right: e.target.value })}
                  className="text-xs font-mono flex-1"
                />
              </div>
              <Input
                value={ex.default ?? ""}
                placeholder="valor por defecto (opcional)"
                onChange={(e) =>
                  update(i, { ...ex, default: e.target.value || undefined })
                }
                className="text-xs font-mono"
              />
            </>
          )}
        </SectionRow>
      ))}
      <Button
        variant="ghost"
        size="sm"
        icon={<IconPlus />}
        onClick={() => onChange([...value, defaultExtractor("json_path")])}
        className="self-start text-slate-500"
      >
        Agregar extractor
      </Button>
    </div>
  );
};

// ─── Timers editor (used for HTTP timers and the standalone Timer step) ──────────

const TIMER_KINDS: { value: Timer["timer"]; label: string }[] = [
  { value: "constant", label: "Constante" },
  { value: "uniform_random", label: "Aleatorio uniforme" },
  { value: "gaussian", label: "Gaussiano" },
  { value: "constant_throughput", label: "Throughput constante" },
];

function defaultTimer(kind: Timer["timer"]): Timer {
  switch (kind) {
    case "constant":
      return { timer: "constant", delay_ms: 1000 };
    case "uniform_random":
      return { timer: "uniform_random", base_ms: 200, range_ms: 400 };
    case "gaussian":
      return { timer: "gaussian", offset_ms: 300, deviation_ms: 100 };
    case "constant_throughput":
      return { timer: "constant_throughput", target_per_minute: 60 };
  }
}

const TimerFields: React.FC<{
  timer: Timer;
  onChange: (t: Timer) => void;
}> = ({ timer, onChange }) => (
  <>
    <Select
      value={timer.timer}
      onChange={(e) => onChange(defaultTimer(e.target.value as Timer["timer"]))}
      className="text-xs"
    >
      {TIMER_KINDS.map((k) => (
        <option key={k.value} value={k.value}>{k.label}</option>
      ))}
    </Select>

    {timer.timer === "constant" && (
      <Field label="Pausa (ms)">
        <Input
          type="number"
          min={0}
          value={timer.delay_ms}
          onChange={(e) => onChange({ ...timer, delay_ms: Number(e.target.value) })}
          className="text-xs tabular-nums"
        />
      </Field>
    )}

    {timer.timer === "uniform_random" && (
      <div className="grid grid-cols-2 gap-2">
        <Field label="Base (ms)">
          <Input
            type="number"
            min={0}
            value={timer.base_ms}
            onChange={(e) => onChange({ ...timer, base_ms: Number(e.target.value) })}
            className="text-xs tabular-nums"
          />
        </Field>
        <Field label="Rango (ms)">
          <Input
            type="number"
            min={0}
            value={timer.range_ms}
            onChange={(e) => onChange({ ...timer, range_ms: Number(e.target.value) })}
            className="text-xs tabular-nums"
          />
        </Field>
      </div>
    )}

    {timer.timer === "gaussian" && (
      <div className="grid grid-cols-2 gap-2">
        <Field label="Offset (ms)">
          <Input
            type="number"
            min={0}
            value={timer.offset_ms}
            onChange={(e) => onChange({ ...timer, offset_ms: Number(e.target.value) })}
            className="text-xs tabular-nums"
          />
        </Field>
        <Field label="Desviación (ms)">
          <Input
            type="number"
            min={0}
            value={timer.deviation_ms}
            onChange={(e) =>
              onChange({ ...timer, deviation_ms: Number(e.target.value) })
            }
            className="text-xs tabular-nums"
          />
        </Field>
      </div>
    )}

    {timer.timer === "constant_throughput" && (
      <Field label="Objetivo por minuto">
        <Input
          type="number"
          min={0}
          value={timer.target_per_minute}
          onChange={(e) =>
            onChange({ ...timer, target_per_minute: Number(e.target.value) })
          }
          className="text-xs tabular-nums"
        />
      </Field>
    )}
  </>
);

const TimersEditor: React.FC<{
  value: Timer[];
  onChange: (v: Timer[]) => void;
}> = ({ value, onChange }) => {
  const update = (i: number, t: Timer) =>
    onChange(value.map((x, idx) => (idx === i ? t : x)));
  const remove = (i: number) => onChange(value.filter((_, idx) => idx !== i));

  return (
    <div className="flex flex-col gap-2">
      {value.map((t, i) => (
        <SectionRow key={i} onRemove={() => remove(i)}>
          <TimerFields timer={t} onChange={(nt) => update(i, nt)} />
        </SectionRow>
      ))}
      <Button
        variant="ghost"
        size="sm"
        icon={<IconPlus />}
        onClick={() => onChange([...value, defaultTimer("constant")])}
        className="self-start text-slate-500"
      >
        Agregar temporizador
      </Button>
    </div>
  );
};

// ─── Key-Value editor ─────────────────────────────────────────────────────────

interface KVEditorProps {
  value: Record<string, string>;
  onChange: (v: Record<string, string>) => void;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
}

const KVEditor: React.FC<KVEditorProps> = ({
  value,
  onChange,
  keyPlaceholder = "Key",
  valuePlaceholder = "Value",
}) => {
  const entries = Object.entries(value);

  const set = (k: string, v: string, idx: number) => {
    const next: Record<string, string> = {};
    entries.forEach(([ek, ev], i) => {
      const nk = i === idx ? k : ek;
      const nv = i === idx ? v : ev;
      if (nk) next[nk] = nv;
    });
    onChange(next);
  };

  const remove = (idx: number) => {
    const next: Record<string, string> = {};
    entries.forEach(([k, v], i) => {
      if (i !== idx && k) next[k] = v;
    });
    onChange(next);
  };

  const add = () => {
    onChange({ ...value, "": "" });
  };

  return (
    <div className="flex flex-col gap-1.5">
      {entries.map(([k, v], i) => (
        <div key={i} className="flex gap-1.5 items-center">
          <Input
            value={k}
            placeholder={keyPlaceholder}
            onChange={(e) => set(e.target.value, v, i)}
            className="flex-1 text-xs"
          />
          <Input
            value={v}
            placeholder={valuePlaceholder}
            onChange={(e) => set(k, e.target.value, i)}
            className="flex-1 text-xs font-mono"
          />
          <Button
            variant="ghost"
            size="sm"
            onClick={() => remove(i)}
            className="text-slate-400 hover:text-red-500 px-1.5"
          >
            <IconTrash />
          </Button>
        </div>
      ))}
      <Button variant="ghost" size="sm" icon={<IconPlus />} onClick={add} className="self-start text-slate-500">
        Agregar fila
      </Button>
    </div>
  );
};

// ─── HTTP Request editor ──────────────────────────────────────────────────────

interface HttpEditorProps {
  request: HttpRequest;
  onChange: (r: HttpRequest) => void;
}

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

const HttpEditor: React.FC<HttpEditorProps> = ({ request, onChange }) => {
  const up = (patch: Partial<HttpRequest>) => onChange({ ...request, ...patch });

  const bodyKind = request.body?.kind ?? "raw";

  const handleBodyKindChange = (kind: "raw" | "form") => {
    if (kind === "raw") {
      up({ body: { kind: "raw", data: "", content_type: "application/json" } });
    } else {
      up({ body: { kind: "form", fields: {} } });
    }
  };

  return (
    <div className="flex flex-col gap-5 p-4">
      {/* Name */}
      <Field label="Nombre">
        <Input value={request.name} onChange={(e) => up({ name: e.target.value })} />
      </Field>

      {/* Method + URL */}
      <div className="flex gap-2">
        <Field label="Método" className="w-28 shrink-0">
          <Select
            value={request.method}
            onChange={(e) => up({ method: e.target.value })}
          >
            {METHODS.map((m) => (
              <option key={m} value={m}>{m}</option>
            ))}
          </Select>
        </Field>
        <Field label="URL" className="flex-1">
          <Input
            value={request.url}
            onChange={(e) => up({ url: e.target.value })}
            className="font-mono text-xs"
            placeholder="https://… o ruta relativa"
          />
        </Field>
      </div>

      {/* Headers */}
      <Field label="Cabeceras">
        <KVEditor
          value={request.headers ?? {}}
          onChange={(h) => up({ headers: h })}
          keyPlaceholder="Content-Type"
          valuePlaceholder="application/json"
        />
      </Field>

      {/* Query params */}
      <Field label="Parámetros de consulta">
        <KVEditor
          value={request.query ?? {}}
          onChange={(q) => up({ query: q })}
          keyPlaceholder="page"
          valuePlaceholder="1"
        />
      </Field>

      {/* Body */}
      <Field label="Cuerpo">
        <div className="flex gap-2 mb-2">
          {(["raw", "form"] as const).map((k) => (
            <button
              key={k}
              onClick={() => handleBodyKindChange(k)}
              className={`px-3 py-1 text-xs font-medium rounded-md transition-colors ${
                bodyKind === k
                  ? "bg-indigo-600 text-white"
                  : "bg-slate-100 text-slate-600 hover:bg-slate-200"
              }`}
            >
              {k === "raw" ? "Raw / JSON" : "Formulario"}
            </button>
          ))}
        </div>
        {bodyKind === "raw" ? (
          <Textarea
            value={(request.body as { kind: "raw"; data: string } | undefined)?.data ?? ""}
            onChange={(e) => {
              const existing = request.body?.kind === "raw" ? request.body : { kind: "raw" as const, data: "", content_type: "application/json" };
              up({ body: { ...existing, data: e.target.value } });
            }}
            placeholder='{"key": "value"}'
            rows={5}
            className="text-xs"
          />
        ) : (
          <KVEditor
            value={(request.body as { kind: "form"; fields: Record<string, string> } | undefined)?.fields ?? {}}
            onChange={(fields) => up({ body: { kind: "form", fields } })}
            keyPlaceholder="campo"
            valuePlaceholder="valor"
          />
        )}
      </Field>

      {/* Assertions (editable) */}
      <Field label="Assertions">
        <AssertionsEditor
          value={request.assertions ?? []}
          onChange={(assertions) => up({ assertions })}
        />
      </Field>

      {/* Extractors (editable) */}
      <Field label="Extractores">
        <ExtractorsEditor
          value={request.extractors ?? []}
          onChange={(extractors) => up({ extractors })}
        />
      </Field>

      {/* Timers (editable) */}
      <Field label="Temporizadores">
        <TimersEditor
          value={request.timers ?? []}
          onChange={(timers) => up({ timers })}
        />
      </Field>
    </div>
  );
};

// ─── Kafka editor ────────────────────────────────────────────────────────────

const KafkaEditor: React.FC<{
  request: KafkaRequest;
  onChange: (r: KafkaRequest) => void;
}> = ({ request, onChange }) => {
  const up = (patch: Partial<KafkaRequest>) => onChange({ ...request, ...patch });

  return (
    <div className="flex flex-col gap-5 p-4">
      <Field label="Nombre">
        <Input value={request.name} onChange={(e) => up({ name: e.target.value })} />
      </Field>

      <Field label="Brokers" help="Una dirección host:puerto por línea">
        <Textarea
          value={request.brokers.join("\n")}
          onChange={(e) =>
            up({
              brokers: e.target.value
                .split("\n")
                .map((b) => b.trim())
                .filter(Boolean),
            })
          }
          placeholder="localhost:9092"
          rows={2}
          className="text-xs"
        />
      </Field>

      <div className="grid grid-cols-2 gap-4">
        <Field label="Topic">
          <Input
            value={request.topic}
            onChange={(e) => up({ topic: e.target.value })}
            className="font-mono text-xs"
          />
        </Field>
        <Field label="Partición" help="Opcional">
          <Input
            type="number"
            min={0}
            value={request.partition ?? ""}
            onChange={(e) =>
              up({ partition: e.target.value === "" ? undefined : Number(e.target.value) })
            }
            className="tabular-nums"
          />
        </Field>
      </div>

      <Field label="Clave (key)" help="Opcional">
        <Input
          value={request.key ?? ""}
          onChange={(e) => up({ key: e.target.value || undefined })}
          className="font-mono text-xs"
        />
      </Field>

      <Field label="Payload">
        <Textarea
          value={request.payload}
          onChange={(e) => up({ payload: e.target.value })}
          placeholder='{"event":"value"}'
          rows={5}
          className="text-xs"
        />
      </Field>

      <Field label="Cabeceras">
        <KVEditor
          value={request.headers ?? {}}
          onChange={(headers) => up({ headers })}
          keyPlaceholder="trace-id"
          valuePlaceholder="abc123"
        />
      </Field>
    </div>
  );
};

// ─── Standalone Timer step editor ──────────────────────────────────────────────

const TimerStepEditor: React.FC<{
  step: TimerStep;
  onChange: (t: TimerStep) => void;
}> = ({ step, onChange }) => (
  <div className="flex flex-col gap-4 p-4">
    <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">
      Temporizador
    </p>
    <TimerFields
      timer={step}
      onChange={(t) => onChange({ ...t, type: "timer" })}
    />
  </div>
);

// ─── Thread Group editor ──────────────────────────────────────────────────────

interface GroupEditorProps {
  group: ThreadGroup;
  gi: number;
  onChange: (gi: number, g: ThreadGroup) => void;
}

const GroupEditor: React.FC<GroupEditorProps> = ({ group, gi, onChange }) => {
  const up = (patch: Partial<ThreadGroup>) => onChange(gi, { ...group, ...patch });
  const upLoad = (patch: Partial<ThreadGroup["load"]>) =>
    up({ load: { ...group.load, ...patch } });

  return (
    <div className="flex flex-col gap-5 p-4">
      <Field label="Nombre del grupo">
        <Input value={group.name} onChange={(e) => up({ name: e.target.value })} />
      </Field>

      <div className="grid grid-cols-2 gap-4">
        <Field label="Usuarios virtuales">
          <Input
            type="number"
            min={1}
            value={group.load.virtual_users}
            onChange={(e) => upLoad({ virtual_users: Number(e.target.value) })}
            className="tabular-nums"
          />
        </Field>
        <Field label="Rampa de subida (s)">
          <Input
            type="number"
            min={0}
            value={group.load.ramp_up_secs}
            onChange={(e) => upLoad({ ramp_up_secs: Number(e.target.value) })}
            className="tabular-nums"
          />
        </Field>
        <Field label="Iteraciones" help="Dejar en 0 para usar duración">
          <Input
            type="number"
            min={0}
            value={group.load.iterations ?? 0}
            onChange={(e) => upLoad({ iterations: Number(e.target.value) || null })}
            className="tabular-nums"
          />
        </Field>
        <Field label="Duración (s)" help="Dejar en 0 para usar iteraciones">
          <Input
            type="number"
            min={0}
            value={group.load.duration_secs ?? 0}
            onChange={(e) => upLoad({ duration_secs: Number(e.target.value) || null })}
            className="tabular-nums"
          />
        </Field>
      </div>

      <Field label="En caso de error">
        <Select
          value={group.on_error}
          onChange={(e) =>
            up({ on_error: e.target.value as ThreadGroup["on_error"] })
          }
        >
          <option value="continue">Continuar</option>
          <option value="stop_thread">Detener hilo</option>
          <option value="stop_test">Detener prueba</option>
        </Select>
      </Field>
    </div>
  );
};

// ─── Root / scenario editor ───────────────────────────────────────────────────

interface RootEditorProps {
  scenario: Scenario;
  onChange: (s: Scenario) => void;
}

const RootEditor: React.FC<RootEditorProps> = ({ scenario, onChange }) => {
  const up = (patch: Partial<Scenario>) => onChange({ ...scenario, ...patch });

  return (
    <div className="flex flex-col gap-5 p-4">
      <Field label="Nombre del escenario">
        <Input value={scenario.name} onChange={(e) => up({ name: e.target.value })} />
      </Field>

      <Field label="URL base" help="Prefijo para todas las peticiones sin URL absoluta">
        <Input
          value={scenario.defaults?.base_url ?? ""}
          onChange={(e) =>
            up({ defaults: { ...scenario.defaults, base_url: e.target.value } })
          }
          placeholder="https://api.example.com"
          className="font-mono text-xs"
        />
      </Field>

      <Field label="Variables globales">
        <KVEditor
          value={scenario.variables ?? {}}
          onChange={(v) => up({ variables: v })}
          keyPlaceholder="VARIABLE"
          valuePlaceholder="valor"
        />
      </Field>

      {scenario.datasets && scenario.datasets.length > 0 && (
        <Field label="Datasets CSV">
          <div className="flex flex-col gap-2">
            {scenario.datasets.map((ds, i) => (
              <div
                key={i}
                className="flex items-center gap-2 px-3 py-2 rounded-lg bg-slate-50 border border-slate-200 text-sm"
              >
                <span className="text-slate-400">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
                    <polyline points="13 2 13 9 20 9"/>
                  </svg>
                </span>
                <span className="font-medium text-slate-700">{ds.name}</span>
                <span className="text-slate-400 text-xs">{ds.path}</span>
                <span className="text-slate-400 text-xs ml-auto">
                  {ds.variable_names.join(", ")}
                </span>
              </div>
            ))}
          </div>
        </Field>
      )}
    </div>
  );
};

// ─── Main StepEditor export ───────────────────────────────────────────────────

interface StepEditorProps {
  selected:
    | { kind: "root"; scenario: Scenario }
    | { kind: "group"; group: ThreadGroup; gi: number }
    | { kind: "step"; step: Step; gi: number; path: number[] }
    | null;
  scenario: Scenario;
  onScenarioChange: (s: Scenario) => void;
}

function setNestedStep(
  steps: Step[],
  pathTail: number[],
  updater: (s: Step) => Step,
): Step[] {
  return steps.map((s, i) => {
    if (i !== pathTail[0]) return s;
    if (pathTail.length === 1) return updater(s);
    if (isContainerStep(s)) {
      return { ...s, steps: setNestedStep(s.steps, pathTail.slice(1), updater) } as Step;
    }
    return s;
  });
}

export const StepEditor: React.FC<StepEditorProps> = ({
  selected,
  scenario,
  onScenarioChange,
}) => {
  if (!selected) {
    return (
      <div className="flex items-center justify-center h-full text-slate-400 text-sm">
        Selecciona un elemento del plan para editarlo.
      </div>
    );
  }

  if (selected.kind === "root") {
    return (
      <RootEditor
        scenario={selected.scenario}
        onChange={onScenarioChange}
      />
    );
  }

  if (selected.kind === "group") {
    return (
      <GroupEditor
        group={selected.group}
        gi={selected.gi}
        onChange={(gi, g) => {
          const groups = scenario.thread_groups.map((tg, i) => (i === gi ? g : tg));
          onScenarioChange({ ...scenario, thread_groups: groups });
        }}
      />
    );
  }

  // Step
  const { step, gi, path } = selected;

  // Helper to replace this step in the scenario via the path.
  const replaceStep = (updated: Step) => {
    const groups = scenario.thread_groups.map((tg, i) => {
      if (i !== gi) return tg;
      return { ...tg, steps: setNestedStep(tg.steps, path, () => updated) };
    });
    onScenarioChange({ ...scenario, thread_groups: groups });
  };

  if (step.type === "timer") {
    return <TimerStepEditor step={step} onChange={replaceStep} />;
  }

  if (step.type === "http") {
    return <HttpEditor request={step} onChange={replaceStep} />;
  }

  if (step.type === "kafka") {
    return <KafkaEditor request={step} onChange={replaceStep} />;
  }

  // Controllers (transaction / loop / if / while / throughput / interleave / random)
  return (
    <div className="p-4 flex flex-col gap-4">
      <Field label="Nombre">
        <Input
          value={step.name}
          onChange={(e) => replaceStep({ ...step, name: e.target.value } as Step)}
        />
      </Field>

      {step.type === "loop" && (
        <Field label="Iteraciones">
          <Input
            type="number"
            min={1}
            value={step.count}
            onChange={(e) =>
              replaceStep({ ...step, count: Number(e.target.value) })
            }
            className="tabular-nums"
          />
        </Field>
      )}

      {(step.type === "if" || step.type === "while") && (
        <Field label="Condición">
          <Input
            value={step.condition}
            onChange={(e) => replaceStep({ ...step, condition: e.target.value })}
            className="font-mono text-xs"
          />
        </Field>
      )}

      {step.type === "while" && (
        <Field label="Máx. iteraciones" help="0 = sin límite">
          <Input
            type="number"
            min={0}
            value={step.max_iterations}
            onChange={(e) =>
              replaceStep({ ...step, max_iterations: Number(e.target.value) })
            }
            className="tabular-nums"
          />
        </Field>
      )}

      {step.type === "throughput" && (
        <Field label="Porcentaje (%)" help="Proporción de ejecuciones que entran al controlador">
          <Input
            type="number"
            min={0}
            max={100}
            value={(step as ThroughputController).percent}
            onChange={(e) =>
              replaceStep({ ...step, percent: Number(e.target.value) })
            }
            className="tabular-nums"
          />
        </Field>
      )}

      <p className="text-xs text-slate-400 mt-2">
        Este nodo contiene {isContainerStep(step) ? step.steps.length : 0} paso(s)
        anidado(s). Usa la barra de acciones para añadir pasos dentro, o selecciónalos
        en el árbol para editarlos individualmente.
      </p>
    </div>
  );
};
