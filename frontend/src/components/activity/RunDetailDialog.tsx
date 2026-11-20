import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { toast } from "sonner";
import { ChevronDown, ChevronRight, GitBranch, Loader2, Wrench, X } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Id, RunDetail, RunStatus, ToolCall } from "@/lib/types";
import { AGENT_LABEL, RUN_STATUS_LABEL, cn, formatDateTime } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ApprovalCard } from "@/components/chat/ApprovalCard";

export const RUN_BADGE_VARIANT: Record<RunStatus, "default" | "secondary" | "success" | "destructive" | "outline" | "warning"> = {
  pending: "secondary",
  running: "default",
  waiting_approval: "warning",
  completed: "success",
  failed: "destructive",
  cancelled: "outline",
};

const TOOL_BADGE_VARIANT: Record<ToolCall["status"], "outline" | "warning" | "success" | "destructive"> = {
  pending: "outline",
  pending_approval: "warning",
  ok: "success",
  error: "destructive",
  denied: "destructive",
};

const TOOL_STATUS_LABEL: Record<ToolCall["status"], string> = {
  pending: "En curso",
  pending_approval: "Esperando aprobación",
  ok: "OK",
  error: "Error",
  denied: "Denegada",
};

export function runDuration(started: string | null, finished: string | null): string {
  if (!started) return "—";
  const end = finished ? new Date(finished).getTime() : Date.now();
  const secs = Math.max(0, (end - new Date(started).getTime()) / 1000);
  return `${secs.toFixed(1)} s`;
}

function JsonBlock({ label, value }: { label: string; value: unknown }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground cursor-pointer"
      >
        {open ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        {label}
      </button>
      {open && (
        <pre className="mt-1 max-h-48 overflow-auto rounded-md bg-muted p-2 font-mono text-[11px] leading-relaxed animate-fade-in">
          {typeof value === "string" ? value : JSON.stringify(value, null, 2)}
        </pre>
      )}
    </div>
  );
}

export function RunDetailDialog({ runId, onClose }: { runId: Id | null; onClose: () => void }) {
  const qc = useQueryClient();
  const detail = useQuery({
    queryKey: ["run", runId],
    queryFn: () => invoke<RunDetail>("agents.run_detail", { id: runId }),
    enabled: runId != null,
  });

  const cancel = useMutation({
    mutationFn: () => invoke("agents.cancel", { run_id: runId }),
    onSuccess: () => {
      toast.success("Ejecución cancelada");
      qc.invalidateQueries({ queryKey: ["runs"] });
      qc.invalidateQueries({ queryKey: ["run", runId] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo cancelar"),
  });

  const run = detail.data?.run;
  const isActive = run != null && ["pending", "running", "waiting_approval"].includes(run.status);

  return (
    <Dialog open={runId != null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 pr-6">
            Detalle de ejecución
            {run && (
              <Badge variant={RUN_BADGE_VARIANT[run.status]} className={cn(run.status === "running" && "animate-pulse")}>
                {RUN_STATUS_LABEL[run.status]}
              </Badge>
            )}
          </DialogTitle>
          <DialogDescription>
            {run
              ? `${AGENT_LABEL[run.agent_id] ?? run.agent_id} · ${formatDateTime(run.created_at)} · ${runDuration(run.started_at, run.finished_at)} · ${run.iterations} iteraciones · ${run.tokens_used} tokens`
              : "Cargando…"}
          </DialogDescription>
        </DialogHeader>

        {detail.isLoading ? (
          <div className="space-y-3">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-24 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : detail.isError || !detail.data || !run ? (
          <p className="text-xs text-destructive">
            {detail.error instanceof ApiError ? detail.error.message : "No se pudo cargar el detalle."}
          </p>
        ) : (
          <div className="space-y-5">
            <section>
              <h4 className="mb-1 text-xs font-semibold text-muted-foreground">Objetivo</h4>
              <p className="whitespace-pre-wrap rounded-lg border border-border bg-card p-3 text-sm">{run.goal}</p>
            </section>

            {run.result && (
              <section>
                <h4 className="mb-1 text-xs font-semibold text-muted-foreground">Resultado</h4>
                <div className="markdown-body rounded-lg border border-border bg-card p-3">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{run.result}</ReactMarkdown>
                </div>
              </section>
            )}

            {run.error && (
              <section>
                <h4 className="mb-1 text-xs font-semibold text-destructive">Error</h4>
                <pre className="max-h-48 overflow-auto rounded-lg border border-destructive/40 bg-destructive/10 p-3 font-mono text-xs text-destructive">
                  {run.error}
                </pre>
              </section>
            )}

            {detail.data.approvals.length > 0 && (
              <section className="space-y-2">
                <h4 className="text-xs font-semibold text-muted-foreground">Aprobaciones</h4>
                {detail.data.approvals.map((a) => (
                  <ApprovalCard key={a.id} approval={a} />
                ))}
              </section>
            )}

            {detail.data.tool_calls.length > 0 && (
              <section className="space-y-2">
                <h4 className="text-xs font-semibold text-muted-foreground">
                  Herramientas ({detail.data.tool_calls.length})
                </h4>
                <ol className="space-y-2">
                  {detail.data.tool_calls.map((call) => (
                    <li key={call.id} className="rounded-lg border border-border bg-card p-3">
                      <div className="flex flex-wrap items-center gap-2">
                        {call.status === "pending" || call.status === "pending_approval" ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin text-primary" />
                        ) : (
                          <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
                        )}
                        <span className="font-mono text-xs font-medium">{call.tool_name}</span>
                        <Badge variant={TOOL_BADGE_VARIANT[call.status]}>{TOOL_STATUS_LABEL[call.status]}</Badge>
                        {call.duration_ms != null && (
                          <span className="text-[11px] text-muted-foreground">{call.duration_ms} ms</span>
                        )}
                        <span className="ml-auto text-[11px] text-muted-foreground">{formatDateTime(call.created_at)}</span>
                      </div>
                      <div className="mt-2 space-y-1">
                        <JsonBlock label="Argumentos" value={call.args} />
                        {call.result != null && <JsonBlock label="Resultado" value={call.result} />}
                        {call.error && (
                          <pre className="max-h-32 overflow-auto rounded-md bg-destructive/10 p-2 font-mono text-[11px] text-destructive">
                            {call.error}
                          </pre>
                        )}
                      </div>
                    </li>
                  ))}
                </ol>
              </section>
            )}

            {detail.data.children.length > 0 && (
              <section className="space-y-2">
                <h4 className="text-xs font-semibold text-muted-foreground">Delegaciones</h4>
                <ul className="space-y-1.5">
                  {detail.data.children.map((child) => (
                    <li key={child.id} className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2">
                      <GitBranch className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                      <span className="text-xs font-medium">{AGENT_LABEL[child.agent_id] ?? child.agent_id}</span>
                      <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">{child.goal}</span>
                      <Badge variant={RUN_BADGE_VARIANT[child.status]}>{RUN_STATUS_LABEL[child.status]}</Badge>
                    </li>
                  ))}
                </ul>
              </section>
            )}

            {isActive && (
              <div className="flex justify-end">
                <Button variant="destructive" size="sm" onClick={() => cancel.mutate()} disabled={cancel.isPending}>
                  {cancel.isPending ? <Loader2 className="animate-spin" /> : <X />}
                  Cancelar ejecución
                </Button>
              </div>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
