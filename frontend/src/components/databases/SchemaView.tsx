import { useMemo } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  Position,
  MarkerType,
  type Node,
  type NodeProps,
  type Edge,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { KeyRound, Link2 } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import type { DatabaseDetail, DatabaseSummary, DbField } from "./types";
import { CARDINALITY_LABEL, DbIcon, FIELD_KIND_ICON, FIELD_KIND_LABEL, dbColor } from "./shared";

const NODE_W = 260;
const HEADER_H = 44;
const ROW_H = 26;

const CARDINALITY_SHORT: Record<string, { src: string; dst: string }> = {
  one_to_one: { src: "1", dst: "1" },
  one_to_many: { src: "1", dst: "N" },
  many_to_many: { src: "N", dst: "M" },
};

type TableNodeData = {
  db: DatabaseSummary;
  fields: DbField[];
  isCurrent: boolean;
  onOpen: (id: Id) => void;
};

function TableNode({ data }: NodeProps<Node<TableNodeData>>) {
  const { db, fields, isCurrent, onOpen } = data;
  const color = dbColor(db.color);
  return (
    <div
      className={cn(
        "w-[260px] overflow-hidden rounded-xl border bg-card shadow-sm",
        isCurrent ? "border-ring ring-2 ring-ring/30" : "border-border",
      )}
    >
      <Handle type="target" position={Position.Left} id="table" className="!h-2.5 !w-2.5 !border-2 !border-card !bg-muted-foreground" />
      <button
        className={cn("flex w-full items-center gap-2 px-3 py-2.5 text-left cursor-pointer", color.bg)}
        onClick={() => onOpen(db.id)}
        title="Abrir esta base de datos"
      >
        <DbIcon icon={db.icon} className={cn("h-4 w-4 shrink-0", color.text)} />
        <span className="min-w-0 flex-1 truncate text-xs font-semibold">{db.name}</span>
        <Badge variant="secondary" className="shrink-0 px-1.5 py-0 text-[9px]">{db.record_count} reg.</Badge>
      </button>
      <div className="relative">
        <div className="flex items-center gap-2 border-b border-border/60 bg-muted/40 px-3 py-1">
          <KeyRound className="h-3 w-3 shrink-0 text-warning" />
          <span className="min-w-0 flex-1 truncate text-[11px] font-medium">id</span>
          <span className="text-[9px] uppercase tracking-wide text-muted-foreground">PK · uuid</span>
        </div>
        {fields.map((f) => {
          const Icon = FIELD_KIND_ICON[f.kind];
          const isFk = f.kind === "relation";
          return (
            <div
              key={f.id}
              className={cn(
                "relative flex items-center gap-2 border-b border-border/40 px-3 py-1 last:border-0",
                isFk && "bg-primary/5",
              )}
            >
              {isFk ? (
                <Link2 className="h-3 w-3 shrink-0 text-primary" />
              ) : (
                <Icon className="h-3 w-3 shrink-0 text-muted-foreground" />
              )}
              <span className="min-w-0 flex-1 truncate text-[11px]">{f.name}</span>
              <span className="shrink-0 text-[9px] uppercase tracking-wide text-muted-foreground">
                {isFk ? "FK" : FIELD_KIND_LABEL[f.kind]}
              </span>
              {isFk && (
                <Handle
                  type="source"
                  position={Position.Right}
                  id={f.id}
                  className="!h-2 !w-2 !border-2 !border-card !bg-primary"
                  style={{ top: "50%" }}
                />
              )}
            </div>
          );
        })}
        {fields.length === 0 && (
          <p className="px-3 py-2 text-[10px] text-muted-foreground">Sin campos todavía</p>
        )}
      </div>
    </div>
  );
}

const nodeTypes = { table: TableNode };

interface SchemaGraph {
  nodes: Node[];
  edges: Edge[];
}

