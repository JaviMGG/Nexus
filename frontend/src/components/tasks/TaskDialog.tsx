import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { invoke } from "@/lib/api";
import type { CreateTask, Id, Priority, Project, Task, TaskStatus, UpdateTask } from "@/lib/types";
import { PRIORITY_LABEL, STATUS_LABEL } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SubtaskList } from "./SubtaskList";

const NONE = "__none__";
const STATUSES: TaskStatus[] = ["todo", "in_progress", "done", "cancelled"];
const PRIORITIES: Priority[] = ["low", "medium", "high", "urgent"];

interface TaskDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  task?: Task | null;
  defaultProjectId?: Id | null;
  defaultParentId?: Id | null;
}

export function TaskDialog({ open, onOpenChange, task, defaultProjectId, defaultParentId }: TaskDialogProps) {
  const isEdit = Boolean(task);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{isEdit ? "Editar tarea" : "Nueva tarea"}</DialogTitle>
          <DialogDescription>
            {isEdit ? "Modifica los detalles de la tarea." : "Añade una tarea a tu centro de control."}
          </DialogDescription>
        </DialogHeader>
        {open && (
          <TaskForm
            key={task?.id ?? "new"}
            task={task}
            defaultProjectId={defaultProjectId}
            defaultParentId={defaultParentId}
            onDone={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function TaskForm({
  task,
  defaultProjectId,
  defaultParentId,
  onDone,
}: {
  task?: Task | null;
  defaultProjectId?: Id | null;
  defaultParentId?: Id | null;
  onDone: () => void;
}) {
  const qc = useQueryClient();
  const isEdit = Boolean(task);
  const [title, setTitle] = useState(task?.title ?? "");
  const [description, setDescription] = useState(task?.description ?? "");
  const [status, setStatus] = useState<TaskStatus>(task?.status ?? "todo");
  const [priority, setPriority] = useState<Priority>(task?.priority ?? "medium");
  const [due, setDue] = useState(task?.due_date ? task.due_date.slice(0, 10) : "");
  const [projectId, setProjectId] = useState<string>(task?.project_id ?? defaultProjectId ?? NONE);
  const [category, setCategory] = useState(task?.category ?? "");
  const [tags, setTags] = useState(task?.tags.join(", ") ?? "");

  const projects = useQuery({
    queryKey: ["projects", { include_archived: false }],
    queryFn: () => invoke<Project[]>("projects.list", { include_archived: false }),
  });

  const invalidate = () => {
    ["tasks", "dashboard", "projects", "project"].forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
    if (task) qc.invalidateQueries({ queryKey: ["task", task.id] });
    if (task?.parent_id) qc.invalidateQueries({ queryKey: ["task", task.parent_id] });
  };

  const create = useMutation<Task, Error, CreateTask>({
    mutationFn: (data) => invoke<Task>("tasks.create", { data }),
    onSuccess: () => {
      toast.success("Tarea creada");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo crear la tarea", { description: err.message }),
  });

  const update = useMutation<Task, Error, UpdateTask>({
    mutationFn: (data) => invoke<Task>("tasks.update", { id: task!.id, data }),
    onSuccess: () => {
      toast.success("Tarea guardada");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo guardar la tarea", { description: err.message }),
  });

  const pending = create.isPending || update.isPending;

  const submit = () => {
    const t = title.trim();
    if (!t) {
      toast.error("El título es obligatorio");
      return;
    }
    const tagList = tags
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const pid = projectId === NONE ? null : projectId;
    if (isEdit) {
      update.mutate({
        title: t,
        description,
        status,
        priority,
        due_date: due || null,
        project_id: pid,
        category: category.trim() || null,
        tags: tagList,
      });
    } else {
      const data: CreateTask = {
        title: t,
        description,
        status,
        priority,
        due_date: due || null,
        project_id: pid,
        category: category.trim() || null,
        tags: tagList,
      };
      if (defaultParentId) data.parent_id = defaultParentId;
      create.mutate(data);
    }
  };

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <Label htmlFor="task-title">Título</Label>
        <Input
          id="task-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="¿Qué hay que hacer?"
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="task-desc">Descripción</Label>
        <Textarea
          id="task-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Detalles opcionales…"
          rows={3}
        />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label>Estado</Label>
          <Select value={status} onValueChange={(v) => setStatus(v as TaskStatus)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {STATUS_LABEL[s]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
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
        <div className="space-y-1.5">
          <Label htmlFor="task-due">Fecha límite</Label>
          <Input id="task-due" type="date" value={due} onChange={(e) => setDue(e.target.value)} />
        </div>
        <div className="space-y-1.5">
          <Label>Proyecto</Label>
          <Select value={projectId} onValueChange={setProjectId} disabled={projects.isLoading}>
            <SelectTrigger>
              <SelectValue placeholder="Sin proyecto" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NONE}>Sin proyecto</SelectItem>
              {(projects.data ?? []).map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="task-cat">Categoría</Label>
          <Input
            id="task-cat"
            value={category}
            onChange={(e) => setCategory(e.target.value)}
            placeholder="p. ej. Trabajo"
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="task-tags">Etiquetas (separadas por comas)</Label>
          <Input
            id="task-tags"
            value={tags}
            onChange={(e) => setTags(e.target.value)}
            placeholder="casa, urgente"
          />
        </div>
      </div>
      {isEdit && task && !task.parent_id && (
        <div className="space-y-1.5">
          <Label>Subtareas</Label>
          <div className="rounded-lg border border-border p-2">
            <SubtaskList parentId={task.id} projectId={task.project_id} />
          </div>
        </div>
      )}
      <DialogFooter>
        <Button variant="outline" onClick={onDone} disabled={pending}>
          Cancelar
        </Button>
        <Button onClick={submit} disabled={pending || !title.trim()}>
          {pending ? "Guardando…" : isEdit ? "Guardar cambios" : "Crear tarea"}
        </Button>
      </DialogFooter>
    </div>
  );
}
