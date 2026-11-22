import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { ExternalLink, File as FileIcon, Paperclip, Star, Upload, X } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { DatabaseSummary, DbField, DbRecord, RelationChip } from "./types";
import {
  OptionBadge,
  fieldValue,
  isEmptyValue,
  fetchAttachmentUrl,
  toastDbError,
  uploadAttachment,
  useAttachmentUrl,
} from "./shared";

// ---------------------------------------------------------------------------
// Visualización de celdas (solo lectura)
// ---------------------------------------------------------------------------

export function RelationChips({
  field,
  record,
  onNavigate,
}: {
  field: DbField;
  record: DbRecord;
  onNavigate?: (databaseId: Id, recordId: Id) => void;
}) {
  const chips: RelationChip[] = record.relation_titles[field.id] ?? [];
  const target = field.config.target_database_id;
  if (chips.length === 0) return <span className="text-muted-foreground/50">—</span>;
  return (
    <span className="flex flex-wrap gap-1">
      {chips.map((c) => (
        <button
          key={c.id}
          onClick={(e) => {
            e.stopPropagation();
            if (target && onNavigate) onNavigate(target, c.id);
          }}
          className="inline-flex max-w-[180px] items-center gap-1 rounded-full bg-primary/15 px-2 py-0.5 text-[11px] font-medium text-primary transition-colors hover:bg-primary/25 cursor-pointer"
          title="Abrir registro relacionado"
        >
          <span className="truncate">{c.title}</span>
          <ExternalLink className="h-2.5 w-2.5 shrink-0" />
        </button>
      ))}
    </span>
  );
}

function ImageThumb({ fileId }: { fileId: Id }) {
  const img = useAttachmentUrl(fileId);
  if (img.isLoading || img.isError || !img.data) {
    return <div className="h-8 w-8 rounded bg-muted" />;
  }
  return <img src={img.data} alt="adjunto" className="h-8 w-8 rounded object-cover" loading="lazy" />;
}

export function FileLink({ fileId }: { fileId: Id }) {
  const open = async () => {
    try {
      const url = await fetchAttachmentUrl(fileId);
      const a = document.createElement("a");
      a.href = url;
      a.download = fileId;
      a.click();
    } catch (e) {
      toastDbError(e, "No se pudo abrir el archivo");
    }
  };
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        void open();
      }}
      className="inline-flex items-center gap-1 text-xs text-primary hover:underline cursor-pointer"
    >
      <Paperclip className="h-3 w-3" /> Adjunto
    </button>
  );
}

