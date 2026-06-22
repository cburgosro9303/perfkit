import React, { useState } from "react";
import type { Scenario, Step, ThreadGroup } from "../types";
import { IconChevronDown, IconChevronRight, IconFile } from "./ui";

// ─── Node ID helpers ─────────────────────────────────────────────────────────

export type NodeId =
  | { kind: "root" }
  | { kind: "group"; gi: number }
  | { kind: "step"; gi: number; path: number[] };

export function nodeIdKey(id: NodeId): string {
  if (id.kind === "root") return "root";
  if (id.kind === "group") return `g${id.gi}`;
  return `g${id.gi}:${id.path.join(".")}`;
}

// ─── Method chip ─────────────────────────────────────────────────────────────

const METHOD_COLORS: Record<string, string> = {
  GET: "bg-blue-100 text-blue-700",
  POST: "bg-emerald-100 text-emerald-700",
  PUT: "bg-amber-100 text-amber-700",
  PATCH: "bg-orange-100 text-orange-700",
  DELETE: "bg-red-100 text-red-700",
  HEAD: "bg-slate-100 text-slate-600",
  OPTIONS: "bg-slate-100 text-slate-600",
};

const MethodChip: React.FC<{ method: string }> = ({ method }) => (
  <span
    className={`inline-flex items-center px-1.5 py-0.5 text-[10px] font-bold rounded uppercase tabular-nums ${
      METHOD_COLORS[method.toUpperCase()] ?? "bg-slate-100 text-slate-600"
    }`}
  >
    {method}
  </span>
);

// ─── Step type chip ──────────────────────────────────────────────────────────

const TYPE_LABELS: Record<string, string> = {
  transaction: "TX",
  loop: "LOOP",
  if: "IF",
  while: "WHILE",
  throughput: "THRPT",
  interleave: "INTLV",
  random: "RAND",
  kafka: "KAFKA",
  timer: "TIMER",
};

function childSteps(step: Step): Step[] {
  switch (step.type) {
    case "transaction":
    case "loop":
    case "if":
    case "while":
    case "throughput":
    case "interleave":
    case "random":
      return step.steps;
    default:
      return [];
  }
}

const StepTypeIcon: React.FC<{ type: string }> = ({ type }) => {
  if (type === "http") {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-slate-400">
        <circle cx="12" cy="12" r="10"/>
        <line x1="2" y1="12" x2="22" y2="12"/>
        <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
      </svg>
    );
  }
  if (type === "transaction") {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-indigo-400">
        <rect x="2" y="7" width="20" height="14" rx="2"/>
        <path d="M16 3H8a2 2 0 0 0-2 2v2h12V5a2 2 0 0 0-2-2z"/>
      </svg>
    );
  }
  if (type === "loop") {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-amber-500">
        <polyline points="17 1 21 5 17 9"/>
        <path d="M3 11V9a4 4 0 0 1 4-4h14"/>
        <polyline points="7 23 3 19 7 15"/>
        <path d="M21 13v2a4 4 0 0 1-4 4H3"/>
      </svg>
    );
  }
  if (type === "if" || type === "while") {
    return (
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-purple-400">
        <polygon points="12 2 2 7 2 17 12 22 22 17 22 7 12 2"/>
      </svg>
    );
  }
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-slate-400">
      <circle cx="12" cy="12" r="3"/>
    </svg>
  );
};

// ─── Single tree row ─────────────────────────────────────────────────────────

interface RowProps {
  label: string;
  type: string;
  depth: number;
  isSelected: boolean;
  isExpanded: boolean;
  canExpand: boolean;
  onSelect: () => void;
  onToggle: () => void;
  extra?: React.ReactNode;
}

const TreeRow: React.FC<RowProps> = ({
  label,
  type,
  depth,
  isSelected,
  isExpanded,
  canExpand,
  onSelect,
  onToggle,
  extra,
}) => (
  <div
    className={`flex items-center gap-1.5 px-2 py-1.5 cursor-pointer rounded-md transition-colors select-none group ${
      isSelected
        ? "bg-indigo-50 text-indigo-800"
        : "text-slate-700 hover:bg-slate-50"
    }`}
    style={{ paddingLeft: `${8 + depth * 16}px` }}
    onClick={onSelect}
  >
    {/* Expand toggle */}
    <span
      className={`shrink-0 w-4 h-4 flex items-center justify-center transition-colors ${
        canExpand ? "text-slate-400 hover:text-slate-700" : "opacity-0 pointer-events-none"
      }`}
      onClick={(e) => {
        e.stopPropagation();
        if (canExpand) onToggle();
      }}
    >
      {canExpand ? (isExpanded ? <IconChevronDown /> : <IconChevronRight />) : null}
    </span>

    <span className="shrink-0">
      <StepTypeIcon type={type} />
    </span>

    <span className="flex-1 text-sm font-medium truncate min-w-0">{label}</span>

    {extra && <span className="shrink-0">{extra}</span>}

    {type !== "http" && type !== "group" && type !== "root" && (
      <span className="shrink-0 text-[9px] font-bold uppercase px-1 py-0.5 bg-slate-100 text-slate-500 rounded">
        {TYPE_LABELS[type] ?? type}
      </span>
    )}
  </div>
);

