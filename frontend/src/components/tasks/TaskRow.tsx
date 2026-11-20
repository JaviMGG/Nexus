import { useState } from "react";
import { Calendar, ChevronRight, Folder, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import type { Project, Task } from "@/lib/types";
import { cn, formatDate, isOverdue } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { TaskCheckbox } from "./TaskCheckbox";
import { PriorityBadge } from "./StatusBadges";
import { SubtaskList } from "./SubtaskList";
import { useToggleTask } from "@/hooks/useTaskMutations";

export function TaskRow({
  task,
  projects,
  onEdit,
  onDelete,
  expandable = true,
}: {
  task: Task;
  projects?: Project[];
  onEdit: (task: Task) => void;
  onDelete: (task: Task) => void;
  expandable?: boolean;
}) {
  const { toggle } = useToggleTask();
  const [expanded, setExpanded] = useState(false);
  const done = task.status === "done";
  const overdue = isOverdue(task.due_date, task.status);
  const project = projects?.find((p) => p.id === task.project_id);
  const canExpand = expandable && !task.parent_id;

  return (
    <div className="group animate-fade-in">
      <div className="flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors hover:bg-accent/60">
        {canExpand ? (
          <button
            type="button"
            aria-label={expanded ? "Contraer subtareas" : "Expandir subtareas"}
            onClick={() => setExpanded((v) => !v)}
            className="flex size-4 shrink-0 items-center justify-center text-muted-foreground/60 transition-transform hover:text-foreground cursor-pointer"
          >
            <ChevronRight className={cn("size-3.5 transition-transform duration-150", expanded && "rotate-90")} />
          </button>
        ) : (
          <span className="w-4 shrink-0" />
        )}
        <TaskCheckbox checked={done} onChange={() => toggle(task)} />
        <button
          type="button"
          onClick={() => onEdit(task)}
          className="min-w-0 flex-1 text-left cursor-pointer"
        >
          <span
            className={cn(
              "block truncate text-sm transition-colors",
              done ? "line-through text-muted-foreground" : "text-foreground",
              task.status === "cancelled" && "line-through text-muted-foreground/70",
            )}
          >
            {task.title}
          </span>
        </button>
        <div className="flex shrink-0 items-center gap-2">
          {task.tags.slice(0, 3).map((t) => (
            <Badge key={t} variant="outline" className="hidden sm:inline-flex">
              {t}
            </Badge>
          ))}
          {project && (
            <span className="hidden items-center gap-1 text-[11px] text-muted-foreground md:flex">
              <Folder className="size-3" />
              {project.name}
            </span>
          )}
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
          {task.priority !== "low" && <PriorityBadge priority={task.priority} />}
          {task.status === "in_progress" && <Badge variant="warning">En curso</Badge>}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                className="opacity-0 transition-opacity group-hover:opacity-100 data-[state=open]:opacity-100"
                aria-label="Acciones de la tarea"
              >
                <MoreHorizontal />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={() => onEdit(task)}>
                <Pencil /> Editar
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                className="text-destructive focus:text-destructive"
                onClick={() => onDelete(task)}
              >
                <Trash2 /> Eliminar
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
      {expanded && canExpand && (
        <div className="ml-10 border-l border-border pl-4 pr-3 pb-2 animate-fade-in">
          <SubtaskList parentId={task.id} projectId={task.project_id} />
        </div>
      )}
    </div>
  );
}
