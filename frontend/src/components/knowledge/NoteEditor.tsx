import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { toast } from "sonner";
import {
  Eye,
  ExternalLink,
  Link2,
  Loader2,
  Pencil,
  Pin,
  Save,
  Star,
  Trash2,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Id, Note, NoteKind, Project, UpdateNote } from "@/lib/types";
import { NOTE_KIND_LABEL, cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

const NONE = "__none__";

interface Draft {
  title: string;
  content: string;
  kind: NoteKind;
  url: string;
  projectId: string;
  tags: string;
  pinned: boolean;
}

function toDraft(note: Note | null): Draft {
  if (!note) {
    return { title: "", content: "", kind: "note", url: "", projectId: NONE, tags: "", pinned: false };
  }
  return {
    title: note.title,
    content: note.content,
    kind: note.kind,
    url: note.url ?? "",
    projectId: note.project_id ?? NONE,
    tags: note.tags.join(", "),
    pinned: note.pinned,
  };
}

function isDirty(draft: Draft, note: Note | null): boolean {
  const base = toDraft(note);
  return (
    draft.title !== base.title ||
    draft.content !== base.content ||
    draft.kind !== base.kind ||
    draft.url !== base.url ||
    draft.projectId !== base.projectId ||
    draft.tags !== base.tags ||
    draft.pinned !== base.pinned
  );
}

function parseTags(raw: string): string[] {
  return [...new Set(raw.split(",").map((t) => t.trim()).filter(Boolean))];
}

interface NoteEditorProps {
  note: Note | null;
  isNew: boolean;
  onSaved: (id: Id) => void;
  onDeleted: () => void;
}

export function NoteEditor({ note, isNew, onSaved, onDeleted }: NoteEditorProps) {
  const qc = useQueryClient();
  const [draft, setDraft] = useState<Draft>(() => toDraft(note));
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [confirmDelete, setConfirmDelete] = useState(false);

  const noteId = note?.id ?? null;
  useEffect(() => {
    setDraft(toDraft(note));
    setMode("edit");
    setConfirmDelete(false);
  }, [noteId, isNew]);

  const projects = useQuery({
    queryKey: ["projects"],
    queryFn: () => invoke<Project[]>("projects.list"),
  });

  const dirty = isDirty(draft, note);
  const canSave = draft.title.trim().length > 0 && (isNew || dirty);

  const save = useMutation({
    mutationFn: async () => {
      const tags = parseTags(draft.tags);
      const projectId = draft.projectId === NONE ? null : draft.projectId;
      const url = draft.kind === "link" ? draft.url.trim() || null : null;
      if (isNew || !note) {
        return invoke<Note>("notes.create", {
          data: {
            title: draft.title.trim(),
            content: draft.content,
            kind: draft.kind,
            url,
            project_id: projectId,
            tags,
            pinned: draft.pinned,
          },
        });
      }
      const data: UpdateNote = {
        title: draft.title.trim(),
        content: draft.content,
        kind: draft.kind,
        url,
        project_id: projectId,
        tags,
        pinned: draft.pinned,
      };
      return invoke<Note>("notes.update", { id: note.id, data });
    },
    onSuccess: (saved) => {
      toast.success(isNew ? "Nota creada" : "Nota guardada");
      qc.invalidateQueries({ queryKey: ["notes"] });
      onSaved(saved.id);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar la nota"),
  });

  const remove = useMutation({
    mutationFn: () => invoke("notes.delete", { id: note!.id }),
    onSuccess: () => {
      toast.success("Nota eliminada");
      qc.invalidateQueries({ queryKey: ["notes"] });
      setConfirmDelete(false);
      onDeleted();
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar la nota"),
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (canSave && !save.isPending) save.mutate();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [canSave, save.isPending, save.mutate]);

  const set = <K extends keyof Draft>(key: K, value: Draft[K]) =>
    setDraft((d) => ({ ...d, [key]: value }));

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-3 border-b border-border px-5 py-3">
        <Input
          value={draft.title}
          onChange={(e) => set("title", e.target.value)}
          placeholder="Título de la nota…"
          className="h-9 flex-1 border-0 bg-transparent px-0 text-base font-semibold shadow-none focus-visible:ring-0"
        />
        {dirty && !isNew && (
          <span className="flex items-center gap-1.5 whitespace-nowrap text-[11px] text-warning">
            <span className="h-1.5 w-1.5 rounded-full bg-warning" />
            Sin guardar
          </span>
        )}
        {isNew && <span className="whitespace-nowrap text-[11px] text-muted-foreground">Nueva nota</span>}
        <Button size="sm" onClick={() => save.mutate()} disabled={!canSave || save.isPending}>
          {save.isPending ? <Loader2 className="animate-spin" /> : <Save />}
          Guardar
          <kbd className="ml-0.5 hidden rounded bg-primary-foreground/20 px-1 text-[10px] sm:inline">Ctrl+S</kbd>
        </Button>
        {!isNew && note && (
          <Button
            size="icon-sm"
            variant="ghost"
            className="text-muted-foreground hover:text-destructive"
            onClick={() => setConfirmDelete(true)}
            aria-label="Eliminar nota"
          >
            <Trash2 />
          </Button>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-border px-5 py-2.5">
        <div className="flex items-center gap-2">
          <Label className="whitespace-nowrap">Tipo</Label>
          <Select value={draft.kind} onValueChange={(v) => set("kind", v as NoteKind)}>
            <SelectTrigger className="h-8 w-36 text-xs"><SelectValue /></SelectTrigger>
            <SelectContent>
              {Object.entries(NOTE_KIND_LABEL).map(([v, l]) => (
                <SelectItem key={v} value={v}>{l}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex min-w-44 flex-1 items-center gap-2">
          <Label className="whitespace-nowrap">Proyecto</Label>
          <Select value={draft.projectId} onValueChange={(v) => set("projectId", v)}>
            <SelectTrigger className="h-8 flex-1 text-xs"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value={NONE}>Sin proyecto</SelectItem>
              {(projects.data ?? []).map((p) => (
                <SelectItem key={p.id} value={p.id}>{p.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex items-center gap-2">
          <Switch
            id="note-pinned"
            checked={draft.pinned}
            onCheckedChange={(v) => set("pinned", v)}
          />
          <Label htmlFor="note-pinned" className="flex cursor-pointer items-center gap-1">
            <Pin className="h-3 w-3" /> Fijada
          </Label>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-border px-5 py-2.5">
        {draft.kind === "link" && (
          <div className="flex min-w-56 flex-1 items-center gap-2">
            <Link2 className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
            <Input
              value={draft.url}
              onChange={(e) => set("url", e.target.value)}
              placeholder="https://…"
              className="h-8 text-xs"
            />
            {draft.url.trim() && (
              <Button size="icon-sm" variant="ghost" asChild aria-label="Abrir enlace">
                <a href={draft.url} target="_blank" rel="noreferrer"><ExternalLink /></a>
              </Button>
            )}
          </div>
        )}
        <div className="flex min-w-56 flex-1 items-center gap-2">
          <Label className="whitespace-nowrap">Tags</Label>
          <Input
            value={draft.tags}
            onChange={(e) => set("tags", e.target.value)}
            placeholder="rust, aprendizaje, ideas…"
            className="h-8 text-xs"
          />
        </div>
      </div>

      <div className="flex items-center justify-between px-5 pt-3">
        <Tabs value={mode} onValueChange={(v) => setMode(v as "edit" | "preview")}>
          <TabsList className="h-8">
            <TabsTrigger value="edit"><Pencil /> Editar</TabsTrigger>
            <TabsTrigger value="preview"><Eye /> Vista previa</TabsTrigger>
          </TabsList>
        </Tabs>
        {note && !isNew && (
          <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
            <Star className={cn("h-3 w-3", note.pinned && "fill-warning text-warning")} />
            {note.pinned ? "Fijada" : "Sin fijar"}
          </span>
        )}
      </div>

      <div className="min-h-0 flex-1 px-5 pb-5 pt-3">
        {mode === "edit" ? (
          <Textarea
            value={draft.content}
            onChange={(e) => set("content", e.target.value)}
            placeholder="Escribe en Markdown… # títulos, - listas, **negrita**, `código`"
            className="h-full min-h-[200px] resize-none font-mono text-[13px] leading-relaxed"
          />
        ) : (
          <div className="h-full overflow-y-auto rounded-md border border-border bg-card p-4">
            {draft.content.trim() ? (
              <div className="markdown-body">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{draft.content}</ReactMarkdown>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">Nada que previsualizar todavía.</p>
            )}
          </div>
        )}
      </div>

      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Eliminar nota</DialogTitle>
            <DialogDescription>
              «{note?.title}» se moverá a la papelera. Podrás deshacerlo desde Actividad → Auditoría.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmDelete(false)}>Cancelar</Button>
            <Button variant="destructive" onClick={() => remove.mutate()} disabled={remove.isPending}>
              {remove.isPending && <Loader2 className="animate-spin" />}
              Eliminar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
