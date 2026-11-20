import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Plus } from "lucide-react";
import { invoke } from "@/lib/api";
import type { CreateTask, Id, Task } from "@/lib/types";
import { isOverdue, formatDate, cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { TaskCheckbox } from "./TaskCheckbox";
import { useToggleTask } from "@/hooks/useTaskMutations";

export function SubtaskList({ parentId, projectId }: { parentId: Id; projectId?: Id | null }) {
  const qc = useQueryClient();
  const { toggle } = useToggleTask();
  const [title, setTitle] = useState("");

  const query = useQuery({
    queryKey: ["tasks", { parent_id: parentId }],
    queryFn: () => invoke<Task[]>("tasks.list", { filter: { parent_id: parentId } }),
  });

  const create = useMutation<Task, Error, string>({
    mutationFn: (t) => {
      const data: CreateTask = { title: t, parent_id: parentId };
      if (projectId) data.project_id = projectId;
      return invoke<Task>("tasks.create", { data });
    },
    onSuccess: () => {
      setTitle("");
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: ["task", parentId] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      qc.invalidateQueries({ queryKey: ["project"] });
    },
    onError: (err) => toast.error("No se pudo crear la subtarea", { description: err.message }),
  });

  const submit = () => {
    const t = title.trim();
    if (t && !create.isPending) create.mutate(t);
  };

  if (query.isLoading) {
    return (
      <div className="space-y-2 py-1">
        <Skeleton className="h-5 w-3/4" />
        <Skeleton className="h-5 w-1/2" />
      </div>
    );
  }

  const subtasks = query.data ?? [];

  return (
    <div className="space-y-0.5">
      {subtasks.map((s) => (
        <div key={s.id} className="flex items-center gap-2.5 rounded-md px-1 py-1 hover:bg-accent/50 transition-colors">
          <TaskCheckbox checked={s.status === "done"} onChange={() => toggle(s)} />
          <span
            className={cn(
              "flex-1 truncate text-xs",
              s.status === "done" ? "line-through text-muted-foreground" : "text-foreground",
            )}
          >
            {s.title}
          </span>
          {s.due_date && (
            <span className={cn("text-[11px] text-muted-foreground", isOverdue(s.due_date, s.status) && "text-destructive")}>
              {formatDate(s.due_date)}
            </span>
          )}
        </div>
      ))}
      <div className="flex items-center gap-2.5 px-1 pt-1">
        <Plus className="size-3.5 shrink-0 text-muted-foreground" />
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
          placeholder="Añadir subtarea…"
          disabled={create.isPending}
          className="h-7 border-transparent bg-transparent px-1 text-xs shadow-none focus-visible:ring-1"
        />
      </div>
    </div>
  );
}
