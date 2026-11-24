import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { toPng, toSvg } from "html-to-image";
import {
  AlignCenterHorizontal,
  AlignCenterVertical,
  AlignEndHorizontal,
  AlignEndVertical,
  AlignStartHorizontal,
  AlignStartVertical,
  ArrowLeft,
  ArrowRight,
  Check,
  Circle,
  Copy,
  Download,
  FileJson,
  Group,
  Hand,
  Image as ImageIcon,
  Keyboard,
  LayoutTemplate,
  Link2,
  ListPlus,
  Loader2,
  Minus,
  MousePointer2,
  MoveHorizontal,
  MoveVertical,
  Network,
  Pencil,
  Redo2,
  Save,
  Sparkles,
  Square,
  StickyNote,
  Trash2,
  Type,
  Undo2,
  Ungroup,
  Upload,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { Canvas, CanvasTemplateInfo } from "./types";
import { downloadBlob, downloadDataUrl, safeFilename } from "./utils";

export type Tool = "select" | "hand" | "sticky" | "text" | "rect" | "ellipse" | "line" | "arrow";

const TOOLS: { id: Tool; label: string; icon: typeof Square }[] = [
  { id: "select", label: "Seleccionar", icon: MousePointer2 },
  { id: "hand", label: "Mover el plano (mano)", icon: Hand },
  { id: "sticky", label: "Nota adhesiva", icon: StickyNote },
  { id: "text", label: "Texto", icon: Type },
  { id: "rect", label: "Rectángulo", icon: Square },
  { id: "ellipse", label: "Elipse", icon: Circle },
  { id: "line", label: "Línea", icon: Minus },
  { id: "arrow", label: "Flecha", icon: ArrowRight },
];

export interface ToolbarProps {
  canvas: Canvas;
  tool: Tool;
  onToolChange: (t: Tool) => void;
  snap: boolean;
  onToggleSnap: () => void;
  saveState: "saved" | "saving" | "error";
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  selectionCount: number;
  onAlign: (mode: "left" | "center-x" | "right" | "top" | "center-y" | "bottom") => void;
  onDistribute: (axis: "h" | "v") => void;
  onGroup: () => void;
  onUngroup: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onInsertRef: () => void;
  onAttach: (accept: "image" | "any") => void;
  onAddLink: () => void;
  onShortcuts: () => void;
  onAi: (kind: "mindmap" | "summarize" | "organize") => void;
  getViewportElement: () => HTMLElement | null;
  onRename: (name: string) => void;
  onApplyTemplate: (templateId: string) => void;
}

export function CanvasToolbar(p: ToolbarProps) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [exporting, setExporting] = useState(false);
  const [templateOpen, setTemplateOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [nameDraft, setNameDraft] = useState(p.canvas.name);

  const templates = useQuery({
    queryKey: ["canvas-templates"],
    queryFn: () => invoke<CanvasTemplateInfo[]>("canvas.templates"),
    staleTime: Infinity,
    enabled: templateOpen,
  });

  const exportImage = async (kind: "png" | "svg") => {
    const viewport = p.getViewportElement();
    if (!viewport) return;
    setExporting(true);
    const bg = getComputedStyle(document.documentElement).getPropertyValue("--background").trim() || "#ffffff";
    const nodes = Array.from(viewport.querySelectorAll<HTMLElement>(".react-flow__node"));
    const edgesSvg = viewport.querySelector<SVGSVGElement>(".react-flow__edges");
    let minX = 0, minY = 0, maxX = 0, maxY = 0;
    for (const n of nodes) {
      const m = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(n.style.transform);
      const x = m ? parseFloat(m[1]) : n.offsetLeft;
      const y = m ? parseFloat(m[2]) : n.offsetTop;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x + n.offsetWidth);
      maxY = Math.max(maxY, y + n.offsetHeight);
    }
    if (edgesSvg) {
      try {
        const bb = edgesSvg.getBBox();
        minX = Math.min(minX, bb.x);
        minY = Math.min(minY, bb.y);
        maxX = Math.max(maxX, bb.x + bb.width);
        maxY = Math.max(maxY, bb.y + bb.height);
      } catch {
        /* sin aristas renderizadas */
      }
    }
    const pad = 48;
    const width = Math.max(64, Math.ceil(maxX - minX + pad * 2));
    const height = Math.max(64, Math.ceil(maxY - minY + pad * 2));
    const style: Partial<CSSStyleDeclaration> = {
      width: `${width}px`,
      height: `${height}px`,
      transform: `translate(${-minX + pad}px, ${-minY + pad}px)`,
    };
    const filter = (node: HTMLElement) => {
      const cl = node.classList;
      if (!cl) return true;
      return (
        !cl.contains("react-flow__minimap") &&
        !cl.contains("react-flow__controls") &&
        !cl.contains("react-flow__panel") &&
        !cl.contains("react-flow__background") &&
        !cl.contains("react-flow__handle") &&
        !cl.contains("react-flow__resize-control") &&
        !cl.contains("react-flow__nodesselection") &&
        !cl.contains("react-flow__selection")
      );
    };
    const options = { backgroundColor: bg, width, height, style, filter };
    try {
      const dataUrl = kind === "png"
        ? await toPng(viewport, { ...options, pixelRatio: 2 })
        : await toSvg(viewport, options);
      downloadDataUrl(`${safeFilename(p.canvas.name)}.${kind}`, dataUrl);
      toast.success(`Exportado como ${kind.toUpperCase()}`);
    } catch {
      if (kind === "svg") {
        try {
          const dataUrl = await toSvg(viewport, {
            ...options,
            filter: (node: HTMLElement) => filter(node) && node.tagName !== "IMG",
          });
          downloadDataUrl(`${safeFilename(p.canvas.name)}.svg`, dataUrl);
          toast.warning("SVG exportado sin las imágenes (no se pudieron incrustar)");
        } catch {
          toast.error("No se pudo exportar el SVG");
        }
      } else {
        toast.error("No se pudo exportar el PNG");
      }
    } finally {
      setExporting(false);
    }
  };

  const exportJson = useMutation({
    mutationFn: () => invoke<Record<string, unknown>>("canvas.export_json", { id: p.canvas.id }),
    onSuccess: (data) => {
      downloadBlob(`${safeFilename(p.canvas.name)}.canvas.json`, new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
      toast.success("Documento exportado como JSON");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo exportar"),
  });

  const importInputId = "canvas-import-json-input";
  const onImportFile = async (file: File) => {
    try {
      const text = await file.text();
      const data = JSON.parse(text) as Record<string, unknown>;
      const created = await invoke<Canvas>("canvas.import_json", { data });
      toast.success(`Canvas «${created.name}» importado`);
      qc.invalidateQueries({ queryKey: ["canvases"] });
      navigate(`/canvas/${created.id}`);
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "El archivo no es un canvas válido");
    }
  };

  const sep = <span className="mx-0.5 h-5 w-px bg-border" />;

  return (
    <div className="pointer-events-auto flex max-w-[calc(100vw-2rem)] flex-wrap items-center gap-0.5 rounded-xl border border-border bg-card/95 px-2 py-1.5 shadow-lg backdrop-blur">
      <Button size="icon-sm" variant="ghost" title="Volver a mis canvases" onClick={() => navigate("/canvas")}>
        <ArrowLeft />
      </Button>
      {renaming ? (
        <Input
          autoFocus
          className="h-7 w-44 text-xs"
          value={nameDraft}
          onChange={(e) => setNameDraft(e.target.value)}
          onBlur={() => {
            setRenaming(false);
            if (nameDraft.trim() && nameDraft !== p.canvas.name) p.onRename(nameDraft.trim());
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setNameDraft(p.canvas.name);
              setRenaming(false);
            }
          }}
        />
      ) : (
        <button
          className="flex max-w-44 items-center gap-1 truncate rounded-md px-2 py-1 text-xs font-semibold hover:bg-accent cursor-pointer"
          title="Renombrar canvas"
          onClick={() => {
            setNameDraft(p.canvas.name);
            setRenaming(true);
          }}
        >
          <span className="truncate">{p.canvas.name}</span>
          <Pencil className="h-3 w-3 shrink-0 text-muted-foreground" />
        </button>
      )}
      {sep}
      {TOOLS.map((t) => (
        <Button
          key={t.id}
          size="icon-sm"
          variant="ghost"
          title={t.label}
          className={cn(p.tool === t.id && "bg-primary/15 text-primary")}
          onClick={() => p.onToolChange(t.id)}
        >
          <t.icon />
        </Button>
      ))}
      {sep}
      <Button size="icon-sm" variant="ghost" title="Insertar referencia (tarea, proyecto, nota…)" onClick={p.onInsertRef}>
        <ListPlus />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Adjuntar imagen" onClick={() => p.onAttach("image")}>
        <ImageIcon />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Adjuntar documento" onClick={() => p.onAttach("any")}>
        <Upload />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Añadir enlace" onClick={p.onAddLink}>
        <Link2 />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Insertar plantilla" onClick={() => setTemplateOpen(true)}>
        <LayoutTemplate />
      </Button>
      {sep}
      <Button size="icon-sm" variant="ghost" title="Deshacer (Ctrl+Z)" disabled={!p.canUndo} onClick={p.onUndo}>
        <Undo2 />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Rehacer (Ctrl+Y)" disabled={!p.canRedo} onClick={p.onRedo}>
        <Redo2 />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Guardar (Ctrl+S)" onClick={p.onSave}>
        <Save />
      </Button>
      <Button
        size="icon-sm"
        variant="ghost"
        title={p.snap ? "Desactivar ajuste a cuadrícula" : "Activar ajuste a cuadrícula"}
        className={cn(p.snap && "bg-primary/15 text-primary")}
        onClick={p.onToggleSnap}
      >
        <Network />
      </Button>
      {p.selectionCount >= 2 && (
        <>
          {sep}
          <Button size="icon-sm" variant="ghost" title="Alinear a la izquierda" onClick={() => p.onAlign("left")}><AlignStartVertical /></Button>
          <Button size="icon-sm" variant="ghost" title="Centrar horizontalmente" onClick={() => p.onAlign("center-x")}><AlignCenterVertical /></Button>
          <Button size="icon-sm" variant="ghost" title="Alinear a la derecha" onClick={() => p.onAlign("right")}><AlignEndVertical /></Button>
          <Button size="icon-sm" variant="ghost" title="Alinear arriba" onClick={() => p.onAlign("top")}><AlignStartHorizontal /></Button>
          <Button size="icon-sm" variant="ghost" title="Centrar verticalmente" onClick={() => p.onAlign("center-y")}><AlignCenterHorizontal /></Button>
          <Button size="icon-sm" variant="ghost" title="Alinear abajo" onClick={() => p.onAlign("bottom")}><AlignEndHorizontal /></Button>
          <Button size="icon-sm" variant="ghost" title="Distribuir horizontalmente" onClick={() => p.onDistribute("h")}><MoveHorizontal /></Button>
          <Button size="icon-sm" variant="ghost" title="Distribuir verticalmente" onClick={() => p.onDistribute("v")}><MoveVertical /></Button>
          <Button size="icon-sm" variant="ghost" title="Agrupar (Ctrl+G)" onClick={p.onGroup}><Group /></Button>
          <Button size="icon-sm" variant="ghost" title="Desagrupar (Ctrl+Shift+G)" onClick={p.onUngroup}><Ungroup /></Button>
        </>
      )}
      {p.selectionCount >= 1 && (
        <>
          {sep}
          <Button size="icon-sm" variant="ghost" title="Duplicar (Ctrl+D)" onClick={p.onDuplicate}><Copy /></Button>
          <Button size="icon-sm" variant="ghost" title="Eliminar (Supr)" className="text-destructive" onClick={p.onDelete}><Trash2 /></Button>
        </>
      )}
      {sep}
      <Button size="icon-sm" variant="ghost" title="IA: generar mapa mental" onClick={() => p.onAi("mindmap")}><Sparkles /></Button>
      <Button size="icon-sm" variant="ghost" title="IA: resumir canvas" onClick={() => p.onAi("summarize")}><FileJson /></Button>
      <Button size="icon-sm" variant="ghost" title="IA: proponer conexiones" onClick={() => p.onAi("organize")}><Network /></Button>
      {sep}
      <Button size="icon-sm" variant="ghost" title="Exportar PNG" disabled={exporting} onClick={() => exportImage("png")}>
        {exporting ? <Loader2 className="animate-spin" /> : <ImageIcon />}
      </Button>
      <Button size="sm" variant="ghost" className="h-7 px-1.5 text-[10px]" title="Exportar SVG" disabled={exporting} onClick={() => exportImage("svg")}>
        SVG
      </Button>
      <Button size="icon-sm" variant="ghost" title="Exportar JSON" onClick={() => exportJson.mutate()}>
        <Download />
      </Button>
      <Button size="icon-sm" variant="ghost" title="Importar JSON (crea un canvas nuevo)" onClick={() => document.getElementById(importInputId)?.click()}>
        <Upload />
      </Button>
      <input
        id={importInputId}
        type="file"
        accept="application/json,.json"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void onImportFile(f);
          e.target.value = "";
        }}
      />
      <Button size="icon-sm" variant="ghost" title="Atajos de teclado" onClick={p.onShortcuts}>
        <Keyboard />
      </Button>
      {sep}
      <span
        className={cn(
          "flex items-center gap-1 whitespace-nowrap px-1.5 text-[10px]",
          p.saveState === "saved" ? "text-success" : p.saveState === "saving" ? "text-muted-foreground" : "text-destructive",
        )}
      >
        {p.saveState === "saving" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
        {p.saveState === "saved" ? "Guardado" : p.saveState === "saving" ? "Guardando…" : "Error al guardar"}
      </span>

      <Dialog open={templateOpen} onOpenChange={setTemplateOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Insertar plantilla</DialogTitle>
            <DialogDescription>Los elementos de la plantilla se añaden al canvas actual con ids nuevos.</DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {(templates.data ?? []).filter((t) => t.element_count > 0).map((t) => (
              <button
                key={t.id}
                className="rounded-lg border border-border bg-card p-3 text-left transition-colors hover:bg-accent cursor-pointer"
                onClick={() => {
                  p.onApplyTemplate(t.id);
                  setTemplateOpen(false);
                }}
              >
                <p className="text-xs font-semibold">{t.name}</p>
                <p className="mt-0.5 text-[11px] text-muted-foreground">{t.description}</p>
                <p className="mt-1 text-[10px] text-muted-foreground">{t.element_count} elementos</p>
              </button>
            ))}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Diálogo de atajos
// ---------------------------------------------------------------------------

const SHORTCUTS: [string, string][] = [
  ["Ctrl / Cmd + S", "Guardar el canvas ahora"],
  ["Herramienta mano", "Arrastrar el plano con el botón izquierdo"],
  ["Ctrl + C / Ctrl + V", "Copiar y pegar selección (con desplazamiento)"],
  ["Ctrl + D", "Duplicar selección"],
  ["Ctrl + Z", "Deshacer"],
  ["Ctrl + Y / Ctrl + Shift + Z", "Rehacer"],
  ["Supr / Retroceso", "Eliminar selección"],
  ["Ctrl + G", "Agrupar selección"],
  ["Ctrl + Shift + G", "Desagrupar"],
  ["Ctrl + A", "Seleccionar todo"],
  ["Shift + click", "Añadir a la selección"],
  ["Shift + arrastrar", "Selección por caja"],
  ["Doble click", "Editar texto / etiqueta / abrir referencia"],
  ["Click derecho", "Menú contextual del elemento"],
  ["Rueda / pellizco", "Zoom · arrastrar el fondo: mover el lienzo"],
  ["Esc", "Cancelar edición o selección"],
];

export function ShortcutsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Atajos de teclado</DialogTitle>
          <DialogDescription>Todo el lienzo se puede manejar sin ratón una vez seleccionado.</DialogDescription>
        </DialogHeader>
        <ul className="space-y-1.5">
          {SHORTCUTS.map(([keys, desc]) => (
            <li key={keys} className="flex items-center justify-between gap-3 text-xs">
              <kbd className="shrink-0 rounded-md border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px]">{keys}</kbd>
              <span className="text-right text-muted-foreground">{desc}</span>
            </li>
          ))}
        </ul>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Diálogo de enlace
// ---------------------------------------------------------------------------

export function LinkDialog({
  open,
  onClose,
  onSubmit,
}: {
  open: boolean;
  onClose: () => void;
  onSubmit: (url: string, title: string) => void;
}) {
  const [url, setUrl] = useState("https://");
  const [title, setTitle] = useState("");
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Añadir enlace</DialogTitle>
          <DialogDescription>Se creará un elemento de enlace en el centro de la vista.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>URL</Label>
            <Input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://ejemplo.com" />
          </div>
          <div className="space-y-1.5">
            <Label>Título</Label>
            <Input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Nombre del enlace" />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button
            disabled={!/^https?:\/\/.+/.test(url.trim())}
            onClick={() => {
              onSubmit(url.trim(), title.trim() || url.trim());
              setUrl("https://");
              setTitle("");
              onClose();
            }}
          >
            <Link2 /> Añadir
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Panel flotante de edición rápida (IA mindmap usa Textarea; aquí reexport)
// ---------------------------------------------------------------------------

export function AiDescriptionDialog({
  open,
  onClose,
  onSubmit,
  pending,
}: {
  open: boolean;
  onClose: () => void;
  onSubmit: (description: string) => void;
  pending: boolean;
}) {
  const [description, setDescription] = useState("");
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Generar mapa mental con IA</DialogTitle>
          <DialogDescription>La IA propone un mapa; podrás revisarlo antes de añadirlo al canvas.</DialogDescription>
        </DialogHeader>
        <Textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Describe el tema. Ej.: plan de lanzamiento de mi app de recetas…"
          rows={4}
        />
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button disabled={pending || !description.trim()} onClick={() => onSubmit(description.trim())}>
            {pending ? <Loader2 className="animate-spin" /> : <Sparkles />} Generar propuesta
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
