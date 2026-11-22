import { memo, useCallback, useEffect, useRef, useState } from "react";
import {
  Handle,
  Position,
  NodeResizer,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckSquare,
  Database,
  FileText,
  FolderKanban,
  Image as ImageIcon,
  Link2,
  Loader2,
  Square,
  StickyNote,
  UtensilsCrossed,
} from "lucide-react";
import { invoke } from "@/lib/api";
import type { Task } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import type { CanvasElement } from "./types";
import { getFile, resolveColor, textColorFor } from "./utils";

export type CanvasNodeData = {
  element: CanvasElement;
  editing: boolean;
  onStartEdit: (id: string) => void;
  onCommitText: (id: string, value: string) => void;
  onCommitCard: (id: string, title: string, body: string) => void;
  onToggleTask: (id: string, taskId: string, done: boolean) => void;
  [key: string]: unknown;
};

export type CanvasNode = Node<CanvasNodeData, "canvas">;

const HANDLE_CLASS =
  "!h-2.5 !w-2.5 !border-2 !border-card !bg-muted-foreground opacity-0 transition-opacity group-hover/canvasnode:opacity-100";

const HANDLE_SIDES: { id: string; position: Position }[] = [
  { id: "top", position: Position.Top },
  { id: "right", position: Position.Right },
  { id: "bottom", position: Position.Bottom },
  { id: "left", position: Position.Left },
];

const TASK_STATUS_LABEL: Record<string, string> = {
  todo: "Pendiente",
  in_progress: "En curso",
  done: "Completada",
  cancelled: "Cancelada",
};

function InlineText({
  value,
  singleLine,
  className,
  style,
  onCommit,
}: {
  value: string;
  singleLine?: boolean;
  className?: string;
  style?: React.CSSProperties;
  onCommit: (v: string) => void;
}) {
  const [text, setText] = useState(value);
  const ref = useRef<HTMLTextAreaElement | HTMLInputElement | null>(null);
  const committed = useRef(false);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const commit = useCallback(
    (v: string) => {
      if (committed.current) return;
      committed.current = true;
      onCommit(v);
    },
    [onCommit],
  );
  const shared = {
    className: cn("nodrag nowheel w-full bg-transparent outline-none", className),
    style,
    value: text,
    onBlur: () => commit(text),
    onClick: (e: React.MouseEvent) => e.stopPropagation(),
    onDoubleClick: (e: React.MouseEvent) => e.stopPropagation(),
  };
  if (singleLine) {
    return (
      <input
        {...shared}
        ref={ref as React.Ref<HTMLInputElement>}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") commit(text);
          if (e.key === "Escape") commit(value);
        }}
      />
    );
  }
  return (
    <textarea
      {...shared}
      ref={ref as React.Ref<HTMLTextAreaElement>}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") commit(value);
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) commit(text);
      }}
      className={cn(shared.className, "h-full resize-none")}
    />
  );
}

function FileImage({ fileId }: { fileId: string }) {
  const q = useQuery({
    queryKey: ["canvas-file", fileId],
    queryFn: () => getFile(fileId),
    staleTime: Infinity,
    retry: false,
  });
  if (q.isLoading) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-muted">
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (q.isError || !q.data) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-1 bg-muted text-muted-foreground">
        <ImageIcon className="h-6 w-6" />
        <span className="text-[10px]">Imagen no disponible</span>
      </div>
    );
  }
  return (
    <img
      src={q.data.dataUrl}
      alt={q.data.meta.name}
      className="pointer-events-none h-full w-full object-contain"
      draggable={false}
    />
  );
}

