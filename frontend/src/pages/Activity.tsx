import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  Activity as ActivityIcon,
  Bot,
  Camera,
  CheckCircle2,
  FilePlus2,
  History,
  ListChecks,
  Loader2,
  Pencil,
  Play,
  RotateCcw,
  ShieldCheck,
  Trash2,
  Undo2,
  User,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { ActivityEvent, AgentRun, Approval, Id } from "@/lib/types";
import { AGENT_LABEL, RUN_STATUS_LABEL, cn, formatDateTime, relativeTime } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EmptyState } from "@/components/ui/empty-state";
import { ApprovalCard } from "@/components/chat/ApprovalCard";
import { RunDetailDialog, RUN_BADGE_VARIANT, runDuration } from "@/components/activity/RunDetailDialog";

const KIND_ICON: Record<string, LucideIcon> = {
  created: FilePlus2,
  updated: Pencil,
  deleted: Trash2,
  restored: RotateCcw,
  undone: Undo2,
  captured: Camera,
  agent_run: Play,
};

function RunsTab() {
  const [openRunId, setOpenRunId] = useState<Id | null>(null);
  const runs = useQuery({
    queryKey: ["runs", "activity"],
    queryFn: () => invoke<AgentRun[]>("agents.runs", { limit: 100 }),
  });

  return (
    <>
      {runs.isLoading ? (
        <div className="space-y-2">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-14 w-full rounded-lg" />
          ))}
        </div>
      ) : runs.isError ? (
        <EmptyState
          icon={ListChecks}
          title="No se pudieron cargar las ejecuciones"
          description={runs.error instanceof ApiError ? runs.error.message : undefined}
          action={<Button size="sm" variant="outline" onClick={() => runs.refetch()}>Reintentar</Button>}
        />
      ) : (runs.data ?? []).length === 0 ? (
        <EmptyState
          icon={ListChecks}
          title="Sin ejecuciones todavía"
          description="Cuando lances un agente desde la página Agentes o el chat, sus ejecuciones aparecerán aquí."
        />
      ) : (
        <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-[11px] uppercase tracking-wide text-muted-foreground">
                <th className="px-4 py-2.5 font-medium">Estado</th>
                <th className="px-3 py-2.5 font-medium">Agente</th>
                <th className="px-3 py-2.5 font-medium">Objetivo</th>
                <th className="hidden px-3 py-2.5 font-medium md:table-cell">Duración</th>
                <th className="hidden px-3 py-2.5 font-medium lg:table-cell">Iter.</th>
                <th className="hidden px-3 py-2.5 font-medium lg:table-cell">Tokens</th>
                <th className="px-3 py-2.5 text-right font-medium">Cuándo</th>
              </tr>
            </thead>
            <tbody>
              {(runs.data ?? []).map((run) => (
                <tr
                  key={run.id}
                  onClick={() => setOpenRunId(run.id)}
                  className="cursor-pointer border-b border-border/60 transition-colors last:border-0 hover:bg-muted/60"
                >
                  <td className="px-4 py-2.5">
                    <Badge
                      variant={RUN_BADGE_VARIANT[run.status]}
                      className={cn(run.status === "running" && "animate-pulse")}
                    >
                      {RUN_STATUS_LABEL[run.status]}
                    </Badge>
                  </td>
                  <td className="whitespace-nowrap px-3 py-2.5 text-xs">
                    {AGENT_LABEL[run.agent_id] ?? run.agent_id}
                  </td>
                  <td className="max-w-[280px] truncate px-3 py-2.5 text-xs">{run.goal}</td>
                  <td className="hidden whitespace-nowrap px-3 py-2.5 text-xs text-muted-foreground md:table-cell">
                    {runDuration(run.started_at, run.finished_at)}
                  </td>
                  <td className="hidden px-3 py-2.5 text-xs text-muted-foreground lg:table-cell">{run.iterations}</td>
                  <td className="hidden px-3 py-2.5 text-xs text-muted-foreground lg:table-cell">{run.tokens_used}</td>
                  <td className="whitespace-nowrap px-3 py-2.5 text-right text-xs text-muted-foreground">
                    {relativeTime(run.created_at)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <RunDetailDialog runId={openRunId} onClose={() => setOpenRunId(null)} />
    </>
  );
}

function ApprovalsTab() {
  const pending = useQuery({
    queryKey: ["approvals", "pending"],
    queryFn: () => invoke<Approval[]>("agents.approvals", { status: "pending" }),
  });
  const history = useQuery({
    queryKey: ["approvals", "all"],
    queryFn: () => invoke<Approval[]>("agents.approvals"),
  });

  const decided = (history.data ?? []).filter((a) => a.status !== "pending");

  return (
    <div className="space-y-6">
      <section className="space-y-2">
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          Pendientes
          {(pending.data ?? []).length > 0 && <Badge variant="warning">{pending.data?.length}</Badge>}
        </h3>
        {pending.isLoading ? (
          <Skeleton className="h-24 w-full rounded-lg" />
        ) : (pending.data ?? []).length === 0 ? (
          <div className="flex items-center gap-2 rounded-lg border border-dashed border-border px-4 py-5 text-xs text-muted-foreground">
            <CheckCircle2 className="h-4 w-4 text-success" />
            No hay acciones esperando tu autorización.
          </div>
        ) : (
          <div className="space-y-2">
            {(pending.data ?? []).map((a) => (
              <ApprovalCard key={a.id} approval={a} />
            ))}
          </div>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-semibold">Historial</h3>
        {history.isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-12 w-full rounded-lg" />
            <Skeleton className="h-12 w-full rounded-lg" />
          </div>
        ) : decided.length === 0 ? (
          <p className="rounded-lg border border-dashed border-border px-4 py-5 text-xs text-muted-foreground">
            Aún no has aprobado ni rechazado ninguna acción.
          </p>
        ) : (
          <ul className="space-y-1.5">
            {decided.map((a) => (
              <li key={a.id} className="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-3 py-2.5 shadow-sm">
                <ShieldCheck className={cn("h-4 w-4", a.status === "approved" ? "text-success" : "text-destructive")} />
                <span className="font-mono text-xs font-medium">{a.tool_name}</span>
                <Badge variant={a.status === "approved" ? "success" : "destructive"}>
                  {a.status === "approved" ? "Aprobada" : "Rechazada"}
                </Badge>
                <span className="ml-auto text-[11px] text-muted-foreground">
                  {a.decided_at ? formatDateTime(a.decided_at) : relativeTime(a.created_at)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function AuditTab() {
  const qc = useQueryClient();
  const events = useQuery({
    queryKey: ["activity", "audit"],
    queryFn: () => invoke<ActivityEvent[]>("activity.list", { limit: 200 }),
  });

  const undo = useMutation({
    mutationFn: (event: ActivityEvent) => invoke("activity.undo", { id: event.id }),
    onSuccess: () => {
      toast.success("Acción deshecha");
      qc.invalidateQueries({ queryKey: ["activity"] });
      ["tasks", "notes", "projects", "inbox", "dashboard"].forEach((k) =>
        qc.invalidateQueries({ queryKey: [k] }),
      );
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo deshacer"),
  });

  if (events.isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-10 w-full rounded-lg" />
        ))}
      </div>
    );
  }

  if (events.isError || (events.data ?? []).length === 0) {
    return (
      <EmptyState
        icon={History}
        title={events.isError ? "No se pudo cargar la auditoría" : "Sin actividad registrada"}
        description={
          events.isError
            ? events.error instanceof ApiError ? events.error.message : undefined
            : "Cada cambio que hagas tú o tus agentes quedará registrado aquí."
        }
      />
    );
  }

  return (
    <ol className="relative space-y-0.5">
      {(events.data ?? []).map((event) => {
        const Icon = KIND_ICON[event.kind] ?? ActivityIcon;
        const isAgent = event.actor.startsWith("agent");
        return (
          <li key={event.id} className="group flex items-start gap-3 rounded-lg px-2 py-2 transition-colors hover:bg-muted/60">
            <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-border bg-card">
              <Icon className="h-3.5 w-3.5 text-muted-foreground" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="text-sm leading-snug">{event.summary}</p>
              <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
                <Badge variant={isAgent ? "default" : "secondary"} className="px-1.5 py-0 text-[10px]">
                  {isAgent ? <Bot /> : <User />}
                  {isAgent ? AGENT_LABEL[event.actor.replace(/^agent:/, "")] ?? event.actor : "Tú"}
                </Badge>
                <span className="text-[11px] text-muted-foreground">{event.entity_type}</span>
                <span className="text-[11px] text-muted-foreground">· {relativeTime(event.created_at)}</span>
              </div>
            </div>
            {event.undo_json != null && (
              <Button
                size="sm"
                variant="outline"
                className="mt-0.5 h-7 shrink-0 text-xs opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                onClick={() => undo.mutate(event)}
                disabled={undo.isPending}
              >
                {undo.isPending && undo.variables?.id === event.id ? (
                  <Loader2 className="animate-spin" />
                ) : (
                  <Undo2 />
                )}
                Deshacer
              </Button>
            )}
          </li>
        );
      })}
    </ol>
  );
}

export function ActivityPage() {
  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <div className="mb-4">
        <h1 className="text-lg font-semibold tracking-tight">Actividad</h1>
        <p className="text-xs text-muted-foreground">
          Ejecuciones de agentes, aprobaciones y el registro completo de cambios.
        </p>
      </div>
      <Tabs defaultValue="runs">
        <TabsList>
          <TabsTrigger value="runs"><ListChecks /> Ejecuciones</TabsTrigger>
          <TabsTrigger value="approvals"><ShieldCheck /> Aprobaciones</TabsTrigger>
          <TabsTrigger value="audit"><History /> Auditoría</TabsTrigger>
        </TabsList>
        <TabsContent value="runs"><RunsTab /></TabsContent>
        <TabsContent value="approvals"><ApprovalsTab /></TabsContent>
        <TabsContent value="audit"><AuditTab /></TabsContent>
      </Tabs>
    </div>
  );
}
