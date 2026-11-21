import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  Handle,
  Position,
  MarkerType,
  useNodesState,
  useEdgesState,
  useReactFlow,
  type Node,
  type NodeProps,
  type Edge,
  type Connection,
  type NodeChangeset,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useQuery } from "@tanstack/react-query";
import {
  Bell,
  Bot,
  CheckCircle2,
  Flag,
  GitBranch,
  Plus,
  Timer,
  Trash2,
  Wrench,
  X,
  Zap,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke } from "@/lib/api";
import type { AutomationFlow, FlowStep, Id, StepKind, Trigger } from "@/lib/types";
import { AGENT_LABEL, cn, triggerLabel } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { StepEditor } from "./StepEditor";

const KIND_META: Record<string, { label: string; icon: LucideIcon; chip: string }> = {
  action: { label: "Acción", icon: Wrench, chip: "bg-primary/15 text-primary" },
  agent: { label: "Agente IA", icon: Bot, chip: "bg-warning/15 text-warning" },
  condition: { label: "IF / ELSE", icon: GitBranch, chip: "bg-success/15 text-success" },
  delay: { label: "Espera", icon: Timer, chip: "bg-muted text-muted-foreground" },
  notification: { label: "Notificación", icon: Bell, chip: "bg-primary/15 text-primary" },
  end: { label: "Fin", icon: Flag, chip: "bg-muted text-muted-foreground" },
};

const NODE_W = 230;

function toXY(p: [number, number]): { x: number; y: number } {
  return { x: p[0], y: p[1] };
}

function newId(): Id {
  return `s_${Math.random().toString(36).slice(2, 10)}`;
}

function defaultKind(type: string): StepKind {
  switch (type) {
    case "agent":
      return { type: "agent", agent_id: "planner", goal: "", approval: null };
    case "condition":
      return {
        type: "condition",
        condition: { type: "metric", metric: "open_tasks", op: "gt", value: 0, params: {} },
        on_true: null,
        on_false: null,
      };
    case "delay":
      return { type: "delay", seconds: 10 };
    case "notification":
      return { type: "notification", title: "", body: "" };
    case "end":
      return { type: "end" };
    default:
      return { type: "action", action: "tasks.create", params: { title: "" }, approval: null };
  }
}

function stepSummary(step: FlowStep): string {
  switch (step.type) {
    case "action":
      return step.action;
    case "agent":
      return `${AGENT_LABEL[step.agent_id] ?? step.agent_id}: ${step.goal || "(sin objetivo)"}`;
    case "notification":
      return step.title || "(sin título)";
    case "delay":
      return `${step.seconds}s`;
    case "condition":
      return step.condition.type === "metric"
        ? `${step.condition.metric} ${step.condition.op} ${JSON.stringify(step.condition.value)}`
        : step.condition.type === "event_field"
          ? `${step.condition.field} ${step.condition.op} ${JSON.stringify(step.condition.value)}`
          : "condición compuesta";
    case "end":
      return "termina el flujo";
  }
}

// ---------------------------------------------------------------------------
// Nodos custom
// ---------------------------------------------------------------------------

type StepNodeData = {
  step: FlowStep;
  isEntry: boolean;
  selected: boolean;
  onSelect: (id: Id) => void;
  onDelete: (id: Id) => void;
  onSetEntry: (id: Id) => void;
};

const handleClass = "!h-2.5 !w-2.5 !border-2 !border-card !bg-muted-foreground";