// ─── Recursive step tree ─────────────────────────────────────────────────────

interface StepNodeProps {
  step: Step;
  gi: number;
  path: number[];
  selectedKey: string;
  expandedKeys: Set<string>;
  onSelect: (id: NodeId) => void;
  onToggle: (key: string) => void;
  depth: number;
}

const StepNode: React.FC<StepNodeProps> = ({
  step,
  gi,
  path,
  selectedKey,
  expandedKeys,
  onSelect,
  onToggle,
  depth,
}) => {
  const id: NodeId = { kind: "step", gi, path };
  const key = nodeIdKey(id);
  const isSelected = selectedKey === key;
  const isExpanded = expandedKeys.has(key);

  const children: Step[] = childSteps(step);

  const stepLabel =
    step.type === "timer"
      ? `Timer (${(step as unknown as { timer: string }).timer})`
      : step.name;

  return (
    <>
      <TreeRow
        label={stepLabel}
        type={step.type}
        depth={depth}
        isSelected={isSelected}
        isExpanded={isExpanded}
        canExpand={children.length > 0}
        onSelect={() => onSelect(id)}
        onToggle={() => onToggle(key)}
        extra={step.type === "http" ? <MethodChip method={step.method} /> : undefined}
      />
      {isExpanded &&
        children.map((child, i) => (
          <StepNode
            key={i}
            step={child}
            gi={gi}
            path={[...path, i]}
            selectedKey={selectedKey}
            expandedKeys={expandedKeys}
            onSelect={onSelect}
            onToggle={onToggle}
            depth={depth + 1}
          />
        ))}
    </>
  );
};

// ─── PlanTree (main export) ───────────────────────────────────────────────────

interface PlanTreeProps {
  scenario: Scenario;
  selectedId: NodeId;
  onSelect: (id: NodeId) => void;
}

export const PlanTree: React.FC<PlanTreeProps> = ({ scenario, selectedId, onSelect }) => {
  const rootKey = "root";
  const initExpanded = new Set<string>();
  scenario.thread_groups.forEach((_, gi) => {
    initExpanded.add(`g${gi}`);
  });

  const [expanded, setExpanded] = useState<Set<string>>(initExpanded);

  const toggle = (key: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const selectedKey = nodeIdKey(selectedId);

  return (
    <div className="flex flex-col gap-0.5 py-2 overflow-y-auto">
      {/* Root / scenario */}
      <TreeRow
        label={scenario.name}
        type="root"
        depth={0}
        isSelected={selectedKey === rootKey}
        isExpanded={true}
        canExpand={false}
        onSelect={() => onSelect({ kind: "root" })}
        onToggle={() => {}}
        extra={
          <span className="shrink-0">
            <IconFile />
          </span>
        }
      />

      {/* Thread groups */}
      {scenario.thread_groups.map((group, gi) => {
        const gKey = `g${gi}`;
        const gId: NodeId = { kind: "group", gi };
        const isExpanded = expanded.has(gKey);
        return (
          <React.Fragment key={gi}>
            <TreeRow
              label={group.name}
              type="group"
              depth={1}
              isSelected={selectedKey === gKey}
              isExpanded={isExpanded}
              canExpand={group.steps.length > 0}
              onSelect={() => onSelect(gId)}
              onToggle={() => toggle(gKey)}
              extra={
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-indigo-400 shrink-0">
                  <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
                  <circle cx="9" cy="7" r="4"/>
                  <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
                  <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
                </svg>
              }
            />
            {isExpanded &&
              group.steps.map((step, si) => (
                <StepNode
                  key={si}
                  step={step}
                  gi={gi}
                  path={[si]}
                  selectedKey={selectedKey}
                  expandedKeys={expanded}
                  onSelect={onSelect}
                  onToggle={toggle}
                  depth={2}
                />
              ))}
          </React.Fragment>
        );
      })}
    </div>
  );
};

// ─── Helper: resolve a NodeId to the actual data ─────────────────────────────

export function resolveNode(
  scenario: Scenario,
  id: NodeId,
): { kind: "root"; scenario: Scenario } | { kind: "group"; group: ThreadGroup; gi: number } | { kind: "step"; step: Step; gi: number; path: number[] } | null {
  if (id.kind === "root") return { kind: "root", scenario };
  if (id.kind === "group") {
    const group = scenario.thread_groups[id.gi];
    if (!group) return null;
    return { kind: "group", group, gi: id.gi };
  }
  // Walk the step path
  const group = scenario.thread_groups[id.gi];
  if (!group) return null;
  let steps = group.steps;
  let step: Step | null = null;
  for (let i = 0; i < id.path.length; i++) {
    const idx = id.path[i];
    if (idx >= steps.length) return null;
    step = steps[idx];
    if (i < id.path.length - 1) {
      steps = childSteps(step);
    }
  }
  if (!step) return null;
  return { kind: "step", step, gi: id.gi, path: id.path };
}
