import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { GripVertical, Plus, Trash2 } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type {
  DatabaseSummary,
  DbField,
  DbFieldConfig,
  DbFieldKind,
  DbFilter,
  DbFilterGroup,
  DbSort,
  DbView,
  DbViewConfig,
  DbViewKind,
} from "./types";
import {
  CARDINALITY_LABEL,
  DB_COLOR_STYLE,
  FILTER_OP_LABEL,
  FIELD_KIND_LABEL,
  OPTION_COLORS,
  VIEW_KIND_LABEL,
  needsFilterValue,
  opsForKind,
  optionClass,
} from "./shared";

const ALL_KINDS: DbFieldKind[] = [
  "text", "long_text", "integer", "decimal", "date", "datetime", "checkbox",
  "select", "multiselect", "url", "email", "phone", "image", "file", "rating",
  "relation", "formula",
];

// ---------------------------------------------------------------------------
// Editor de campo
// ---------------------------------------------------------------------------

export function FieldDialog({
  databaseId,
  field,
  onClose,
  onSaved,
}: {
  databaseId: Id;
  field: DbField | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = useState(field?.name ?? "");
  const [kind, setKind] = useState<DbFieldKind>(field?.kind ?? "text");
  const [config, setConfig] = useState<DbFieldConfig>(field?.config ?? {});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const databases = useQuery({
    queryKey: ["databases", "list"],
    queryFn: () => invoke<DatabaseSummary[]>("databases.list"),
  });

  useEffect(() => {
    setName(field?.name ?? "");
    setKind(field?.kind ?? "text");
    setConfig(field?.config ?? {});
    setError(null);
  }, [field]);

  const options = config.options ?? [];
  const setOptions = (opts: DbFieldConfig["options"]) => setConfig((c) => ({ ...c, options: opts }));

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const data = { name: name.trim(), kind, config };
      if (field) {
        await invoke("databases.fields.update", { id: field.id, data });
      } else {
        await invoke("databases.fields.create", { database_id: databaseId, data });
      }
      onSaved();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Error desconocido");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{field ? "Editar campo" : "Nuevo campo"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label>Nombre</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="p. ej. Estado" autoFocus />
            </div>
            <div className="space-y-1.5">
              <Label>Tipo</Label>
              <Select value={kind} onValueChange={(v) => setKind(v as DbFieldKind)}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {ALL_KINDS.map((k) => (
                    <SelectItem key={k} value={k}>{FIELD_KIND_LABEL[k]}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {(kind === "select" || kind === "multiselect") && (
            <div className="space-y-2">
              <Label>Opciones</Label>
              {options.map((o, i) => (
                <div key={o.id} className="flex items-center gap-2">
                  <Input
                    value={o.label}
                    onChange={(e) =>
                      setOptions(options.map((x, j) => (j === i ? { ...x, label: e.target.value } : x)))
                    }
                    className="h-8 flex-1 text-xs"
                  />
                  <div className="flex gap-1">
                    {OPTION_COLORS.map((c) => (
                      <button
                        key={c}
                        onClick={() => setOptions(options.map((x, j) => (j === i ? { ...x, color: c } : x)))}
                        className={cn(
                          "h-5 w-5 rounded-full border cursor-pointer",
                          optionClass(c).split(" ")[0],
                          o.color === c ? "border-foreground" : "border-transparent",
                        )}
                        title={c}
                      />
                    ))}
                  </div>
                  <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-destructive" onClick={() => setOptions(options.filter((_, j) => j !== i))}>
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))}
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  setOptions([
                    ...options,
                    { id: `opt-${Date.now()}-${options.length}`, label: `Opción ${options.length + 1}`, color: OPTION_COLORS[options.length % OPTION_COLORS.length] },
                  ])
                }
              >
                <Plus className="h-3.5 w-3.5" /> Añadir opción
              </Button>
              <p className="text-[11px] text-muted-foreground">
                Los registros guardan el id interno de la opción: renombrar etiquetas no pierde datos.
              </p>
            </div>
          )}

          {kind === "relation" && (
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Base destino</Label>
                <Select
                  value={config.target_database_id ?? undefined}
                  onValueChange={(v) => setConfig((c) => ({ ...c, target_database_id: v }))}
                >
                  <SelectTrigger><SelectValue placeholder="Elegir base…" /></SelectTrigger>
                  <SelectContent>
                    {(databases.data ?? []).map((d) => (
                      <SelectItem key={d.id} value={d.id}>{d.name}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label>Cardinalidad</Label>
                <Select
                  value={config.cardinality ?? "many_to_many"}
                  onValueChange={(v) => setConfig((c) => ({ ...c, cardinality: v as DbFieldConfig["cardinality"] }))}
                >
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {Object.entries(CARDINALITY_LABEL).map(([k, label]) => (
                      <SelectItem key={k} value={k}>{label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
          )}

          {kind === "formula" && (
            <div className="space-y-1.5">
              <Label>Expresión</Label>
              <Textarea
                value={config.expression ?? ""}
                onChange={(e) => setConfig((c) => ({ ...c, expression: e.target.value }))}
                placeholder={'p. ej. ROUND({Precio} * 1.21, 2)'}
                rows={2}
                className="font-mono text-xs"
              />
              <p className="text-[11px] text-muted-foreground">
                Referencias con {"{Nombre}"} · operadores + - * / % &amp; y comparaciones · funciones IF, CONCAT, ROUND, ABS, LEN, UPPER, LOWER, TODAY, DAYS. Se calcula al leer; no se almacena.
              </p>
            </div>
          )}

          {kind === "rating" && (
            <div className="space-y-1.5">
              <Label>Máximo (1-10)</Label>
              <Input
                type="number"
                min={1}
                max={10}
                value={config.max_rating ?? 5}
                onChange={(e) => setConfig((c) => ({ ...c, max_rating: Number(e.target.value) }))}
                className="w-24"
              />
            </div>
          )}

          {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => void save()} disabled={busy || !name.trim()}>
            {busy ? "Guardando…" : "Guardar campo"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Editor de vista
// ---------------------------------------------------------------------------

export function ViewDialog({
  databaseId,
  fields,
  view,
  onClose,
  onSaved,
}: {
  databaseId: Id;
  fields: DbField[];
  view: DbView | null;
  onClose: () => void;
  onSaved: (view: DbView) => void;
}) {
  const [name, setName] = useState(view?.name ?? "");
  const [kind, setKind] = useState<DbViewKind>(view?.kind ?? "table");
  const [config, setConfig] = useState<DbViewConfig>(
    view?.config ?? { visible_field_ids: [], card_field_ids: [], filters: { conjunction: "and", filters: [] }, sorts: [] },
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setName(view?.name ?? "");
    setKind(view?.kind ?? "table");
    setConfig(view?.config ?? { visible_field_ids: [], card_field_ids: [], filters: { conjunction: "and", filters: [] }, sorts: [] });
    setError(null);
  }, [view]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const data = { name: name.trim(), kind, config };
      const saved = view
        ? await invoke<DbView>("databases.views.update", { id: view.id, data })
        : await invoke<DbView>("databases.views.create", { database_id: databaseId, data });
      onSaved(saved);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Error desconocido");
    } finally {
      setBusy(false);
    }
  };

  const selectField = (label: string, kinds: DbFieldKind[], value: Id | null | undefined, set: (v: Id | null) => void) => (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <Select value={value ?? "__none__"} onValueChange={(v) => set(v === "__none__" ? null : v)}>
        <SelectTrigger><SelectValue placeholder="Elegir campo…" /></SelectTrigger>
        <SelectContent>
          <SelectItem value="__none__">(ninguno)</SelectItem>
          {fields.filter((f) => kinds.includes(f.kind)).map((f) => (
            <SelectItem key={f.id} value={f.id}>{f.name}</SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );

  const toggleVisible = (fid: Id) => {
    setConfig((c) => {
      const current = c.visible_field_ids.length === 0 ? fields.map((f) => f.id) : c.visible_field_ids;
      const next = current.includes(fid) ? current.filter((x) => x !== fid) : [...current, fid];
      return { ...c, visible_field_ids: next.length === fields.length ? [] : next };
    });
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{view ? "Editar vista" : "Nueva vista"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label>Nombre</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="p. ej. Por estado" autoFocus />
            </div>
            <div className="space-y-1.5">
              <Label>Tipo de vista</Label>
              <Select value={kind} onValueChange={(v) => setKind(v as DbViewKind)}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {(Object.keys(VIEW_KIND_LABEL) as DbViewKind[]).map((k) => (
                    <SelectItem key={k} value={k}>{VIEW_KIND_LABEL[k]}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {kind === "kanban" && selectField("Agrupar por (selección)", ["select"], config.group_field_id, (v) => setConfig((c) => ({ ...c, group_field_id: v })))}
          {kind === "calendar" && selectField("Campo de fecha", ["date", "datetime"], config.date_field_id, (v) => setConfig((c) => ({ ...c, date_field_id: v })))}
          {kind === "gallery" && (
            <>
              {selectField("Campo de imagen", ["image"], config.image_field_id, (v) => setConfig((c) => ({ ...c, image_field_id: v })))}
              <div className="space-y-1.5">
                <Label>Campos en la tarjeta</Label>
                <div className="flex flex-wrap gap-1.5">
                  {fields.map((f) => (
                    <button
                      key={f.id}
                      onClick={() =>
                        setConfig((c) => ({
                          ...c,
                          card_field_ids: c.card_field_ids.includes(f.id)
                            ? c.card_field_ids.filter((x) => x !== f.id)
                            : [...c.card_field_ids, f.id],
                        }))
                      }
                      className={cn(
                        "rounded-full border px-2.5 py-1 text-[11px] cursor-pointer",
                        config.card_field_ids.includes(f.id)
                          ? "border-primary bg-primary/15 text-primary"
                          : "border-border text-muted-foreground hover:bg-muted",
                      )}
                    >
                      {f.name}
                    </button>
                  ))}
                </div>
              </div>
            </>
          )}
          {kind === "table" && (
            <div className="space-y-1.5">
              <Label>Columnas visibles (vacío = todas)</Label>
              <div className="flex flex-wrap gap-1.5">
                {fields.map((f) => {
                  const active = config.visible_field_ids.length === 0 || config.visible_field_ids.includes(f.id);
                  return (
                    <button
                      key={f.id}
                      onClick={() => toggleVisible(f.id)}
                      className={cn(
                        "rounded-full border px-2.5 py-1 text-[11px] cursor-pointer",
                        active ? "border-primary bg-primary/15 text-primary" : "border-border text-muted-foreground hover:bg-muted",
                      )}
                    >
                      {f.name}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          <FilterSortEditor fields={fields} config={config} onChange={setConfig} />

          {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => void save()} disabled={busy || !name.trim()}>
            {busy ? "Guardando…" : "Guardar vista"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Filtros + orden (reutilizable en vista y barra temporal)
// ---------------------------------------------------------------------------

export function FilterSortEditor({
  fields,
  config,
  onChange,
}: {
  fields: DbField[];
  config: DbViewConfig;
  onChange: (c: DbViewConfig) => void;
}) {
  const filterable = fields.filter((f) => f.kind !== "formula");
  const filters = config.filters ?? { conjunction: "and" as const, filters: [] };
  const sorts = config.sorts ?? [];

  const setFilters = (g: DbFilterGroup) => onChange({ ...config, filters: g });
  const setSorts = (s: DbSort[]) => onChange({ ...config, sorts: s });

  const updateFilter = (i: number, patch: Partial<DbFilter>) => {
    setFilters({ ...filters, filters: filters.filters.map((f, j) => (j === i ? { ...f, ...patch } : f)) });
  };

  const fieldOf = (fid: Id) => fields.find((f) => f.id === fid);

  const valueEditor = (f: DbFilter, i: number) => {
    if (!needsFilterValue(f.op)) return null;
    const field = fieldOf(f.field_id);
    if (!field) return null;
    if (field.kind === "select" || field.kind === "multiselect") {
      return (
        <Select value={typeof f.value === "string" ? f.value : undefined} onValueChange={(v) => updateFilter(i, { value: v })}>
          <SelectTrigger className="h-8 w-36 text-xs"><SelectValue placeholder="Opción…" /></SelectTrigger>
          <SelectContent>
            {(field.config.options ?? []).map((o) => (
              <SelectItem key={o.id} value={o.id}>{o.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      );
    }
    if (field.kind === "checkbox") {
      return (
        <Select value={f.value === true ? "true" : f.value === false ? "false" : undefined} onValueChange={(v) => updateFilter(i, { value: v === "true" })}>
          <SelectTrigger className="h-8 w-28 text-xs"><SelectValue placeholder="Valor…" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="true">Marcado</SelectItem>
            <SelectItem value="false">Sin marcar</SelectItem>
          </SelectContent>
        </Select>
      );
    }
    return (
      <Input
        type={field.kind === "date" ? "date" : field.kind === "integer" || field.kind === "decimal" || field.kind === "rating" ? "number" : "text"}
        value={f.value === null || f.value === undefined ? "" : String(f.value)}
        onChange={(e) =>
          updateFilter(i, {
            value:
              field.kind === "integer" || field.kind === "decimal" || field.kind === "rating"
                ? e.target.value === "" ? null : Number(e.target.value)
                : e.target.value,
          })
        }
        placeholder="Valor…"
        className="h-8 w-36 text-xs"
      />
    );
  };

  return (
    <div className="space-y-3">
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label>Filtros</Label>
          {filters.filters.length > 1 && (
            <Select value={filters.conjunction} onValueChange={(v) => setFilters({ ...filters, conjunction: v as "and" | "or" })}>
              <SelectTrigger className="h-7 w-24 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="and">Todos (Y)</SelectItem>
                <SelectItem value="or">Alguno (O)</SelectItem>
              </SelectContent>
            </Select>
          )}
        </div>
        {filters.filters.map((f, i) => {
          const field = fieldOf(f.field_id);
          const ops = field ? opsForKind(field.kind) : ["contains"];
          return (
            <div key={i} className="flex flex-wrap items-center gap-2">
              <Select
                value={f.field_id}
                onValueChange={(v) => {
                  const nf = fieldOf(v);
                  updateFilter(i, { field_id: v, op: (nf ? opsForKind(nf.kind)[0] : "contains") as DbFilter["op"], value: null });
                }}
              >
                <SelectTrigger className="h-8 w-36 text-xs"><SelectValue placeholder="Campo…" /></SelectTrigger>
                <SelectContent>
                  {filterable.map((x) => (
                    <SelectItem key={x.id} value={x.id}>{x.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Select value={f.op} onValueChange={(v) => updateFilter(i, { op: v as DbFilter["op"] })}>
                <SelectTrigger className="h-8 w-40 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {ops.map((op) => (
                    <SelectItem key={op} value={op}>{FILTER_OP_LABEL[op]}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {valueEditor(f, i)}
              <Button
                size="sm"
                variant="ghost"
                className="h-7 w-7 p-0 text-destructive"
                onClick={() => setFilters({ ...filters, filters: filters.filters.filter((_, j) => j !== i) })}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          );
        })}
        <Button
          size="sm"
          variant="outline"
          disabled={filterable.length === 0}
          onClick={() =>
            setFilters({
              ...filters,
              filters: [...filters.filters, { field_id: filterable[0].id, op: opsForKind(filterable[0].kind)[0] as DbFilter["op"], value: null }],
            })
          }
        >
          <Plus className="h-3.5 w-3.5" /> Añadir filtro
        </Button>
      </div>

      <div className="space-y-2">
        <Label>Ordenación</Label>
        {sorts.map((s, i) => (
          <div key={i} className="flex items-center gap-2">
            <Select value={s.field_id} onValueChange={(v) => setSorts(sorts.map((x, j) => (j === i ? { ...x, field_id: v } : x)))}>
              <SelectTrigger className="h-8 w-36 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                {fields.filter((f) => !["relation", "multiselect"].includes(f.kind)).map((f) => (
                  <SelectItem key={f.id} value={f.id}>{f.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={s.dir} onValueChange={(v) => setSorts(sorts.map((x, j) => (j === i ? { ...x, dir: v as DbSort["dir"] } : x)))}>
              <SelectTrigger className="h-8 w-32 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="asc">Ascendente</SelectItem>
                <SelectItem value="desc">Descendente</SelectItem>
              </SelectContent>
            </Select>
            <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-destructive" onClick={() => setSorts(sorts.filter((_, j) => j !== i))}>
              <Trash2 className="h-3.5 w-3.5" />
            </Button>
          </div>
        ))}
        <Button
          size="sm"
          variant="outline"
          disabled={fields.length === 0}
          onClick={() => setSorts([...sorts, { field_id: fields[0].id, dir: "asc" }])}
        >
          <Plus className="h-3.5 w-3.5" /> Añadir orden
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Gestor de campos (crear/editar/borrar/reordenar con drag)
// ---------------------------------------------------------------------------

export function FieldsManagerDialog({
  databaseId,
  fields,
  onClose,
  onChanged,
}: {
  databaseId: Id;
  fields: DbField[];
  onClose: () => void;
  onChanged: () => void;
}) {
  const [editing, setEditing] = useState<DbField | null | undefined>(undefined);
  const [order, setOrder] = useState<DbField[]>(fields);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => setOrder(fields), [fields]);

  const persistOrder = async (next: DbField[]) => {
    setOrder(next);
    try {
      await invoke("databases.fields.reorder", { database_id: databaseId, field_ids: next.map((f) => f.id) });
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "No se pudo reordenar");
    }
  };

  const remove = async (f: DbField) => {
    try {
      await invoke("databases.fields.delete", { id: f.id });
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "No se pudo eliminar");
    }
  };

  return (
    <>
      <Dialog open={editing === undefined} onOpenChange={(o) => !o && onClose()}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Campos de la base</DialogTitle>
          </DialogHeader>
          <div className="space-y-1.5">
            {order.map((f, i) => (
              <div
                key={f.id}
                draggable
                onDragStart={() => setDragIndex(i)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={() => {
                  if (dragIndex === null || dragIndex === i) return;
                  const next = [...order];
                  const [moved] = next.splice(dragIndex, 1);
                  next.splice(i, 0, moved);
                  setDragIndex(null);
                  void persistOrder(next);
                }}
                className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2"
              >
                <GripVertical className="h-4 w-4 cursor-grab text-muted-foreground/50" />
                <span className="flex-1 text-sm">{f.name}</span>
                <span className={cn("rounded-full px-2 py-0.5 text-[10px]", DB_COLOR_STYLE.gray.bg, "text-muted-foreground")}>
                  {FIELD_KIND_LABEL[f.kind]}
                </span>
                <Button size="sm" variant="ghost" className="h-7 px-2 text-xs" onClick={() => setEditing(f)}>Editar</Button>
                <Button size="sm" variant="ghost" className="h-7 w-7 p-0 text-destructive" onClick={() => void remove(f)} title="Eliminar (los valores se conservan)">
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            ))}
            {order.length === 0 && (
              <p className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
                Sin campos. Crea el primero (un campo de texto hará de título).
              </p>
            )}
          </div>
          {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditing(null)}><Plus className="h-3.5 w-3.5" /> Nuevo campo</Button>
            <Button onClick={onClose}>Hecho</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {editing !== undefined && (
        <FieldDialog
          databaseId={databaseId}
          field={editing}
          onClose={() => setEditing(undefined)}
          onSaved={onChanged}
        />
      )}
    </>
  );
}
