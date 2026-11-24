import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";
import {
  Copy,
  LayoutTemplate,
  Loader2,
  Pencil,
  Plus,
  Shapes,
  Trash2,
  Upload,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import { relativeTime } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { CanvasEditor } from "@/components/canvas/CanvasEditor";
import type { Canvas, CanvasSummary, CanvasTemplateInfo } from "@/components/canvas/types";

// ---------------------------------------------------------------------------
// Lista de canvases
// ---------------------------------------------------------------------------

function CreateDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [template, setTemplate] = useState("mind_map");

  const templates = useQuery({
    queryKey: ["canvas-templates"],
    queryFn: () => invoke<CanvasTemplateInfo[]>("canvas.templates"),
    staleTime: Infinity,
    enabled: open,
  });

  const create = useMutation({
    mutationFn: () =>
      invoke<Canvas>("canvas.create", {
        data: { name: name.trim() || "Canvas sin título", description: description.trim(), template },
      }),
    onSuccess: (c) => {
      qc.invalidateQueries({ queryKey: ["canvases"] });
      toast.success(`Canvas «${c.name}» creado`);
      onClose();
      setName("");
      setDescription("");
      navigate(`/canvas/${c.id}`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo crear el canvas"),
  });

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Nuevo canvas</DialogTitle>
          <DialogDescription>Elige una plantilla con estructura inicial o empieza con un lienzo libre.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Nombre</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Mi pizarra" autoFocus />
          </div>
          <div className="space-y-1.5">
            <Label>Descripción (opcional)</Label>
            <Textarea rows={2} value={description} onChange={(e) => setDescription(e.target.value)} placeholder="¿Para qué es esta pizarra?" />
          </div>
          <div className="space-y-1.5">
            <Label>Plantilla</Label>
            {templates.isLoading ? (
              <Skeleton className="h-32 rounded-lg" />
            ) : (
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                {(templates.data ?? []).map((t) => (
                  <button
                    key={t.id}
                    className={cn(
                      "rounded-lg border p-2.5 text-left transition-colors cursor-pointer",
                      template === t.id ? "border-primary bg-primary/10" : "border-border bg-card hover:bg-accent",
                    )}
                    onClick={() => setTemplate(t.id)}
                  >
                    <p className="text-xs font-semibold">{t.name}</p>
                    <p className="mt-0.5 line-clamp-2 text-[10px] text-muted-foreground">{t.description}</p>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button disabled={create.isPending} onClick={() => create.mutate()}>
            {create.isPending ? <Loader2 className="animate-spin" /> : <Plus />} Crear canvas
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RenameDialog({
  canvas,
  onClose,
}: {
  canvas: CanvasSummary | null;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [name, setName] = useState(canvas?.name ?? "");
  const [description, setDescription] = useState(canvas?.description ?? "");
  const rename = useMutation({
    mutationFn: () => invoke("canvas.update", { id: canvas!.id, data: { name: name.trim(), description: description.trim() } }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["canvases"] });
      toast.success("Canvas actualizado");
      onClose();
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo renombrar"),
  });
  return (
    <Dialog open={canvas != null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Renombrar canvas</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Nombre</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </div>
          <div className="space-y-1.5">
            <Label>Descripción</Label>
            <Textarea rows={2} value={description} onChange={(e) => setDescription(e.target.value)} />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button disabled={rename.isPending || !name.trim()} onClick={() => rename.mutate()}>Guardar</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CanvasList() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<CanvasSummary | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<CanvasSummary | null>(null);

  const list = useQuery({
    queryKey: ["canvases"],
    queryFn: () => invoke<CanvasSummary[]>("canvas.list"),
  });

  const duplicate = useMutation({
    mutationFn: (id: string) => invoke<Canvas>("canvas.duplicate", { id }),
    onSuccess: (c) => {
      qc.invalidateQueries({ queryKey: ["canvases"] });
      toast.success(`Duplicado como «${c.name}»`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo duplicar"),
  });

  const remove = useMutation({
    mutationFn: (id: string) => invoke("canvas.delete", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["canvases"] });
      toast.success("Canvas eliminado");
      setDeleteTarget(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar"),
  });

  const importJson = async (file: File) => {
    try {
      const data = JSON.parse(await file.text()) as Record<string, unknown>;
      const created = await invoke<Canvas>("canvas.import_json", { data });
      qc.invalidateQueries({ queryKey: ["canvases"] });
      toast.success(`Canvas «${created.name}» importado`);
      navigate(`/canvas/${created.id}`);
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : "El archivo no es un canvas válido");
    }
  };

  if (list.isLoading) {
    return (
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-36 rounded-xl" />
        ))}
      </div>
    );
  }
  if (list.isError) {
    return (
      <EmptyState
        icon={Shapes}
        title="No se pudieron cargar los canvases"
        description={list.error instanceof ApiError ? list.error.message : undefined}
        action={<Button size="sm" variant="outline" onClick={() => list.refetch()}>Reintentar</Button>}
      />
    );
  }
  const items = list.data ?? [];

  return (
    <>
      <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Canvas</h1>
          <p className="text-xs text-muted-foreground">
            Pizarras infinitas para pensar en visual: mapas mentales, diagramas, ideas y referencias vivas a tu NEXUS.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onClick={() => document.getElementById("canvas-list-import")?.click()}>
            <Upload /> Importar JSON
          </Button>
          <input
            id="canvas-list-import"
            type="file"
            accept="application/json,.json"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void importJson(f);
              e.target.value = "";
            }}
          />
          <Button size="sm" onClick={() => setCreateOpen(true)}>
            <Plus /> Nuevo canvas
          </Button>
        </div>
      </div>

      {items.length === 0 ? (
        <EmptyState
          icon={LayoutTemplate}
          title="Sin canvases todavía"
          description="Crea tu primera pizarra con una plantilla (mapa mental, diagrama de flujo, lluvia de ideas…) o empieza en blanco."
          action={<Button size="sm" onClick={() => setCreateOpen(true)}><Plus /> Crear canvas</Button>}
        />
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {items.map((c) => (
            <div
              key={c.id}
              className="group flex cursor-pointer flex-col rounded-xl border border-border bg-card p-4 shadow-sm transition-shadow hover:shadow-md"
              onClick={() => navigate(`/canvas/${c.id}`)}
            >
              <div className="flex items-start gap-2.5">
                <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/15 text-primary">
                  <Shapes className="h-4 w-4" />
                </span>
                <div className="min-w-0 flex-1">
                  <h3 className="truncate text-sm font-semibold">{c.name}</h3>
                  <p className="mt-0.5 line-clamp-2 min-h-8 text-xs text-muted-foreground">{c.description || "Sin descripción"}</p>
                </div>
              </div>
              <div className="mt-3 flex items-center justify-between text-[11px] text-muted-foreground">
                <span>
                  {c.element_count} elementos · {c.edge_count} conexiones
                </span>
                <span>{relativeTime(c.updated_at)}</span>
              </div>
              <div className="mt-2 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                <Button
                  size="icon-sm"
                  variant="ghost"
                  title="Renombrar"
                  onClick={(e) => {
                    e.stopPropagation();
                    setRenameTarget(c);
                  }}
                >
                  <Pencil />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  title="Duplicar"
                  disabled={duplicate.isPending}
                  onClick={(e) => {
                    e.stopPropagation();
                    duplicate.mutate(c.id);
                  }}
                >
                  <Copy />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  className="text-destructive"
                  title="Eliminar"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDeleteTarget(c);
                  }}
                >
                  <Trash2 />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      <CreateDialog open={createOpen} onClose={() => setCreateOpen(false)} />
      {renameTarget && (
        <RenameDialog key={renameTarget.id} canvas={renameTarget} onClose={() => setRenameTarget(null)} />
      )}
      <Dialog open={deleteTarget != null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Eliminar canvas</DialogTitle>
            <DialogDescription>
              «{deleteTarget?.name}» y todo su contenido se eliminarán. Esta acción no se puede deshacer.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancelar</Button>
            <Button variant="destructive" disabled={remove.isPending} onClick={() => deleteTarget && remove.mutate(deleteTarget.id)}>
              {remove.isPending ? <Loader2 className="animate-spin" /> : <Trash2 />} Eliminar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

// ---------------------------------------------------------------------------
// Página: lista en /canvas, editor a pantalla completa en /canvas/:id
// ---------------------------------------------------------------------------

export function CanvasPage() {
  const { id } = useParams<{ id: string }>();

  const canvasQuery = useQuery({
    queryKey: ["canvas", id],
    queryFn: () => invoke<Canvas>("canvas.get", { id }),
    enabled: !!id,
    refetchOnWindowFocus: false,
    retry: false,
  });

  if (!id) {
    return (
      <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
        <CanvasList />
      </div>
    );
  }

  if (canvasQuery.isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (canvasQuery.isError || !canvasQuery.data) {
    return (
      <div className="p-6">
        <EmptyState
          icon={Shapes}
          title="Canvas no encontrado"
          description={canvasQuery.error instanceof ApiError ? canvasQuery.error.message : "Puede que haya sido eliminado."}
          action={<Button size="sm" variant="outline" onClick={() => (window.location.hash = "#/canvas")}>Volver a mis canvases</Button>}
        />
      </div>
    );
  }

  return (
    <div className="h-full animate-fade-in">
      <CanvasEditor canvas={canvasQuery.data} />
    </div>
  );
}
