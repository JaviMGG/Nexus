import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  Archive,
  ArrowRight,
  CheckCircle2,
  FileText,
  Inbox,
  ListTodo,
  Loader2,
  Send,
  Sparkles,
  Trash2,
} from "lucide-react";
import { invoke } from "@/lib/api";
import type { CaptureResult, CreateNote, CreateTask, InboxItem, InboxStatus, Priority } from "@/lib/types";
import { PRIORITY_LABEL, cn, relativeTime } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { QueryError } from "@/components/tasks/QueryError";

const STATUSES: InboxStatus[] = ["pending", "processed", "archived"];
const PRIORITIES: Priority[] = ["low", "medium", "high", "urgent"];

export function InboxPage() {
  const qc = useQueryClient();
  const [tab, setTab] = useState<InboxStatus>("pending");
  const [content, setContent] = useState("");
  const [analysis, setAnalysis] = useState<{ item: InboxItem; result: CaptureResult } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<InboxItem | null>(null);

  const queries = {
    pending: useInboxQuery("pending"),
    processed: useInboxQuery("processed"),
    archived: useInboxQuery("archived"),
  };

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["inbox"] });
    qc.invalidateQueries({ queryKey: ["dashboard"] });
    qc.invalidateQueries({ queryKey: ["tasks"] });
    qc.invalidateQueries({ queryKey: ["notes"] });
  };

  const add = useMutation<InboxItem, Error, string>({
    mutationFn: (c) => invoke<InboxItem>("inbox.create", { content: c }),
    onSuccess: () => {
      setContent("");
      toast.success("Capturado en el inbox");
      invalidate();
    },
    onError: (err) => toast.error("No se pudo capturar", { description: err.message }),
  });

  const analyze = useMutation<CaptureResult, Error, InboxItem>({
    mutationFn: (item) => invoke<CaptureResult>("capture.analyze", { content: item.content }),
    onSuccess: (result, item) => setAnalysis({ item, result }),
    onError: (err) => toast.error("No se pudo analizar", { description: err.message }),
  });

  const process = useMutation<{ created_id: string }, Error, { id: string; kind: "task" | "note"; data?: CreateTask | CreateNote }>({
    mutationFn: (vars) => invoke("inbox.process", vars),
    onSuccess: (_r, vars) => {
      toast.success(vars.kind === "task" ? "Convertido en tarea" : "Convertido en nota");
      setAnalysis(null);
      invalidate();
    },
    onError: (err) => toast.error("No se pudo procesar", { description: err.message }),
  });

  const archive = useMutation<void, Error, InboxItem>({
    mutationFn: (item) => invoke<void>("inbox.archive", { id: item.id }),
    onSuccess: () => {
      toast.success("Elemento archivado");
      invalidate();
    },
    onError: (err) => toast.error("No se pudo archivar", { description: err.message }),
  });

  const remove = useMutation<void, Error, InboxItem>({
    mutationFn: (item) => invoke<void>("inbox.delete", { id: item.id }),
    onSuccess: () => {
      toast.success("Elemento eliminado");
      setDeleteTarget(null);
      invalidate();
    },
    onError: (err) => toast.error("No se pudo eliminar", { description: err.message }),
  });

  const submit = () => {
    const c = content.trim();
    if (c && !add.isPending) add.mutate(c);
  };

  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <header className="mb-6">
        <h1 className="text-xl font-semibold tracking-tight">Inbox</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Captura ideas al vuelo y procésalas cuando tengas contexto.
        </p>
      </header>

      <div className="mb-6 flex gap-2">
        <Input
          value={content}
          onChange={(e) => setContent(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
          placeholder="Escribe una idea, recordatorio o enlace… (Enter para capturar)"
          disabled={add.isPending}
        />
        <Button onClick={submit} disabled={add.isPending || !content.trim()}>
          {add.isPending ? <Loader2 className="animate-spin" /> : <Send />}
          Capturar
        </Button>
      </div>

      <Tabs value={tab} onValueChange={(v) => setTab(v as InboxStatus)}>
        <TabsList>
          {STATUSES.map((s) => (
            <TabsTrigger key={s} value={s}>
              {s === "pending" ? "Pendientes" : s === "processed" ? "Procesados" : "Archivados"}
              {queries[s].data && (
                <span className="rounded-full bg-muted px-1.5 text-[10px] tabular-nums">
                  {queries[s].data!.length}
                </span>
              )}
            </TabsTrigger>
          ))}
        </TabsList>
        {STATUSES.map((s) => (
          <TabsContent key={s} value={s}>
            {queries[s].isLoading ? (
              <div className="space-y-2">
                {Array.from({ length: 4 }).map((_, i) => (
                  <Skeleton key={i} className="h-16 rounded-xl" />
                ))}
              </div>
            ) : queries[s].isError ? (
              <QueryError error={queries[s].error} onRetry={() => queries[s].refetch()} />
            ) : (queries[s].data ?? []).length === 0 ? (
              <EmptyState
                icon={s === "pending" ? Inbox : s === "processed" ? CheckCircle2 : Archive}
                title={
                  s === "pending"
                    ? "Bandeja vacía"
                    : s === "processed"
                      ? "Nada procesado todavía"
                      : "No hay elementos archivados"
                }
                description={
                  s === "pending"
                    ? "Captura cualquier idea arriba y procésala más tarde."
                    : s === "processed"
                      ? "Los elementos que conviertas en tareas o notas aparecerán aquí."
                      : "Los elementos que archives se guardarán aquí."
                }
              />
            ) : (
              <div className="space-y-2">
                {queries[s].data!.map((item) => (
                  <div
                    key={item.id}
                    className="group flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3 shadow-sm transition-colors hover:border-ring/40 animate-fade-in"
                  >
                    <div className="min-w-0 flex-1">
                      <p className={cn("text-sm", s !== "pending" && "text-muted-foreground")}>{item.content}</p>
                      <div className="mt-0.5 flex items-center gap-2 text-[11px] text-muted-foreground">
                        <span>{relativeTime(item.created_at)}</span>
                        {s === "processed" && (
                          <Badge variant="success">
                            <CheckCircle2 /> Convertido
                          </Badge>
                        )}
                        {s === "archived" && <Badge variant="secondary">Archivado</Badge>}
                      </div>
                    </div>
                    {s === "pending" && (
                      <div className="flex shrink-0 items-center gap-1">
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={analyze.isPending && analyze.variables?.id === item.id}
                          onClick={() => analyze.mutate(item)}
                        >
                          {analyze.isPending && analyze.variables?.id === item.id ? (
                            <Loader2 className="animate-spin" />
                          ) : (
                            <Sparkles />
                          )}
                          Analizar
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          disabled={process.isPending}
                          onClick={() => process.mutate({ id: item.id, kind: "task" })}
                        >
                          <ListTodo /> <ArrowRight className="!size-3" /> Tarea
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          disabled={process.isPending}
                          onClick={() => process.mutate({ id: item.id, kind: "note" })}
                        >
                          <FileText /> <ArrowRight className="!size-3" /> Nota
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label="Archivar"
                          disabled={archive.isPending}
                          onClick={() => archive.mutate(item)}
                        >
                          <Archive />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          aria-label="Eliminar"
                          className="text-muted-foreground hover:text-destructive"
                          onClick={() => setDeleteTarget(item)}
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    )}
                    {s !== "pending" && (
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Eliminar"
                        className="text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive"
                        onClick={() => setDeleteTarget(item)}
                      >
                        <Trash2 />
                      </Button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </TabsContent>
        ))}
      </Tabs>

      {analysis && (
        <AnalyzeDialog
          item={analysis.item}
          result={analysis.result}
          pending={process.isPending}
          onClose={() => setAnalysis(null)}
          onConfirm={(kind, data) => process.mutate({ id: analysis.item.id, kind, data })}
        />
      )}

      <Dialog open={deleteTarget !== null} onOpenChange={(open) => !open && setDeleteTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Eliminar elemento</DialogTitle>
            <DialogDescription>
              ¿Seguro que quieres eliminar «{deleteTarget?.content}»? Esta acción no se puede deshacer.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)} disabled={remove.isPending}>
              Cancelar
            </Button>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={() => deleteTarget && remove.mutate(deleteTarget)}
            >
              {remove.isPending ? "Eliminando…" : "Eliminar"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function useInboxQuery(status: InboxStatus) {
  return useQuery({
    queryKey: ["inbox", status],
    queryFn: () => invoke<InboxItem[]>("inbox.list", { status }),
  });
}

function AnalyzeDialog({
  item,
  result,
  pending,
  onClose,
  onConfirm,
}: {
  item: InboxItem;
  result: CaptureResult;
  pending: boolean;
  onClose: () => void;
  onConfirm: (kind: "task" | "note", data: CreateTask | CreateNote) => void;
}) {
  const isTask = result.kind === "task";
  const [title, setTitle] = useState(result.task?.title ?? result.note?.title ?? item.content);
  const [due, setDue] = useState(result.task?.due_date?.slice(0, 10) ?? "");
  const [priority, setPriority] = useState<Priority>(result.task?.priority ?? "medium");
  const [noteContent, setNoteContent] = useState(result.note?.content ?? item.content);

  const confirm = () => {
    const t = title.trim();
    if (!t) {
      toast.error("El título es obligatorio");
      return;
    }
    if (isTask) {
      const data: CreateTask = { title: t, priority, due_date: due || null };
      onConfirm("task", data);
    } else {
      const data: CreateNote = { title: t, content: noteContent };
      onConfirm("note", data);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            Sugerencia: {isTask ? "Tarea" : "Nota"}
            <Badge variant={result.ai_used ? "default" : "secondary"}>
              {result.ai_used ? "IA" : "Local"}
            </Badge>
          </DialogTitle>
          <DialogDescription>{result.explanation}</DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="capture-title">Título</Label>
            <Input id="capture-title" value={title} onChange={(e) => setTitle(e.target.value)} autoFocus />
          </div>
          {isTask ? (
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="capture-due">Fecha límite</Label>
                <Input id="capture-due" type="date" value={due} onChange={(e) => setDue(e.target.value)} />
              </div>
              <div className="space-y-1.5">
                <Label>Prioridad</Label>
                <Select value={priority} onValueChange={(v) => setPriority(v as Priority)}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PRIORITIES.map((p) => (
                      <SelectItem key={p} value={p}>
                        {PRIORITY_LABEL[p]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
          ) : (
            <div className="space-y-1.5">
              <Label htmlFor="capture-content">Contenido</Label>
              <Textarea
                id="capture-content"
                value={noteContent}
                onChange={(e) => setNoteContent(e.target.value)}
                rows={5}
              />
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={pending}>
            Cancelar
          </Button>
          <Button onClick={confirm} disabled={pending || !title.trim()}>
            {pending ? <Loader2 className="animate-spin" /> : isTask ? <ListTodo /> : <FileText />}
            {pending ? "Creando…" : isTask ? "Crear tarea" : "Crear nota"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