function StepNode({ data }: NodeProps<Node<StepNodeData>>) {
  const { step, isEntry, selected, onSelect, onDelete, onSetEntry } = data;
  const meta = KIND_META[step.type] ?? KIND_META.action;
  const Icon = meta.icon;
  return (
    <div
      onClick={() => onSelect(step.id)}
      className={cn(
        "relative w-[230px] cursor-pointer rounded-xl border bg-card shadow-sm transition-shadow",
        selected ? "border-ring ring-2 ring-ring/30" : "border-border hover:shadow-md",
      )}
    >
      {step.type !== "end" && (
        <Handle type="target" position={Position.Left} className={handleClass} />
      )}
      <div className="flex items-center gap-2 px-3 pt-2.5">
        <span className={cn("flex h-6 w-6 shrink-0 items-center justify-center rounded-md", meta.chip)}>
          <Icon className="h-3.5 w-3.5" />
        </span>
        <span className="min-w-0 flex-1 truncate text-xs font-semibold">{meta.label}</span>
        {isEntry && <Badge variant="default" className="px-1.5 py-0 text-[9px]">Entrada</Badge>}
      </div>
      <p className="truncate px-3 pb-2 pt-1 text-[11px] text-muted-foreground" title={stepSummary(step)}>
        {stepSummary(step)}
      </p>
      <div className="flex items-center justify-between border-t border-border/60 px-2 py-1">
        {!isEntry ? (
          <button
            className="flex items-center gap-1 rounded px-1 py-0.5 text-[10px] text-muted-foreground hover:text-foreground cursor-pointer"
            onClick={(e) => { e.stopPropagation(); onSetEntry(step.id); }}
            title="Marcar como paso de entrada"
          >
            <Zap className="h-3 w-3" /> entrada
          </button>
        ) : <span />}
        <button
          className="rounded p-0.5 text-muted-foreground hover:text-destructive cursor-pointer"
          onClick={(e) => { e.stopPropagation(); onDelete(step.id); }}
          title="Eliminar nodo"
        >
          <Trash2 className="h-3 w-3" />
        </button>
      </div>
      {step.type === "condition" ? (
        <>
          <Handle type="source" position={Position.Right} id="true" className={cn(handleClass, "!top-[30%] !bg-success")} />
          <Handle type="source" position={Position.Right} id="false" className={cn(handleClass, "!top-[70%] !bg-destructive")} />
          <div className="absolute -right-7 top-[30%] -translate-y-1/2 text-[9px] font-medium text-success">sí</div>
          <div className="absolute -right-7 top-[70%] -translate-y-1/2 text-[9px] font-medium text-destructive">no</div>
        </>
      ) : step.type !== "end" ? (
        <Handle type="source" position={Position.Right} className={handleClass} />
      ) : null}
    </div>
  );
}

type TriggerNodeData = { trigger: Trigger };

function TriggerNode({ data }: NodeProps<Node<TriggerNodeData>>) {
  return (
    <div className="w-[230px] rounded-xl border-2 border-primary/50 bg-card shadow-sm">
      <div className="flex items-center gap-2 px-3 pt-2.5">
        <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-primary/15 text-primary">
          <Zap className="h-3.5 w-3.5" />
        </span>
        <span className="text-xs font-semibold">Disparador</span>
      </div>
      <p className="px-3 pb-2.5 pt-1 text-[11px] text-muted-foreground">{triggerLabel(data.trigger)}</p>
      <Handle type="source" position={Position.Right} className={cn(handleClass, "!bg-primary")} />
    </div>
  );
}

const nodeTypes = { step: StepNode, trigger: TriggerNode };

// ---------------------------------------------------------------------------
// Conversión flujo ↔ nodos/aristas
// ---------------------------------------------------------------------------

function autoLayout(flow: AutomationFlow): Record<Id, [number, number]> {
  const positions: Record<Id, [number, number]> = {};
  const steps = flow.steps;
  const byId = new Map(steps.map((s) => [s.id, s]));
  const visited = new Set<Id>();
  let maxDepth = 0;

  const walk = (id: Id | null, depth: number, slot: number): number => {
    if (!id || visited.has(id) || !byId.has(id)) return slot;
    visited.add(id);
    maxDepth = Math.max(maxDepth, depth);
    positions[id] = [depth * 300, slot * 170];
    const step = byId.get(id)!;
    let nextSlot = slot;
    if (step.type === "condition") {
      const left = walk(step.on_true, depth + 1, slot);
      nextSlot = walk(step.on_false, depth + 1, Math.max(left, slot + 1));
    } else {
      nextSlot = walk(step.next, depth + 1, slot);
    }
    return Math.max(nextSlot, slot + 1);
  };

  walk(flow.entry ?? steps[0]?.id ?? null, 1, 0);
  let orphanSlot = 0;
  for (const s of steps) {
    if (!visited.has(s.id)) {
      positions[s.id] = [(maxDepth + 2) * 300, orphanSlot * 170];
      orphanSlot += 1;
    }
  }
  return positions;
}

