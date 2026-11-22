import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { Download, FileUp, Sparkles, Upload } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type {
  CreateDbField,
  DbField,
  DbFieldKind,
  DbView,
  ExportResult,
  ImportMapping,
  ImportPreview,
  ImportResult,
} from "./types";
import { FIELD_KIND_LABEL, downloadText, toastDbError } from "./shared";

const MAX_IMPORT_BYTES = 5 * 1024 * 1024;

const IMPORTABLE_KINDS: DbFieldKind[] = [
  "text", "long_text", "integer", "decimal", "date", "datetime", "checkbox",
  "select", "multiselect", "url", "email", "phone", "rating",
];

interface ColumnMap {
  enabled: boolean;
  target: string; // field id | "__new__" | "__skip__"
  newKind: DbFieldKind;
  optionsText: string;
}

export function ImportDialog({
  databaseId,
  fields,
  onClose,
  onImported,
}: {
  databaseId: Id;
  fields: DbField[];
  onClose: () => void;
  onImported: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [format, setFormat] = useState<"csv" | "json">("csv");
  const [content, setContent] = useState<string | null>(null);
  const [filename, setFilename] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [maps, setMaps] = useState<Record<string, ColumnMap>>({});
  const [skipInvalid, setSkipInvalid] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pickFile = async (file: File) => {
    setError(null);
    if (file.size > MAX_IMPORT_BYTES) {
      setError("El archivo supera el máximo de 5 MB.");
      return;
    }
    const fmt = file.name.toLowerCase().endsWith(".json") ? "json" : "csv";
    const text = await file.text();
    setBusy(true);
    try {
      const p = await invoke<ImportPreview>("databases.import_preview", { format: fmt, content: text });
      setFormat(fmt);
      setContent(text);
      setFilename(file.name);
      setPreview(p);
      const initial: Record<string, ColumnMap> = {};
      for (const col of p.columns) {
        const match = fields.find((f) => f.name.toLowerCase() === col.toLowerCase());
        initial[col] = {
          enabled: true,
          target: match?.id ?? "__new__",
          newKind: match?.kind ?? guessKind(col, p),
          optionsText: "",
        };
      }
      setMaps(initial);
    } catch (e) {
      setError(e instanceof Error ? e.message : "No se pudo leer el archivo");
    } finally {
      setBusy(false);
    }
  };

  const doImport = async () => {
    if (!preview || !content) return;
    setBusy(true);
    setError(null);
    try {
      const mappings: ImportMapping[] = [];
      for (const col of preview.columns) {
        const m = maps[col];
        if (!m || !m.enabled || m.target === "__skip__") continue;
        if (m.target === "__new__") {
          const newField: CreateDbField = { name: col, kind: m.newKind };
          if (m.newKind === "select" || m.newKind === "multiselect") {
            const labels = [...new Set(m.optionsText.split(/[,;\n]/).map((s) => s.trim()).filter(Boolean))];
            if (labels.length === 0) {
              setError(`El campo nuevo «${col}» necesita opciones (separadas por comas).`);
              setBusy(false);
              return;
            }
            newField.config = {
              options: labels.map((label, i) => ({
                id: `opt-${i + 1}`,
                label,
                color: ["gray", "blue", "green", "yellow", "orange", "red", "purple", "pink"][i % 8],
              })),
            };
          }
          mappings.push({ column: col, new_field: newField });
        } else {
          mappings.push({ column: col, field_id: m.target });
        }
      }
      if (mappings.length === 0) {
        setError("Mapea al menos una columna.");
        setBusy(false);
        return;
      }
      const result = await invoke<ImportResult>("databases.import", {
        data: { database_id: databaseId, format, content, mappings, skip_invalid: skipInvalid },
      });
      toast.success(`${result.imported} registro(s) importados`, {
        description: result.skipped > 0 ? `${result.skipped} fila(s) omitidas por errores.` : undefined,
      });
      onImported();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "No se pudo importar");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Importar registros (CSV o JSON)</DialogTitle>
        </DialogHeader>
        {!preview ? (
          <div className="space-y-3">
            <input
              ref={inputRef}
              type="file"
              accept=".csv,.json,text/csv,application/json"
              className="hidden"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void pickFile(f);
              }}
            />
            <button
              onClick={() => inputRef.current?.click()}
              className="flex w-full flex-col items-center gap-2 rounded-xl border border-dashed border-border px-6 py-10 text-center transition-colors hover:bg-muted/50 cursor-pointer"
            >
              <FileUp className="h-8 w-8 text-muted-foreground" />
              <span className="text-sm font-medium">Elegir archivo</span>
              <span className="text-xs text-muted-foreground">CSV con cabeceras o JSON (array de objetos) · máx. 5 MB / 10 000 filas</span>
            </button>
            {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
          </div>
        ) : (
          <div className="space-y-4">
            <p className="text-xs text-muted-foreground">
              <span className="font-medium text-foreground">{filename}</span> · {preview.total_rows} filas · formato {preview.format.toUpperCase()}
            </p>
            <div className="space-y-2">
              <Label>Mapeo de columnas</Label>
              {preview.columns.map((col) => {
                const m = maps[col];
                if (!m) return null;
                return (
                  <div key={col} className="space-y-1.5 rounded-lg border border-border p-2.5">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="w-32 truncate text-xs font-medium" title={col}>{col}</span>
                      <Select value={m.target} onValueChange={(v) => setMaps((p) => ({ ...p, [col]: { ...m, target: v } }))}>
                        <SelectTrigger className="h-8 w-48 text-xs"><SelectValue /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="__skip__">(ignorar columna)</SelectItem>
                          <SelectItem value="__new__">Crear campo nuevo…</SelectItem>
                          {fields.filter((f) => !["formula", "relation", "image", "file"].includes(f.kind)).map((f) => (
                            <SelectItem key={f.id} value={f.id}>{f.name} · {FIELD_KIND_LABEL[f.kind]}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      {m.target === "__new__" && (
                        <Select value={m.newKind} onValueChange={(v) => setMaps((p) => ({ ...p, [col]: { ...m, newKind: v as DbFieldKind } }))}>
                          <SelectTrigger className="h-8 w-36 text-xs"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            {IMPORTABLE_KINDS.map((k) => (
                              <SelectItem key={k} value={k}>{FIELD_KIND_LABEL[k]}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      )}
                    </div>
                    {m.target === "__new__" && (m.newKind === "select" || m.newKind === "multiselect") && (
                      <Input
                        value={m.optionsText}
                        onChange={(e) => setMaps((p) => ({ ...p, [col]: { ...m, optionsText: e.target.value } }))}
                        placeholder="Opciones separadas por comas: Activo, Archivado…"
                        className="h-8 text-xs"
                      />
                    )}
                    <p className="truncate text-[10px] text-muted-foreground">
                      Ejemplo: {preview.rows.slice(0, 3).map((r) => r[preview.columns.indexOf(col)]).filter(Boolean).join(" · ") || "(vacío)"}
                    </p>
                  </div>
                );
              })}
            </div>
            <div className="flex items-center gap-2">
              <Switch checked={skipInvalid} onCheckedChange={setSkipInvalid} id="skip-invalid" />
              <Label htmlFor="skip-invalid" className="text-xs">Omitir filas con errores (si se desactiva, cualquier fila inválida aborta la importación)</Label>
            </div>
            {error && <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">{error}</p>}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          {preview && (
            <Button onClick={() => void doImport()} disabled={busy}>
              <Upload className="h-3.5 w-3.5" /> {busy ? "Importando…" : `Importar ${preview.total_rows} filas`}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function guessKind(col: string, preview: ImportPreview): DbFieldKind {
  const idx = preview.columns.indexOf(col);
  const samples = preview.rows.map((r) => r[idx]).filter((s) => s && s.trim());
  if (samples.length === 0) return "text";
  if (samples.every((s) => /^\d+$/.test(s.trim()))) return "integer";
  if (samples.every((s) => /^\d+[.,]?\d*$/.test(s.trim()))) return "decimal";
  if (samples.every((s) => /^\d{4}-\d{2}-\d{2}$/.test(s.trim()))) return "date";
  if (samples.every((s) => /^(true|false|sí|si|no|0|1)$/i.test(s.trim()))) return "checkbox";
  const lower = col.toLowerCase();
  if (lower.includes("email") || lower.includes("correo")) return "email";
  if (lower.includes("url") || lower.includes("enlace") || lower.includes("web")) return "url";
  return "text";
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

export function ExportDialog({
  databaseId,
  views,
  onClose,
}: {
  databaseId: Id;
  views: DbView[];
  onClose: () => void;
}) {
  const [format, setFormat] = useState<"csv" | "json">("csv");
  const [viewId, setViewId] = useState<string>("__all__");
  const [busy, setBusy] = useState(false);

  const doExport = async () => {
    setBusy(true);
    try {
      const result = await invoke<ExportResult>("databases.export", {
        database_id: databaseId,
        format,
        view_id: viewId === "__all__" ? null : viewId,
      });
      downloadText(result.filename, result.mime, result.content);
      toast.success(`Exportados ${result.row_count} registro(s)`, { description: result.filename });
      onClose();
    } catch (e) {
      toastDbError(e, "No se pudo exportar");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Exportar base de datos</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Formato</Label>
            <div className="flex gap-2">
              {(["csv", "json"] as const).map((f) => (
                <button
                  key={f}
                  onClick={() => setFormat(f)}
                  className={cn(
                    "flex-1 rounded-lg border px-3 py-2 text-sm font-medium uppercase cursor-pointer",
                    format === f ? "border-primary bg-primary/15 text-primary" : "border-border hover:bg-muted",
                  )}
                >
                  {f}
                </button>
              ))}
            </div>
          </div>
          <div className="space-y-1.5">
            <Label>Datos</Label>
            <Select value={viewId} onValueChange={setViewId}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="__all__">Todos los registros</SelectItem>
                {views.map((v) => (
                  <SelectItem key={v.id} value={v.id}>Vista «{v.name}» (con sus filtros y orden)</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <p className="text-[11px] text-muted-foreground">
            Las selecciones se exportan con sus etiquetas y las relaciones con los títulos de los registros destino.
          </p>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => void doExport()} disabled={busy}>
            <Download className="h-3.5 w-3.5" /> {busy ? "Exportando…" : "Descargar"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// IA: propuestas revisables (crear estructura / sugerir campos / duplicados)
// ---------------------------------------------------------------------------

import type { AiFieldProposal, AiStructureProposal, DuplicatePair } from "./types";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";

export function AiStructureDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (id: Id) => void }) {
  const [description, setDescription] = useState("");
  const [proposal, setProposal] = useState<AiStructureProposal | null>(null);
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);

  const propose = async () => {
    setBusy(true);
    try {
      const p = await invoke<AiStructureProposal>("databases.ai_structure", { description });
      setProposal(p);
      setChecked(new Set(p.fields.map((_, i) => i)));
    } catch (e) {
      toastDbError(e, "No se pudo generar la propuesta");
    } finally {
      setBusy(false);
    }
  };

  const confirm = async () => {
    if (!proposal) return;
    setBusy(true);
    try {
      const db = await invoke<{ id: Id }>("databases.create", {
        data: { name: proposal.name, description: proposal.description, icon: proposal.icon, color: proposal.color },
      });
      for (const [i, f] of proposal.fields.entries()) {
        if (!checked.has(i)) continue;
        await invoke("databases.fields.create", {
          database_id: db.id,
          data: { name: f.name, kind: f.kind, config: f.config },
        });
      }
      toast.success(`Base «${proposal.name}» creada`, { description: "Revisa y ajusta los campos cuando quieras." });
      onCreated(db.id);
      onClose();
    } catch (e) {
      toastDbError(e, "No se pudo crear la base propuesta");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><Sparkles className="h-4 w-4 text-primary" /> Crear base con IA</DialogTitle>
        </DialogHeader>
        {!proposal ? (
          <div className="space-y-3">
            <Textarea
              autoFocus
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe qué quieres organizar. p. ej. «Quiero llevar un registro de mis plantas: especie, riego, ubicación y fotos»."
              rows={4}
            />
            <p className="text-[11px] text-muted-foreground">La IA solo propone: nada se crea hasta que confirmes.</p>
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-sm font-medium">{proposal.name} <span className="text-xs font-normal text-muted-foreground">· {proposal.description}</span></p>
            <div className="space-y-1.5">
              {proposal.fields.map((f, i) => (
                <label key={i} className="flex cursor-pointer items-start gap-2 rounded-lg border border-border px-3 py-2">
                  <input
                    type="checkbox"
                    checked={checked.has(i)}
                    onChange={() =>
                      setChecked((prev) => {
                        const next = new Set(prev);
                        if (next.has(i)) next.delete(i);
                        else next.add(i);
                        return next;
                      })
                    }
                    className="mt-0.5 accent-[var(--primary)]"
                  />
                  <span className="flex-1">
                    <span className="text-xs font-medium">{f.name}</span>
                    <Badge variant="secondary" className="ml-2">{FIELD_KIND_LABEL[f.kind]}</Badge>
                    {f.reason && <span className="block text-[10px] text-muted-foreground">{f.reason}</span>}
                    {(f.config.options ?? []).length > 0 && (
                      <span className="block text-[10px] text-muted-foreground">Opciones: {(f.config.options ?? []).map((o) => o.label).join(", ")}</span>
                    )}
                  </span>
                </label>
              ))}
            </div>
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          {!proposal ? (
            <Button onClick={() => void propose()} disabled={busy || !description.trim()}>
              <Sparkles className="h-3.5 w-3.5" /> {busy ? "Pensando…" : "Proponer estructura"}
            </Button>
          ) : (
            <>
              <Button variant="outline" onClick={() => setProposal(null)}>Volver</Button>
              <Button onClick={() => void confirm()} disabled={busy || checked.size === 0}>
                {busy ? "Creando…" : `Crear base con ${checked.size} campos`}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function AiSuggestDialog({
  databaseId,
  onClose,
  onApplied,
}: {
  databaseId: Id;
  onClose: () => void;
  onApplied: () => void;
}) {
  const [proposals, setProposals] = useState<AiFieldProposal[] | null>(null);
  const [checked, setChecked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);

  const propose = async () => {
    setBusy(true);
    try {
      const p = await invoke<AiFieldProposal[]>("databases.ai_suggest_fields", { database_id: databaseId });
      setProposals(p);
      setChecked(new Set(p.map((_, i) => i)));
    } catch (e) {
      toastDbError(e, "No se pudieron sugerir campos");
    } finally {
      setBusy(false);
    }
  };

  const apply = async () => {
    if (!proposals) return;
    setBusy(true);
    try {
      let n = 0;
      for (const [i, f] of proposals.entries()) {
        if (!checked.has(i)) continue;
        await invoke("databases.fields.create", { database_id: databaseId, data: { name: f.name, kind: f.kind, config: f.config } });
        n++;
      }
      toast.success(`${n} campo(s) añadidos`);
      onApplied();
      onClose();
    } catch (e) {
      toastDbError(e, "No se pudieron crear los campos");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><Sparkles className="h-4 w-4 text-primary" /> Sugerir campos con IA</DialogTitle>
        </DialogHeader>
        {!proposals ? (
          <p className="text-xs text-muted-foreground">
            La IA analizará la estructura actual y propondrá campos complementarios. Nada se crea sin tu confirmación.
          </p>
        ) : (
          <div className="space-y-1.5">
            {proposals.map((f, i) => (
              <label key={i} className="flex cursor-pointer items-start gap-2 rounded-lg border border-border px-3 py-2">
                <input
                  type="checkbox"
                  checked={checked.has(i)}
                  onChange={() =>
                    setChecked((prev) => {
                      const next = new Set(prev);
                      if (next.has(i)) next.delete(i);
                      else next.add(i);
                      return next;
                    })
                  }
                  className="mt-0.5 accent-[var(--primary)]"
                />
                <span className="flex-1">
                  <span className="text-xs font-medium">{f.name}</span>
                  <Badge variant="secondary" className="ml-2">{FIELD_KIND_LABEL[f.kind]}</Badge>
                  {f.reason && <span className="block text-[10px] text-muted-foreground">{f.reason}</span>}
                </span>
              </label>
            ))}
            {proposals.length === 0 && <p className="text-xs text-muted-foreground">La IA no propuso campos nuevos.</p>}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Cancelar</Button>
          {!proposals ? (
            <Button onClick={() => void propose()} disabled={busy}>
              <Sparkles className="h-3.5 w-3.5" /> {busy ? "Analizando…" : "Sugerir campos"}
            </Button>
          ) : (
            <Button onClick={() => void apply()} disabled={busy || checked.size === 0}>
              {busy ? "Creando…" : `Añadir ${checked.size} campos`}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function AiDuplicatesDialog({
  databaseId,
  onClose,
  onDelete,
}: {
  databaseId: Id;
  onClose: () => void;
  onDelete: (id: Id) => Promise<void>;
}) {
  const pairs = useQuery({
    queryKey: ["databases", "duplicates", databaseId],
    queryFn: () => invoke<DuplicatePair[]>("databases.ai_duplicates", { database_id: databaseId }),
    retry: false,
  });
  const [removed, setRemoved] = useState<Set<Id>>(new Set());

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><Sparkles className="h-4 w-4 text-primary" /> Posibles duplicados</DialogTitle>
        </DialogHeader>
        {pairs.isLoading ? (
          <p className="py-6 text-center text-xs text-muted-foreground">Analizando registros…</p>
        ) : pairs.isError ? (
          <p className="rounded-lg bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {pairs.error instanceof Error ? pairs.error.message : "No se pudo analizar"}
          </p>
        ) : (pairs.data ?? []).length === 0 ? (
          <p className="py-6 text-center text-xs text-muted-foreground">No se detectaron duplicados sospechosos.</p>
        ) : (
          <div className="space-y-2">
            <p className="text-[11px] text-muted-foreground">
              Heurística local + IA (si hay proveedor). Tú decides: eliminar borra solo el registro elegido.
            </p>
            {(pairs.data ?? []).map((p, i) => (
              <div key={i} className={cn("rounded-lg border border-border p-3", (removed.has(p.record_a) || removed.has(p.record_b)) && "opacity-40")}>
                <div className="mb-1 flex items-center justify-between">
                  <Badge variant={p.score >= 0.95 ? "destructive" : "warning"}>{Math.round(p.score * 100)}% similar</Badge>
                  <span className="text-[10px] text-muted-foreground">{p.reason}</span>
                </div>
                {[{ id: p.record_a, title: p.title_a }, { id: p.record_b, title: p.title_b }].map((r) => (
                  <div key={r.id} className="flex items-center justify-between gap-2 py-0.5">
                    <span className="truncate text-xs">{r.title}</span>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 px-2 text-[10px] text-destructive"
                      disabled={removed.has(p.record_a) || removed.has(p.record_b)}
                      onClick={async () => {
                        await onDelete(r.id);
                        setRemoved((prev) => new Set(prev).add(r.id));
                      }}
                    >
                      Eliminar este
                    </Button>
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}
        <DialogFooter>
          <Button onClick={onClose}>Cerrar</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
