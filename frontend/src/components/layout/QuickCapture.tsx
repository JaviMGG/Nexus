import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Inbox, Loader2, Sparkles, Zap } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { CaptureResult, CreateNote, CreateTask } from "@/lib/types";
import { useUiStore } from "@/stores/ui";
import { useAiStatus } from "./AiStatusPill";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { PRIORITY_LABEL } from "@/lib/utils";

export function QuickCapture() {
  const open = useUiStore((s) => s.quickCaptureOpen);
  const setOpen = useUiStore((s) => s.setQuickCaptureOpen);
  const { data: aiStatus } = useAiStatus();
  const qc = useQueryClient();

  const [content, setContent] = useState("");
  const [result, setResult] = useState<CaptureResult | null>(null);
  const [taskDraft, setTaskDraft] = useState<CreateTask | null>(null);
  const [noteDraft, setNoteDraft] = useState<CreateNote | null>(null);

  useEffect(() => {
    if (open) {
      setContent("");
      setResult(null);
      setTaskDraft(null);
      setNoteDraft(null);
    }
  }, [open]);

  const analyze = useMutation({
    mutationFn: () => invoke<CaptureResult>("capture.analyze", { content }),
    onSuccess: (r) => {
      setResult(r);
      setTaskDraft(r.task);
      setNoteDraft(r.note);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo analizar"),
  });

  const save = useMutation({
    mutationFn: async () => {
      if (result?.kind === "task" && taskDraft) {
        return invoke("tasks.create", { data: taskDraft });
      }
      if (noteDraft) {
        return invoke("notes.create", { data: noteDraft });
      }
      throw new Error("nada que guardar");
    },
    onSuccess: () => {
      toast.success(result?.kind === "task" ? "Tarea creada" : "Nota guardada");
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["notes"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      setOpen(false);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar"),
  });

  const toInbox = useMutation({
    mutationFn: () => invoke("inbox.create", { content }),
    onSuccess: () => {
      toast.success("Guardado en el inbox");
      qc.invalidateQueries({ queryKey: ["inbox"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      setOpen(false);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar"),
  });

  const aiConnected = aiStatus?.state === "connected";

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="max-w-xl top-[22%] translate-y-0">
        <DialogTitle className="sr-only">Captura rápida</DialogTitle>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Zap className="h-3.5 w-3.5 text-primary" />
          Captura rápida
          <span className="ml-auto flex items-center gap-1.5">
            {aiConnected ? (
              <Badge variant="default"><Sparkles /> IA</Badge>
            ) : (
              <Badge variant="outline">Análisis local</Badge>
            )}
          </span>
        </div>

        {!result ? (
          <>
            <Textarea
              autoFocus
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder='Escribe lo que tengas en mente… p. ej. "Tengo que preparar el portfolio antes del viernes"'
              className="min-h-[96px] text-sm border-0 shadow-none focus-visible:ring-0 px-0 bg-transparent"
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && content.trim()) {
                  analyze.mutate();
                }
              }}
            />
            <div className="flex items-center justify-between gap-2">
              <Button variant="ghost" size="sm" onClick={() => toInbox.mutate()} disabled={!content.trim() || toInbox.isPending}>
                <Inbox /> Al inbox
              </Button>
              <Button size="sm" onClick={() => analyze.mutate()} disabled={!content.trim() || analyze.isPending}>
                {analyze.isPending ? <Loader2 className="animate-spin" /> : <Sparkles />}
                Analizar
                <kbd className="ml-1 rounded bg-primary-foreground/20 px-1 text-[10px]">⌘↵</kbd>
              </Button>
            </div>
          </>
        ) : (
          <div className="space-y-3 animate-fade-in">
            <p className="text-xs text-muted-foreground">{result.explanation}</p>
            {result.kind === "task" && taskDraft ? (
              <div className="space-y-3 rounded-lg border border-border p-3">
                <div className="space-y-1.5">
                  <Label>Título</Label>
                  <Input value={taskDraft.title} onChange={(e) => setTaskDraft({ ...taskDraft, title: e.target.value })} />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1.5">
                    <Label>Fecha límite</Label>
                    <Input
                      type="date"
                      value={taskDraft.due_date?.slice(0, 10) ?? ""}
                      onChange={(e) => setTaskDraft({ ...taskDraft, due_date: e.target.value || null })}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label>Prioridad</Label>
                    <Select
                      value={taskDraft.priority ?? "medium"}
                      onValueChange={(v) => setTaskDraft({ ...taskDraft, priority: v as CreateTask["priority"] })}
                    >
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {Object.entries(PRIORITY_LABEL).map(([v, l]) => (
                          <SelectItem key={v} value={v}>{l}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
              </div>
            ) : noteDraft ? (
              <div className="space-y-3 rounded-lg border border-border p-3">
                <div className="space-y-1.5">
                  <Label>Título</Label>
                  <Input value={noteDraft.title} onChange={(e) => setNoteDraft({ ...noteDraft, title: e.target.value })} />
                </div>
                <div className="space-y-1.5">
                  <Label>Contenido</Label>
                  <Textarea value={noteDraft.content ?? ""} onChange={(e) => setNoteDraft({ ...noteDraft, content: e.target.value })} />
                </div>
              </div>
            ) : null}
            <div className="flex items-center justify-between gap-2">
              <Button variant="ghost" size="sm" onClick={() => setResult(null)}>Volver</Button>
              <div className="flex gap-2">
                <Button variant="outline" size="sm" onClick={() => toInbox.mutate()} disabled={toInbox.isPending}>
                  <Inbox /> Al inbox
                </Button>
                <Button size="sm" onClick={() => save.mutate()} disabled={save.isPending}>
                  {save.isPending && <Loader2 className="animate-spin" />}
                  {result.kind === "task" ? "Crear tarea" : "Guardar nota"}
                </Button>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
