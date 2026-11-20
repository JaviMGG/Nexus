import { useNavigate } from "react-router-dom";
import { CalendarDays, CheckSquare, MapPin, Plus } from "lucide-react";
import type { CalendarEvent, Task } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { EVENT_DOT_CLASS } from "./calendar-utils";

interface DayDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  dateKey: string | null;
  events: CalendarEvent[];
  tasks: Task[];
  onAddEvent: (dateKey: string) => void;
  onEditEvent: (event: CalendarEvent) => void;
}

export function DayDialog({
  open,
  onOpenChange,
  dateKey,
  events,
  tasks,
  onAddEvent,
  onEditEvent,
}: DayDialogProps) {
  const navigate = useNavigate();
  const label = dateKey
    ? new Date(dateKey + "T00:00:00").toLocaleDateString("es-ES", {
        weekday: "long",
        day: "numeric",
        month: "long",
      })
    : "";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="capitalize">{label}</DialogTitle>
          <DialogDescription>
            {events.length + tasks.length === 0
              ? "Nada planificado para este día."
              : `${events.length} evento${events.length === 1 ? "" : "s"} · ${tasks.length} tarea${tasks.length === 1 ? "" : "s"}`}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-1.5 max-h-[50vh] overflow-y-auto">
          {events.map((e) => (
            <button
              key={e.id}
              type="button"
              onClick={() => onEditEvent(e)}
              className="flex w-full items-start gap-2.5 rounded-lg border border-border px-3 py-2 text-left transition-colors hover:bg-accent cursor-pointer animate-fade-in"
            >
              <span className={cn("mt-1 size-2.5 shrink-0 rounded-full", EVENT_DOT_CLASS[e.color])} />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">{e.title}</span>
                <span className="mt-0.5 flex items-center gap-2 text-xs text-muted-foreground">
                  <span>{e.all_day ? "Todo el día" : [e.start_time, e.end_time].filter(Boolean).join(" – ") || "Sin hora"}</span>
                  {e.location && (
                    <span className="flex min-w-0 items-center gap-1">
                      <MapPin className="size-3 shrink-0" />
                      <span className="truncate">{e.location}</span>
                    </span>
                  )}
                </span>
              </span>
            </button>
          ))}
          {tasks.map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => {
                onOpenChange(false);
                navigate("/tasks");
              }}
              className="flex w-full items-start gap-2.5 rounded-lg border border-dashed border-border px-3 py-2 text-left transition-colors hover:bg-accent cursor-pointer animate-fade-in"
            >
              <CheckSquare className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm">{t.title}</span>
                <span className="mt-0.5 block text-xs text-muted-foreground">Tarea con fecha límite</span>
              </span>
            </button>
          ))}
          {events.length + tasks.length === 0 && (
            <div className="flex flex-col items-center gap-2 py-6 text-muted-foreground">
              <CalendarDays className="size-6" />
              <p className="text-xs">Aprovecha para planificar algo.</p>
            </div>
          )}
        </div>
        {dateKey && (
          <Button
            variant="outline"
            onClick={() => {
              onOpenChange(false);
              onAddEvent(dateKey);
            }}
          >
            <Plus /> Añadir evento
          </Button>
        )}
      </DialogContent>
    </Dialog>
  );
}
