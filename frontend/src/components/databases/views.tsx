import { useMemo, useState } from "react";
import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Image as ImageIcon, Plus, Trash2 } from "lucide-react";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import type { DbField, DbRecord, DbSort, DbView } from "./types";
import { CellDisplay, CellEditor } from "./cells";
import { OptionBadge, fieldValue, isEmptyValue, recordTitle, titleFieldOf, useAttachmentUrl } from "./shared";

export interface ViewProps {
  databaseId: Id;
  fields: DbField[];
  records: DbRecord[];
  view: DbView;
  sorts: DbSort[];
  onSort: (fieldId: Id) => void;
  onUpdateValue: (recordId: Id, fieldId: Id, value: unknown) => Promise<void>;
  onOpenRecord: (record: DbRecord) => void;
  onNewRecord: (preset?: Record<Id, unknown>) => void;
  onNavigate: (databaseId: Id, recordId: Id) => void;
  highlightId?: Id | null;
}

// ---------------------------------------------------------------------------
// Tabla: edición inline, orden por columna, selección múltiple
// ---------------------------------------------------------------------------

export function TableView(props: ViewProps & { onDeleteSelected: (ids: Id[]) => Promise<void> }) {
  const { fields, records, view, sorts, onSort, onUpdateValue, onOpenRecord, onNavigate, onDeleteSelected, highlightId } = props;
  const [editing, setEditing] = useState<{ recordId: Id; fieldId: Id } | null>(null);
  const [selected, setSelected] = useState<Set<Id>>(new Set());

  const visible = useMemo(() => {
    const ids = view.config.visible_field_ids ?? [];
    if (ids.length === 0) return fields;
    return ids.map((id) => fields.find((f) => f.id === id)).filter((f): f is DbField => !!f);
  }, [fields, view.config.visible_field_ids]);

  const toggleAll = () => {
    setSelected((prev) => (prev.size === records.length ? new Set() : new Set(records.map((r) => r.id))));
  };

  const sortOf = (fid: Id) => sorts.find((s) => s.field_id === fid);

  return (
    <div className="space-y-2">
      {selected.size > 0 && (
        <div className="flex items-center gap-3 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2">
          <span className="text-xs font-medium">{selected.size} seleccionado(s)</span>
          <Button
            size="sm"
            variant="destructive"
            className="h-7"
            onClick={async () => {
              await onDeleteSelected([...selected]);
              setSelected(new Set());
            }}
          >
            <Trash2 className="h-3.5 w-3.5" /> Eliminar
          </Button>
          <Button size="sm" variant="ghost" className="h-7" onClick={() => setSelected(new Set())}>Cancelar</Button>
        </div>
      )}
      <div className="overflow-x-auto rounded-xl border border-border bg-card shadow-sm">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left text-[11px] uppercase tracking-wide text-muted-foreground">
              <th className="w-8 px-3 py-2.5">
                <input
                  type="checkbox"
                  checked={records.length > 0 && selected.size === records.length}
                  onChange={toggleAll}
                  className="cursor-pointer accent-[var(--primary)]"
                />
              </th>
              {visible.map((f) => {
                const s = sortOf(f.id);
                return (
                  <th key={f.id} className="whitespace-nowrap px-3 py-2.5 font-medium">
                    <button
                      onClick={() => f.kind !== "relation" && f.kind !== "multiselect" && onSort(f.id)}
                      className={cn(
                        "inline-flex items-center gap-1",
                        f.kind !== "relation" && f.kind !== "multiselect" && "cursor-pointer hover:text-foreground",
                      )}
                    >
                      {f.name}
                      {s && (s.dir === "asc" ? <ArrowUp className="h-3 w-3" /> : <ArrowDown className="h-3 w-3" />)}
                    </button>
                  </th>
                );
              })}
              <th className="w-10 px-3 py-2.5" />
            </tr>
          </thead>
          <tbody>
            {records.map((r) => (
              <tr
                key={r.id}
                className={cn(
                  "border-b border-border/60 transition-colors last:border-0 hover:bg-muted/50",
                  highlightId === r.id && "bg-primary/10",
                )}
              >
                <td className="px-3 py-2">
                  <input
                    type="checkbox"
                    checked={selected.has(r.id)}
                    onChange={() =>
                      setSelected((prev) => {
                        const next = new Set(prev);
                        if (next.has(r.id)) next.delete(r.id);
                        else next.add(r.id);
                        return next;
                      })
                    }
                    className="cursor-pointer accent-[var(--primary)]"
                  />
                </td>
                {visible.map((f) => {
                  const isEditing = editing?.recordId === r.id && editing.fieldId === f.id;
                  const editable = f.kind !== "formula";
                  return (
                    <td
                      key={f.id}
                      className={cn("max-w-[280px] px-3 py-2 align-middle", editable && "cursor-pointer")}
                      onClick={() => {
                        if (editable && !isEditing) setEditing({ recordId: r.id, fieldId: f.id });
                      }}
                    >
                      {isEditing ? (
                        <div onClick={(e) => e.stopPropagation()}>
                          <CellEditor
                            field={f}
                            value={fieldValue(r, f)}
                            onSave={async (v) => {
                              setEditing(null);
                              await onUpdateValue(r.id, f.id, v);
                            }}
                            onCancel={() => setEditing(null)}
                          />
                        </div>
                      ) : (
                        <div className="truncate">
                          <CellDisplay field={f} record={r} onNavigate={onNavigate} />
                        </div>
                      )}
                    </td>
                  );
                })}
                <td className="px-3 py-2 text-right">
                  <Button size="sm" variant="ghost" className="h-6 px-2 text-xs" onClick={() => onOpenRecord(r)}>
                    Abrir
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {records.length === 0 && (
          <p className="px-4 py-8 text-center text-xs text-muted-foreground">Sin registros que coincidan.</p>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Kanban: drag & drop nativo HTML5 entre columnas del campo select
// ---------------------------------------------------------------------------

export function KanbanView(props: ViewProps) {
  const { fields, records, view, onUpdateValue, onOpenRecord, onNewRecord, onNavigate, highlightId } = props;
  const groupField = fields.find((f) => f.id === view.config.group_field_id && f.kind === "select");
  const [dragOver, setDragOver] = useState<string | null>(null);
  const tf = titleFieldOf(fields);

  if (!groupField) {
    return (
      <EmptyState
        icon={ImageIcon}
        title="Configura la agrupación"
        description="El kanban necesita un campo de tipo selección. Edita la vista y elige el campo de agrupación."
      />
    );
  }

  const options = groupField.config.options ?? [];
  const columns: { id: string | null; label: string; color: string }[] = [
    ...options.map((o) => ({ id: o.id as string | null, label: o.label, color: o.color })),
    { id: null, label: "Sin valor", color: "gray" },
  ];

  const byColumn = (optId: string | null) =>
    records.filter((r) => {
      const v = r.values[groupField.id];
      return optId === null ? isEmptyValue(v) : v === optId;
    });

  return (
    <div className="flex gap-3 overflow-x-auto pb-2">
      {columns.map((col) => {
        const items = byColumn(col.id);
        const colKey = col.id ?? "__none__";
        return (
          <div
            key={colKey}
            onDragOver={(e) => {
              e.preventDefault();
              setDragOver(colKey);
            }}
            onDragLeave={() => setDragOver((p) => (p === colKey ? null : p))}
            onDrop={async (e) => {
              e.preventDefault();
              setDragOver(null);
              const recordId = e.dataTransfer.getData("text/nexus-record");
              if (recordId) await onUpdateValue(recordId, groupField.id, col.id);
            }}
            className={cn(
              "flex w-64 shrink-0 flex-col rounded-xl border border-border bg-muted/40 p-2 transition-colors",
              dragOver === colKey && "border-primary bg-primary/10",
            )}
          >
            <div className="mb-2 flex items-center justify-between px-1">
              <OptionBadge label={col.label} color={col.color} />
              <span className="text-[11px] text-muted-foreground">{items.length}</span>
            </div>
            <div className="flex-1 space-y-2">
              {items.map((r) => (
                <div
                  key={r.id}
                  draggable
                  onDragStart={(e) => e.dataTransfer.setData("text/nexus-record", r.id)}
                  onClick={() => onOpenRecord(r)}
                  className={cn(
                    "cursor-grab rounded-lg border border-border bg-card p-2.5 shadow-sm transition-colors hover:bg-muted/60 active:cursor-grabbing",
                    highlightId === r.id && "ring-2 ring-primary",
                  )}
                >
                  <p className="truncate text-xs font-medium">{recordTitle(r, fields)}</p>
                  {fields
                    .filter((f) => f.id !== groupField.id && f.id !== tf?.id && ["rating", "date", "integer", "decimal"].includes(f.kind))
                    .slice(0, 2)
                    .map((f) => (
                      <div key={f.id} className="mt-1 text-[11px] text-muted-foreground">
                        <CellDisplay field={f} record={r} onNavigate={onNavigate} />
                      </div>
                    ))}
                </div>
              ))}
            </div>
            <Button
              size="sm"
              variant="ghost"
              className="mt-2 h-7 justify-start text-xs text-muted-foreground"
              onClick={() => onNewRecord(col.id ? { [groupField.id]: col.id } : undefined)}
            >
              <Plus className="h-3.5 w-3.5" /> Añadir
            </Button>
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Calendario: mes navegable
// ---------------------------------------------------------------------------

const MONTHS = ["Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio", "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre"];
const WEEKDAYS = ["L", "M", "X", "J", "V", "S", "D"];

export function CalendarView(props: ViewProps) {
  const { fields, records, view, onOpenRecord, highlightId } = props;
  const dateField = fields.find((f) => f.id === view.config.date_field_id);
  const now = new Date();
  const [month, setMonth] = useState({ year: now.getFullYear(), month: now.getMonth() });

  if (!dateField) {
    return (
      <EmptyState
        icon={ImageIcon}
        title="Configura el campo de fecha"
        description="El calendario necesita un campo date o datetime. Edita la vista y elígelo."
      />
    );
  }

  const first = new Date(month.year, month.month, 1);
  const startOffset = (first.getDay() + 6) % 7; // lunes primero
  const daysInMonth = new Date(month.year, month.month + 1, 0).getDate();
  const cells: (number | null)[] = [
    ...Array.from({ length: startOffset }, () => null),
    ...Array.from({ length: daysInMonth }, (_, i) => i + 1),
  ];
  while (cells.length % 7 !== 0) cells.push(null);

  const dateKey = (day: number) =>
    `${month.year}-${String(month.month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;

  const recordsOn = (day: number) =>
    records.filter((r) => {
      const v = r.values[dateField.id];
      return typeof v === "string" && v.slice(0, 10) === dateKey(day);
    });

  const shift = (delta: number) => {
    const d = new Date(month.year, month.month + delta, 1);
    setMonth({ year: d.getFullYear(), month: d.getMonth() });
  };

  const todayKey = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;

  return (
    <div className="rounded-xl border border-border bg-card p-3 shadow-sm">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold">{MONTHS[month.month]} {month.year}</h3>
        <div className="flex gap-1">
          <Button size="sm" variant="ghost" className="h-7 w-7 p-0" onClick={() => shift(-1)}><ChevronLeft className="h-4 w-4" /></Button>
          <Button size="sm" variant="ghost" className="h-7 text-xs" onClick={() => setMonth({ year: now.getFullYear(), month: now.getMonth() })}>Hoy</Button>
          <Button size="sm" variant="ghost" className="h-7 w-7 p-0" onClick={() => shift(1)}><ChevronRight className="h-4 w-4" /></Button>
        </div>
      </div>
      <div className="grid grid-cols-7 gap-px overflow-hidden rounded-lg border border-border bg-border">
        {WEEKDAYS.map((d) => (
          <div key={d} className="bg-muted px-2 py-1 text-center text-[10px] font-medium uppercase text-muted-foreground">{d}</div>
        ))}
        {cells.map((day, i) => (
          <div key={i} className={cn("min-h-[76px] bg-card p-1", day === null && "bg-muted/40")}>
            {day !== null && (
              <>
                <span
                  className={cn(
                    "mb-1 inline-flex h-5 w-5 items-center justify-center rounded-full text-[10px]",
                    dateKey(day) === todayKey ? "bg-primary text-primary-foreground font-semibold" : "text-muted-foreground",
                  )}
                >
                  {day}
                </span>
                <div className="space-y-0.5">
                  {recordsOn(day).slice(0, 3).map((r) => (
                    <button
                      key={r.id}
                      onClick={() => onOpenRecord(r)}
                      className={cn(
                        "block w-full truncate rounded bg-primary/15 px-1 py-0.5 text-left text-[10px] text-primary hover:bg-primary/25 cursor-pointer",
                        highlightId === r.id && "ring-1 ring-primary",
                      )}
                    >
                      {recordTitle(r, fields)}
                    </button>
                  ))}
                  {recordsOn(day).length > 3 && (
                    <span className="block px-1 text-[9px] text-muted-foreground">+{recordsOn(day).length - 3} más</span>
                  )}
                </div>
              </>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Galería: tarjetas con imagen
// ---------------------------------------------------------------------------

function GalleryCard({ record, fields, imageField, cardFields, onOpen, highlighted }: {
  record: DbRecord;
  fields: DbField[];
  imageField?: DbField;
  cardFields: DbField[];
  onOpen: () => void;
  highlighted: boolean;
}) {
  const fileId = imageField ? record.values[imageField.id] : undefined;
  const img = useAttachmentUrl(typeof fileId === "string" ? fileId : null);
  return (
    <button
      onClick={onOpen}
      className={cn(
        "overflow-hidden rounded-xl border border-border bg-card text-left shadow-sm transition-all hover:shadow-md cursor-pointer",
        highlighted && "ring-2 ring-primary",
      )}
    >
      <div className="aspect-[4/3] w-full overflow-hidden bg-muted">
        {img.data ? (
          <img src={img.data} alt={recordTitle(record, fields)} className="h-full w-full object-cover" loading="lazy" />
        ) : (
          <div className="flex h-full w-full items-center justify-center bg-gradient-to-br from-primary/10 via-muted to-warning/10">
            <ImageIcon className="h-8 w-8 text-muted-foreground/40" />
          </div>
        )}
      </div>
      <div className="p-3">
        <p className="truncate text-sm font-medium">{recordTitle(record, fields)}</p>
        {cardFields.slice(0, 2).map((f) => (
          <div key={f.id} className="mt-1 truncate text-xs text-muted-foreground">
            <CellDisplay field={f} record={record} />
          </div>
        ))}
      </div>
    </button>
  );
}

export function GalleryView(props: ViewProps) {
  const { fields, records, view, onOpenRecord, highlightId } = props;
  const imageField = fields.find((f) => f.id === view.config.image_field_id);
  const tf = titleFieldOf(fields);
  const cardFields = (view.config.card_field_ids ?? [])
    .map((id) => fields.find((f) => f.id === id))
    .filter((f): f is DbField => !!f && f.id !== tf?.id);
  const shown = cardFields.length > 0 ? cardFields : fields.filter((f) => f.id !== tf?.id && f.id !== imageField?.id).slice(0, 2);

  if (records.length === 0) {
    return <EmptyState icon={ImageIcon} title="Sin registros" description="Añade registros para verlos en la galería." />;
  }
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
      {records.map((r) => (
        <GalleryCard
          key={r.id}
          record={r}
          fields={fields}
          imageField={imageField}
          cardFields={shown}
          onOpen={() => onOpenRecord(r)}
          highlighted={highlightId === r.id}
        />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Lista compacta
// ---------------------------------------------------------------------------

export function ListView(props: ViewProps) {
  const { fields, records, onOpenRecord, onNavigate, highlightId } = props;
  const tf = titleFieldOf(fields);
  const secondary = fields.filter((f) => f.id !== tf?.id && ["select", "date", "rating", "checkbox"].includes(f.kind)).slice(0, 3);
  if (records.length === 0) {
    return <EmptyState icon={ImageIcon} title="Sin registros" description="Añade el primero con el botón «Nuevo registro»." />;
  }
  return (
    <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
      {records.map((r) => (
        <button
          key={r.id}
          onClick={() => onOpenRecord(r)}
          className={cn(
            "flex w-full items-center gap-3 border-b border-border/60 px-4 py-2 text-left transition-colors last:border-0 hover:bg-muted/50 cursor-pointer",
            highlightId === r.id && "bg-primary/10",
          )}
        >
          <span className="min-w-0 flex-1 truncate text-sm">{recordTitle(r, fields)}</span>
          {secondary.map((f) => (
            <span key={f.id} className="hidden shrink-0 sm:inline">
              <CellDisplay field={f} record={r} onNavigate={onNavigate} />
            </span>
          ))}
        </button>
      ))}
    </div>
  );
}
