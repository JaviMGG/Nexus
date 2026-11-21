import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  Activity as ActivityIcon,
  Bot,
  CheckCircle2,
  FlaskConical,
  History,
  LayoutDashboard,
  ListChecks,
  Loader2,
  Pencil,
  Play,
  Plus,
  ShieldCheck,
  Sparkles,
  Trash2,
  Workflow,
  XCircle,
  Zap,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type {
  Automation,
  AutomationApproval,
  AutomationExecution,
  AutomationOverview,
  AutomationTemplate,
  ExecutionFilter,
  Id,
} from "@/lib/types";
import { AGENT_LABEL, cn, formatDateTime, relativeTime, triggerLabel } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { AutomationEditor } from "@/components/automations/AutomationEditor";
import { ExecutionDetailDialog } from "@/components/automations/ExecutionDetailDialog";
import { FlowCanvas } from "@/components/automations/FlowCanvas";
import { AutomationIcon, ExecutionBadge, ModeBadge, executionDuration } from "@/components/automations/shared";

// ---------------------------------------------------------------------------
// Overview
// ---------------------------------------------------------------------------

function StatCard({ label, value, icon: Icon, tone }: { label: string; value: string | number; icon: typeof Zap; tone?: string }) {
  return (
    <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center justify-between">
        <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">{label}</p>
        <Icon className={cn("h-4 w-4 text-muted-foreground", tone)} />
      </div>
      <p className="mt-1.5 text-2xl font-semibold tracking-tight">{value}</p>
    </div>
  );
}

