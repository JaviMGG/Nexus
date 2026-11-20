import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { invoke } from "@/lib/api";
import type { Id, Task, TaskDetail, TaskStatus } from "@/lib/types";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface ToggleVars {
  task: Task;
  status: TaskStatus;
}

interface ToggleContext {
  snapshots: [readonly unknown[], unknown][];
}

export function useToggleTask() {
  const qc = useQueryClient();
  const mutation = useMutation<Task, Error, ToggleVars, ToggleContext>({
    mutationFn: ({ task, status }) => invoke<Task>("tasks.update", { id: task.id, data: { status } }),
    onMutate: async ({ task, status }) => {
      await qc.cancelQueries({ queryKey: ["tasks"] });
      await qc.cancelQueries({ queryKey: ["task", task.id] });
      const snapshots = [
        ...qc.getQueriesData({ queryKey: ["tasks"] }),
        ...qc.getQueriesData({ queryKey: ["task", task.id] }),
      ];
      const patch = (t: Task): Task =>
        t.id === task.id
          ? { ...t, status, completed_at: status === "done" ? new Date().toISOString() : null }
          : t;
      qc.setQueriesData({ queryKey: ["tasks"] }, (old: unknown) =>
        Array.isArray(old) ? (old as Task[]).map(patch) : old,
      );
      qc.setQueryData(["task", task.id], (old: TaskDetail | undefined) =>
        old ? { ...old, task: patch(old.task) } : old,
      );
      return { snapshots };
    },
    onError: (err, _vars, ctx) => {
      ctx?.snapshots.forEach(([key, data]) => qc.setQueryData(key, data));
      toast.error("No se pudo actualizar la tarea", { description: err.message });
    },
    onSuccess: (updated) => {
      toast.success(updated.status === "done" ? "Tarea completada" : "Tarea actualizada");
    },
    onSettled: (_data, _err, vars) => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["task", vars.task.id] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
      qc.invalidateQueries({ queryKey: ["project"] });
    },
  });
  const toggle = (task: Task, status?: TaskStatus) =>
    mutation.mutate({ task, status: status ?? (task.status === "done" ? "todo" : "done") });
  return { toggle, isPending: mutation.isPending };
}

export function useDeleteTask() {
  const qc = useQueryClient();
  const mutation = useMutation<void, Error, Id>({
    mutationFn: (id) => invoke<void>("tasks.delete", { id }),
    onSuccess: () => {
      toast.success("Tarea eliminada");
      ["tasks", "task", "dashboard", "projects", "project"].forEach((k) =>
        qc.invalidateQueries({ queryKey: [k] }),
      );
    },
    onError: (err) => toast.error("No se pudo eliminar la tarea", { description: err.message }),
  });
  const [target, setTarget] = useState<Task | null>(null);
  const dialog = (
    <ConfirmDeleteTaskDialog
      task={target}
      pending={mutation.isPending}
      onCancel={() => setTarget(null)}
      onConfirm={() => {
        if (target) mutation.mutate(target.id, { onSettled: () => setTarget(null) });
      }}
    />
  );
  return { requestDelete: setTarget, dialog };
}

function ConfirmDeleteTaskDialog({
  task,
  pending,
  onCancel,
  onConfirm,
}: {
  task: Task | null;
  pending: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={task !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Eliminar tarea</DialogTitle>
          <DialogDescription>
            ¿Seguro que quieres eliminar «{task?.title}»? Podrás deshacerlo desde la actividad reciente.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={pending}>
            Cancelar
          </Button>
          <Button variant="destructive" onClick={onConfirm} disabled={pending}>
            {pending ? "Eliminando…" : "Eliminar"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
