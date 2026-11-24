import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  ArrowLeft,
  Calendar,
  Copy,
  Database as DatabaseIcon,
  Filter,
  Image as ImageIcon,
  Kanban,
  LayoutList,
  Loader2,
  Network,
  List,
  Pencil,
  Plus,
  Search,
  Settings2,
  Sparkles,
  Table2,
  Trash2,
  Upload,
  Download,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn, relativeTime } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import type {
  DatabaseDetail,
  DatabaseSummary,
  DbRecord,
  DbTemplate,
  DbView,
  DbViewConfig,
  DbViewKind,
  RecordPage,
} from "@/components/databases/types";
import {
  DB_COLORS,
  DB_ICONS,
  DbIcon,
  VIEW_KIND_LABEL,
  dbColor,
  toastDbError,
  useInvalidateDatabases,
} from "@/components/databases/shared";
import { FieldsManagerDialog, FilterSortEditor, ViewDialog } from "@/components/databases/dialogs";
import { RecordDialog } from "@/components/databases/RecordDialog";
import {
  AiDuplicatesDialog,
  AiStructureDialog,
  AiSuggestDialog,
  ExportDialog,
  ImportDialog,
} from "@/components/databases/ImportExport";
import { CalendarView, GalleryView, KanbanView, ListView, TableView } from "@/components/databases/views";
import { SchemaView } from "@/components/databases/SchemaView";

const VIEW_ICON: Record<DbViewKind, LucideIcon> = {
  table: Table2,
  kanban: Kanban,
  gallery: ImageIcon,
  calendar: Calendar,
  list: List,
};

export function DatabasesPage() {
  const { id } = useParams<{ id: Id }>();
  return id ? <DatabaseDetailPage id={id} /> : <DatabaseListPage />;
}

// ===========================================================================
// Listado de bases
// ===========================================================================