export function CellDisplay({
  field,
  record,
  onNavigate,
}: {
  field: DbField;
  record: DbRecord;
  onNavigate?: (databaseId: Id, recordId: Id) => void;
}) {
  const v = fieldValue(record, field);
  if (field.kind === "formula" && record.formula_errors[field.id]) {
    return (
      <span className="text-xs text-destructive" title={record.formula_errors[field.id]}>
        Error de fórmula
      </span>
    );
  }
  if (field.kind === "checkbox") {
    return (
      <span className={cn("inline-block h-4 w-4 rounded border", v ? "border-primary bg-primary/80" : "border-border bg-card")} />
    );
  }
  if (isEmptyValue(v)) return <span className="text-muted-foreground/50">—</span>;
  switch (field.kind) {
    case "select": {
      const opt = (field.config.options ?? []).find((o) => o.id === v);
      return opt ? <OptionBadge label={opt.label} color={opt.color} /> : <span>{String(v)}</span>;
    }
    case "multiselect":
      return (
        <span className="flex flex-wrap gap-1">
          {(v as string[]).map((id) => {
            const opt = (field.config.options ?? []).find((o) => o.id === id);
            return <OptionBadge key={id} label={opt?.label ?? id} color={opt?.color ?? "gray"} />;
          })}
        </span>
      );
    case "relation":
      return <RelationChips field={field} record={record} onNavigate={onNavigate} />;
    case "url":
      return (
        <a
          href={String(v)}
          target="_blank"
          rel="noreferrer"
          onClick={(e) => e.stopPropagation()}
          className="text-xs text-primary hover:underline"
        >
          {String(v).replace(/^https?:\/\//, "").slice(0, 40)}
        </a>
      );
    case "rating": {
      const max = field.config.max_rating ?? 5;
      const n = Number(v);
      return (
        <span className="flex items-center gap-0.5">
          {Array.from({ length: max }).map((_, i) => (
            <Star key={i} className={cn("h-3 w-3", i < n ? "fill-warning text-warning" : "text-muted-foreground/30")} />
          ))}
        </span>
      );
    }
    case "image":
      return <ImageThumb fileId={String(v)} />;
    case "file":
      return <FileLink fileId={String(v)} />;
    case "date":
    case "datetime":
      return <span className="whitespace-nowrap text-xs">{String(v).replace("T", " ").slice(0, field.kind === "date" ? 10 : 16)}</span>;
    default:
      return <span className="text-xs">{String(v)}</span>;
  }
}

// ---------------------------------------------------------------------------
// Editores por tipo
// ---------------------------------------------------------------------------

interface EditorProps {
  field: DbField;
  value: unknown;
  onSave: (value: unknown) => Promise<void> | void;
  onCancel: () => void;
  autoFocus?: boolean;
}

function RelationEditor({ field, value, onSave, onCancel }: EditorProps) {
  const target = field.config.target_database_id;
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<string[]>(Array.isArray(value) ? (value as string[]) : []);
  const single = field.config.cardinality === "one_to_one" || field.config.cardinality === "one_to_many";
  const results = useQuery({
    queryKey: ["databases", "relation-search", target, search],
    queryFn: () =>
      invoke<{ records: DbRecord[] }>("databases.records.query", {
        database_id: target,
        query: { search: search || null, limit: 20 },
      }),
    enabled: !!target,
  });
  const targetDb = useQuery({
    queryKey: ["databases", "detail", target],
    queryFn: () => invoke<{ fields: DbField[]; name: string }>("databases.get", { id: target }),
    enabled: !!target,
  });

  const titleOf = (r: DbRecord): string => {
    const tf = (targetDb.data?.fields ?? []).find((f) => f.kind === "text");
    const v = tf ? r.values[tf.id] : undefined;
    if (typeof v === "string" && v) return v;
    const first = Object.values(r.values).find((x) => typeof x === "string" && x);
    return (first as string) ?? "(sin título)";
  };

  const toggle = (id: Id) => {
    setSelected((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      return single ? [id] : [...prev, id];
    });
  };

  return (
    <div className="w-72 space-y-2 rounded-lg border border-border bg-popover p-3 shadow-lg">
      <Input
        autoFocus
        placeholder={`Buscar en ${targetDb.data?.name ?? "base destino"}…`}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="h-8 text-xs"
      />
      <div className="max-h-48 space-y-0.5 overflow-y-auto">
        {(results.data?.records ?? []).map((r) => (
          <button
            key={r.id}
            onClick={() => toggle(r.id)}
            className={cn(
              "flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-xs transition-colors cursor-pointer",
              selected.includes(r.id) ? "bg-primary/15 text-primary" : "hover:bg-muted",
            )}
          >
            <span className="truncate">{titleOf(r)}</span>
            {selected.includes(r.id) && <span>✓</span>}
          </button>
        ))}
        {results.isSuccess && (results.data?.records ?? []).length === 0 && (
          <p className="px-2 py-2 text-xs text-muted-foreground">Sin resultados.</p>
        )}
      </div>
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>Cancelar</Button>
        <Button size="sm" onClick={() => void onSave(selected)}>Guardar</Button>
      </div>
    </div>
  );
}

function AttachmentEditor({ field, value, onSave, onCancel }: EditorProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const current = typeof value === "string" && value ? value : null;

  const pick = async (file: File) => {
    setBusy(true);
    try {
      const meta = await uploadAttachment(file);
      await onSave(meta.id);
    } catch (e) {
      toastDbError(e, "No se pudo subir el archivo");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="w-60 space-y-2 rounded-lg border border-border bg-popover p-3 shadow-lg">
      <input
        ref={inputRef}
        type="file"
        accept={field.kind === "image" ? "image/*" : undefined}
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void pick(f);
        }}
      />
      {current && (
        <div className="flex items-center justify-between text-xs">
          <span className="flex items-center gap-1 text-muted-foreground"><FileIcon className="h-3 w-3" /> Adjunto actual</span>
          <Button size="sm" variant="ghost" className="h-6 px-2 text-destructive" onClick={() => void onSave(null)}>
            <X className="h-3 w-3" /> Quitar
          </Button>
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>Cancelar</Button>
        <Button size="sm" disabled={busy} onClick={() => inputRef.current?.click()}>
          <Upload className="h-3.5 w-3.5" /> {busy ? "Subiendo…" : "Subir"}
        </Button>
      </div>
      <p className="text-[10px] text-muted-foreground">Máximo 25 MB.</p>
    </div>
  );
}

function MultiselectEditor({ field, value, onSave, onCancel }: EditorProps) {
  const [selected, setSelected] = useState<string[]>(Array.isArray(value) ? (value as string[]) : []);
  const options = field.config.options ?? [];
  return (
    <div className="w-56 space-y-2 rounded-lg border border-border bg-popover p-3 shadow-lg">
      <div className="max-h-48 space-y-0.5 overflow-y-auto">
        {options.map((o) => (
          <button
            key={o.id}
            onClick={() =>
              setSelected((prev) => (prev.includes(o.id) ? prev.filter((x) => x !== o.id) : [...prev, o.id]))
            }
            className={cn(
              "flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-xs cursor-pointer",
              selected.includes(o.id) ? "bg-primary/15" : "hover:bg-muted",
            )}
          >
            <OptionBadge label={o.label} color={o.color} />
            {selected.includes(o.id) && <span className="text-primary">✓</span>}
          </button>
        ))}
      </div>
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>Cancelar</Button>
        <Button size="sm" onClick={() => void onSave(selected)}>Guardar</Button>
      </div>
    </div>
  );
}

function RatingEditor({ field, value, onSave, onCancel }: EditorProps) {
  const max = field.config.max_rating ?? 5;
  const current = typeof value === "number" ? value : 0;
  return (
    <div className="flex items-center gap-1 rounded-lg border border-border bg-popover p-2 shadow-lg">
      {Array.from({ length: max }).map((_, i) => (
        <button key={i} onClick={() => void onSave(i + 1 === current ? null : i + 1)} className="cursor-pointer">
          <Star className={cn("h-4 w-4", i < current ? "fill-warning text-warning" : "text-muted-foreground/40 hover:text-warning")} />
        </button>
      ))}
      <Button size="sm" variant="ghost" className="h-6 px-1" onClick={onCancel}><X className="h-3 w-3" /></Button>
    </div>
  );
}

/** Editor inline genérico: elige el control según el tipo de campo. */
export function CellEditor(props: EditorProps) {
  const { field, value, onSave, onCancel, autoFocus = true } = props;
  const [text, setText] = useState(value === null || value === undefined ? "" : String(value));

  useEffect(() => {
    setText(value === null || value === undefined ? "" : String(value));
  }, [value]);

  const commitText = () => {
    const t = text.trim();
    void onSave(t === "" ? null : field.kind === "integer" || field.kind === "decimal" || field.kind === "rating" ? Number(t.replace(",", ".")) : t);
  };

  switch (field.kind) {
    case "checkbox":
      return (
        <button
          autoFocus={autoFocus}
          onClick={() => void onSave(!value)}
          onBlur={onCancel}
          className={cn("inline-block h-4 w-4 rounded border cursor-pointer", value ? "border-primary bg-primary/80" : "border-border bg-card")}
        />
      );
    case "select":
      return (
        <Select
          open
          value={typeof value === "string" ? value : undefined}
          onValueChange={(v) => void onSave(v === "__none__" ? null : v)}
          onOpenChange={(open) => {
            if (!open) onCancel();
          }}
        >
          <SelectTrigger className="h-8 w-44 text-xs"><SelectValue placeholder="Elegir…" /></SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__">(vacío)</SelectItem>
            {(field.config.options ?? []).map((o) => (
              <SelectItem key={o.id} value={o.id}>
                <OptionBadge label={o.label} color={o.color} />
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      );
    case "multiselect":
      return <MultiselectEditor {...props} />;
    case "relation":
      return <RelationEditor {...props} />;
    case "rating":
      return <RatingEditor {...props} />;
    case "image":
    case "file":
      return <AttachmentEditor {...props} />;
    case "long_text":
      return (
        <Textarea
          autoFocus={autoFocus}
          value={text}
          rows={4}
          onChange={(e) => setText(e.target.value)}
          onBlur={commitText}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) commitText();
          }}
          className="w-72 text-xs"
        />
      );
    case "date":
    case "datetime":
      return (
        <Input
          autoFocus={autoFocus}
          type={field.kind === "date" ? "date" : "datetime-local"}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onBlur={commitText}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") commitText();
          }}
          className="h-8 w-44 text-xs"
        />
      );
    case "integer":
    case "decimal":
      return (
        <Input
          autoFocus={autoFocus}
          type="number"
          step={field.kind === "integer" ? 1 : "any"}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onBlur={commitText}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") commitText();
          }}
          className="h-8 w-28 text-xs"
        />
      );
    default:
      return (
        <Input
          autoFocus={autoFocus}
          type={field.kind === "email" ? "email" : field.kind === "url" ? "url" : "text"}
          value={text}
          placeholder={field.kind === "url" ? "https://…" : field.kind === "email" ? "nombre@dominio.com" : undefined}
          onChange={(e) => setText(e.target.value)}
          onBlur={commitText}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") commitText();
          }}
          className="h-8 w-56 text-xs"
        />
      );
  }
}

export { ApiError };