function buildGraph(flow: AutomationFlow, trigger: Trigger, selectedId: Id | null, handlers: Omit<StepNodeData, "step" | "isEntry" | "selected">) {
  const positions = { ...autoLayout(flow), ...(flow.positions ?? {}) };
  const entry = flow.entry ?? flow.steps[0]?.id ?? null;
  const nodes: Node[] = [
    {
      id: "__trigger__",
      type: "trigger",
      position: toXY(positions.__trigger__ ?? [0, 0]),
      data: { trigger },
      deletable: false,
    },
  ];
  for (const step of flow.steps) {
    nodes.push({
      id: step.id,
      type: "step",
      position: toXY(positions[step.id] ?? [300, 0]),
      data: { step, isEntry: entry === step.id, selected: selectedId === step.id, ...handlers },
      deletable: false,
    });
  }
  const edges: Edge[] = [];
  const base = {
    type: "smoothstep" as const,
    markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
    style: { strokeWidth: 1.5 },
  };
  if (entry) {
    edges.push({ ...base, id: "__trigger__->entry", source: "__trigger__", target: entry, style: { ...base.style, stroke: "var(--primary)" } });
  }
  for (const step of flow.steps) {
    if (step.type === "condition") {
      if (step.on_true) edges.push({ ...base, id: `${step.id}:true->${step.on_true}`, source: step.id, sourceHandle: "true", target: step.on_true, label: "sí", labelStyle: { fill: "var(--success)", fontSize: 10 }, labelBgStyle: { fill: "var(--card)" }, style: { ...base.style, stroke: "var(--success)" } });
      if (step.on_false) edges.push({ ...base, id: `${step.id}:false->${step.on_false}`, source: step.id, sourceHandle: "false", target: step.on_false, label: "no", labelStyle: { fill: "var(--destructive)", fontSize: 10 }, labelBgStyle: { fill: "var(--card)" }, style: { ...base.style, stroke: "var(--destructive)" } });
    } else if (step.type !== "end" && step.next) {
      edges.push({ ...base, id: `${step.id}->${step.next}`, source: step.id, target: step.next, style: { ...base.style, stroke: "var(--muted-foreground)" } });
    }
  }
  return { nodes, edges };
}

// ---------------------------------------------------------------------------
// Lienzo
// ---------------------------------------------------------------------------