function DatabaseListPage() {
  const navigate = useNavigate();
  const invalidate = useInvalidateDatabases();
  const [creating, setCreating] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [templatesOpen, setTemplatesOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<DatabaseSummary | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<DatabaseSummary | null>(null);

  const list = useQuery({
    queryKey: ["databases", "list"],
    queryFn: () => invoke<DatabaseSummary[]>("databases.list"),
  });

  const duplicate = useMutation({
    mutationFn: (id: string) => invoke<DatabaseDetail>("databases.duplicate", { id }),
    onSuccess: (d) => {
      invalidate();
      toast.success(`Duplicada como «${d.name}»`);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo duplicar"),
  });

  const remove = useMutation({
    mutationFn: (id: string) => invoke("databases.delete", { id }),
    onSuccess: () => {
      invalidate();
      toast.success("Base de datos eliminada");
      setDeleteTarget(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar"),
  });

  return (
    <div className="mx-auto max-w-[1200px] space-y-5 p-6 animate-fade-in">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="flex items-center gap-2 text-lg font-semibold tracking-tight">
            <DatabaseIcon className="h-5 w-5 text-primary" /> Bases de datos
          </h1>
          <p className="text-xs text-muted-foreground">
            Organiza cualquier cosa con campos a tu medida: colecciones, inventarios, seguimiento…
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => setAiOpen(true)}>
            <Sparkles className="h-3.5 w-3.5" /> Crear con IA
          </Button>
          <Button variant="outline" size="sm" onClick={() => setTemplatesOpen(true)}>
            <LayoutList className="h-3.5 w-3.5" /> Plantillas
          </Button>
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus className="h-3.5 w-3.5" /> Nueva base
          </Button>
        </div>
      </div>

      {list.isLoading ? (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-32 rounded-xl" />
          ))}
        </div>
      ) : list.isError ? (
        <EmptyState
          icon={DatabaseIcon}
          title="No se pudieron cargar las bases"
          description={list.error instanceof ApiError ? list.error.message : undefined}
          action={<Button size="sm" variant="outline" onClick={() => list.refetch()}>Reintentar</Button>}
        />
      ) : (list.data ?? []).length === 0 ? (
        <EmptyState
          icon={DatabaseIcon}
          title="Todavía no tienes bases de datos"
          description="Crea una desde cero, parte de una plantilla (videojuegos, libros, inventario…) o deja que la IA proponga la estructura."
          action={
            <div className="flex gap-2">
              <Button size="sm" variant="outline" onClick={() => setTemplatesOpen(true)}>Ver plantillas</Button>
              <Button size="sm" onClick={() => setCreating(true)}>Crear la primera</Button>
            </div>
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {(list.data ?? []).map((d) => {
            const c = dbColor(d.color);
            return (
              <div
                key={d.id}
                onClick={() => navigate(`/databases/${d.id}`)}
                className="group flex cursor-pointer flex-col rounded-xl border border-border bg-card p-4 text-left shadow-sm transition-all hover:shadow-md"
              >
                <div className="flex items-center gap-3">
                  <span className={cn("flex h-10 w-10 items-center justify-center rounded-lg", c.bg)}>
                    <DbIcon icon={d.icon} className={cn("h-5 w-5", c.text)} />
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-semibold">{d.name}</p>
                    <p className="truncate text-xs text-muted-foreground">{d.description || "Sin descripción"}</p>
                  </div>
                </div>
                <div className="mt-3 flex items-center gap-3 text-[11px] text-muted-foreground">
                  <span>{d.record_count} registro(s)</span>
                  <span>{d.field_count} campo(s)</span>
                  <span className="ml-auto">{relativeTime(d.updated_at)}</span>
                </div>
                <div className="mt-2 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    title="Renombrar"
                    onClick={(e) => {
                      e.stopPropagation();
                      setRenameTarget(d);
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
                      duplicate.mutate(d.id);
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
                      setDeleteTarget(d);
                    }}
                  >
                    <Trash2 />
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {creating && (
        <DatabaseFormDialog
          onClose={() => setCreating(false)}
          onSaved={(newId) => {
            invalidate();
            navigate(`/databases/${newId}`);
          }}
        />
      )}
      {aiOpen && (
        <AiStructureDialog
          onClose={() => setAiOpen(false)}
          onCreated={(newId) => {
            invalidate();
            navigate(`/databases/${newId}`);
          }}
        />
      )}
      {templatesOpen && (
        <TemplatesDialog
          onClose={() => setTemplatesOpen(false)}
          onInstalled={(newId) => {
            invalidate();
            navigate(`/databases/${newId}`);
          }}
        />
      )}
      {renameTarget && (
        <DatabaseFormDialog
          key={renameTarget.id}
          database={renameTarget}
          onClose={() => setRenameTarget(null)}
          onSaved={() => invalidate()}
        />
      )}
      <Dialog open={deleteTarget != null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Eliminar base de datos</DialogTitle>
            <DialogDescription>
              «{deleteTarget?.name}» y todos sus registros se eliminarán. Esta acción no se puede deshacer.
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
    </div>
  );
}

function DatabaseFormDialog({
  database,
  onClose,
  onSaved,
}: {
  database?: DatabaseSummary;
  onClose: () => void;
  onSaved: (id: Id) => void;
}) {
  const [name, setName] = useState(database?.name ?? "");
  const [description, setDescription] = useState(database?.description ?? "");
  const [icon, setIcon] = useState(database?.icon ?? "database");
  const [color, setColor] = useState(database?.color ?? "blue");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const data = { name: name.trim(), description, icon, color };
      const saved = database
        ? await invoke<{ id: Id }>("databases.update", { id: database.id, data })
        : await invoke<{ id: Id }>("databases.create", { data });
      onSaved(saved.id);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Error desconocido");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{database ? "Editar base de datos" : "Nueva base de datos"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Nombre</Label>
            <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="p. ej. Videojuegos" />
          </div>
          <div className="space-y-1.5">
            <Label>Descripción</Label>
            <Input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Opcional" />
          </div>
          <div className="space-y-1.5">
            <Label>Icono</Label>
            <div className="flex flex-wrap gap-1.5">
              {Object.keys(DB_ICONS).map((k) => (
                <button
                  key={k}
                  onClick={() => setIcon(k)}
                  className={cn(
                    "flex h-8 w-8 items-center justify-center rounded-lg border cursor-pointer",
                    icon === k ? "border-primary bg-primary/15 text-primary" : "border-border text-muted-foreground hover:bg-muted",
                  )}
                  title={k}
                >
                  <DbIcon icon={k} />
                </button>
              ))}
            </div>
          </div>
          <div className="space-y-1.5">
            <Label>Color</Label>
            <div className="flex gap-1.5">
              {DB_COLORS.map((c) => (
                <button
                  key={c}
                  onClick={() => setColor(c)}
                  className={cn(
                    "h-6 w-6 rounded-full border-2 cursor-pointer",
                    dbColor(c).dot,
                    color === c ? "border-foreground" : "border-transparent",
                  )}
                  title={c}
                />
              ))}
            </div>
          </div>
          {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => void save()} disabled={busy || !name.trim()}>
            {busy ? "Guardando…" : database ? "Guardar" : "Crear base"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function TemplatesDialog({ onClose, onInstalled }: { onClose: () => void; onInstalled: (id: Id) => void }) {
  const [installing, setInstalling] = useState<string | null>(null);
  const templates = useQuery({
    queryKey: ["databases", "templates"],
    queryFn: () => invoke<DbTemplate[]>("databases.templates"),
    staleTime: Infinity,
  });

  const install = async (t: DbTemplate) => {
    setInstalling(t.id);
    try {
      const detail = await invoke<DatabaseDetail>("databases.install_template", { template_id: t.id });
      toast.success(`Base «${t.name}» creada`, {
        description: `${detail.record_count} registros de ejemplo reales, listos para editar o borrar.`,
      });
      onInstalled(detail.id);
      onClose();
    } catch (e) {
      toastDbError(e, "No se pudo instalar la plantilla");
    } finally {
      setInstalling(null);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Plantillas</DialogTitle>
        </DialogHeader>
        {templates.isLoading ? (
          <div className="grid grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => <Skeleton key={i} className="h-24 rounded-xl" />)}
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {(templates.data ?? []).map((t) => {
              const c = dbColor(t.color);
              return (
                <button
                  key={t.id}
                  onClick={() => void install(t)}
                  disabled={installing !== null}
                  className="rounded-xl border border-border bg-card p-3 text-left shadow-sm transition-colors hover:bg-muted/50 disabled:opacity-50 cursor-pointer"
                >
                  <div className="flex items-center gap-2">
                    <span className={cn("flex h-8 w-8 items-center justify-center rounded-lg", c.bg)}>
                      <DbIcon icon={t.icon} className={c.text} />
                    </span>
                    <p className="flex-1 text-sm font-medium">{t.name}</p>
                    {installing === t.id && <span className="text-[10px] text-muted-foreground">Instalando…</span>}
                  </div>
                  <p className="mt-1.5 text-[11px] text-muted-foreground">{t.description}</p>
                  <p className="mt-1 text-[10px] text-muted-foreground/70">
                    {t.fields.length} campos · {t.sample_records.length} ejemplos · {t.views.length} vistas
                  </p>
                </button>
              );
            })}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cerrar</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ===========================================================================
// Detalle de una base
// ===========================================================================

function DatabaseDetailPage({ id }: { id: Id }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const invalidate = useInvalidateDatabases();
  const [searchParams, setSearchParams] = useSearchParams();
  const highlightId = searchParams.get("record");

  const detail = useQuery({
    queryKey: ["databases", "detail", id],
    queryFn: () => invoke<DatabaseDetail>("databases.get", { id }),
  });

  const [activeViewId, setActiveViewId] = useState<Id | null>(null);
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search, 250);
  const [tempConfig, setTempConfig] = useState<DbViewConfig | null>(null);
  const [filterOpen, setFilterOpen] = useState(false);

  const [recordDialog, setRecordDialog] = useState<{ record: DbRecord | null; preset?: Record<Id, unknown> } | null>(null);
  const [viewDialog, setViewDialog] = useState<{ view: DbView | null } | null>(null);
  const [fieldsOpen, setFieldsOpen] = useState(false);
  const [editDbOpen, setEditDbOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [aiSuggestOpen, setAiSuggestOpen] = useState(false);
  const [aiDupOpen, setAiDupOpen] = useState(false);
  const [deleteDbOpen, setDeleteDbOpen] = useState(false);
  const [schemaMode, setSchemaMode] = useState(false);

  const views = detail.data?.views ?? [];
  const fields = detail.data?.fields ?? [];
  const activeView = views.find((v) => v.id === activeViewId) ?? views[0] ?? null;

  useEffect(() => {
    setActiveViewId(null);
    setTempConfig(null);
    setSearch("");
    setSchemaMode(false);
  }, [id]);

  useEffect(() => {
    setTempConfig(null);
  }, [activeView?.id]);

  const effectiveConfig: DbViewConfig = tempConfig ?? activeView?.config ?? {
    visible_field_ids: [],
    card_field_ids: [],
    filters: { conjunction: "and", filters: [] },
    sorts: [],
  };

  const recordsQuery = useQuery({
    queryKey: ["databases", "records", id, debouncedSearch, effectiveConfig.filters, effectiveConfig.sorts, highlightId],
    queryFn: () =>
      invoke<RecordPage>("databases.records.query", {
        database_id: id,
        query: {
          search: debouncedSearch || null,
          filters: effectiveConfig.filters,
          sorts: effectiveConfig.sorts,
          highlight_id: highlightId,
          limit: 1000,
        },
      }),
    enabled: !!detail.data,
  });

  const records = recordsQuery.data?.records ?? [];

  const refreshRecords = () => {
    qc.invalidateQueries({ queryKey: ["databases", "records", id] });
    qc.invalidateQueries({ queryKey: ["databases", "detail", id] });
    qc.invalidateQueries({ queryKey: ["databases", "list"] });
  };

  const updateValue = useMutation({
    mutationFn: ({ recordId, fieldId, value }: { recordId: Id; fieldId: Id; value: unknown }) =>
      invoke("databases.records.update", { database_id: id, id: recordId, data: { values: { [fieldId]: value } } }),
    onSuccess: refreshRecords,
    onError: (e) => toastDbError(e, "No se pudo guardar el valor"),
  });

  const deleteRecords = async (ids: Id[]) => {
    try {
      await invoke("databases.records.delete", { database_id: id, ids });
      toast.success(`${ids.length} registro(s) eliminados`);
      refreshRecords();
    } catch (e) {
      toastDbError(e, "No se pudieron eliminar los registros");
    }
  };

  const saveRecord = async (values: Record<Id, unknown>) => {
    if (recordDialog?.record) {
      const patch: Record<Id, unknown> = {};
      for (const f of fields) {
        if (f.kind === "formula") continue;
        patch[f.id] = values[f.id] ?? null;
      }
      await invoke("databases.records.update", { database_id: id, id: recordDialog.record.id, data: { values: patch } });
      toast.success("Registro actualizado");
    } else {
      await invoke("databases.records.create", { database_id: id, data: { values } });
      toast.success("Registro creado");
    }
    refreshRecords();
  };

  const onSortColumn = (fieldId: Id) => {
    const sorts = [...(effectiveConfig.sorts ?? [])];
    const idx = sorts.findIndex((s) => s.field_id === fieldId);
    if (idx === -1) sorts.unshift({ field_id: fieldId, dir: "asc" });
    else if (sorts[idx].dir === "asc") sorts[idx] = { field_id: fieldId, dir: "desc" };
    else sorts.splice(idx, 1);
    setTempConfig({ ...effectiveConfig, sorts });
  };

  const onNavigateRelation = (databaseId: Id, recordId: Id) => {
    if (databaseId === id) {
      setSearchParams({ record: recordId });
    } else {
      navigate(`/databases/${databaseId}?record=${recordId}`);
    }
  };

  const deleteView = async (v: DbView) => {
    try {
      await invoke("databases.views.delete", { id: v.id });
      invalidate();
      if (activeView?.id === v.id) setActiveViewId(null);
      toast.success(`Vista «${v.name}» eliminada`);
    } catch (e) {
      toastDbError(e, "No se pudo eliminar la vista");
    }
  };

  if (detail.isLoading) {
    return (
      <div className="mx-auto max-w-[1200px] space-y-4 p-6">
        <Skeleton className="h-10 w-72" />
        <Skeleton className="h-8 w-full" />
        <Skeleton className="h-96 w-full rounded-xl" />
      </div>
    );
  }
  if (detail.isError || !detail.data) {
    return (
      <div className="mx-auto max-w-[1200px] p-6">
        <EmptyState
          icon={DatabaseIcon}
          title="Base de datos no encontrada"
          description={detail.error instanceof ApiError ? detail.error.message : undefined}
          action={<Button size="sm" variant="outline" onClick={() => navigate("/databases")}>Volver al listado</Button>}
        />
      </div>
    );
  }

  const db = detail.data;
  const c = dbColor(db.color);
  const filterCount = effectiveConfig.filters?.filters.length ?? 0;

  const viewProps = {
    databaseId: id,
    fields,
    records,
    view: activeView!,
    sorts: effectiveConfig.sorts ?? [],
    onSort: onSortColumn,
    onUpdateValue: async (recordId: Id, fieldId: Id, value: unknown) => {
      await updateValue.mutateAsync({ recordId, fieldId, value });
    },
    onOpenRecord: (record: DbRecord) => setRecordDialog({ record }),
    onNewRecord: (preset?: Record<Id, unknown>) => setRecordDialog({ record: null, preset }),
    onNavigate: onNavigateRelation,
    highlightId,
  };

  return (
    <div className="mx-auto max-w-[1200px] space-y-4 p-6 animate-fade-in">
      {/* Cabecera */}
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" size="sm" className="h-8 w-8 p-0" onClick={() => navigate("/databases")}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <span className={cn("flex h-10 w-10 items-center justify-center rounded-lg", c.bg)}>
          <DbIcon icon={db.icon} className={cn("h-5 w-5", c.text)} />
        </span>
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-lg font-semibold tracking-tight">{db.name}</h1>
          <p className="truncate text-xs text-muted-foreground">
            {db.description || "Sin descripción"} · {recordsQuery.data?.total ?? 0} registro(s)
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" onClick={() => setFieldsOpen(true)}>
            <Settings2 className="h-3.5 w-3.5" /> Campos
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm">Más</Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={() => setEditDbOpen(true)}>
                <Pencil className="h-3.5 w-3.5" /> Editar base
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setImportOpen(true)}>
                <Upload className="h-3.5 w-3.5" /> Importar CSV/JSON
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setExportOpen(true)}>
                <Download className="h-3.5 w-3.5" /> Exportar CSV/JSON
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => setAiSuggestOpen(true)}>
                <Sparkles className="h-3.5 w-3.5" /> Sugerir campos (IA)
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setAiDupOpen(true)}>
                <Sparkles className="h-3.5 w-3.5" /> Detectar duplicados
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem className="text-destructive" onClick={() => setDeleteDbOpen(true)}>
                <Trash2 className="h-3.5 w-3.5" /> Eliminar base
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button size="sm" onClick={() => setRecordDialog({ record: null })} disabled={fields.length === 0}>
            <Plus className="h-3.5 w-3.5" /> Nuevo registro
          </Button>
        </div>
      </div>

      {fields.length === 0 && (
        <EmptyState
          icon={Settings2}
          title="Define los campos de tu base"
          description="Añade campos (texto, números, fechas, selecciones, relaciones, fórmulas…) para empezar a guardar registros."
          action={<Button size="sm" onClick={() => setFieldsOpen(true)}>Crear campos</Button>}
        />
      )}

      {fields.length > 0 && (
        <>
          {/* Selector de vistas + búsqueda + filtros */}
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex flex-wrap items-center gap-1 rounded-lg border border-border bg-card p-1">
              {views.map((v) => {
                const Icon = VIEW_ICON[v.kind];
                const active = activeView?.id === v.id;
                return (
                  <span key={v.id} className="group relative">
                    <button
                      onClick={() => setActiveViewId(v.id)}
                      className={cn(
                        "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer",
                        active ? "bg-primary/15 text-primary" : "text-muted-foreground hover:bg-muted",
                      )}
                    >
                      <Icon className="h-3.5 w-3.5" /> {v.name}
                    </button>
                    {active && (
                      <span className="absolute -right-1 -top-1 hidden gap-0.5 group-hover:flex">
                        <button
                          onClick={() => setViewDialog({ view: v })}
                          className="rounded-full border border-border bg-card p-0.5 text-muted-foreground hover:text-foreground cursor-pointer"
                          title="Editar vista"
                        >
                          <Pencil className="h-2.5 w-2.5" />
                        </button>
                        {views.length > 1 && (
                          <button
                            onClick={() => void deleteView(v)}
                            className="rounded-full border border-border bg-card p-0.5 text-destructive cursor-pointer"
                            title="Eliminar vista"
                          >
                            <X className="h-2.5 w-2.5" />
                          </button>
                        )}
                      </span>
                    )}
                  </span>
                );
              })}
              <button
                onClick={() => setViewDialog({ view: null })}
                className="flex items-center gap-1 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-muted cursor-pointer"
                title="Nueva vista"
              >
                <Plus className="h-3.5 w-3.5" />
              </button>
              <div className="mx-0.5 h-5 w-px bg-border" />
              <button
                onClick={() => setSchemaMode((v) => !v)}
                className={cn(
                  "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer",
                  schemaMode ? "bg-primary/15 text-primary" : "text-muted-foreground hover:bg-muted",
                )}
                title="Ver tablas y relaciones (claves primarias y ajenas)"
              >
                <Network className="h-3.5 w-3.5" /> Esquema
              </button>
            </div>

            {!schemaMode && (
            <>
            <div className="relative min-w-[180px] flex-1">
              <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Buscar en los registros…"
                className="h-8 pl-8 text-xs"
              />
            </div>

            <Button
              variant={filterCount > 0 ? "default" : "outline"}
              size="sm"
              className="h-8"
              onClick={() => setFilterOpen(true)}
            >
              <Filter className="h-3.5 w-3.5" /> Filtros{filterCount > 0 ? ` (${filterCount})` : ""}
            </Button>
            {tempConfig && (
              <Button variant="ghost" size="sm" className="h-8 text-xs" onClick={() => setTempConfig(null)}>
                Restablecer vista
              </Button>
            )}
            {highlightId && (
              <Badge variant="default" className="gap-1">
                Registro destacado
                <button onClick={() => setSearchParams({})} className="cursor-pointer"><X className="h-3 w-3" /></button>
              </Badge>
            )}
            </>
            )}
          </div>

          {/* Renderizador de la vista activa */}
          {schemaMode ? (
            <SchemaView databaseId={id} />
          ) : recordsQuery.isLoading ? (
            <Skeleton className="h-80 w-full rounded-xl" />
          ) : recordsQuery.isError ? (
            <EmptyState
              icon={DatabaseIcon}
              title="No se pudieron cargar los registros"
              description={recordsQuery.error instanceof ApiError ? recordsQuery.error.message : undefined}
              action={<Button size="sm" variant="outline" onClick={() => recordsQuery.refetch()}>Reintentar</Button>}
            />
          ) : records.length === 0 && !debouncedSearch && filterCount === 0 && !highlightId ? (
            <EmptyState
              icon={DatabaseIcon}
              title="Sin registros todavía"
              description="Crea el primero o importa un CSV/JSON con tus datos."
              action={
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={() => setImportOpen(true)}>Importar</Button>
                  <Button size="sm" onClick={() => setRecordDialog({ record: null })}>Nuevo registro</Button>
                </div>
              }
            />
          ) : activeView ? (
            <>
              {activeView.kind === "table" && <TableView {...viewProps} onDeleteSelected={deleteRecords} />}
              {activeView.kind === "kanban" && <KanbanView {...viewProps} />}
              {activeView.kind === "calendar" && <CalendarView {...viewProps} />}
              {activeView.kind === "gallery" && <GalleryView {...viewProps} />}
              {activeView.kind === "list" && <ListView {...viewProps} />}
              {records.length === 0 && (debouncedSearch || filterCount > 0 || highlightId) && activeView.kind === "table" && null}
              {records.length === 0 && (debouncedSearch || filterCount > 0 || highlightId) && activeView.kind !== "table" && (
                <p className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
                  Ningún registro coincide con la búsqueda o los filtros.
                </p>
              )}
            </>
          ) : null}
        </>
      )}

      {/* Diálogos */}
      {recordDialog && (
        <RecordDialog
          databaseId={id}
          fields={fields}
          record={recordDialog.record}
          preset={recordDialog.preset}
          onClose={() => setRecordDialog(null)}
          onSave={saveRecord}
          onDelete={recordDialog.record ? () => deleteRecords([recordDialog.record!.id]) : undefined}
        />
      )}
      {viewDialog && (
        <ViewDialog
          databaseId={id}
          fields={fields}
          view={viewDialog.view}
          onClose={() => setViewDialog(null)}
          onSaved={(v) => {
            invalidate();
            setActiveViewId(v.id);
          }}
        />
      )}
      {fieldsOpen && (
        <FieldsManagerDialog
          databaseId={id}
          fields={fields}
          onClose={() => setFieldsOpen(false)}
          onChanged={invalidate}
        />
      )}
      {filterOpen && (
        <FilterDialog
          fields={fields}
          config={effectiveConfig}
          onClose={() => setFilterOpen(false)}
          onApply={(cfg) => {
            setTempConfig(cfg);
            setFilterOpen(false);
          }}
          onSaveToView={
            activeView
              ? async (cfg) => {
                  await invoke("databases.views.update", { id: activeView.id, data: { config: cfg } });
                  invalidate();
                  setTempConfig(null);
                  setFilterOpen(false);
                  toast.success("Filtros guardados en la vista");
                }
              : undefined
          }
        />
      )}
      {editDbOpen && (
        <DatabaseFormDialog database={db} onClose={() => setEditDbOpen(false)} onSaved={() => invalidate()} />
      )}
      {importOpen && (
        <ImportDialog databaseId={id} fields={fields} onClose={() => setImportOpen(false)} onImported={refreshRecords} />
      )}
      {exportOpen && <ExportDialog databaseId={id} views={views} onClose={() => setExportOpen(false)} />}
      {aiSuggestOpen && (
        <AiSuggestDialog databaseId={id} onClose={() => setAiSuggestOpen(false)} onApplied={invalidate} />
      )}
      {aiDupOpen && (
        <AiDuplicatesDialog
          databaseId={id}
          onClose={() => {
            setAiDupOpen(false);
            refreshRecords();
          }}
          onDelete={async (rid) => {
            await invoke("databases.records.delete", { database_id: id, ids: [rid] });
            toast.success("Registro eliminado");
          }}
        />
      )}
      {deleteDbOpen && (
        <Dialog open onOpenChange={(o) => !o && setDeleteDbOpen(false)}>
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle>¿Eliminar «{db.name}»?</DialogTitle>
            </DialogHeader>
            <p className="text-xs text-muted-foreground">
              Se eliminarán sus {db.record_count} registro(s), campos y vistas. Las referencias desde otras bases se
              limpiarán automáticamente. Esta acción no se puede deshacer.
            </p>
            <DialogFooter>
              <Button variant="ghost" onClick={() => setDeleteDbOpen(false)}>Cancelar</Button>
              <Button
                variant="destructive"
                onClick={async () => {
                  try {
                    await invoke("databases.delete", { id });
                    toast.success(`Base «${db.name}» eliminada`);
                    invalidate();
                    navigate("/databases");
                  } catch (e) {
                    toastDbError(e, "No se pudo eliminar la base");
                  }
                }}
              >
                <Trash2 className="h-3.5 w-3.5" /> Eliminar definitivamente
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}

function FilterDialog({
  fields,
  config,
  onClose,
  onApply,
  onSaveToView,
}: {
  fields: DatabaseDetail["fields"];
  config: DbViewConfig;
  onClose: () => void;
  onApply: (c: DbViewConfig) => void;
  onSaveToView?: (c: DbViewConfig) => Promise<void>;
}) {
  const [draft, setDraft] = useState<DbViewConfig>(config);
  const [busy, setBusy] = useState(false);
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>Filtros y ordenación</DialogTitle>
        </DialogHeader>
        <FilterSortEditor fields={fields} config={draft} onChange={setDraft} />
        <DialogFooter className="sm:justify-between">
          <Button variant="ghost" onClick={() => setDraft({ ...draft, filters: { conjunction: "and", filters: [] }, sorts: [] })}>
            Limpiar todo
          </Button>
          <div className="flex gap-2">
            {onSaveToView && (
              <Button
                variant="outline"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    await onSaveToView(draft);
                  } catch (e) {
                    toastDbError(e, "No se pudo guardar en la vista");
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                Guardar en la vista
              </Button>
            )}
            <Button onClick={() => onApply(draft)}>Aplicar</Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
