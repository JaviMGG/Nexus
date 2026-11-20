import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  ArrowLeft,
  Calendar,
  FileText,
  History,
  ListTodo,
  Pencil,
  Plus,
  Target,
  Trash2,
  Undo2,
} from "lucide-react";
import { invoke } from "@/lib/api";
import type { ActivityEvent, CreateNote, Note, ProjectDetail, Task } from "@/lib/types";
import { NOTE_KIND_LABEL, cn, formatDate, isOverdue, relativeTime } from "@/lib/utils";
import { useToggleTask, useDeleteTask } from "@/hooks/useTaskMutations";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
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
import { ProjectDialog } from "@/components/projects/ProjectDialog";
import { TaskCheckbox } from "@/components/tasks/TaskCheckbox";
import { TaskDialog } from "@/components/tasks/TaskDialog";
import { PriorityBadge, ProjectStatusBadge } from "@/components/tasks/StatusBadges";
import { QueryError } from "@/components/tasks/QueryError";

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const { toggle } = useToggleTask();
  const { requestDelete, dialog: deleteTaskDialog } = useDeleteTask();
  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [taskDialog, setTaskDialog] = useState<{ open: boolean; task: Task | null }>({ open: false, task: null });
  const [noteDialogOpen, setNoteDialogOpen] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState("");

  const detail = useQuery({
    queryKey: ["project", id],
    queryFn: () => invoke<ProjectDetail>("projects.get", { id }),
    enabled: Boolean(id),
  });
  const notes = useQuery({
    queryKey: ["notes", { project_id: id }],
    queryFn: () => invoke<Note[]>("notes.list", { filter: { project_id: id } }),
    enabled: Boolean(id),
  });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["project", id] });
    qc.invalidateQueries({ queryKey: ["projects"] });
    qc.invalidateQueries({ queryKey: ["tasks"] });
    qc.invalidateQueries({ queryKey: ["notes"] });
    qc.invalidateQueries({ queryKey: ["dashboard"] });
    qc.invalidateQueries({ queryKey: ["activity"] });
  };

  const addTask = useMutation<Task, Error, string>({
    mutationFn: (title) => invoke<Task>("tasks.create", { data: { title, project_id: id } }),
    onSuccess: () => {
      setNewTaskTitle("");
      toast.success("Tarea añadida al proyecto");
      invalidate();
    },
    onError: (err) => toast.error("No se pudo crear la tarea", { description: err.message }),
  });

  const removeProject = useMutation<void, Error, void>({
    mutationFn: () => invoke<void>("projects.delete", { id }),
    onSuccess: () => {
      toast.success("Proyecto eliminado");
      qc.invalidateQueries({ queryKey: ["projects"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      navigate("/projects");
    },
    onError: (err) => toast.error("No se pudo eliminar el proyecto", { description: err.message }),
  });

  const undo = useMutation<void, Error, ActivityEvent>({
    mutationFn: (e) => invoke<void>("activity.undo", { id: e.id }),
    onSuccess: () => {
      toast.success("Acción deshecha");
      invalidate();
    },
    onError: (err) => toast.error("No se pudo deshacer", { description: err.message }),
  });

  if (detail.isLoading) {
    return (
      <div className="mx-auto max-w-[1200px] p-6">
        <Skeleton className="mb-4 h-8 w-64" />
        <Skeleton className="mb-6 h-24 rounded-xl" />
        <Skeleton className="h-64 rounded-xl" />
      </div>
    );
  }
  if (detail.isError) {
    return (
      <div className="mx-auto max-w-[1200px] p-6">
        <QueryError error={detail.error} onRetry={() => detail.refetch()} />
      </div>
    );
  }

  const { project: p, tasks, progress, activity } = detail.data!;
  const rootTasks = tasks.filter((t) => !t.parent_id);
  const overdueTasks = tasks.filter((t) => isOverdue(t.due_date, t.status));

  const submitTask = () => {
    const t = newTaskTitle.trim();
    if (t && !addTask.isPending) addTask.mutate(t);
  };

  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <Button variant="ghost" size="sm" className="mb-4 -ml-2 text-muted-foreground" onClick={() => navigate("/projects")}>
        <ArrowLeft /> Proyectos
      </Button>

      <header className="mb-6 flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2.5">
            <h1 className="text-xl font-semibold tracking-tight">{p.name}</h1>
            <ProjectStatusBadge status={p.status} />
            <PriorityBadge priority={p.priority} />
          </div>
          {p.description && <p className="mt-1.5 max-w-2xl text-sm text-muted-foreground">{p.description}</p>}
          {p.goal && (
            <p className="mt-2 flex items-center gap-1.5 text-xs text-muted-foreground">
              <Target className="size-3.5 text-primary" />
              <span className="font-medium text-foreground">Objetivo:</span> {p.goal}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => setEditOpen(true)}>
            <Pencil /> Editar
          </Button>
          <Button variant="outline" size="sm" className="text-destructive hover:text-destructive" onClick={() => setDeleteOpen(true)}>
            <Trash2 /> Eliminar
          </Button>
        </div>
      </header>

      <Card className="mb-6">
        <CardContent className="p-4">
          <div className="mb-2 flex items-center justify-between text-xs">
            <span className="font-medium text-muted-foreground">Progreso</span>
            <span className="tabular-nums text-muted-foreground">
              {progress.done}/{progress.total} tareas · {Math.round(progress.progress * 100)}%
            </span>
          </div>
          <Progress value={progress.progress * 100} className="h-2" />
          {overdueTasks.length > 0 && (
            <p className="mt-2.5 text-xs font-medium text-destructive">
              {overdueTasks.length} {overdueTasks.length === 1 ? "tarea atrasada" : "tareas atrasadas"} en este proyecto
            </p>
          )}
        </CardContent>
      </Card>

      <Tabs defaultValue="tasks">
        <TabsList>
          <TabsTrigger value="tasks">
            <ListTodo /> Tareas
            <span className="rounded-full bg-muted px-1.5 text-[10px] tabular-nums">{tasks.length}</span>
          </TabsTrigger>
          <TabsTrigger value="notes">
            <FileText /> Notas
            {notes.data && (
              <span className="rounded-full bg-muted px-1.5 text-[10px] tabular-nums">{notes.data.length}</span>
            )}
          </TabsTrigger>
          <TabsTrigger value="activity">
            <History /> Actividad
            <span className="rounded-full bg-muted px-1.5 text-[10px] tabular-nums">{activity.length}</span>
          </TabsTrigger>
        </TabsList>

        <TabsContent value="tasks">
          <Card>
            <CardContent className="p-3">
              <div className="mb-2 flex gap-2 px-1">
                <Input
                  value={newTaskTitle}
                  onChange={(e) => setNewTaskTitle(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      submitTask();
                    }
                  }}
                  placeholder="Añadir tarea a este proyecto… (Enter)"
                  disabled={addTask.isPending}
                  className="h-8 text-xs"
                />
                <Button size="sm" variant="secondary" onClick={submitTask} disabled={addTask.isPending || !newTaskTitle.trim()}>
                  <Plus /> Añadir
                </Button>
              </div>
              {rootTasks.length === 0 ? (
                <EmptyState
                  icon={ListTodo}
                  title="Sin tareas en este proyecto"
                  description="Añade la primera tarea arriba para empezar a avanzar."
                />
              ) : (
                <div className="space-y-0.5">
                  {rootTasks.map((t) => (
                    <div
                      key={t.id}
                      className="group flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors hover:bg-accent/60"
                    >
                      <TaskCheckbox checked={t.status === "done"} onChange={() => toggle(t)} />
                      <button
                        type="button"
                        onClick={() => setTaskDialog({ open: true, task: t })}
                        className="min-w-0 flex-1 text-left cursor-pointer"
                      >
                        <span
                          className={cn(
                            "block truncate text-sm",
                            (t.status === "done" || t.status === "cancelled") && "line-through text-muted-foreground",
                          )}
                        >
                          {t.title}
                        </span>
                      </button>
                      {t.due_date && (
                        <span
                          className={cn(
                            "flex items-center gap-1 text-[11px]",
                            isOverdue(t.due_date, t.status) ? "font-medium text-destructive" : "text-muted-foreground",
                          )}
                        >
                          <Calendar className="size-3" />
                          {formatDate(t.due_date)}
                        </span>
                      )}
                      {t.status === "in_progress" && <Badge variant="warning">En curso</Badge>}
                      {t.priority !== "low" && <PriorityBadge priority={t.priority} />}
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Eliminar tarea"
                        className="text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive"
                        onClick={() => requestDelete(t)}
                      >
                        <Trash2 />
                      </Button>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="notes">
          <div className="mb-3 flex justify-end">
            <Button size="sm" onClick={() => setNoteDialogOpen(true)}>
              <Plus /> Nueva nota
            </Button>
          </div>
          {notes.isLoading ? (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {Array.from({ length: 4 }).map((_, i) => (
                <Skeleton key={i} className="h-28 rounded-xl" />
              ))}
            </div>
          ) : notes.isError ? (
            <QueryError error={notes.error} onRetry={() => notes.refetch()} />
          ) : (notes.data ?? []).length === 0 ? (
            <EmptyState
              icon={FileText}
              title="Sin notas en este proyecto"
              description="Documenta decisiones, ideas y contexto junto a tu trabajo."
              action={
                <Button size="sm" onClick={() => setNoteDialogOpen(true)}>
                  <Plus /> Nueva nota
                </Button>
              }
            />
          ) : (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {notes.data!.map((n) => (
                <Card
                  key={n.id}
                  onClick={() => navigate("/knowledge")}
                  className="cursor-pointer p-4 transition-all hover:border-ring/50 hover:shadow animate-fade-in"
                >
                  <div className="mb-1.5 flex items-center gap-2">
                    <h3 className="min-w-0 flex-1 truncate text-sm font-medium">{n.title}</h3>
                    <Badge variant="outline">{NOTE_KIND_LABEL[n.kind]}</Badge>
                  </div>
                  <p className="line-clamp-3 text-xs text-muted-foreground">{n.content || "Sin contenido"}</p>
                  <p className="mt-2 text-[11px] text-muted-foreground">{relativeTime(n.updated_at)}</p>
                </Card>
              ))}
            </div>
          )}
        </TabsContent>

        <TabsContent value="activity">
          <Card>
            <CardContent className="p-4">
              {activity.length === 0 ? (
                <EmptyState
                  icon={History}
                  title="Sin actividad"
                  description="Los cambios en este proyecto aparecerán aquí."
                />
              ) : (
                <div className="space-y-0">
                  {activity.map((e, i) => (
                    <div key={e.id} className="relative flex gap-3 pb-4 last:pb-0">
                      {i < activity.length - 1 && (
                        <span className="absolute left-[5px] top-4 h-full w-px bg-border" />
                      )}
                      <span className="relative mt-1.5 size-[11px] shrink-0 rounded-full border-2 border-primary/50 bg-card" />
                      <div className="flex min-w-0 flex-1 items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-sm">{e.summary}</p>
                          <p className="mt-0.5 text-[11px] text-muted-foreground">
                            {e.actor} · {relativeTime(e.created_at)}
                          </p>
                        </div>
                        {e.undo_json && (
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={undo.isPending}
                            onClick={() => undo.mutate(e)}
                          >
                            <Undo2 /> Deshacer
                          </Button>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      <ProjectDialog open={editOpen} onOpenChange={setEditOpen} project={p} />
      <TaskDialog
        open={taskDialog.open}
        onOpenChange={(open) => setTaskDialog((d) => ({ ...d, open }))}
        task={taskDialog.task}
        defaultProjectId={p.id}
      />
      <NoteDialog open={noteDialogOpen} onOpenChange={setNoteDialogOpen} projectId={p.id} />
      {deleteTaskDialog}

      <Dialog open={deleteOpen} onOpenChange={setDeleteOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Eliminar proyecto</DialogTitle>
            <DialogDescription>
              ¿Seguro que quieres eliminar «{p.name}»? Podrás deshacerlo desde la actividad reciente.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteOpen(false)} disabled={removeProject.isPending}>
              Cancelar
            </Button>
            <Button variant="destructive" disabled={removeProject.isPending} onClick={() => removeProject.mutate()}>
              {removeProject.isPending ? "Eliminando…" : "Eliminar"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function NoteDialog({
  open,
  onOpenChange,
  projectId,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectId: string;
}) {
  const qc = useQueryClient();
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");

  const create = useMutation<Note, Error, CreateNote>({
    mutationFn: (data) => invoke<Note>("notes.create", { data }),
    onSuccess: () => {
      toast.success("Nota creada");
      setTitle("");
      setContent("");
      qc.invalidateQueries({ queryKey: ["notes"] });
      qc.invalidateQueries({ queryKey: ["project", projectId] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      onOpenChange(false);
    },
    onError: (err) => toast.error("No se pudo crear la nota", { description: err.message }),
  });

  const submit = () => {
    const t = title.trim();
    if (!t) {
      toast.error("El título es obligatorio");
      return;
    }
    create.mutate({ title: t, content, project_id: projectId });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Nueva nota</DialogTitle>
          <DialogDescription>Se vinculará a este proyecto.</DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="note-title">Título</Label>
            <Input
              id="note-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Título de la nota"
              autoFocus
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="note-content">Contenido</Label>
            <Textarea
              id="note-content"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="Escribe en Markdown…"
              rows={6}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={create.isPending}>
            Cancelar
          </Button>
          <Button onClick={submit} disabled={create.isPending || !title.trim()}>
            {create.isPending ? "Creando…" : "Crear nota"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
