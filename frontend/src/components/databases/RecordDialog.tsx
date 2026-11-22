import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import type { Id } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { DbField, DbRecord } from "./types";
import { CellEditor } from "./cells";
import { FIELD_KIND_LABEL, fieldValue, recordTitle } from "./shared";

/** Editor completo de un registro: todos los campos con su control por tipo. */
export function RecordDialog({
  databaseId,
  fields,
  record,
  preset,
  onClose,
  onSave,
  onDelete,
}: {
  databaseId: Id;
  fields: DbField[];
  record: DbRecord | null;
  preset?: Record<Id, unknown>;
  onClose: () => void;
  onSave: (values: Record<Id, unknown>) => Promise<void>;
  onDelete?: () => Promise<void>;
}) {
  const [values, setValues] = useState<Record<Id, unknown>>(record?.values ?? preset ?? {});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingField, setEditingField] = useState<Id | null>(null);

  useEffect(() => {
    setValues(record?.values ?? preset ?? {});
    setError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [record?.id]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSave(values);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Error desconocido");
    } finally {
      setBusy(false);
    }
  };

  const editable = fields.filter((f) => f.kind !== "formula");
  const formulas = fields.filter((f) => f.kind === "formula");

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{record ? recordTitle(record, fields) : "Nuevo registro"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          {editable.length === 0 && (
            <p className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
              Esta base no tiene campos todavía. Crea campos con el botón «Campos».
            </p>
          )}
          {editable.map((f) => (
            <div key={f.id} className="grid grid-cols-[140px_1fr] items-start gap-3">
              <div className="pt-1.5">
                <p className="text-xs font-medium">{f.name}</p>
                <p className="text-[10px] text-muted-foreground">{FIELD_KIND_LABEL[f.kind]}</p>
              </div>
              <div>
                {editingField === f.id || f.kind === "checkbox" || f.kind === "rating" ? (
                  <CellEditor
                    field={f}
                    value={values[f.id] ?? null}
                    autoFocus={editingField === f.id}
                    onSave={(v) => {
                      setValues((prev) => ({ ...prev, [f.id]: v }));
                      setEditingField(null);
                    }}
                    onCancel={() => setEditingField(null)}
                  />
                ) : (
                  <button
                    onClick={() => setEditingField(f.id)}
                    className="flex min-h-9 w-full items-center rounded-md border border-input bg-card px-3 py-1.5 text-left text-xs shadow-sm transition-colors hover:bg-muted/50 cursor-pointer"
                  >
                    <PreviewValue field={f} value={values[f.id]} record={record} />
                  </button>
                )}
              </div>
            </div>
          ))}
          {record && formulas.length > 0 && (
            <div className="space-y-1 rounded-lg bg-muted/50 p-3">
              <p className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">Campos calculados</p>
              {formulas.map((f) => (
                <div key={f.id} className="flex items-center justify-between text-xs">
                  <span>{f.name}</span>
                  {record.formula_errors[f.id] ? (
                    <span className="text-destructive" title={record.formula_errors[f.id]}>Error de fórmula</span>
                  ) : (
                    <span className="font-medium">{String(fieldValue(record, f) ?? "—")}</span>
                  )}
                </div>
              ))}
              <p className="text-[10px] text-muted-foreground">Se recalculan al guardar.</p>
            </div>
          )}
          {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
        </div>
        <DialogFooter className="sm:justify-between">
          {record && onDelete ? (
            <Button
              variant="ghost"
              className="text-destructive"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await onDelete();
                  onClose();
                } finally {
                  setBusy(false);
                }
              }}
            >
              <Trash2 className="h-3.5 w-3.5" /> Eliminar
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button variant="ghost" onClick={onClose}>Cancelar</Button>
            <Button onClick={() => void save()} disabled={busy}>
              {busy ? "Guardando…" : record ? "Guardar cambios" : "Crear registro"}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PreviewValue({ field, value, record }: { field: DbField; value: unknown; record: DbRecord | null }) {
  if (value === null || value === undefined || value === "" || (Array.isArray(value) && value.length === 0)) {
    return <span className="text-muted-foreground/50">Vacío — click para editar</span>;
  }
  if (field.kind === "select") {
    const opt = (field.config.options ?? []).find((o) => o.id === value);
    return <span>{opt?.label ?? String(value)}</span>;
  }
  if (field.kind === "multiselect") {
    return (
      <span>
        {(value as string[]).map((id) => (field.config.options ?? []).find((o) => o.id === id)?.label ?? id).join(", ")}
      </span>
    );
  }
  if (field.kind === "relation") {
    const chips = record?.relation_titles[field.id] ?? [];
    if (chips.length > 0) return <span>{chips.map((c) => c.title).join(", ")}</span>;
    return <span>{(value as string[]).length} registro(s) relacionado(s)</span>;
  }
  if (field.kind === "image" || field.kind === "file") {
    return <span>Adjunto guardado — click para cambiar</span>;
  }
  return <span className="truncate">{String(value)}</span>;
}
