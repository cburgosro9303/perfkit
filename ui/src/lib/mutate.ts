// Helpers inmutables para autorar el árbol del plan (añadir / borrar / duplicar /
// reordenar). Operan sobre Scenario + NodeId y NUNCA mutan la entrada: cada
// función devuelve un nuevo Scenario y una nueva NodeId de selección cuando aplica.

import type { NodeId } from "../components/PlanTree";
import type { Scenario, Step, ThreadGroup } from "../types";
import { blankThreadGroup } from "./scaffold";

// ─── Inspección de contenedores ─────────────────────────────────────────────────

/** Tipos de paso que pueden contener pasos anidados (tienen `.steps`). */
export function isContainerStep(step: Step): step is Extract<Step, { steps: Step[] }> {
  switch (step.type) {
    case "transaction":
    case "loop":
    case "if":
    case "while":
    case "throughput":
    case "interleave":
    case "random":
      return true;
    default:
      return false;
  }
}

/** Hijos de un paso (vacío para hojas: http / timer / kafka). */
function childrenOf(step: Step): Step[] {
  return isContainerStep(step) ? step.steps : [];
}

// ─── Edición inmutable de listas de pasos por ruta ──────────────────────────────

/**
 * Devuelve una copia de `steps` aplicando `fn` a la lista que contiene al paso
 * apuntado por `path` (la ruta completa hasta el paso). `fn` recibe la lista del
 * contenedor y el índice del paso dentro de ella, y devuelve la nueva lista.
 */
function editParentList(
  steps: Step[],
  path: number[],
  fn: (siblings: Step[], index: number) => Step[],
): Step[] {
  if (path.length === 0) return steps;
  const [head, ...rest] = path;
  if (rest.length === 0) {
    // El padre es esta misma lista.
    return fn(steps, head);
  }
  // Hay que descender dentro del contenedor en `head`.
  return steps.map((s, i) => {
    if (i !== head) return s;
    if (!isContainerStep(s)) return s; // ruta inválida: no tocar
    return { ...s, steps: editParentList(s.steps, rest, fn) } as Step;
  });
}

/** Devuelve la lista de pasos del contenedor apuntado por `path` (o null). */
function listAtContainer(steps: Step[], path: number[]): Step[] | null {
  let cur = steps;
  for (let i = 0; i < path.length; i++) {
    const idx = path[i];
    const node = cur[idx];
    if (!node || !isContainerStep(node)) return null;
    cur = node.steps;
  }
  return cur;
}

/** Devuelve el paso apuntado por `path` (o null). */
function stepAt(steps: Step[], path: number[]): Step | null {
  let cur = steps;
  let node: Step | null = null;
  for (let i = 0; i < path.length; i++) {
    const idx = path[i];
    node = cur[idx] ?? null;
    if (!node) return null;
    if (i < path.length - 1) cur = childrenOf(node);
  }
  return node;
}

/** Inserta `step` en la lista `steps` del grupo `gi`, devolviendo nuevo Scenario. */
function withGroupSteps(
  s: Scenario,
  gi: number,
  fn: (steps: Step[]) => Step[],
): Scenario {
  return {
    ...s,
    thread_groups: s.thread_groups.map((g, i) =>
      i === gi ? { ...g, steps: fn(g.steps) } : g,
    ),
  };
}

// ─── Deep clone ─────────────────────────────────────────────────────────────────

function clone<T>(v: T): T {
  // structuredClone existe en navegadores modernos y en el WebView de Tauri 2.
  if (typeof structuredClone === "function") return structuredClone(v);
  return JSON.parse(JSON.stringify(v)) as T;
}

// ─── Resultado de mutación (Scenario + selección sugerida) ──────────────────────

export interface MutateResult {
  scenario: Scenario;
  select: NodeId;
}

// ─── Añadir grupo de hilos ──────────────────────────────────────────────────────

export function addThreadGroup(s: Scenario): MutateResult {
  const group: ThreadGroup = blankThreadGroup();
  const scenario: Scenario = { ...s, thread_groups: [...s.thread_groups, group] };
  return { scenario, select: { kind: "group", gi: scenario.thread_groups.length - 1 } };
}

