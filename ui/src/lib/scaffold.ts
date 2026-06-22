// Fábricas para crear planes y pasos nativos desde cero (sin importar un JMX).
// Devuelven estructuras nuevas (nunca comparten referencias) alineadas con types.ts.

import type {
  HttpRequest,
  InterleaveController,
  IfController,
  KafkaRequest,
  LoopController,
  RandomController,
  Scenario,
  Step,
  ThreadGroup,
  ThroughputController,
  TimerStep,
  Transaction,
  WhileController,
} from "../types";

/** Un plan mínimo pero válido para empezar a construir desde cero. */
export function blankScenario(): Scenario {
  return {
    version: "0.3.0",
    name: "Nuevo plan",
    variables: {},
    datasets: [],
    defaults: { base_url: "" },
    thread_groups: [blankThreadGroup()],
  };
}

/** Un grupo de hilos vacío con una carga razonable por defecto. */
export function blankThreadGroup(): ThreadGroup {
  return {
    name: "Grupo de hilos",
    load: {
      virtual_users: 10,
      ramp_up_secs: 1,
      hold_secs: 0,
      ramp_down_secs: 0,
      iterations: 5,
      duration_secs: null,
    },
    on_error: "continue",
    steps: [],
  };
}

// ─── Fábricas de pasos ─────────────────────────────────────────────────────────

export function newHttpRequest(): HttpRequest {
  return {
    type: "http",
    name: "Nueva petición",
    method: "GET",
    url: "/",
    headers: {},
    query: {},
    assertions: [{ assert: "status_code", codes: [200] }],
    extractors: [],
    timers: [],
  };
}

export function newTransaction(): Transaction {
  return { type: "transaction", name: "Nueva transacción", steps: [] };
}

export function newLoop(): LoopController {
  return { type: "loop", name: "Nuevo bucle", count: 3, steps: [] };
}

export function newIf(): IfController {
  return { type: "if", name: "Condición (If)", condition: "true", steps: [] };
}

export function newWhile(): WhileController {
  return {
    type: "while",
    name: "Bucle While",
    condition: "true",
    steps: [],
    max_iterations: 0,
  };
}

export function newThroughput(): ThroughputController {
  return { type: "throughput", name: "Throughput", percent: 50, steps: [] };
}

export function newInterleave(): InterleaveController {
  return { type: "interleave", name: "Interleave", steps: [] };
}

export function newRandom(): RandomController {
  return { type: "random", name: "Random", steps: [] };
}

export function newTimer(): TimerStep {
  return { type: "timer", timer: "constant", delay_ms: 1000 };
}

export function newKafka(): KafkaRequest {
  return {
    type: "kafka",
    name: "Mensaje Kafka",
    brokers: ["localhost:9092"],
    topic: "topic",
    payload: "",
  };
}

// ─── Catálogo de tipos de paso para el menú "Añadir" ────────────────────────────

export type StepKind = Step["type"];

export interface StepKindOption {
  kind: StepKind;
  label: string;
  factory: () => Step;
}

export const STEP_KIND_OPTIONS: StepKindOption[] = [
  { kind: "http", label: "HTTP Request", factory: newHttpRequest },
  { kind: "transaction", label: "Transacción", factory: newTransaction },
  { kind: "loop", label: "Loop", factory: newLoop },
  { kind: "if", label: "If", factory: newIf },
  { kind: "while", label: "While", factory: newWhile },
  { kind: "throughput", label: "Throughput", factory: newThroughput },
  { kind: "interleave", label: "Interleave", factory: newInterleave },
  { kind: "random", label: "Random", factory: newRandom },
  { kind: "timer", label: "Timer", factory: newTimer },
  { kind: "kafka", label: "Kafka", factory: newKafka },
];