function OverviewTab({ onOpenExecution }: { onOpenExecution: (id: Id) => void }) {
  const ov = useQuery({
    queryKey: ["automation-overview"],
    queryFn: () => invoke<AutomationOverview>("automations.overview"),
    refetchInterval: 15000,
  });

  if (ov.isLoading) {
    return (
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-24 rounded-xl" />
        ))}
      </div>
    );
  }
  if (ov.isError || !ov.data) {
    return (
      <EmptyState
        icon={LayoutDashboard}
        title="No se pudo cargar el resumen"
        description={ov.error instanceof ApiError ? ov.error.message : undefined}
        action={<Button size="sm" variant="outline" onClick={() => ov.refetch()}>Reintentar</Button>}
      />
    );
  }
  const d = ov.data;
  const tokenPct = d.daily_token_limit > 0 ? Math.min(100, Math.round((d.tokens_today / d.daily_token_limit) * 100)) : 0;

  return (
    <div className="space-y-5">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatCard label="Activas" value={`${d.active_count} / ${d.total_count}`} icon={Zap} tone="text-primary" />
        <StatCard label="Ejecuciones" value={d.executions_total} icon={ActivityIcon} />
        <StatCard label="Correctas" value={d.executions_ok} icon={CheckCircle2} tone="text-success" />
        <StatCard label="Fallidas" value={d.executions_failed} icon={XCircle} tone="text-destructive" />
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
        <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
          <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Consumo de IA hoy</p>
          <p className="mt-1.5 text-lg font-semibold">
            {d.tokens_today.toLocaleString("es-ES")} <span className="text-xs font-normal text-muted-foreground">/ {d.daily_token_limit.toLocaleString("es-ES")} tokens</span>
          </p>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
            <div className={cn("h-full rounded-full transition-all", tokenPct > 85 ? "bg-destructive" : "bg-primary")} style={{ width: `${tokenPct}%` }} />
          </div>
          <p className="mt-2 text-[11px] text-muted-foreground">
            Estado IA:{" "}
            <Badge variant={d.ai_status.state === "connected" ? "success" : d.ai_status.state === "error" ? "destructive" : "secondary"}>
              {d.ai_status.state === "connected" ? "Conectada" : d.ai_status.state === "error" ? "Error" : "Sin proveedor"}
            </Badge>
            {" · "}Las automatizaciones locales funcionan sin IA.
          </p>
        </div>

        <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
          <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Agentes más utilizados</p>
          {d.top_agents.length === 0 ? (
            <p className="mt-2 text-xs text-muted-foreground">Todavía no hay ejecuciones con agentes.</p>
          ) : (
            <ul className="mt-2 space-y-1.5">
              {d.top_agents.map((a) => (
                <li key={a.agent_id} className="flex items-center gap-2 text-xs">
                  <Bot className="h-3.5 w-3.5 text-muted-foreground" />
                  <span className="flex-1">{AGENT_LABEL[a.agent_id] ?? a.agent_id}</span>
                  <Badge variant="secondary">{a.calls}</Badge>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
          <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Próximas programadas</p>
          {d.upcoming.length === 0 ? (
            <p className="mt-2 text-xs text-muted-foreground">Ninguna automatización temporal activa.</p>
          ) : (
            <ul className="mt-2 space-y-1.5">
              {d.upcoming.slice(0, 5).map((a) => (
                <li key={a.id} className="flex items-center gap-2 text-xs">
                  <AutomationIcon icon={a.icon} className="text-muted-foreground" />
                  <span className="min-w-0 flex-1 truncate">{a.name}</span>
                  <span className="whitespace-nowrap text-muted-foreground">{a.next_run_at ? formatDateTime(a.next_run_at) : ""}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      <section>
        <h3 className="mb-2 text-sm font-semibold">Últimas ejecuciones</h3>
        {d.recent_executions.length === 0 ? (
          <p className="rounded-lg border border-dashed border-border px-4 py-5 text-xs text-muted-foreground">
            Aún no hay ejecuciones. Activa una plantilla o crea tu primera automatización.
          </p>
        ) : (
          <ul className="space-y-1.5">
            {d.recent_executions.map((e) => (
              <li key={e.id}>
                <button
                  onClick={() => onOpenExecution(e.id)}
                  className="flex w-full items-center gap-3 rounded-lg border border-border bg-card px-3 py-2.5 text-left shadow-sm transition-colors hover:bg-muted/60 cursor-pointer"
                >
                  <ExecutionBadge status={e.status} />
                  <span className="min-w-0 flex-1 truncate text-sm">{e.automation_name}</span>
                  {e.test_run && <Badge variant="secondary">Prueba</Badge>}
                  <span className="hidden text-xs text-muted-foreground sm:inline">{e.trigger_detail}</span>
                  <span className="whitespace-nowrap text-xs text-muted-foreground">{relativeTime(e.created_at)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// My Automations
// ---------------------------------------------------------------------------

function MyAutomationsTab({ onEdit, onOpenExecution }: { onEdit: (a: Automation | null) => void; onOpenExecution: (id: Id) => void }) {
  const qc = useQueryClient();
  const [deleteTarget, setDeleteTarget] = useState<Automation | null>(null);
  const list = useQuery({
    queryKey: ["automations"],
    queryFn: () => invoke<Automation[]>("automations.list"),
  });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["automations"] });
    qc.invalidateQueries({ queryKey: ["automation-overview"] });
  };

  const toggle = useMutation({
    mutationFn: ({ id, enabled }: { id: Id; enabled: boolean }) => invoke("automations.set_enabled", { id, enabled }),
    onSuccess: invalidate,
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo cambiar el estado"),
  });

  const run = useMutation({
    mutationFn: ({ id, test }: { id: Id; test: boolean }) => invoke<AutomationExecution>("automations.run", { id, test }),
    onSuccess: (e, vars) => {
      toast.success(vars.test ? "Prueba lanzada (no modifica datos)" : "Ejecución lanzada");
      qc.invalidateQueries({ queryKey: ["automation-executions"] });
      onOpenExecution(e.id);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo ejecutar"),
  });

  const remove = useMutation({
    mutationFn: (id: Id) => invoke("automations.delete", { id }),
    onSuccess: () => {
      toast.success("Automatización eliminada");
      setDeleteTarget(null);
      invalidate();
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar"),
  });

  if (list.isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 4 }).map((_, i) => (
          <Skeleton key={i} className="h-24 rounded-xl" />
        ))}
      </div>
    );
  }
  if (list.isError) {
    return (
      <EmptyState
        icon={Workflow}
        title="No se pudieron cargar las automatizaciones"
        description={list.error instanceof ApiError ? list.error.message : undefined}
        action={<Button size="sm" variant="outline" onClick={() => list.refetch()}>Reintentar</Button>}
      />
    );
  }
  const items = list.data ?? [];
  if (items.length === 0) {
    return (
      <EmptyState
        icon={Workflow}
        title="Sin automatizaciones todavía"
        description="Crea la tuya con el constructor o activa una plantilla preconfigurada en la pestaña Plantillas."
        action={<Button size="sm" onClick={() => onEdit(null)}><Plus /> Nueva automatización</Button>}
      />
    );
  }

  return (
    <>
      <div className="mb-3 flex justify-end">
        <Button size="sm" onClick={() => onEdit(null)}><Plus /> Nueva automatización</Button>
      </div>
      <div className="space-y-2">
        {items.map((a) => (
          <div key={a.id} className={cn("rounded-xl border bg-card p-4 shadow-sm transition-opacity", !a.enabled && "opacity-70")}>
            <div className="flex flex-wrap items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/15">
                <AutomationIcon icon={a.icon} className="text-primary" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <h3 className="text-sm font-semibold">{a.name}</h3>
                  <ModeBadge mode={a.mode} />
                  {a.last_status && <ExecutionBadge status={a.last_status} />}
                  {a.template_id && <Badge variant="outline">Plantilla</Badge>}
                </div>
                {a.description && <p className="mt-0.5 text-xs text-muted-foreground">{a.description}</p>}
                <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                  <span>{triggerLabel(a.trigger)}</span>
                  <span>· {a.run_count} ejecuciones</span>
                  {a.last_run_at && <span>· última: {relativeTime(a.last_run_at)}</span>}
                  {a.enabled && a.next_run_at && <span>· próxima: {formatDateTime(a.next_run_at)}</span>}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <Switch
                  checked={a.enabled}
                  onCheckedChange={(v) => toggle.mutate({ id: a.id, enabled: v })}
                  aria-label={`Activar ${a.name}`}
                />
                <Button size="sm" variant="outline" onClick={() => run.mutate({ id: a.id, test: false })} disabled={run.isPending}>
                  {run.isPending && run.variables?.id === a.id && !run.variables.test ? <Loader2 className="animate-spin" /> : <Play />}
                  Ejecutar
                </Button>
                <Button size="sm" variant="ghost" onClick={() => run.mutate({ id: a.id, test: true })} disabled={run.isPending} title="Ejecución de prueba (no modifica datos)">
                  <FlaskConical />
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onEdit(a)} title="Configurar">
                  <Pencil />
                </Button>
                <Button size="sm" variant="ghost" className="text-destructive" onClick={() => setDeleteTarget(a)} title="Eliminar">
                  <Trash2 />
                </Button>
              </div>
            </div>
          </div>
        ))}
      </div>

      <Dialog open={deleteTarget != null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Eliminar automatización</DialogTitle>
            <DialogDescription>
              «{deleteTarget?.name}» dejará de ejecutarse. Su historial de ejecuciones se conserva.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancelar</Button>
            <Button variant="destructive" onClick={() => deleteTarget && remove.mutate(deleteTarget.id)} disabled={remove.isPending}>
              {remove.isPending ? <Loader2 className="animate-spin" /> : <Trash2 />}
              Eliminar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

const DIFFICULTY_LABEL: Record<number, string> = {
  1: "Muy simple",
  2: "Simple",
  3: "Intermedia",
  4: "Avanzada",
  5: "Experta",
};

function DifficultyDots({ level }: { level: number }) {
  const lvl = Math.min(5, Math.max(1, level));
  return (
    <span
      className="flex items-center gap-1.5"
      title={`Dificultad: ${DIFFICULTY_LABEL[lvl] ?? lvl}`}
    >
      <span className="flex items-center gap-0.5">
        {[1, 2, 3, 4, 5].map((i) => (
          <span
            key={i}
            className={cn(
              "h-1.5 w-3 rounded-full",
              i <= lvl ? (lvl >= 4 ? "bg-warning" : "bg-primary") : "bg-muted",
            )}
          />
        ))}
      </span>
      <span className="text-[10px] font-medium text-muted-foreground">{DIFFICULTY_LABEL[lvl] ?? lvl}</span>
    </span>
  );
}

function TemplatesTab() {
  const qc = useQueryClient();
  const [modeFilter, setModeFilter] = useState<string>("__all__");
  const templates = useQuery({
    queryKey: ["automation-templates"],
    queryFn: () => invoke<AutomationTemplate[]>("automations.templates"),
    staleTime: Infinity,
  });
  const installed = useQuery({
    queryKey: ["automations"],
    queryFn: () => invoke<Automation[]>("automations.list"),
  });

  const install = useMutation({
    mutationFn: (templateId: string) => invoke<Automation>("automations.install_template", { template_id: templateId }),
    onSuccess: (a) => {
      toast.success(`«${a.name}» instalada`, { description: "Está desactivada: revísala, personalízala y actívala cuando quieras." });
      qc.invalidateQueries({ queryKey: ["automations"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo instalar"),
  });

  if (templates.isLoading) {
    return (
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-48 rounded-xl" />
        ))}
      </div>
    );
  }
  if (templates.isError) {
    return (
      <EmptyState
        icon={Sparkles}
        title="No se pudieron cargar las plantillas"
        description={templates.error instanceof ApiError ? templates.error.message : undefined}
        action={<Button size="sm" variant="outline" onClick={() => templates.refetch()}>Reintentar</Button>}
      />
    );
  }

  const installedIds = new Set((installed.data ?? []).map((a) => a.template_id).filter(Boolean));

  const sorted = [...(templates.data ?? [])].sort(
    (a, b) => a.difficulty - b.difficulty || a.name.localeCompare(b.name),
  );
  const visible = modeFilter === "__all__" ? sorted : sorted.filter((t) => t.default_mode === modeFilter);

  return (
    <>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <span className="text-xs text-muted-foreground">Ordenadas de más simple a más potente:</span>
        <Select value={modeFilter} onValueChange={setModeFilter}>
          <SelectTrigger className="h-8 w-44 text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__" className="text-xs">Todos los modos</SelectItem>
            <SelectItem value="local" className="text-xs">Local (sin IA)</SelectItem>
            <SelectItem value="ai" className="text-xs">IA</SelectItem>
            <SelectItem value="hybrid" className="text-xs">Híbrido</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {visible.map((t) => {
        const isInstalled = installedIds.has(t.id);
        return (
          <div key={t.id} className="flex flex-col rounded-xl border border-border bg-card p-4 shadow-sm">
            <div className="flex items-center gap-2.5">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/15">
                <AutomationIcon icon={t.icon} className="text-primary" />
              </div>
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-semibold">{t.name}</h3>
                <p className="text-[11px] text-muted-foreground">{t.trigger_label}</p>
              </div>
            </div>
            <div className="mt-2">
              <DifficultyDots level={t.difficulty} />
            </div>
            <p className="mt-2 flex-1 text-xs leading-relaxed text-muted-foreground">{t.description}</p>
            <div className="mt-3 flex flex-wrap items-center gap-1.5">
              <ModeBadge mode={t.default_mode} />
              {t.agents.map((a) => (
                <Badge key={a} variant="secondary"><Bot /> {AGENT_LABEL[a] ?? a}</Badge>
              ))}
              {t.requires_ai ? (
                <Badge variant="warning">~{t.estimated_tokens.toLocaleString("es-ES")} tokens/ejecución</Badge>
              ) : (
                <Badge variant="success">Sin coste de IA</Badge>
              )}
            </div>
            <div className="mt-3">
              <Button
                size="sm"
                variant={isInstalled ? "outline" : "default"}
                className="w-full"
                disabled={isInstalled || install.isPending}
                onClick={() => install.mutate(t.id)}
              >
                {install.isPending && install.variables === t.id ? <Loader2 className="animate-spin" /> : isInstalled ? <CheckCircle2 /> : <Plus />}
                {isInstalled ? "Instalada" : "Activar y personalizar"}
              </Button>
            </div>
          </div>
        );
      })}
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Execution History + autorizaciones
// ---------------------------------------------------------------------------

function HistoryTab({ onOpenExecution }: { onOpenExecution: (id: Id) => void }) {
  const qc = useQueryClient();
  const [statusFilter, setStatusFilter] = useState<string>("__all__");
  const filter: ExecutionFilter = { limit: 150 };
  if (statusFilter !== "__all__") filter.status = statusFilter as ExecutionFilter["status"];

  const execs = useQuery({
    queryKey: ["automation-executions", statusFilter],
    queryFn: () => invoke<AutomationExecution[]>("automations.executions", { filter }),
    refetchInterval: 10000,
  });
  const approvals = useQuery({
    queryKey: ["automation-approvals", "pending"],
    queryFn: () => invoke<AutomationApproval[]>("automations.approvals", { status: "pending" }),
    refetchInterval: 10000,
  });

  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: Id; approved: boolean }) =>
      invoke(approved ? "automations.approve" : "automations.reject", { approval_id: id }),
    onSuccess: (_, vars) => {
      toast.success(vars.approved ? "Acción autorizada" : "Acción rechazada");
      qc.invalidateQueries({ queryKey: ["automation-approvals"] });
      qc.invalidateQueries({ queryKey: ["automation-executions"] });
      qc.invalidateQueries({ queryKey: ["automation-overview"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo decidir"),
  });

  const pending = approvals.data ?? [];

  return (
    <div className="space-y-5">
      {pending.length > 0 && (
        <section className="space-y-2">
          <h3 className="flex items-center gap-2 text-sm font-semibold">
            <ShieldCheck className="h-4 w-4 text-warning" />
            Autorizaciones pendientes
            <Badge variant="warning">{pending.length}</Badge>
          </h3>
          {pending.map((a) => (
            <div key={a.id} className="rounded-lg border border-warning/40 bg-warning/10 px-3 py-2.5">
              <div className="flex flex-wrap items-center gap-2">
                <p className="min-w-0 flex-1 text-sm">{a.preview}</p>
                <Button size="sm" onClick={() => decide.mutate({ id: a.id, approved: true })} disabled={decide.isPending}>
                  Autorizar
                </Button>
                <Button size="sm" variant="outline" onClick={() => decide.mutate({ id: a.id, approved: false })} disabled={decide.isPending}>
                  Rechazar
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onOpenExecution(a.execution_id)}>
                  Ver ejecución
                </Button>
              </div>
              <p className="mt-1 text-[11px] text-muted-foreground">
                {a.action} · {relativeTime(a.created_at)} {a.agent_id ? `· agente ${AGENT_LABEL[a.agent_id] ?? a.agent_id}` : ""}
              </p>
            </div>
          ))}
        </section>
      )}

      <div className="flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold">Historial de ejecuciones</h3>
        <Select value={statusFilter} onValueChange={setStatusFilter}>
          <SelectTrigger className="h-8 w-48 text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__" className="text-xs">Todos los estados</SelectItem>
            <SelectItem value="completed" className="text-xs">Completadas</SelectItem>
            <SelectItem value="failed" className="text-xs">Fallidas</SelectItem>
            <SelectItem value="running" className="text-xs">En curso</SelectItem>
            <SelectItem value="waiting_approval" className="text-xs">Esperando autorización</SelectItem>
            <SelectItem value="cancelled" className="text-xs">Canceladas</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {execs.isLoading ? (
        <div className="space-y-2">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-12 rounded-lg" />
          ))}
        </div>
      ) : execs.isError ? (
        <EmptyState
          icon={History}
          title="No se pudo cargar el historial"
          description={execs.error instanceof ApiError ? execs.error.message : undefined}
          action={<Button size="sm" variant="outline" onClick={() => execs.refetch()}>Reintentar</Button>}
        />
      ) : (execs.data ?? []).length === 0 ? (
        <EmptyState
          icon={History}
          title="Sin ejecuciones registradas"
          description="Cada ejecución de una automatización quedará registrada aquí con sus pasos, agentes y consumo."
        />
      ) : (
        <div className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-[11px] uppercase tracking-wide text-muted-foreground">
                <th className="px-4 py-2.5 font-medium">Estado</th>
                <th className="px-3 py-2.5 font-medium">Automatización</th>
                <th className="hidden px-3 py-2.5 font-medium md:table-cell">Disparo</th>
                <th className="hidden px-3 py-2.5 font-medium lg:table-cell">Modo</th>
                <th className="hidden px-3 py-2.5 font-medium lg:table-cell">Tokens</th>
                <th className="hidden px-3 py-2.5 font-medium md:table-cell">Duración</th>
                <th className="px-3 py-2.5 text-right font-medium">Cuándo</th>
              </tr>
            </thead>
            <tbody>
              {(execs.data ?? []).map((e) => (
                <tr
                  key={e.id}
                  onClick={() => onOpenExecution(e.id)}
                  className="cursor-pointer border-b border-border/60 transition-colors last:border-0 hover:bg-muted/60"
                >
                  <td className="px-4 py-2.5">
                    <div className="flex items-center gap-1.5">
                      <ExecutionBadge status={e.status} />
                      {e.test_run && <Badge variant="secondary">Prueba</Badge>}
                    </div>
                  </td>
                  <td className="max-w-[240px] truncate px-3 py-2.5 text-xs font-medium">{e.automation_name}</td>
                  <td className="hidden max-w-[180px] truncate px-3 py-2.5 text-xs text-muted-foreground md:table-cell">{e.trigger_detail}</td>
                  <td className="hidden px-3 py-2.5 lg:table-cell"><ModeBadge mode={e.mode} /></td>
                  <td className="hidden px-3 py-2.5 text-xs text-muted-foreground lg:table-cell">{e.tokens_used}</td>
                  <td className="hidden whitespace-nowrap px-3 py-2.5 text-xs text-muted-foreground md:table-cell">{executionDuration(e.duration_ms)}</td>
                  <td className="whitespace-nowrap px-3 py-2.5 text-right text-xs text-muted-foreground">{relativeTime(e.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Página
// ---------------------------------------------------------------------------

export function AutomationsPage() {
  const [editor, setEditor] = useState<{ automation: Automation | null } | null>(null);
  const [openExecutionId, setOpenExecutionId] = useState<Id | null>(null);

  const openEditor = (a: Automation | null) => setEditor({ automation: a });

  if (editor) {
    return (
      <div className="mx-auto max-w-[1400px] p-6">
        <AutomationEditor
          key={editor.automation?.id ?? "new"}
          automation={editor.automation}
          onClose={() => setEditor(null)}
        />
        <ExecutionDetailDialog executionId={openExecutionId} onClose={() => setOpenExecutionId(null)} />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <div className="mb-4">
        <h1 className="text-lg font-semibold tracking-tight">Automatizaciones</h1>
        <p className="text-xs text-muted-foreground">
          Motor local de reglas y agentes: configura una vez y olvídate de las tareas repetitivas. Funciona mientras NEXUS esté abierto.
        </p>
      </div>
      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview"><LayoutDashboard /> Resumen</TabsTrigger>
          <TabsTrigger value="mine"><ListChecks /> Mis automatizaciones</TabsTrigger>
          <TabsTrigger value="builder"><Workflow /> Constructor</TabsTrigger>
          <TabsTrigger value="templates"><Sparkles /> Plantillas</TabsTrigger>
          <TabsTrigger value="history"><History /> Historial</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">
          <OverviewTab onOpenExecution={setOpenExecutionId} />
        </TabsContent>
        <TabsContent value="mine">
          <MyAutomationsTab onEdit={openEditor} onOpenExecution={setOpenExecutionId} />
        </TabsContent>
        <TabsContent value="builder">
          <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-semibold">Constructor visual de nodos</h3>
                <p className="text-xs text-muted-foreground">
                  Arrastra bloques al lienzo y conéctalos con flechas: disparador → acciones, agentes, IF/ELSE, esperas y notificaciones. Sin ciclos ni SQL libre.
                </p>
              </div>
              <Button size="sm" onClick={() => openEditor(null)}><Plus /> Crear automatización</Button>
            </div>
            <BuilderCanvas />
          </div>
        </TabsContent>
        <TabsContent value="templates">
          <TemplatesTab />
        </TabsContent>
        <TabsContent value="history">
          <HistoryTab onOpenExecution={setOpenExecutionId} />
        </TabsContent>
      </Tabs>

      <ExecutionDetailDialog executionId={openExecutionId} onClose={() => setOpenExecutionId(null)} />
    </div>
  );
}

function BuilderCanvas() {
  const qc = useQueryClient();
  const list = useQuery({
    queryKey: ["automations"],
    queryFn: () => invoke<Automation[]>("automations.list"),
  });
  const items = list.data ?? [];
  const [selectedId, setSelectedId] = useState<Id | null>(null);
  const selected = items.find((a) => a.id === selectedId) ?? items[0] ?? null;

  const saveFlow = useMutation({
    mutationFn: ({ id, flow }: { id: Id; flow: Automation["flow"] }) =>
      invoke("automations.update", { id, data: { flow } }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["automations"] });
      qc.invalidateQueries({ queryKey: ["automation-overview"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar el flujo"),
  });

  if (list.isLoading) return <Skeleton className="h-64 rounded-lg" />;
  if (items.length === 0) {
    return (
      <p className="rounded-lg border border-dashed border-border px-4 py-8 text-center text-xs text-muted-foreground">
        Crea una automatización para abrirla en el lienzo de nodos.
      </p>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Select value={selected?.id ?? ""} onValueChange={setSelectedId}>
          <SelectTrigger className="h-8 w-64 text-xs"><SelectValue placeholder="Elige una automatización" /></SelectTrigger>
          <SelectContent>
            {items.map((a) => (
              <SelectItem key={a.id} value={a.id} className="text-xs">{a.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        {selected && <ModeBadge mode={selected.mode} />}
        {saveFlow.isPending && <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />}
        {selected && (
          <span className="text-[11px] text-muted-foreground">Los cambios del lienzo se guardan solos.</span>
        )}
      </div>
      {selected && (
        <FlowCanvas
          key={selected.id}
          flow={selected.flow}
          trigger={selected.trigger}
          onChange={(flow) => saveFlow.mutate({ id: selected.id, flow })}
          height={560}
        />
      )}
    </div>
  );
}
