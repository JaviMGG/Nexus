import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { invoke } from "@/lib/api";
import type { CreateProject, Priority, Project, ProjectStatus, UpdateProject } from "@/lib/types";
import { PRIORITY_LABEL, PROJECT_STATUS_LABEL } from "@/lib/utils";
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

const STATUSES: ProjectStatus[] = ["active", "paused", "completed", "archived"];
const PRIORITIES: Priority[] = ["low", "medium", "high", "urgent"];

export function ProjectDialog({
  open,
  onOpenChange,
  project,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  project?: Project | null;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{project ? "Editar proyecto" : "Nuevo proyecto"}</DialogTitle>
          <DialogDescription>
            {project ? "Actualiza los detalles del proyecto." : "Agrupa tareas y notas bajo un objetivo común."}
          </DialogDescription>
        </DialogHeader>
        {open && <ProjectForm key={project?.id ?? "new"} project={project} onDone={() => onOpenChange(false)} />}
      </DialogContent>
    </Dialog>
  );
}

function ProjectForm({ project, onDone }: { project?: Project | null; onDone: () => void }) {
  const qc = useQueryClient();
  const isEdit = Boolean(project);
  const [name, setName] = useState(project?.name ?? "");
  const [description, setDescription] = useState(project?.description ?? "");
  const [goal, setGoal] = useState(project?.goal ?? "");
  const [status, setStatus] = useState<ProjectStatus>(project?.status ?? "active");
  const [priority, setPriority] = useState<Priority>(project?.priority ?? "medium");

  const invalidate = () => {
    ["projects", "dashboard", "tasks"].forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
    if (project) qc.invalidateQueries({ queryKey: ["project", project.id] });
  };

  const create = useMutation<Project, Error, CreateProject>({
    mutationFn: (data) => invoke<Project>("projects.create", { data }),
    onSuccess: () => {
      toast.success("Proyecto creado");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo crear el proyecto", { description: err.message }),
  });

  const update = useMutation<Project, Error, UpdateProject>({
    mutationFn: (data) => invoke<Project>("projects.update", { id: project!.id, data }),
    onSuccess: () => {
      toast.success("Proyecto guardado");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo guardar el proyecto", { description: err.message }),
  });

  const pending = create.isPending || update.isPending;

  const submit = () => {
    const n = name.trim();
    if (!n) {
      toast.error("El nombre es obligatorio");
      return;
    }
    const payload = { name: n, description, goal, status, priority };
    if (isEdit) update.mutate(payload);
    else create.mutate(payload);
  };

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <Label htmlFor="proj-name">Nombre</Label>
        <Input
          id="proj-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Nombre del proyecto"
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
        <Label htmlFor="proj-desc">Descripción</Label>
        <Textarea
          id="proj-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="¿De qué trata este proyecto?"
          rows={2}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="proj-goal">Objetivo</Label>
        <Textarea
          id="proj-goal"
          value={goal}
          onChange={(e) => setGoal(e.target.value)}
          placeholder="¿Qué quieres conseguir?"
          rows={2}
        />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label>Estado</Label>
          <Select value={status} onValueChange={(v) => setStatus(v as ProjectStatus)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {STATUSES.map((s) => (
                <SelectItem key={s} value={s}>
                  {PROJECT_STATUS_LABEL[s]}
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
      </div>
      <DialogFooter>
        <Button variant="outline" onClick={onDone} disabled={pending}>
          Cancelar
        </Button>
        <Button onClick={submit} disabled={pending || !name.trim()}>
          {pending ? "Guardando…" : isEdit ? "Guardar cambios" : "Crear proyecto"}
        </Button>
      </DialogFooter>
    </div>
  );
}