function RefBody({ element }: { element: CanvasElement }) {
  const c = element.content;
  const title = typeof c.title === "string" && c.title ? c.title : "Referencia";
  const meta = (() => {
    switch (element.type) {
      case "project_ref":
        return { icon: <FolderKanban className="h-3.5 w-3.5" />, label: "Proyecto" };
      case "note_ref":
        return { icon: <StickyNote className="h-3.5 w-3.5" />, label: "Nota" };
      case "database_ref":
        return { icon: <Database className="h-3.5 w-3.5" />, label: "Base de datos" };
      default:
        return { icon: <UtensilsCrossed className="h-3.5 w-3.5" />, label: "Receta" };
    }
  })();
  return (
    <div className="flex h-full flex-col gap-1 p-2.5">
      <span className="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        {meta.icon} {meta.label}
      </span>
      <span className="line-clamp-3 text-xs font-semibold leading-snug">{title}</span>
      <span className="mt-auto text-[9px] text-muted-foreground">Doble click para abrir</span>
    </div>
  );
}

function TaskRefBody({
  element,
  onToggleTask,
}: {
  element: CanvasElement;
  onToggleTask: (id: string, taskId: string, done: boolean) => void;
}) {
  const qc = useQueryClient();
  const taskId = typeof element.content.ref_id === "string" ? element.content.ref_id : "";
  const tasks = useQuery({
    queryKey: ["tasks"],
    queryFn: () => invoke<Task[]>("tasks.list", { filter: {} }),
    staleTime: 15000,
  });
  const live = tasks.data?.find((t) => t.id === taskId);
  const cachedStatus = typeof element.content.status === "string" ? element.content.status : "todo";
  const status = live?.status ?? cachedStatus;
  const title = live?.title ?? (typeof element.content.title === "string" ? element.content.title : "Tarea");
  const done = status === "done";
  return (
    <div className="flex h-full items-start gap-2 p-2.5">
      <button
        className="nodrag mt-0.5 shrink-0 cursor-pointer text-muted-foreground hover:text-primary"
        title={done ? "Marcar como pendiente" : "Completar tarea"}
        onClick={(e) => {
          e.stopPropagation();
          if (!taskId) return;
          onToggleTask(element.id, taskId, done);
          qc.invalidateQueries({ queryKey: ["tasks"] });
        }}
        onDoubleClick={(e) => e.stopPropagation()}
      >
        {done ? <CheckSquare className="h-4 w-4 text-success" /> : <Square className="h-4 w-4" />}
      </button>
      <div className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
          <CheckSquare className="h-3 w-3" /> Tarea
        </span>
        <p className={cn("line-clamp-2 text-xs font-semibold leading-snug", done && "text-muted-foreground line-through")}>
          {title}
        </p>
        <Badge variant={done ? "success" : status === "in_progress" ? "warning" : "secondary"} className="mt-1 px-1.5 py-0 text-[9px]">
          {TASK_STATUS_LABEL[status] ?? status}
        </Badge>
      </div>
    </div>
  );
}