// ─── Añadir paso ────────────────────────────────────────────────────────────────

/**
 * Inserta `step`:
 *  - root  → lo añade al primer grupo (creándolo si no hay ninguno).
 *  - group → lo añade al final de los pasos del grupo.
 *  - step contenedor → lo añade al final de sus `steps`.
 *  - step hoja → lo inserta como hermano siguiente dentro del padre.
 */
export function addStep(s: Scenario, target: NodeId, step: Step): MutateResult {
  // root → primer grupo (crear si hace falta)
  if (target.kind === "root") {
    if (s.thread_groups.length === 0) {
      const group: ThreadGroup = { ...blankThreadGroup(), steps: [step] };
      const scenario: Scenario = { ...s, thread_groups: [group] };
      return { scenario, select: { kind: "step", gi: 0, path: [0] } };
    }
    const scenario = withGroupSteps(s, 0, (steps) => [...steps, step]);
    const gi = 0;
    const newIndex = s.thread_groups[0].steps.length;
    return { scenario, select: { kind: "step", gi, path: [newIndex] } };
  }

  // group → append a sus pasos
  if (target.kind === "group") {
    const group = s.thread_groups[target.gi];
    if (!group) return { scenario: s, select: target };
    const scenario = withGroupSteps(s, target.gi, (steps) => [...steps, step]);
    return {
      scenario,
      select: { kind: "step", gi: target.gi, path: [group.steps.length] },
    };
  }

  // step
  const group = s.thread_groups[target.gi];
  if (!group) return { scenario: s, select: target };
  const node = stepAt(group.steps, target.path);
  if (!node) return { scenario: s, select: target };

  if (isContainerStep(node)) {
    // Append dentro del contenedor.
    const childList = listAtContainer(group.steps, target.path);
    const childCount = childList ? childList.length : 0;
    const scenario = withGroupSteps(s, target.gi, (steps) =>
      editParentList(steps, target.path, (siblings, idx) =>
        siblings.map((sib, i) =>
          i === idx && isContainerStep(sib)
            ? ({ ...sib, steps: [...sib.steps, step] } as Step)
            : sib,
        ),
      ),
    );
    return {
      scenario,
      select: { kind: "step", gi: target.gi, path: [...target.path, childCount] },
    };
  }

  // Hoja → insertar como hermano siguiente.
  const insertIdx = target.path[target.path.length - 1] + 1;
  const scenario = withGroupSteps(s, target.gi, (steps) =>
    editParentList(steps, target.path, (siblings, idx) => {
      const next = siblings.slice();
      next.splice(idx + 1, 0, step);
      return next;
    }),
  );
  const newPath = [...target.path.slice(0, -1), insertIdx];
  return { scenario, select: { kind: "step", gi: target.gi, path: newPath } };
}

// ─── Borrar nodo ────────────────────────────────────────────────────────────────

export function deleteNode(s: Scenario, target: NodeId): MutateResult {
  if (target.kind === "root") return { scenario: s, select: target };

  if (target.kind === "group") {
    const thread_groups = s.thread_groups.filter((_, i) => i !== target.gi);
    const scenario: Scenario = { ...s, thread_groups };
    // Selecciona el grupo previo, o root.
    if (thread_groups.length === 0) return { scenario, select: { kind: "root" } };
    const gi = Math.max(0, target.gi - 1);
    return { scenario, select: { kind: "group", gi } };
  }

  const group = s.thread_groups[target.gi];
  if (!group) return { scenario: s, select: target };
  const scenario = withGroupSteps(s, target.gi, (steps) =>
    editParentList(steps, target.path, (siblings, idx) =>
      siblings.filter((_, i) => i !== idx),
    ),
  );

  // Selección tras borrar: hermano previo, padre, o grupo.
  const removedIdx = target.path[target.path.length - 1];
  const parentPath = target.path.slice(0, -1);
  const newGroup = scenario.thread_groups[target.gi];
  const siblingList =
    parentPath.length === 0
      ? newGroup.steps
      : listAtContainer(newGroup.steps, parentPath) ?? [];
  let select: NodeId;
  if (siblingList.length === 0) {
    select =
      parentPath.length === 0
        ? { kind: "group", gi: target.gi }
        : { kind: "step", gi: target.gi, path: parentPath };
  } else {
    const newIdx = Math.min(removedIdx, siblingList.length - 1);
    select = { kind: "step", gi: target.gi, path: [...parentPath, newIdx] };
  }
  return { scenario, select };
}

