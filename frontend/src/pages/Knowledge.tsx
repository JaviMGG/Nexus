import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  BookOpen,
  FileText,
  Layers,
  Lightbulb,
  Link2,
  NotebookPen,
  Plus,
  Search,
  Star,
  StickyNote,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Note, NoteFilter, NoteKind } from "@/lib/types";
import { NOTE_KIND_LABEL, cn, relativeTime } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EmptyState } from "@/components/ui/empty-state";
import { NoteEditor } from "@/components/knowledge/NoteEditor";

const KIND_TABS: { value: string; label: string; icon: typeof Layers }[] = [
  { value: "all", label: "Todas", icon: Layers },
  { value: "note", label: "Notas", icon: StickyNote },
  { value: "idea", label: "Ideas", icon: Lightbulb },
  { value: "document", label: "Documentos", icon: FileText },
  { value: "link", label: "Enlaces", icon: Link2 },
];

function snippet(content: string): string {
  return content
    .replace(/[#*`>\-\[\]]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

export function KnowledgePage() {
  const qc = useQueryClient();
  const [search, setSearch] = useState("");
  const [debounced, setDebounced] = useState("");
  const [kind, setKind] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    const t = setTimeout(() => setDebounced(search.trim()), 300);
    return () => clearTimeout(t);
  }, [search]);

  const filter = useMemo<NoteFilter>(() => {
    const f: NoteFilter = {};
    if (debounced) f.search = debounced;
    if (kind !== "all") f.kind = kind as NoteKind;
    return f;
  }, [debounced, kind]);

  const notes = useQuery({
    queryKey: ["notes", filter],
    queryFn: () => invoke<Note[]>("notes.list", { filter }),
  });

  const sorted = useMemo(() => {
    const list = [...(notes.data ?? [])];
    list.sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.updated_at.localeCompare(a.updated_at);
    });
    return list;
  }, [notes.data]);

  useEffect(() => {
    if (creating || notes.isLoading || notes.isFetching) return;
    if (selectedId && sorted.some((n) => n.id === selectedId)) return;
    setSelectedId(sorted[0]?.id ?? null);
  }, [sorted, selectedId, creating, notes.isLoading, notes.isFetching]);

  const selected = sorted.find((n) => n.id === selectedId) ?? null;

  const togglePin = useMutation({
    mutationFn: (note: Note) => invoke("notes.update", { id: note.id, data: { pinned: !note.pinned } }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["notes"] }),
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo fijar la nota"),
  });

  const startNew = () => {
    setCreating(true);
    setSelectedId(null);
  };

  return (
    <div className="mx-auto flex h-full max-w-[1200px] flex-col p-6 animate-fade-in">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Conocimiento</h1>
          <p className="text-xs text-muted-foreground">Tus notas, ideas, documentos y enlaces en un solo lugar.</p>
        </div>
        <Button size="sm" onClick={startNew}>
          <Plus /> Nueva nota
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 overflow-hidden rounded-xl border border-border bg-card shadow-sm">
        <div className="flex w-[320px] shrink-0 flex-col border-r border-border">
          <div className="space-y-2.5 border-b border-border p-3">
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Buscar en tus notas…"
                className="h-8 pl-8 text-xs"
              />
            </div>
            <Tabs value={kind} onValueChange={setKind}>
              <TabsList className="grid h-8 w-full grid-cols-5">
                {KIND_TABS.map((t) => (
                  <TabsTrigger key={t.value} value={t.value} className="px-1 text-[11px]" title={t.label}>
                    <t.icon />
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            {notes.isLoading ? (
              <div className="space-y-2 p-1">
                {Array.from({ length: 6 }).map((_, i) => (
                  <div key={i} className="space-y-2 rounded-lg p-2">
                    <Skeleton className="h-4 w-3/4" />
                    <Skeleton className="h-3 w-full" />
                    <Skeleton className="h-3 w-1/2" />
                  </div>
                ))}
              </div>
            ) : notes.isError ? (
              <EmptyState
                icon={BookOpen}
                title="No se pudieron cargar las notas"
                description={notes.error instanceof ApiError ? notes.error.message : "Comprueba que el servicio local está activo."}
                action={<Button size="sm" variant="outline" onClick={() => notes.refetch()}>Reintentar</Button>}
              />
            ) : sorted.length === 0 ? (
              <EmptyState
                icon={NotebookPen}
                title={debounced || kind !== "all" ? "Sin resultados" : "Aún no hay notas"}
                description={
                  debounced || kind !== "all"
                    ? "Prueba con otra búsqueda o cambia el filtro de tipo."
                    : "Crea tu primera nota para empezar a construir tu base de conocimiento."
                }
                action={<Button size="sm" onClick={startNew}><Plus /> Nueva nota</Button>}
                className="py-10"
              />
            ) : (
              <ul className="space-y-0.5">
                {sorted.map((note) => (
                  <li key={note.id}>
                    <button
                      onClick={() => { setCreating(false); setSelectedId(note.id); }}
                      className={cn(
                        "w-full rounded-lg px-3 py-2.5 text-left transition-colors cursor-pointer",
                        selectedId === note.id && !creating
                          ? "bg-accent text-accent-foreground"
                          : "hover:bg-muted",
                      )}
                    >
                      <div className="flex items-center gap-2">
                        <span className="min-w-0 flex-1 truncate text-sm font-medium">
                          {note.title || "Sin título"}
                        </span>
                        <span
                          role="button"
                          tabIndex={0}
                          aria-label={note.pinned ? "Desfijar" : "Fijar"}
                          onClick={(e) => { e.stopPropagation(); togglePin.mutate(note); }}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                              e.preventDefault();
                              e.stopPropagation();
                              togglePin.mutate(note);
                            }
                          }}
                          className="shrink-0 rounded p-0.5 transition-colors hover:bg-accent"
                        >
                          <Star
                            className={cn(
                              "h-3.5 w-3.5",
                              note.pinned ? "fill-warning text-warning" : "text-muted-foreground/50",
                            )}
                          />
                        </span>
                      </div>
                      {note.content && (
                        <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
                          {snippet(note.content)}
                        </p>
                      )}
                      <div className="mt-1.5 flex items-center gap-1.5">
                        <Badge variant="secondary" className="px-1.5 py-0 text-[10px]">
                          {NOTE_KIND_LABEL[note.kind]}
                        </Badge>
                        {note.tags.slice(0, 2).map((t) => (
                          <span key={t} className="truncate rounded-full bg-muted px-1.5 py-0 text-[10px] text-muted-foreground">
                            #{t}
                          </span>
                        ))}
                        {note.tags.length > 2 && (
                          <span className="text-[10px] text-muted-foreground">+{note.tags.length - 2}</span>
                        )}
                        <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">
                          {relativeTime(note.updated_at)}
                        </span>
                      </div>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>

        <div className="min-w-0 flex-1">
          {creating || selected ? (
            <NoteEditor
              note={creating ? null : selected}
              isNew={creating}
              onSaved={(id) => { setCreating(false); setSelectedId(id); }}
              onDeleted={() => setSelectedId(null)}
            />
          ) : (
            <EmptyState
              icon={NotebookPen}
              title="Selecciona una nota"
              description="Elige una nota de la lista para editarla, o crea una nueva con Markdown, tags y proyecto."
              action={<Button size="sm" onClick={startNew}><Plus /> Nueva nota</Button>}
              className="h-full"
            />
          )}
        </div>
      </div>
    </div>
  );
}