function buildGraph(
  currentId: Id,
  summaries: DatabaseSummary[],
  details: Map<Id, DatabaseDetail>,
  onOpen: (id: Id) => void,
): SchemaGraph {
  // Incluye la base actual + todas las conectadas por relaciones (en cualquier dirección).
  const connected = new Set<Id>([currentId]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const d of details.values()) {
      for (const f of d.fields) {
        const target = f.config.target_database_id;
        if (f.kind !== "relation" || !target) continue;
        if (connected.has(d.id) && !connected.has(target)) {
          connected.add(target);
          changed = true;
        }
        if (connected.has(target) && !connected.has(d.id)) {
          connected.add(d.id);
          changed = true;
        }
      }
    }
  }

  const included = summaries.filter((s) => connected.has(s.id));
  // Layout por columnas: la base actual a la izquierda; el resto en columnas de 3.
  const ordered = [
    ...included.filter((s) => s.id === currentId),
    ...included.filter((s) => s.id !== currentId),
  ];
  const positions = new Map<Id, { x: number; y: number }>();
  const COL_W = NODE_W + 140;
  const ROW_GAP = 40;
  let col = 0;
  let y = 0;
  let colMaxH = 0;
  ordered.forEach((s, i) => {
    const detail = details.get(s.id);
    const h = HEADER_H + ROW_H * ((detail?.fields.length ?? 0) + 1) + 8;
    if (i > 0 && i % 3 === 0) {
      col += 1;
      y = 0;
    }
    positions.set(s.id, { x: col * COL_W, y });
    y += h + ROW_GAP;
    colMaxH = Math.max(colMaxH, y);
  });
  void colMaxH;

  const nodes: Node[] = ordered.map((s) => ({
    id: s.id,
    type: "table",
    position: positions.get(s.id) ?? { x: 0, y: 0 },
    data: {
      db: s,
      fields: details.get(s.id)?.fields ?? [],
      isCurrent: s.id === currentId,
      onOpen,
    },
    draggable: true,
  }));

  const edges: Edge[] = [];
  for (const d of details.values()) {
    if (!connected.has(d.id)) continue;
    for (const f of d.fields) {
      const target = f.config.target_database_id;
      if (f.kind !== "relation" || !target || !connected.has(target)) continue;
      const card = CARDINALITY_SHORT[f.config.cardinality ?? "one_to_many"] ?? CARDINALITY_SHORT.one_to_many;
      edges.push({
        id: `${f.id}->${target}`,
        source: d.id,
        sourceHandle: f.id,
        target,
        targetHandle: "table",
        type: "smoothstep",
        label: `${f.name} · ${CARDINALITY_LABEL[f.config.cardinality ?? "one_to_many"] ?? ""}`,
        labelStyle: { fill: "var(--foreground)", fontSize: 10 },
        labelBgStyle: { fill: "var(--card)", fillOpacity: 0.9 },
        labelBgPadding: [4, 2] as [number, number],
        markerStart: { type: MarkerType.Arrow, width: 14, height: 14, color: "var(--primary)" },
        markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14, color: "var(--primary)" },
        style: { stroke: "var(--primary)", strokeWidth: 1.5 },
        data: { cardinality: card },
      });
    }
  }
  return { nodes, edges };
}

function SchemaInner({ databaseId }: { databaseId: Id }) {
  const navigate = useNavigate();
  const list = useQuery({
    queryKey: ["databases"],
    queryFn: () => invoke<DatabaseSummary[]>("databases.list"),
  });
  const summaries = list.data ?? [];
  const details = useQueries({
    queries: summaries.map((s) => ({
      queryKey: ["databases", "detail", s.id],
      queryFn: () => invoke<DatabaseDetail>("databases.get", { id: s.id }),
      staleTime: 15_000,
    })),
  });

  const loading = list.isLoading || details.some((d) => d.isLoading);
  const graph = useMemo(() => {
    if (loading) return { nodes: [], edges: [] };
    const map = new Map<Id, DatabaseDetail>();
    details.forEach((d) => {
      if (d.data) map.set(d.data.id, d.data);
    });
    return buildGraph(databaseId, summaries, map, (id) => navigate(`/databases/${id}`));
  }, [loading, details, summaries, databaseId, navigate]);

  if (loading) {
    return (
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <Skeleton className="h-56 rounded-xl" />
        <Skeleton className="h-40 rounded-xl" />
        <Skeleton className="h-48 rounded-xl" />
      </div>
    );
  }

  const relationCount = graph.edges.length;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <Badge variant="secondary">{graph.nodes.length} tablas</Badge>
        <Badge variant={relationCount > 0 ? "default" : "outline"}>{relationCount} relaciones</Badge>
        <span>
          Cada tabla muestra sus campos; <KeyRound className="inline h-3 w-3 text-warning" /> id es la clave primaria (PK)
          y los campos de relación son claves ajenas (<Link2 className="inline h-3 w-3 text-primary" /> FK) que apuntan a otra tabla.
          Las flechas indican la cardinalidad (1:1, 1:N, N:M). Clic en la cabecera de una tabla para abrirla.
        </span>
      </div>
      <div className="h-[560px] overflow-hidden rounded-xl border border-border bg-background">
        <ReactFlow
          nodes={graph.nodes}
          edges={graph.edges}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.15 }}
          minZoom={0.2}
          maxZoom={1.5}
          nodesConnectable={false}
          edgesFocusable={false}
          proOptions={{ hideAttribution: true }}
        >
          <Background variant={BackgroundVariant.Dots} gap={18} size={1} color="var(--border)" />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
      {relationCount === 0 && (
        <p className="rounded-lg border border-dashed border-border px-4 py-3 text-xs text-muted-foreground">
          Aún no hay relaciones entre tus bases. Crea un campo de tipo <b>Relación</b> en cualquier base
          (Campos → nuevo campo → Relación) apuntando a otra, y aquí aparecerá la flecha con su cardinalidad.
        </p>
      )}
    </div>
  );
}

export function SchemaView({ databaseId }: { databaseId: Id }) {
  return (
    <ReactFlowProvider>
      <SchemaInner databaseId={databaseId} />
    </ReactFlowProvider>
  );
}