// ─── Duplicar nodo ──────────────────────────────────────────────────────────────

export function duplicateNode(s: Scenario, target: NodeId): MutateResult {
  if (target.kind === "root") return { scenario: s, select: target };

  if (target.kind === "group") {
    const group = s.thread_groups[target.gi];
    if (!group) return { scenario: s, select: target };
    const copy: ThreadGroup = { ...clone(group), name: `${group.name} (copia)` };
    const thread_groups = s.thread_groups.slice();
    thread_groups.splice(target.gi + 1, 0, copy);
    return {
      scenario: { ...s, thread_groups },
      select: { kind: "group", gi: target.gi + 1 },
    };
  }

  const group = s.thread_groups[target.gi];
  if (!group) return { scenario: s, select: target };
  const node = stepAt(group.steps, target.path);
  if (!node) return { scenario: s, select: target };
  const copy = clone(node);

  const insertIdx = target.path[target.path.length - 1] + 1;
  const scenario = withGroupSteps(s, target.gi, (steps) =>
    editParentList(steps, target.path, (siblings, idx) => {
      const next = siblings.slice();
      next.splice(idx + 1, 0, copy);
      return next;
    }),
  );
  const newPath = [...target.path.slice(0, -1), insertIdx];
  return { scenario, select: { kind: "step", gi: target.gi, path: newPath } };
}

// ─── Reordenar (subir / bajar) ──────────────────────────────────────────────────

export function moveNode(
  s: Scenario,
  target: NodeId,
  dir: "up" | "down",
): MutateResult {
  if (target.kind === "root") return { scenario: s, select: target };
  const delta = dir === "up" ? -1 : 1;

  if (target.kind === "group") {
    const j = target.gi + delta;
    if (j < 0 || j >= s.thread_groups.length) return { scenario: s, select: target };
    const thread_groups = s.thread_groups.slice();
    [thread_groups[target.gi], thread_groups[j]] = [
      thread_groups[j],
      thread_groups[target.gi],
    ];
    return { scenario: { ...s, thread_groups }, select: { kind: "group", gi: j } };
  }

  const group = s.thread_groups[target.gi];
  if (!group) return { scenario: s, select: target };
  const idx = target.path[target.path.length - 1];
  const parentPath = target.path.slice(0, -1);
  const siblings =
    parentPath.length === 0 ? group.steps : listAtContainer(group.steps, parentPath);
  if (!siblings) return { scenario: s, select: target };
  const j = idx + delta;
  if (j < 0 || j >= siblings.length) return { scenario: s, select: target };

  const scenario = withGroupSteps(s, target.gi, (steps) =>
    editParentList(steps, target.path, (sibs, i) => {
      const next = sibs.slice();
      [next[i], next[j]] = [next[j], next[i]];
      return next;
    }),
  );
  return {
    scenario,
    select: { kind: "step", gi: target.gi, path: [...parentPath, j] },
  };
}

// ─── Helpers de habilitación para la barra de acciones ──────────────────────────

export function canMove(s: Scenario, target: NodeId, dir: "up" | "down"): boolean {
  if (target.kind === "root") return false;
  const delta = dir === "up" ? -1 : 1;
  if (target.kind === "group") {
    const j = target.gi + delta;
    return j >= 0 && j < s.thread_groups.length;
  }
  const group = s.thread_groups[target.gi];
  if (!group) return false;
  const idx = target.path[target.path.length - 1];
  const parentPath = target.path.slice(0, -1);
  const siblings =
    parentPath.length === 0 ? group.steps : listAtContainer(group.steps, parentPath);
  if (!siblings) return false;
  const j = idx + delta;
  return j >= 0 && j < siblings.length;
}
