import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  ArrowRight,
  Bot,
  Calendar,
  CalendarClock,
  CheckCircle2,
  CircleDot,
  Flag,
  FolderKanban,
  Inbox,
  ListTodo,
  NotebookPen,
  Sparkles,
  Zap,
} from "lucide-react";
import { invoke } from "@/lib/api";
import type { DashboardSummary, SettingsPayload, Task } from "@/lib/types";
import { AGENT_LABEL, cn, formatDate, greeting, isOverdue, relativeTime } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import { useNow } from "@/hooks/useNow";
import { useToggleTask } from "@/hooks/useTaskMutations";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";
import { TaskCheckbox } from "@/components/tasks/TaskCheckbox";
import { PriorityBadge, RunStatusBadge } from "@/components/tasks/StatusBadges";
import { QueryError } from "@/components/tasks/QueryError";

export function Dashboard() {
  const navigate = useNavigate();
  const setQuickCaptureOpen = useUiStore((s) => s.setQuickCaptureOpen);
  const now = useNow(30_000);
  const { toggle } = useToggleTask();

  const summary = useQuery({
    queryKey: ["dashboard"],
    queryFn: () => invoke<DashboardSummary>("dashboard.summary"),
  });
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => invoke<SettingsPayload>("settings.get"),
  });

  const userName = settings.data?.app.user_name?.trim() || summary.data?.user_name?.trim() || "";
  const dateStr = now.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  const timeStr = now.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });

  if (summary.isLoading) return <DashboardSkeleton />;
  if (summary.isError) {
    return (
      <div className="mx-auto max-w-[1200px] p-6">
        <QueryError error={summary.error} onRetry={() => summary.refetch()} />
      </div>
    );
  }

  const d = summary.data!;
  const isEmpty =
    d.stats.open_tasks === 0 &&
    d.stats.completed_today === 0 &&
    d.stats.overdue === 0 &&
    d.stats.inbox_pending === 0 &&
    d.stats.active_projects === 0 &&
    d.stats.total_notes === 0 &&
    d.recent_activity.length === 0 &&
    d.recent_runs.length === 0;

  const stats = [
    { label: "Abiertas", value: d.stats.open_tasks, icon: ListTodo, to: "/tasks" },
    { label: "Hechas hoy", value: d.stats.completed_today, icon: CheckCircle2, to: "/tasks" },
    { label: "Atrasadas", value: d.stats.overdue, icon: CalendarClock, to: "/tasks", danger: d.stats.overdue > 0 },
    { label: "Inbox", value: d.stats.inbox_pending, icon: Inbox, to: "/inbox" },
    { label: "Proyectos", value: d.stats.active_projects, icon: FolderKanban, to: "/projects" },
    { label: "Notas", value: d.stats.total_notes, icon: NotebookPen, to: "/knowledge" },
  ];

  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <header className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">
            {greeting()}
            {userName && <span className="text-muted-foreground">, {userName}</span>}
          </h1>
          <p className="mt-1 text-sm text-muted-foreground capitalize">
            {dateStr} · <span className="tabular-nums">{timeStr}</span>
          </p>
        </div>
        <Button onClick={() => setQuickCaptureOpen(true)}>
          <Zap /> Captura rápida
          <kbd className="ml-1 rounded border border-primary-foreground/30 px-1.5 py-0.5 text-[10px] font-medium text-primary-foreground/80">
            ⌘K
          </kbd>
        </Button>
      </header>

      {isEmpty ? (
        <Card>
          <EmptyState
            icon={Sparkles}
            title="Tu centro de control está listo"
            description="Captura una idea, crea tu primera tarea o inicia un proyecto. NEXUS se encarga del resto."
            action={
              <Button onClick={() => setQuickCaptureOpen(true)}>
                <Zap /> Capturar algo
              </Button>
            }
          />
        </Card>
      ) : (
        <>
          <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
            {stats.map(({ label, value, icon: Icon, to, danger }) => (
              <button
                key={label}
                type="button"
                onClick={() => navigate(to)}
                className="group rounded-xl border border-border bg-card p-3.5 text-left shadow-sm transition-all hover:border-ring/50 hover:shadow cursor-pointer"
              >
                <div className="flex items-center justify-between">
                  <Icon className={cn("size-4", danger ? "text-destructive" : "text-muted-foreground")} />
                  <ArrowRight className="size-3 text-muted-foreground/0 transition-all group-hover:text-muted-foreground" />
                </div>
                <div className={cn("mt-2 text-2xl font-semibold tabular-nums", danger && "text-destructive")}>
                  {value}
                </div>
                <div className="text-xs text-muted-foreground">{label}</div>
              </button>
            ))}
          </div>

          <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
            <SectionCard title="Hoy" icon={CircleDot} count={d.tasks_today.length}>
              {d.tasks_today.length === 0 ? (
                <SectionEmpty text="Nada para hoy. Disfruta el margen." />
              ) : (
                <div className="space-y-0.5">
                  {d.tasks_today.map((t) => (
                    <DashboardTask key={t.id} task={t} onToggle={() => toggle(t)} />
                  ))}
                </div>
              )}
            </SectionCard>

            {d.overdue.length > 0 && (
              <SectionCard title="Atrasadas" icon={CalendarClock} count={d.overdue.length} danger>
                <div className="space-y-0.5">
                  {d.overdue.map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      onClick={() => navigate("/tasks")}
                      className="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-destructive/10 cursor-pointer"
                    >
                      <span className="size-1.5 shrink-0 rounded-full bg-destructive" />
                      <span className="min-w-0 flex-1 truncate text-sm">{t.title}</span>
                      <span className="shrink-0 text-[11px] font-medium text-destructive">
                        {formatDate(t.due_date)}
                      </span>
                    </button>
                  ))}
                </div>
              </SectionCard>
            )}

            <SectionCard title="Prioritarias" icon={Flag} count={d.priority_tasks.length}>
              {d.priority_tasks.length === 0 ? (
                <SectionEmpty text="Sin tareas urgentes o de alta prioridad." />
              ) : (
                <div className="space-y-0.5">
                  {d.priority_tasks.map((t) => (
                    <DashboardTask
                      key={t.id}
                      task={t}
                      onToggle={() => toggle(t)}
                      trailing={<PriorityBadge priority={t.priority} />}
                    />
                  ))}
                </div>
              )}
            </SectionCard>

            <SectionCard title="Próximas fechas" icon={Calendar} count={d.upcoming.length}>
              {d.upcoming.length === 0 ? (
                <SectionEmpty text="No hay vencimientos próximos." />
              ) : (
                <div className="space-y-0.5">
                  {d.upcoming.map((t) => (
                    <DashboardTask
                      key={t.id}
                      task={t}
                      onToggle={() => toggle(t)}
                      trailing={
                        <span
                          className={cn(
                            "shrink-0 text-[11px]",
                            isOverdue(t.due_date, t.status) ? "font-medium text-destructive" : "text-muted-foreground",
                          )}
                        >
                          {formatDate(t.due_date)}
                        </span>
                      }
                    />
                  ))}
                </div>
              )}
            </SectionCard>

            <SectionCard title="Proyectos activos" icon={FolderKanban} count={d.active_projects.length}>
              {d.active_projects.length === 0 ? (
                <SectionEmpty text="Crea un proyecto para agrupar tu trabajo." />
              ) : (
                <div className="space-y-1">
                  {d.active_projects.map((p) => (
                    <button
                      key={p.project.id}
                      type="button"
                      onClick={() => navigate(`/projects/${p.project.id}`)}
                      className="w-full rounded-md px-2 py-2 text-left transition-colors hover:bg-accent/60 cursor-pointer"
                    >
                      <div className="mb-1.5 flex items-center gap-2">
                        <span className="min-w-0 flex-1 truncate text-sm font-medium">{p.project.name}</span>
                        {p.overdue > 0 && <Badge variant="destructive">{p.overdue} atrasadas</Badge>}
                        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
                          {p.done}/{p.total}
                        </span>
                      </div>
                      <Progress value={p.progress * 100} />
                    </button>
                  ))}
                </div>
              )}
            </SectionCard>

            <SectionCard title="Actividad reciente" icon={Activity} count={d.recent_activity.length}>
              {d.recent_activity.length === 0 ? (
                <SectionEmpty text="Aún no hay actividad registrada." />
              ) : (
                <div className="space-y-0.5">
                  {d.recent_activity.slice(0, 7).map((e) => (
                    <div key={e.id} className="flex items-start gap-2.5 rounded-md px-2 py-1.5">
                      <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-primary/60" />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-xs">{e.summary}</p>
                        <p className="text-[11px] text-muted-foreground">
                          {e.actor} · {relativeTime(e.created_at)}
                        </p>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </SectionCard>

            <SectionCard title="Últimas ejecuciones de agentes" icon={Bot} count={d.recent_runs.length}>
              {d.recent_runs.length === 0 ? (
                <SectionEmpty text="Los agentes aún no han ejecutado nada." />
              ) : (
                <div className="space-y-0.5">
                  {d.recent_runs.slice(0, 6).map((r) => (
                    <button
                      key={r.id}
                      type="button"
                      onClick={() => navigate("/activity")}
                      className="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent/60 cursor-pointer"
                    >
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-xs font-medium">{AGENT_LABEL[r.agent_id] ?? r.agent_id}</p>
                        <p className="truncate text-[11px] text-muted-foreground">
                          {r.goal} · {relativeTime(r.created_at)}
                        </p>
                      </div>
                      <RunStatusBadge status={r.status} />
                    </button>
                  ))}
                </div>
              )}
            </SectionCard>
          </div>
        </>
      )}
    </div>
  );
}

function SectionCard({
  title,
  icon: Icon,
  count,
  danger,
  children,
}: {
  title: string;
  icon: typeof CircleDot;
  count: number;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Card className={cn("animate-slide-up", danger && "border-destructive/30")}>
      <CardHeader className="flex-row items-center justify-between space-y-0 p-4 pb-2">
        <CardTitle className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          <Icon className={cn("size-3.5", danger && "text-destructive")} />
          {title}
        </CardTitle>
        {count > 0 && (
          <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] tabular-nums text-muted-foreground">
            {count}
          </span>
        )}
      </CardHeader>
      <CardContent className="p-2 pt-0">{children}</CardContent>
    </Card>
  );
}

function SectionEmpty({ text }: { text: string }) {
  return <p className="px-2 py-4 text-center text-xs text-muted-foreground">{text}</p>;
}

function DashboardTask({
  task,
  onToggle,
  trailing,
}: {
  task: Task;
  onToggle: () => void;
  trailing?: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors hover:bg-accent/60">
      <TaskCheckbox checked={task.status === "done"} onChange={onToggle} />
      <span
        className={cn(
          "min-w-0 flex-1 truncate text-sm",
          task.status === "done" && "line-through text-muted-foreground",
        )}
      >
        {task.title}
      </span>
      {trailing}
    </div>
  );
}

function DashboardSkeleton() {
  return (
    <div className="mx-auto max-w-[1200px] p-6">
      <Skeleton className="mb-2 h-8 w-72" />
      <Skeleton className="mb-6 h-4 w-56" />
      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-24 rounded-xl" />
        ))}
      </div>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-56 rounded-xl" />
        ))}
      </div>
    </div>
  );
}
