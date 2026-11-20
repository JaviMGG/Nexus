import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { CalendarDays, CheckSquare, ChevronLeft, ChevronRight, Plus } from "lucide-react";
import { invoke } from "@/lib/api";
import type { CalendarEvent, EventFilter, Task } from "@/lib/types";
import { cn, todayStr } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { QueryError } from "@/components/tasks/QueryError";
import { EventChip, TaskChip } from "@/components/calendar/EventChip";
import { EventDialog } from "@/components/calendar/EventDialog";
import { DayDialog } from "@/components/calendar/DayDialog";
import { EVENT_DOT_CLASS, dateKey, monthGrid } from "@/components/calendar/calendar-utils";

const WEEKDAYS = ["lu", "ma", "mi", "ju", "vi", "sá", "do"];
const MAX_CHIPS = 3;

export function CalendarPage() {
  const navigate = useNavigate();
  const now = new Date();
  const [cursor, setCursor] = useState({ year: now.getFullYear(), month: now.getMonth() });
  const [showTasks, setShowTasks] = useState(true);
  const [eventDialog, setEventDialog] = useState<{
    open: boolean;
    event: CalendarEvent | null;
    date: string | null;
  }>({ open: false, event: null, date: null });
  const [dayDialog, setDayDialog] = useState<string | null>(null);

  const days = useMemo(() => monthGrid(cursor.year, cursor.month), [cursor]);
  const from = days[0].key;
  const to = days[days.length - 1].key;

  const filter = useMemo<EventFilter>(() => ({ from, to }), [from, to]);
  const events = useQuery({
    queryKey: ["events", filter],
    queryFn: () => invoke<CalendarEvent[]>("events.list", { filter }),
  });
  const tasks = useQuery({
    queryKey: ["tasks", {}],
    queryFn: () => invoke<Task[]>("tasks.list", { filter: {} }),
    enabled: showTasks,
  });

  const eventsByDay = useMemo(() => {
    const map = new Map<string, CalendarEvent[]>();
    for (const e of events.data ?? []) {
      const list = map.get(e.date) ?? [];
      list.push(e);
      map.set(e.date, list);
    }
    return map;
  }, [events.data]);

  const tasksByDay = useMemo(() => {
    const map = new Map<string, Task[]>();
    if (!showTasks) return map;
    for (const t of tasks.data ?? []) {
      if (!t.due_date || t.status === "done" || t.status === "cancelled") continue;
      const key = t.due_date.slice(0, 10);
      if (key < from || key > to) continue;
      const list = map.get(key) ?? [];
      list.push(t);
      map.set(key, list);
    }
    return map;
  }, [tasks.data, showTasks, from, to]);

  const monthLabel = new Date(cursor.year, cursor.month, 1).toLocaleDateString("es-ES", {
    month: "long",
    year: "numeric",
  });

  const move = (delta: number) =>
    setCursor(({ year, month }) => {
      const d = new Date(year, month + delta, 1);
      return { year: d.getFullYear(), month: d.getMonth() };
    });

  const goToday = () => {
    const d = new Date();
    setCursor({ year: d.getFullYear(), month: d.getMonth() });
  };

  const openCreate = (date: string | null) => setEventDialog({ open: true, event: null, date });
  const openEdit = (event: CalendarEvent) => setEventDialog({ open: true, event, date: null });

  const monthEmpty =
    !events.isLoading &&
    !events.isError &&
    days.every((d) => !d.inMonth || ((eventsByDay.get(d.key)?.length ?? 0) + (tasksByDay.get(d.key)?.length ?? 0)) === 0);

  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <header className="mb-5 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Calendario</h1>
          <p className="mt-1 text-sm text-muted-foreground">Planifica tus eventos y visualiza tus fechas límite.</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            <Switch id="cal-tasks" checked={showTasks} onCheckedChange={setShowTasks} />
            <Label htmlFor="cal-tasks" className="flex cursor-pointer items-center gap-1.5 text-xs">
              <CheckSquare className="size-3.5" /> Tareas
            </Label>
          </div>
          <Button onClick={() => openCreate(todayStr())}>
            <Plus /> Nuevo evento
          </Button>
        </div>
      </header>

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1">
          <Button variant="outline" size="icon-sm" aria-label="Mes anterior" onClick={() => move(-1)}>
            <ChevronLeft />
          </Button>
          <Button variant="outline" size="icon-sm" aria-label="Mes siguiente" onClick={() => move(1)}>
            <ChevronRight />
          </Button>
        </div>
        <Button variant="outline" size="sm" onClick={goToday}>
          Hoy
        </Button>
        <h2 className="ml-1 text-sm font-semibold capitalize">{monthLabel}</h2>
        {monthEmpty && (
          <span className="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground animate-fade-in">
            <CalendarDays className="size-3.5" />
            Sin eventos este mes. Haz clic en un día para crear uno.
          </span>
        )}
      </div>

      {events.isError ? (
        <QueryError error={events.error} onRetry={() => events.refetch()} />
      ) : (
        <div className="overflow-x-auto rounded-xl border border-border bg-card shadow-sm">
          <div className="min-w-[640px]">
            <div className="grid grid-cols-7 border-b border-border">
              {WEEKDAYS.map((w) => (
                <div
                  key={w}
                  className="px-2 py-2 text-center text-[11px] font-semibold uppercase tracking-wide text-muted-foreground"
                >
                  {w}
                </div>
              ))}
            </div>
            {events.isLoading ? (
              <div className="grid grid-cols-7">
                {Array.from({ length: 35 }).map((_, i) => (
                  <div key={i} className="min-h-[110px] border-b border-r border-border p-2 last:border-r-0">
                    <Skeleton className="mb-2 size-6 rounded-full" />
                    <Skeleton className="mb-1 h-4 w-full rounded" />
                    <Skeleton className="h-4 w-2/3 rounded" />
                  </div>
                ))}
              </div>
            ) : (
              <div className="grid grid-cols-7">
                {days.map((day) => {
                  const dayEvents = eventsByDay.get(day.key) ?? [];
                  const dayTasks = tasksByDay.get(day.key) ?? [];
                  const items: Array<{ kind: "event"; event: CalendarEvent } | { kind: "task"; task: Task }> = [
                    ...dayEvents.map((event) => ({ kind: "event" as const, event })),
                    ...dayTasks.map((task) => ({ kind: "task" as const, task })),
                  ];
                  const visible = items.slice(0, MAX_CHIPS);
                  const overflow = items.length - visible.length;
                  return (
                    <div
                      key={day.key}
                      onClick={() => openCreate(day.key)}
                      className={cn(
                        "group min-h-[110px] cursor-pointer border-b border-r border-border p-1.5 transition-colors hover:bg-accent/40 [&:nth-child(7n)]:border-r-0",
                        !day.inMonth && "bg-muted/30",
                      )}
                    >
                      <div className="mb-1 flex items-center justify-between">
                        <button
                          type="button"
                          aria-label={`Ver el ${day.key}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            setDayDialog(day.key);
                          }}
                          className={cn(
                            "flex size-6 items-center justify-center rounded-full text-xs tabular-nums transition-colors cursor-pointer",
                            day.isToday
                              ? "bg-primary font-semibold text-primary-foreground ring-2 ring-ring/40"
                              : day.inMonth
                                ? "text-foreground hover:bg-accent"
                                : "text-muted-foreground/60 hover:bg-accent",
                          )}
                        >
                          {day.date.getDate()}
                        </button>
                        <button
                          type="button"
                          aria-label={`Añadir evento el ${day.key}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            openCreate(day.key);
                          }}
                          className="rounded p-0.5 text-muted-foreground opacity-0 transition-opacity hover:bg-accent hover:text-foreground group-hover:opacity-100 cursor-pointer"
                        >
                          <Plus className="size-3.5" />
                        </button>
                      </div>
                      <div className="space-y-1">
                        <div className="hidden space-y-1 sm:block">
                          {visible.map((item) =>
                            item.kind === "event" ? (
                              <EventChip
                                key={item.event.id}
                                event={item.event}
                                onClick={() => openEdit(item.event)}
                              />
                            ) : (
                              <TaskChip
                                key={item.task.id}
                                task={item.task}
                                onClick={() => navigate("/tasks")}
                              />
                            ),
                          )}
                          {overflow > 0 && (
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                setDayDialog(day.key);
                              }}
                              className="w-full rounded px-1.5 py-0.5 text-left text-[11px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground cursor-pointer"
                            >
                              +{overflow} más
                            </button>
                          )}
                        </div>
                        {items.length > 0 && (
                          <div className="flex flex-wrap gap-1 px-0.5 sm:hidden">
                            {items.slice(0, 6).map((item) =>
                              item.kind === "event" ? (
                                <span
                                  key={item.event.id}
                                  className={cn("size-2 rounded-full", EVENT_DOT_CLASS[item.event.color])}
                                />
                              ) : (
                                <span key={item.task.id} className="size-2 rounded-full border border-muted-foreground/60" />
                              ),
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}

      <EventDialog
        open={eventDialog.open}
        onOpenChange={(open) => setEventDialog((d) => ({ ...d, open }))}
        event={eventDialog.event}
        defaultDate={eventDialog.date ?? undefined}
      />
      <DayDialog
        open={dayDialog !== null}
        onOpenChange={(open) => {
          if (!open) setDayDialog(null);
        }}
        dateKey={dayDialog}
        events={dayDialog ? (eventsByDay.get(dayDialog) ?? []) : []}
        tasks={dayDialog ? (tasksByDay.get(dayDialog) ?? []) : []}
        onAddEvent={(date) => openCreate(date)}
        onEditEvent={(event) => {
          setDayDialog(null);
          openEdit(event);
        }}
      />
    </div>
  );
}
