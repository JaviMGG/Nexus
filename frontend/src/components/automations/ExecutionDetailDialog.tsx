import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { toast } from "sonner";
import { Bot, ChevronDown, ChevronRight, Loader2, ShieldCheck, X } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { ExecutionDetail, Id, StepLog } from "@/lib/types";
import { AGENT_LABEL, STEP_STATUS_LABEL, cn, formatDateTime } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { RunDetailDialog } from "@/components/activity/RunDetailDialog";
import { AutomationIcon, ExecutionBadge, ModeBadge, STEP_BADGE_VARIANT, executionDuration } from "./shared";

function StepDetail({ value }: { value: unknown }) {
  const [open, setOpen] = useState(false);
  if (value == null) return null;
  return (
    <div>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground cursor-pointer"
      >
        {open ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        detalle
      </button>
      {open && (
        <pre className="mt-1 max-h-48 overflow-auto rounded-md bg-muted p-2 font-mono text-[11px] leading-relaxed animate-fade-in">
          {typeof value === "string" ? value : JSON.stringify(value, null, 2)}
        </pre>
      )}
    </div>
  );
}

function StepRow({ step, onOpenRun }: { step: StepLog; onOpenRun: (id: Id) => void }) {
  const runId = (step.detail as Record<string, unknown> | null)?.run_id;
  return (
    <li className="flex items-start gap-3 rounded-lg border border-border/60 bg-card px-3 py-2.5">
      <Badge variant={STEP_BADGE_VARIANT[step.status]} className="mt-0.5 shrink-0">
        {STEP_STATUS_LABEL[step.status]}
      </Badge>
      <div className="min-w-0 flex-1">
        <p className="text-sm leading-snug">
          <span className="mr-1.5 text-[10px] uppercase tracking-wide text-muted-foreground">{step.kind}</span>
          {step.name}
        </p>
        {step.error && <p className="mt-0.5 text-xs text-destructive">{step.error}</p>}
        <StepDetail value={step.detail} />
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1">
        <span className="text-[11px] text-muted-foreground">{executionDuration(step.duration_ms)}</span>
        {typeof runId === "string" && (
          <Button size="sm" variant="outline" className="h-6 text-[11px]" onClick={() => onOpenRun(runId)}>
            <Bot /> Ver run
          </Button>
        )}
      </div>
    </li>
  );
}

export function ExecutionDetailDialog({ executionId, onClose }: { executionId: Id | null; onClose: () => void }) {
  const qc = useQueryClient();
  const [openRunId, setOpenRunId] = useState<Id | null>(null);
  const detail = useQuery({
    queryKey: ["automation-execution", executionId],
    queryFn: () => invoke<ExecutionDetail>("automations.execution_detail", { id: executionId }),
    enabled: executionId != null,
    refetchInterval: (q) => {
      const s = q.state.data?.execution.status;
      return s === "running" || s === "pending" || s === "waiting_approval" ? 1500 : false;
    },
  });

  const cancel = useMutation({
    mutationFn: (id: Id) => invoke("automations.cancel", { id }),
    onSuccess: () => {
      toast.success("Ejecución cancelada");
      qc.invalidateQueries({ queryKey: ["automation-executions"] });
      qc.invalidateQueries({ queryKey: ["automation-execution", executionId] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo cancelar"),
  });

  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: Id; approved: boolean }) =>
      invoke(approved ? "automations.approve" : "automations.reject", { approval_id: id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["automation-execution", executionId] });
      qc.invalidateQueries({ queryKey: ["automation-approvals"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo decidir"),
  });

  const d = detail.data;
  const active = d && ["pending", "running", "waiting_approval"].includes(d.execution.status);

  return (
    <>
      <Dialog open={executionId != null} onOpenChange={(o) => !o && onClose()}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              {d && <AutomationIcon icon="workflow" className="text-primary" />}
              {d?.execution.automation_name ?? "Ejecución"}
              {d?.execution.test_run && <Badge variant="secondary">Prueba</Badge>}
            </DialogTitle>
            <DialogDescription>
              {d ? (
                <span className="flex flex-wrap items-center gap-2">
                  <ExecutionBadge status={d.execution.status} />
                  <ModeBadge mode={d.execution.mode} />
                  <span>{d.execution.trigger_detail}</span>
                  <span>· {formatDateTime(d.execution.created_at)}</span>
                  <span>· {executionDuration(d.execution.duration_ms)}</span>
                </span>
              ) : (
                "Detalle de la ejecución"
              )}
            </DialogDescription>
          </DialogHeader>

          {detail.isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-16 w-full rounded-lg" />
              <Skeleton className="h-16 w-full rounded-lg" />
              <Skeleton className="h-16 w-full rounded-lg" />
            </div>
          ) : detail.isError || !d ? (
            <p className="text-sm text-muted-foreground">
              {detail.error instanceof ApiError ? detail.error.message : "No se pudo cargar la ejecución."}
            </p>
          ) : (
            <div className="space-y-4">
              <div className="flex flex-wrap gap-3 text-xs text-muted-foreground">
                <span>Pasos: <b className="text-foreground">{d.execution.steps_done}</b></span>
                <span>Agentes: <b className="text-foreground">{d.execution.agent_calls}</b></span>
                <span>Tokens: <b className="text-foreground">{d.execution.tokens_used}</b></span>
              </div>

              {d.execution.error && (
                <p className="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                  {d.execution.error}
                </p>
              )}

              {d.approvals.filter((a) => a.status === "pending").map((a) => (
                <div key={a.id} className="rounded-lg border border-warning/40 bg-warning/10 px-3 py-2.5">
                  <p className="flex items-center gap-1.5 text-xs font-medium">
                    <ShieldCheck className="h-3.5 w-3.5 text-warning" />
                    Autorización requerida
                  </p>
                  <p className="mt-1 text-sm">{a.preview}</p>
                  <div className="mt-2 flex gap-2">
                    <Button size="sm" onClick={() => decide.mutate({ id: a.id, approved: true })} disabled={decide.isPending}>
                      {decide.isPending ? <Loader2 className="animate-spin" /> : null}
                      Autorizar
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => decide.mutate({ id: a.id, approved: false })} disabled={decide.isPending}>
                      Rechazar
                    </Button>
                  </div>
                </div>
              ))}

              <section>
                <h4 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Pasos ejecutados</h4>
                {d.steps.length === 0 ? (
                  <p className="text-xs text-muted-foreground">Sin pasos registrados todavía.</p>
                ) : (
                  <ol className="space-y-1.5">
                    {d.steps.map((s) => (
                      <StepRow key={s.id} step={s} onOpenRun={setOpenRunId} />
                    ))}
                  </ol>
                )}
              </section>

              {d.agent_runs.length > 0 && (
                <section>
                  <h4 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Agentes implicados</h4>
                  <ul className="space-y-1">
                    {d.agent_runs.map((r) => (
                      <li key={r.id} className="flex items-center gap-2 text-xs">
                        <Bot className="h-3.5 w-3.5 text-muted-foreground" />
                        <button className="font-medium hover:underline cursor-pointer" onClick={() => setOpenRunId(r.id)}>
                          {AGENT_LABEL[r.agent_id] ?? r.agent_id}
                        </button>
                        <span className="text-muted-foreground">{r.tokens_used} tokens</span>
                      </li>
                    ))}
                  </ul>
                </section>
              )}

              {typeof d.execution.result?.agent_summary === "string" && (
                <section className="prose prose-sm dark:prose-invert max-w-none rounded-lg border border-border bg-muted/40 p-3 text-sm">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{d.execution.result.agent_summary as string}</ReactMarkdown>
                </section>
              )}

              {active && (
                <div className="flex justify-end">
                  <Button variant="destructive" size="sm" onClick={() => cancel.mutate(d.execution.id)} disabled={cancel.isPending}>
                    {cancel.isPending ? <Loader2 className="animate-spin" /> : <X />}
                    Cancelar ejecución
                  </Button>
                </div>
              )}
            </div>
          )}
        </DialogContent>
      </Dialog>
      <RunDetailDialog runId={openRunId} onClose={() => setOpenRunId(null)} />
    </>
  );
}
