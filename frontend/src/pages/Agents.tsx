import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import {
  BookOpen,
  CalendarClock,
  FolderKanban,
  Loader2,
  Network,
  Play,
  Search,
  Wrench,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { AgentDef, AgentRun, RunStatus } from "@/lib/types";
import { AGENT_LABEL, RUN_STATUS_LABEL, cn, relativeTime } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { EmptyState } from "@/components/ui/empty-state";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

const AGENT_ICONS: Record<string, LucideIcon> = {
  orchestrator: Network,
  planner: CalendarClock,
  project_manager: FolderKanban,
  knowledge: BookOpen,
  researcher: Search,
  executor: Wrench,
};

const ACTIVE: RunStatus[] = ["pending", "running", "waiting_approval"];

export function AgentsPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [runTarget, setRunTarget] = useState<AgentDef | null>(null);
  const [goal, setGoal] = useState("");

  const agents = useQuery({
    queryKey: ["agents"],
    queryFn: () => invoke<AgentDef[]>("agents.list"),
  });
  const runs = useQuery({
    queryKey: ["runs", "agents-page"],
    queryFn: () => invoke<AgentRun[]>("agents.runs", { limit: 50 }),
  });

  const activeRuns = (runs.data ?? []).filter((r) => ACTIVE.includes(r.status));

  const start = useMutation({
    mutationFn: () =>
      invoke<AgentRun>("agents.start", { goal: goal.trim(), agent_id: runTarget!.id }),
    onSuccess: () => {
      toast.success(`${runTarget?.name}: ejecución iniciada`, {
        description: "Sigue el progreso en Actividad.",
      });
      qc.invalidateQueries({ queryKey: ["runs"] });
      setRunTarget(null);
      setGoal("");
      navigate("/activity");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo iniciar la ejecución"),
  });

  const cancel = useMutation({
    mutationFn: (run: AgentRun) => invoke("agents.cancel", { run_id: run.id }),
    onSuccess: () => {
      toast.success("Ejecución cancelada");
      qc.invalidateQueries({ queryKey: ["runs"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo cancelar"),
  });

  return (
    <div className="mx-auto max-w-[1200px] space-y-8 p-6 animate-fade-in">
      <div>
        <h1 className="text-lg font-semibold tracking-tight">Agentes</h1>
        <p className="max-w-2xl text-xs text-muted-foreground">
          Tu equipo de agentes especializados. El Orchestrator analiza cada objetivo y delega en el
          agente adecuado —planificación, proyectos, conocimiento, investigación o ejecución—;
          los resultados se consolidan en una única respuesta. Todo funciona también sin IA.
        </p>
      </div>

      {agents.isLoading ? (
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Card key={i}>
              <CardHeader>
                <Skeleton className="h-9 w-9 rounded-lg" />
                <Skeleton className="mt-2 h-4 w-1/2" />
                <Skeleton className="h-3 w-full" />
              </CardHeader>
              <CardContent><Skeleton className="h-8 w-full" /></CardContent>
            </Card>
          ))}
        </div>
      ) : agents.isError ? (
        <EmptyState
          icon={Network}
          title="No se pudieron cargar los agentes"
          description={agents.error instanceof ApiError ? agents.error.message : "Comprueba el servicio local."}
          action={<Button size="sm" variant="outline" onClick={() => agents.refetch()}>Reintentar</Button>}
        />
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {(agents.data ?? []).map((agent) => {
            const Icon = AGENT_ICONS[agent.id] ?? Network;
            return (
              <Card key={agent.id} className="flex flex-col transition-all hover:border-ring/40 hover:shadow-md">
                <CardHeader className="pb-3">
                  <div className="mb-1 flex h-9 w-9 items-center justify-center rounded-lg bg-primary/15">
                    <Icon className="h-4 w-4 text-primary" />
                  </div>
                  <CardTitle>{agent.name}</CardTitle>
                  <CardDescription className="line-clamp-2 min-h-8">{agent.description}</CardDescription>
                </CardHeader>
                <CardContent className="mt-auto space-y-3">
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary">
                      <Wrench /> {agent.tools.length} herramientas
                    </Badge>
                    <Badge variant="outline">máx. {agent.max_iterations} iteraciones</Badge>
                  </div>
                  <Button
                    size="sm"
                    className="w-full"
                    onClick={() => { setRunTarget(agent); setGoal(""); }}
                  >
                    <Play /> Ejecutar
                  </Button>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-semibold">Ejecuciones activas</h2>
          {activeRuns.length > 0 && <Badge variant="default">{activeRuns.length}</Badge>}
        </div>
        {runs.isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-14 w-full rounded-lg" />
            <Skeleton className="h-14 w-full rounded-lg" />
          </div>
        ) : activeRuns.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border px-4 py-6 text-center text-xs text-muted-foreground">
            No hay ejecuciones en curso. Lanza un agente con un objetivo para verlo aquí.
          </div>
        ) : (
          <ul className="space-y-2">
            {activeRuns.map((run) => (
              <li
                key={run.id}
                className="flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card px-4 py-3 shadow-sm animate-fade-in"
              >
                <span className="relative flex h-2 w-2 shrink-0">
                  {run.status === "running" && (
                    <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-50" />
                  )}
                  <span className={cn("relative inline-flex h-2 w-2 rounded-full", run.status === "waiting_approval" ? "bg-warning" : "bg-primary")} />
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{run.goal}</p>
                  <p className="text-[11px] text-muted-foreground">
                    {AGENT_LABEL[run.agent_id] ?? run.agent_id} · {relativeTime(run.created_at)} · {run.iterations} iteraciones
                  </p>
                </div>
                <Badge variant={run.status === "waiting_approval" ? "warning" : "default"}>
                  {RUN_STATUS_LABEL[run.status]}
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => cancel.mutate(run)}
                  disabled={cancel.isPending}
                >
                  <X /> Cancelar
                </Button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <Dialog open={runTarget != null} onOpenChange={(o) => !o && setRunTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Ejecutar {runTarget?.name}</DialogTitle>
            <DialogDescription>
              Describe el objetivo. El agente trabajará de forma autónoma respetando tus permisos y límites.
            </DialogDescription>
          </DialogHeader>
          <Textarea
            autoFocus
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            placeholder='p. ej. "Planifica la próxima iteración del proyecto Portfolio y crea las tareas"'
            className="min-h-[110px]"
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setRunTarget(null)}>Cancelar</Button>
            <Button onClick={() => start.mutate()} disabled={!goal.trim() || start.isPending}>
              {start.isPending ? <Loader2 className="animate-spin" /> : <Play />}
              Iniciar ejecución
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