function CanvasInner({ flow, trigger, onChange, height = 520 }: { flow: AutomationFlow; trigger: Trigger; onChange: (f: AutomationFlow) => void; height?: number }) {
  const [selectedId, setSelectedId] = useState<Id | null>(null);
  const flowRef = useRef(flow);
  flowRef.current = flow;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const { screenToFlowPosition } = useReactFlow();

  const updateStep = useCallback((updated: FlowStep) => {
    const f = flowRef.current;
    onChangeRef.current({ ...f, steps: f.steps.map((s) => (s.id === updated.id ? updated : s)) });
  }, []);

  const deleteStep = useCallback((id: Id) => {
    const f = flowRef.current;
    const steps = f.steps
      .filter((s) => s.id !== id)
      .map((s) => {
        const u = { ...s };
        if (u.next === id) u.next = null;
        if (u.type === "condition") {
          if (u.on_true === id) u.on_true = null;
          if (u.on_false === id) u.on_false = null;
        }
        return u;
      });
    const positions = { ...(f.positions ?? {}) };
    delete positions[id];
    onChangeRef.current({ ...f, steps, positions, entry: f.entry === id ? steps[0]?.id ?? null : f.entry });
    setSelectedId((sel) => (sel === id ? null : sel));
  }, []);

  const setEntry = useCallback((id: Id) => {
    onChangeRef.current({ ...flowRef.current, entry: id });
  }, []);

  const handlers = useMemo(
    () => ({ onSelect: setSelectedId, onDelete: deleteStep, onSetEntry: setEntry }),
    [deleteStep, setEntry],
  );

  const initial = useMemo(() => buildGraph(flow, trigger, null, handlers), []); // eslint-disable-line react-hooks/exhaustive-deps
  const [nodes, setNodes, onNodesChange] = useNodesState(initial.nodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initial.edges);

  useEffect(() => {
    const graph = buildGraph(flow, trigger, selectedId, handlers);
    setNodes((prev) => {
      const prevPos = new Map(prev.map((n) => [n.id, n.position]));
      return graph.nodes.map((n) => ({ ...n, position: prevPos.get(n.id) ?? n.position }));
    });
    setEdges(graph.edges);
  }, [flow, trigger, selectedId, handlers, setNodes, setEdges]);

  const handleNodesChange = useCallback(
    (changes: NodeChangeset<Node>) => {
      onNodesChange(changes);
      const moved = changes.getByType("position").filter((c) => c.dragging === false && c.position);
      if (moved.length === 0) return;
      const f = flowRef.current;
      const positions = { ...(f.positions ?? {}) };
      for (const c of moved) {
        if (c.position) positions[c.id] = [c.position.x, c.position.y];
      }
      onChangeRef.current({ ...f, positions });
    },
    [onNodesChange],
  );

  const applyConnection = useCallback((source: Id, sourceHandle: string | null, target: Id) => {
    const f = flowRef.current;
    if (source === target) return;
    if (source === "__trigger__") {
      if (f.steps.some((s) => s.id === target)) onChangeRef.current({ ...f, entry: target });
      return;
    }
    // Evitar ciclos: no conectar si el destino ya alcanza el origen.
    const byId = new Map(f.steps.map((s) => [s.id, s]));
    const stack = [target];
    const seen = new Set<Id>();
    while (stack.length > 0) {
      const cur = stack.pop()!;
      if (cur === source) return;
      if (seen.has(cur)) continue;
      seen.add(cur);
      const s = byId.get(cur);
      if (!s) continue;
      if (s.next) stack.push(s.next);
      if (s.type === "condition") {
        if (s.on_true) stack.push(s.on_true);
        if (s.on_false) stack.push(s.on_false);
      }
    }
    const steps = f.steps.map((s) => {
      if (s.id !== source) return s;
      if (s.type === "condition") {
        if (sourceHandle === "true") return { ...s, on_true: target };
        if (sourceHandle === "false") return { ...s, on_false: target };
        return s;
      }
      if (s.type === "end") return s;
      return { ...s, next: target };
    });
    onChangeRef.current({ ...f, steps });
  }, []);

  const onConnect = useCallback(
    (conn: Connection) => {
      if (conn.source && conn.target) applyConnection(conn.source, conn.sourceHandle, conn.target);
    },
    [applyConnection],
  );

  const onEdgesDelete = useCallback((deleted: Edge[]) => {
    const f = flowRef.current;
    let entry = f.entry;
    const steps = f.steps.map((s) => {
      let u = { ...s };
      for (const e of deleted) {
        if (e.source === "__trigger__" && e.target === entry) entry = null;
        if (e.source !== s.id) continue;
        if (u.type === "condition") {
          if (e.sourceHandle === "true" && u.on_true === e.target) u = { ...u, on_true: null };
          if (e.sourceHandle === "false" && u.on_false === e.target) u = { ...u, on_false: null };
        } else if (u.type !== "end" && u.next === e.target) {
          u = { ...u, next: null };
        }
      }
      return u;
    });
    onChangeRef.current({ ...f, steps, entry });
  }, []);

  const addStep = useCallback(
    (type: string, position?: { x: number; y: number }) => {
      const f = flowRef.current;
      const id = newId();
      const step: FlowStep = { id, next: null, ...defaultKind(type) };
      const pos = position ?? screenToFlowPosition({ x: window.innerWidth / 2, y: window.innerHeight / 2 });
      const positions = { ...(f.positions ?? {}), [id]: [pos.x - NODE_W / 2, pos.y] as [number, number] };
      onChangeRef.current({
        ...f,
        steps: [...f.steps, step],
        positions,
        entry: f.entry ?? id,
      });
      setSelectedId(id);
    },
    [screenToFlowPosition],
  );

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const type = e.dataTransfer.getData("application/nexus-step");
      if (!type) return;
      addStep(type, screenToFlowPosition({ x: e.clientX, y: e.clientY }));
    },
    [addStep, screenToFlowPosition],
  );

  const validation = useQuery({
    queryKey: ["automation-validate", flow],
    queryFn: () => invoke<{ valid: boolean }>("automations.validate", { flow }),
    retry: false,
  });
  const error = validation.isError ? (validation.error as { message?: string })?.message ?? "Flujo inválido" : null;

  const selectedStep = flow.steps.find((s) => s.id === selectedId) ?? null;

  return (
    <div className="min-w-0">
      <div className="mb-2 flex flex-wrap items-center gap-1.5">
        <span className="mr-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Arrastra o pulsa:</span>
        {Object.entries(KIND_META).map(([type, meta]) => (
          <Button
            key={type}
            size="sm"
            variant="outline"
            className="h-7 cursor-grab text-xs"
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData("application/nexus-step", type);
              e.dataTransfer.effectAllowed = "move";
            }}
            onClick={() => addStep(type)}
          >
            <Plus /> <meta.icon /> {meta.label}
          </Button>
        ))}
      </div>

      <div
        className="relative overflow-hidden rounded-xl border border-border bg-background"
        style={{ height }}
        onDrop={onDrop}
        onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "move"; }}
      >
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={handleNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onEdgesDelete={onEdgesDelete}
          onNodeClick={(_, node) => node.id !== "__trigger__" && setSelectedId(node.id)}
          onPaneClick={() => setSelectedId(null)}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.3}
          maxZoom={1.6}
          proOptions={{ hideAttribution: true }}
          deleteKeyCode={null}
        >
          <Background variant={BackgroundVariant.Dots} gap={18} size={1} color="var(--border)" />
          <Controls showInteractive={false} />
          {!selectedStep && (
            <MiniMap
              pannable
              zoomable
              className="!bg-card"
              nodeColor={(n) => (n.type === "trigger" ? "var(--primary)" : "var(--muted-foreground)")}
              maskColor="color-mix(in srgb, var(--background) 70%, transparent)"
            />
          )}
        </ReactFlow>

        {selectedStep && (
          <div className="absolute bottom-3 right-3 top-3 z-10 w-[300px] overflow-y-auto rounded-xl border border-border bg-card/95 p-3 shadow-lg backdrop-blur animate-fade-in">
            <div className="mb-2 flex items-center justify-between gap-1">
              <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {KIND_META[selectedStep.type]?.label ?? selectedStep.type}
              </h4>
              <div className="flex items-center">
                <Button size="sm" variant="ghost" className="h-6 text-[11px] text-destructive" onClick={() => deleteStep(selectedStep.id)}>
                  <Trash2 /> Eliminar
                </Button>
                <Button size="icon-sm" variant="ghost" className="h-6 w-6" onClick={() => setSelectedId(null)} aria-label="Cerrar panel">
                  <X />
                </Button>
              </div>
            </div>
            <StepEditor step={selectedStep} onChange={updateStep} />
          </div>
        )}
      </div>

      <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
        {validation.isLoading ? (
          <span className="text-muted-foreground">Validando flujo…</span>
        ) : error ? (
          <span className="text-destructive">⚠ {error}</span>
        ) : (
          <span className="flex items-center gap-1 text-success">
            <CheckCircle2 className="h-3.5 w-3.5" /> Flujo válido · {flow.steps.length} nodos
          </span>
        )}
        <span className="text-[11px] text-muted-foreground">
          Conecta arrastrando desde el punto derecho de un nodo hasta otro · selecciona una flecha y pulsa Supr para desconectar · el nodo «Disparador» apunta a la entrada.
        </span>
      </div>
    </div>
  );
}

export function FlowCanvas(props: { flow: AutomationFlow; trigger: Trigger; onChange: (f: AutomationFlow) => void; height?: number }) {
  return (
    <ReactFlowProvider>
      <CanvasInner {...props} />
    </ReactFlowProvider>
  );
}
