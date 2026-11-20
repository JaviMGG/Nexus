import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Calendar, KanbanSquare, List, ListTodo, Plus, Search } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id, Priority, Project, Task, TaskFilter, TaskStatus } from "@/lib/types";
import { PRIORITY_LABEL, STATUS_LABEL, cn, formatDate, isOverdue } from "@/lib/utils";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import { useToggleTask, useDeleteTask } from "@/hooks/useTaskMutations";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { TaskRow } from "@/components/tasks/TaskRow";
import { TaskDialog } from "@/components/tasks/TaskDialog";
import { TaskCheckbox } from "@/components/tasks/TaskCheckbox";
import { PriorityBadge } from "@/components/tasks/StatusBadges";
import { QueryError } from "@/components/tasks/QueryError";

const ALL = "__all__";
const KANBAN_COLUMNS: TaskStatus[] = ["todo", "in_progress", "done"];

export function TasksPage() {
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search, 300);
  const [status, setStatus] = useState<string>(ALL);
  const [priority, setPriority] = useState<string>(ALL);
  const [projectId, setProjectId] = useState<string>(ALL);
  const [view, setView] = useState<"list" | "kanban">("list");
  const [dialog, setDialog] = useState<{ open: boolean; task: Task | null }>({ open: false, task: null });
  const { requestDelete, dialog: deleteDialog } = useDeleteTask();

  const filter = useMemo<TaskFilter>(() => {
    const f: TaskFilter = {};
    if (debouncedSearch.trim()) f.search = debouncedSearch.trim();
    if (status !== ALL) f.status = status as TaskStatus;
    if (priority !== ALL) f.priority = priority as Priority;
    if (projectId !== ALL) f.project_id = projectId;
    if (view === "list") f.root_only = true;
    return f;
  }, [debouncedSearch, status, priority, projectId, view]);

  const tasks = useQuery({
    queryKey: ["tasks", filter],
    queryFn: () => invoke<Task[]>("tasks.list", { filter }),
  });
  const projects = useQuery({
    queryKey: ["projects", { include_archived: true }],
    queryFn: () => invoke<Project[]>("projects.list", { include_archived: true }),
  });

  const hasFilters = debouncedSearch.trim() !== "" || status !== ALL || priority !== ALL || projectId !== ALL;

  const openEdit = (task: Task) => setDialog({ open: true, task });
  const openCreate = () => setDialog({ open: true, task: null });

  return (
    <div className={cn("mx-auto p-6 animate-fade-in", view === "kanban" ? "max-w-full" : "max-w-[1200px]")}>
      <header className="mb-5 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Tareas</h1>
          <p className="mt-1 text-sm text-muted-foreground">Organiza tu trabajo, una tarea a la vez.</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex rounded-lg bg-muted p-1">
            <button
              type="button"
              aria-label="Vista lista"
              onClick={() => setView("list")}
              className={cn(
                "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-all cursor-pointer",
                view === "list" ? "bg-card text-foreground shadow" : "text-muted-foreground hover:text-foreground",
              )}
            >
              <List className="size-3.5" /> Lista
            </button>
            <button
              type="button"
              aria-label="Vista kanban"
              onClick={() => setView("kanban")}
              className={cn(
                "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-all cursor-pointer",
                view === "kanban" ? "bg-card text-foreground shadow" : "text-muted-foreground hover:text-foreground",
              )}
            >
              <KanbanSquare className="size-3.5" /> Kanban
            </button>
          </div>
          <Button onClick={openCreate}>
            <Plus /> Nueva tarea
          </Button>
        </div>
      </header>

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <div className="relative min-w-52 flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Buscar tareas…"
            className="pl-8"
          />
        </div>
        {view === "list" && (
          <Select value={status} onValueChange={setStatus}>
            <SelectTrigger className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL}>Todos los estados</SelectItem>
              {(Object.keys(STATUS_LABEL) as TaskStatus[]).map((s) => (
                <SelectItem key={s} value={s}>
                  {STATUS_LABEL[s]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        <Select value={priority} onValueChange={setPriority}>
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Toda prioridad</SelectItem>
            {(Object.keys(PRIORITY_LABEL) as Priority[]).map((p) => (
              <SelectItem key={p} value={p}>
                {PRIORITY_LABEL[p]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={projectId} onValueChange={setProjectId}>
          <SelectTrigger className="w-44">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Todos los proyectos</SelectItem>
            {(projects.data ?? []).map((p) => (
              <SelectItem key={p.id} value={p.id}>
                {p.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {tasks.isLoading ? (
        view === "list" ? (
          <div className="space-y-2">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-11 rounded-lg" />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-80 rounded-xl" />
            ))}
          </div>
        )
      ) : tasks.isError ? (
        <QueryError error={tasks.error} onRetry={() => tasks.refetch()} />
      ) : view === "list" ? (
        (tasks.data ?? []).length === 0 ? (
          <EmptyState
            icon={ListTodo}
            title={hasFilters ? "Sin resultados" : "No hay tareas"}
            description={
              hasFilters
                ? "Prueba a ajustar la búsqueda o los filtros."
                : "Crea tu primera tarea y empieza a organizarte."
            }
            action={
              !hasFilters ? (
                <Button onClick={openCreate}>
                  <Plus /> Nueva tarea
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="rounded-xl border border-border bg-card p-1.5 shadow-sm">
            {tasks.data!.map((t) => (
              <TaskRow
                key={t.id}
                task={t}
                projects={projects.data}
                onEdit={openEdit}
                onDelete={requestDelete}
              />
            ))}
          </div>
        )
      ) : (
        <KanbanBoard
          tasks={tasks.data ?? []}
          projects={projects.data}
          hasFilters={hasFilters}
          onEdit={openEdit}
          onCreate={openCreate}
        />
      )}

      <TaskDialog
        open={dialog.open}
        onOpenChange={(open) => setDialog((d) => ({ ...d, open }))}
        task={dialog.task}
      />
      {deleteDialog}
    </div>
  );
}

function KanbanBoard({
  tasks,
  projects,
  hasFilters,
  onEdit,
  onCreate,
}: {
  tasks: Task[];
  projects?: Project[];
  hasFilters: boolean;
  onEdit: (task: Task) => void;
  onCreate: () => void;
}) {
  const qc = useQueryClient();
  const { toggle } = useToggleTask();
  const [dragOver, setDragOver] = useState<TaskStatus | null>(null);

  const reorder = useMutation<Task, Error, { id: Id; status: TaskStatus; position: number }>({
    mutationFn: (vars) => invoke<Task>("tasks.reorder", vars),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (err) => toast.error("No se pudo mover la tarea", { description: err.message }),
  });

  const visible = tasks.filter((t) => t.status !== "cancelled");

  if (visible.length === 0) {
    return (
      <EmptyState
        icon={ListTodo}
        title={hasFilters ? "Sin resultados" : "No hay tareas"}
        description={
          hasFilters ? "Prueba a ajustar la búsqueda o los filtros." : "Crea tu primera tarea para verla en el tablero."
        }
        action={
          !hasFilters ? (
            <Button onClick={onCreate}>
              <Plus /> Nueva tarea
            </Button>
          ) : undefined
        }
      />
    );
  }

  const handleDrop = (e: React.DragEvent, status: TaskStatus) => {
    e.preventDefault();
    setDragOver(null);
    const id = e.dataTransfer.getData("text/plain");
    if (!id) return;
    const task = tasks.find((t) => t.id === id);
    if (!task || task.status === status) return;
    const maxPosition = Math.max(0, ...tasks.filter((t) => t.status === status).map((t) => t.position));
    reorder.mutate({ id, status, position: (maxPosition + 1) * 1000 });
  };

  return (
    <div className="grid grid-cols-1 items-start gap-4 md:grid-cols-3">
      {KANBAN_COLUMNS.map((status) => {
        const columnTasks = visible
          .filter((t) => t.status === status)
          .sort((a, b) => a.position - b.position);
        return (
          <div
            key={status}
            onDragOver={(e) => {
              e.preventDefault();
              setDragOver(status);
            }}
            onDragLeave={() => setDragOver((s) => (s === status ? null : s))}
            onDrop={(e) => handleDrop(e, status)}
            className={cn(
              "rounded-xl border border-border bg-muted/40 p-3 transition-all",
              dragOver === status && "ring-2 ring-ring border-transparent",
            )}
          >
            <div className="mb-3 flex items-center justify-between px-1">
              <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {STATUS_LABEL[status]}
              </span>
              <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] tabular-nums text-muted-foreground">
                {columnTasks.length}
              </span>
            </div>
            <div className="space-y-2 min-h-16">
              {columnTasks.map((t) => (
                <KanbanCard key={t.id} task={t} projects={projects} onEdit={onEdit} onToggle={() => toggle(t)} />
              ))}
              {columnTasks.length === 0 && (
                <p className="rounded-lg border border-dashed border-border px-3 py-6 text-center text-xs text-muted-foreground">
                  Arrastra tareas aquí
                </p>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function KanbanCard({
  task,
  projects,
  onEdit,
  onToggle,
}: {
  task: Task;
  projects?: Project[];
  onEdit: (task: Task) => void;
  onToggle: () => void;
}) {
  const project = projects?.find((p) => p.id === task.project_id);
  const overdue = isOverdue(task.due_date, task.status);
  return (
    <div
      draggable
      onDragStart={(e) => e.dataTransfer.setData("text/plain", task.id)}
      onClick={() => onEdit(task)}
      className="cursor-grab rounded-lg border border-border bg-card p-3 shadow-sm transition-all hover:border-ring/50 hover:shadow active:cursor-grabbing animate-fade-in"
    >
      <div className="flex items-start gap-2.5">
        <TaskCheckbox checked={task.status === "done"} onChange={onToggle} className="mt-0.5" />
        <span
          className={cn(
            "min-w-0 flex-1 text-sm leading-snug",
            task.status === "done" && "line-through text-muted-foreground",
          )}
        >
          {task.title}
        </span>
      </div>
      {(task.due_date || project || task.priority !== "low" || task.tags.length > 0) && (
        <div className="mt-2.5 flex flex-wrap items-center gap-1.5 pl-7">
          {task.priority !== "low" && <PriorityBadge priority={task.priority} />}
          {task.due_date && (
            <span
              className={cn(
                "flex items-center gap-1 text-[11px]",
                overdue ? "font-medium text-destructive" : "text-muted-foreground",
              )}
            >
              <Calendar className="size-3" />
              {formatDate(task.due_date)}
            </span>
          )}
          {project && <Badge variant="outline">{project.name}</Badge>}
          {task.tags.slice(0, 2).map((t) => (
            <Badge key={t} variant="secondary">
              {t}
            </Badge>
          ))}
        </div>
      )}
    </div>
  );
}
