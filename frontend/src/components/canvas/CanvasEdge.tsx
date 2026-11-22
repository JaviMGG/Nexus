import { memo, useState } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getSmoothStepPath,
  type Edge,
  type EdgeProps,
} from "@xyflow/react";
import type { CanvasEdge } from "./types";
import { resolveColor } from "./utils";

export type CanvasEdgeData = {
  edge: CanvasEdge;
  editing: boolean;
  onStartEdit: (id: string) => void;
  onCommitLabel: (id: string, label: string) => void;
  [key: string]: unknown;
};

export type CanvasFlowEdge = Edge<CanvasEdgeData, "canvas">;

function CanvasEdgeInner({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  markerEnd,
  selected,
  data,
}: EdgeProps<CanvasFlowEdge>) {
  const model = data?.edge;
  const [path, labelX, labelY] = getSmoothStepPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    borderRadius: 12,
  });
  const [draft, setDraft] = useState(model?.label ?? "");
  const color = resolveColor(model?.color, "var(--muted-foreground)");
  const editing = data?.editing ?? false;

  return (
    <>
      <BaseEdge
        id={id}
        path={path}
        markerEnd={markerEnd}
        style={{
          stroke: selected ? "var(--ring)" : color,
          strokeWidth: selected ? 2.5 : 1.8,
          strokeDasharray: model?.style === "dashed" ? "6 4" : undefined,
        }}
        interactionWidth={24}
      />
      {(model?.label || editing || selected) && (
        <EdgeLabelRenderer>
          <div
            className="nodrag nopan absolute"
            style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`, pointerEvents: "all" }}
            onDoubleClick={(e) => {
              e.stopPropagation();
              setDraft(model?.label ?? "");
              data?.onStartEdit(id);
            }}
          >
            {editing ? (
              <input
                autoFocus
                className="w-32 rounded-md border border-ring bg-card px-2 py-0.5 text-[11px] shadow-sm outline-none"
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onBlur={() => data?.onCommitLabel(id, draft)}
                onKeyDown={(e) => {
                  e.stopPropagation();
                  if (e.key === "Enter") data?.onCommitLabel(id, draft);
                  if (e.key === "Escape") data?.onCommitLabel(id, model?.label ?? "");
                }}
                onClick={(e) => e.stopPropagation()}
              />
            ) : model?.label ? (
              <span
                className="cursor-pointer rounded-md border border-border bg-card px-1.5 py-0.5 text-[10px] text-muted-foreground shadow-sm"
                title="Doble click para editar la etiqueta"
              >
                {model.label}
              </span>
            ) : (
              <span className="cursor-pointer rounded-md border border-dashed border-border bg-card/80 px-1.5 py-0.5 text-[9px] text-muted-foreground/60">
                + etiqueta
              </span>
            )}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const CanvasEdgeView = memo(CanvasEdgeInner);
