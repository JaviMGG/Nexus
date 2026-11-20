import { CheckSquare } from "lucide-react";
import type { CalendarEvent, Task } from "@/lib/types";
import { cn } from "@/lib/utils";
import { EVENT_CHIP_CLASS } from "./calendar-utils";

export function EventChip({ event, onClick }: { event: CalendarEvent; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      title={event.title}
      className={cn(
        "flex w-full items-center gap-1 truncate rounded border px-1.5 py-0.5 text-left text-[11px] font-medium leading-4 transition-all hover:brightness-110 cursor-pointer animate-fade-in",
        EVENT_CHIP_CLASS[event.color],
      )}
    >
      {!event.all_day && event.start_time && (
        <span className="shrink-0 tabular-nums opacity-80">{event.start_time}</span>
      )}
      <span className="truncate">{event.title}</span>
    </button>
  );
}

export function TaskChip({ task, onClick }: { task: Task; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      title={`Tarea: ${task.title}`}
      className="flex w-full items-center gap-1 truncate rounded border border-dashed border-border px-1.5 py-0.5 text-left text-[11px] leading-4 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground cursor-pointer animate-fade-in"
    >
      <CheckSquare className="size-3 shrink-0" />
      <span className="truncate">{task.title}</span>
    </button>
  );
}
