import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Check, Loader2, ShieldAlert, X } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Approval, Risk } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

const RISK_LABEL: Record<Risk, string> = {
  read: "Lectura",
  write: "Escritura",
  destructive: "Destructiva",
};

const RISK_VARIANT: Record<Risk, "outline" | "warning" | "destructive"> = {
  read: "outline",
  write: "warning",
  destructive: "destructive",
};

export function ApprovalCard({ approval, className, compact }: { approval: Approval; className?: string; compact?: boolean }) {
  const qc = useQueryClient();

  const decide = useMutation({
    mutationFn: (approve: boolean) =>
      invoke(approve ? "agents.approve" : "agents.reject", { approval_id: approval.id }),
    onSuccess: (_, approve) => {
      toast.success(approve ? "Acción aprobada" : "Acción rechazada");
      qc.invalidateQueries({ queryKey: ["approvals"] });
      qc.invalidateQueries({ queryKey: ["runs"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo resolver la aprobación"),
  });

  const pending = approval.status === "pending";

  return (
    <div
      className={cn(
        "rounded-lg border border-warning/40 bg-warning/10 p-3 animate-slide-up",
        className,
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <ShieldAlert className="h-4 w-4 text-warning" />
        <span className="text-sm font-medium">{approval.tool_name}</span>
        <Badge variant={RISK_VARIANT[approval.risk]}>{RISK_LABEL[approval.risk]}</Badge>
        {!pending && (
          <Badge variant={approval.status === "approved" ? "success" : "destructive"}>
            {approval.status === "approved" ? "Aprobada" : "Rechazada"}
          </Badge>
        )}
        {pending && (
          <div className="ml-auto flex gap-2">
            <Button
              size="sm"
              onClick={() => decide.mutate(true)}
              disabled={decide.isPending}
            >
              {decide.isPending && decide.variables === true ? <Loader2 className="animate-spin" /> : <Check />}
              Aprobar
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => decide.mutate(false)}
              disabled={decide.isPending}
            >
              {decide.isPending && decide.variables === false ? <Loader2 className="animate-spin" /> : <X />}
              Rechazar
            </Button>
          </div>
        )}
      </div>
      {pending && !compact && (
        <p className="mt-1 text-xs text-muted-foreground">
          Un agente solicita autorización para ejecutar esta acción.
        </p>
      )}
      <pre className="mt-2 max-h-40 overflow-auto rounded-md bg-muted p-2 font-mono text-[11px] leading-relaxed select-text">
        {JSON.stringify(approval.args, null, 2)}
      </pre>
    </div>
  );
}