function CardEditor({ element, data, id, textColor }: { element: CanvasElement; data: CanvasNodeData; id: string; textColor: string }) {
  const title = typeof element.content.title === "string" ? element.content.title : "";
  const body = typeof element.content.body === "string" ? element.content.body : "";
  const [titleDraft, setTitleDraft] = useState(title);
  const [bodyDraft, setBodyDraft] = useState(body);
  const done = useRef(false);
  const commit = (t: string, b: string) => {
    if (done.current) return;
    done.current = true;
    data.onCommitCard(id, t, b);
  };
  return (
    <div className="flex h-full flex-col p-2.5">
      <input
        autoFocus
        className="nodrag nowheel w-full bg-transparent text-xs font-semibold outline-none"
        style={{ color: textColor }}
        value={titleDraft}
        onChange={(e) => setTitleDraft(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") commit(titleDraft, bodyDraft);
          if (e.key === "Escape") commit(title, body);
        }}
        onClick={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
      />
      <textarea
        className="nodrag nowheel mt-1 w-full flex-1 resize-none bg-transparent text-xs leading-snug outline-none"
        style={{ color: textColor }}
        value={bodyDraft}
        onChange={(e) => setBodyDraft(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") commit(title, body);
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) commit(titleDraft, bodyDraft);
        }}
        onClick={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
      />
      <button
        className="nodrag mt-1 self-end rounded-md bg-primary/15 px-2 py-0.5 text-[10px] font-medium text-primary cursor-pointer"
        onClick={(e) => {
          e.stopPropagation();
          commit(titleDraft, bodyDraft);
        }}
      >
        Guardar
      </button>
    </div>
  );
}

function CardBody({ element, data, id, textColor }: { element: CanvasElement; data: CanvasNodeData; id: string; textColor: string }) {
  const title = typeof element.content.title === "string" ? element.content.title : "";
  const body = typeof element.content.body === "string" ? element.content.body : "";
  if (data.editing) {
    return <CardEditor element={element} data={data} id={id} textColor={textColor} />;
  }
  return (
    <div className="flex h-full flex-col overflow-hidden p-2.5" style={{ color: textColor }}>
      <p className="truncate text-xs font-semibold">{title || "Tarjeta"}</p>
      <p className="mt-1 line-clamp-5 whitespace-pre-wrap text-xs leading-snug opacity-80">{body}</p>
    </div>
  );
}

function CanvasNodeInner({ id, data, selected, width, height }: NodeProps<CanvasNode>) {
  const el = data.element;
  const w = width ?? el.w;
  const h = height ?? el.h;

  const isLinear = el.type === "line" || el.type === "arrow";
  const bg = resolveColor(
    el.style.background,
    el.type === "text" || isLinear ? "transparent" : "var(--card)",
  );
  const borderColor = resolveColor(el.style.border, isLinear ? "var(--foreground)" : "var(--border)");
  const textColor = el.style.text
    ? resolveColor(el.style.text, "var(--foreground)")
    : textColorFor(el.style.background);
  const fontSize = typeof el.style.font_size === "number" ? el.style.font_size : el.type === "sticky" ? 13 : 14;
  const align = el.style.align === "center" || el.style.align === "right" ? el.style.align : "left";
  const text = typeof el.content.text === "string" ? el.content.text : "";

  const commitText = useCallback((v: string) => data.onCommitText(id, v), [data, id]);
  const startEdit = useCallback(() => data.onStartEdit(id), [data, id]);

  const isShape = el.type === "rect" || el.type === "ellipse";
  const editable = el.type === "sticky" || el.type === "text" || isShape || el.type === "card";

  const inner = (() => {
    switch (el.type) {
      case "sticky":
      case "text":
      case "rect":
      case "ellipse": {
        // En el óvalo el texto va centrado en ambos ejes (elipse simétrica).
        const isEllipse = el.type === "ellipse";
        const effectiveAlign = isEllipse ? "center" : align;
        const pad = isEllipse ? "px-[12%] py-2" : "p-3";
        return data.editing ? (
          isEllipse ? (
            <div className="flex h-full w-full items-center justify-center">
              <InlineText
                value={text}
                className={cn(pad, "leading-snug")}
                style={{ color: textColor, fontSize, textAlign: "center" }}
                onCommit={commitText}
              />
            </div>
          ) : (
            <InlineText
              value={text}
              className={cn(pad, "leading-snug")}
              style={{ color: textColor, fontSize, textAlign: effectiveAlign }}
              onCommit={commitText}
            />
          )
        ) : (
          <div
            className={cn(
              "h-full w-full overflow-hidden whitespace-pre-wrap break-words leading-snug",
              pad,
              isEllipse && "flex items-center justify-center text-center",
            )}
            style={{ color: textColor, fontSize, textAlign: effectiveAlign }}
          >
            {text || <span className="opacity-40">Doble click para escribir…</span>}
          </div>
        );
      }
      case "card":
        return <CardBody element={el} data={data} id={id} textColor={textColor} />;
      case "image":
        return typeof el.content.file_id === "string" ? (
          <FileImage fileId={el.content.file_id} />
        ) : (
          <div className="flex h-full items-center justify-center bg-muted text-[10px] text-muted-foreground">Sin imagen</div>
        );
      case "document": {
        const name = typeof el.content.name === "string" ? el.content.name : "Documento";
        return (
          <div className="flex h-full items-center gap-2.5 p-3" style={{ color: textColor }}>
            <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/15 text-primary">
              <FileText className="h-4 w-4" />
            </span>
            <div className="min-w-0">
              <p className="truncate text-xs font-semibold">{name}</p>
              <p className="text-[10px] text-muted-foreground">Doble click para descargar</p>
            </div>
          </div>
        );
      }
      case "link": {
        const url = typeof el.content.url === "string" ? el.content.url : "";
        const title = typeof el.content.title === "string" && el.content.title ? el.content.title : url;
        return (
          <div className="flex h-full items-center gap-2.5 p-3" style={{ color: textColor }}>
            <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/15 text-primary">
              <Link2 className="h-4 w-4" />
            </span>
            <div className="min-w-0">
              <p className="truncate text-xs font-semibold">{title}</p>
              <p className="truncate text-[10px] text-muted-foreground">{url}</p>
            </div>
          </div>
        );
      }
      case "task_ref":
        return <TaskRefBody element={el} onToggleTask={data.onToggleTask} />;
      case "project_ref":
      case "note_ref":
      case "database_ref":
      case "recipe_ref":
        return <RefBody element={el} />;
      case "line":
      case "arrow":
        return (
          <svg width="100%" height="100%" className="overflow-visible">
            <defs>
              <marker id={`arrow-${id}`} markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto" markerUnits="strokeWidth">
                <path d="M0,0 L8,3 L0,6 Z" fill={borderColor} />
              </marker>
            </defs>
            <line
              x1={0}
              y1={h / 2}
              x2={el.type === "arrow" ? Math.max(w - 8, 0) : w}
              y2={h / 2}
              stroke={borderColor}
              strokeWidth={2}
              markerEnd={el.type === "arrow" ? `url(#arrow-${id})` : undefined}
            />
          </svg>
        );
      default:
        return null;
    }
  })();

  return (
    <div
      className={cn(
        "group/canvasnode relative h-full w-full",
        selected && "outline outline-2 outline-ring outline-offset-2 canvas-node-selected",
        el.group_id && !selected && "ring-1 ring-primary/40 ring-offset-1",
      )}
      style={{
        transform: el.rotation ? `rotate(${el.rotation}deg)` : undefined,
        background: isLinear ? "transparent" : bg,
        border:
          isLinear || el.type === "text" || el.type === "image"
            ? "none"
            : `1.5px solid ${el.type === "sticky" ? "transparent" : borderColor}`,
        borderRadius: el.type === "ellipse" ? "50%" : el.type === "sticky" ? 4 : 12,
        boxShadow: el.type === "sticky" ? "0 4px 10px rgb(0 0 0 / 0.12)" : undefined,
        overflow: el.type === "image" || el.type === "ellipse" ? "hidden" : undefined,
      }}
    >
      {inner}
      {HANDLE_SIDES.map((side) => (
        <Handle
          key={side.id}
          id={side.id}
          type="source"
          position={side.position}
          className={HANDLE_CLASS}
        />
      ))}
      {selected && !isLinear && (
        <NodeResizer
          isVisible={selected}
          minWidth={40}
          minHeight={el.type === "text" ? 24 : 30}
          color="var(--ring)"
          handleStyle={{ width: 8, height: 8, borderRadius: 2 }}
        />
      )}
      {editable && !data.editing && (
        <button
          className="nodrag absolute -top-7 right-0 hidden rounded-md border border-border bg-card px-1.5 py-0.5 text-[10px] text-muted-foreground shadow-sm hover:text-foreground group-hover/canvasnode:block"
          onClick={(e) => {
            e.stopPropagation();
            startEdit();
          }}
          onDoubleClick={(e) => e.stopPropagation()}
          title="Editar texto"
        >
          Editar
        </button>
      )}
    </div>
  );
}

export const CanvasNodeView = memo(CanvasNodeInner);
