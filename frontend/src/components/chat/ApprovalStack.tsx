import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { CheckCheck, Loader2, ShieldAlert, X } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Approval } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { ApprovalCard } from "./ApprovalCard";

/** Pila de aprobaciones pendientes con decisión individual o en bloque. */
export function ApprovalStack({ approvals }: { approvals: Approval[] }) {
  const qc = useQueryClient();
  const pending = approvals.filter((a) => a.status === "pending");

  const decideAll = useMutation({
    mutationFn: async (approve: boolean) => {
      const results = await Promise.allSettled(
        pending.map((a) =>
          invoke(approve ? "agents.approve" : "agents.reject", { approval_id: a.id }),
        ),
      );
      const failed = results.filter((r) => r.status === "rejected").length;
      return { approve, failed, total: pending.length };
    },
    onSuccess: ({ approve, failed, total }) => {
      if (failed > 0) {
        toast.warning(`${total - failed} de ${total} ${approve ? "aprobadas" : "rechazadas"}`, {
          description: "Alguna ya había sido decidida o falló; revisa las pendientes.",
        });
      } else {
        toast.success(approve ? `${total} acciones aprobadas` : `${total} acciones rechazadas`);
      }
      qc.invalidateQueries({ queryKey: ["approvals"] });
      qc.invalidateQueries({ queryKey: ["runs"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudieron resolver las aprobaciones"),
  });

  if (pending.length === 0) return null;

  return (
    <div className="space-y-2 rounded-xl border border-warning/40 bg-warning/5 p-3 animate-slide-up">
      <div className="flex flex-wrap items-center gap-2">
        <ShieldAlert className="h-4 w-4 shrink-0 text-warning" />
        <span className="text-sm font-medium">
          {pending.length === 1
            ? "Un agente pide tu autorización"
            : `${pending.length} acciones esperan tu autorización`}
        </span>
        {pending.length > 1 && (
          <div className="ml-auto flex gap-2">
            <Button
              size="sm"
              className="h-7 text-xs"
              onClick={() => decideAll.mutate(true)}
              disabled={decideAll.isPending}
            >
              {decideAll.isPending && decideAll.variables === true ? <Loader2 className="animate-spin" /> : <CheckCheck />}
              Aprobar todo
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7 text-xs"
              onClick={() => decideAll.mutate(false)}
              disabled={decideAll.isPending}
            >
              {decideAll.isPending && decideAll.variables === false ? <Loader2 className="animate-spin" /> : <X />}
              Rechazar todo
            </Button>
          </div>
        )}
      </div>
      {pending.map((a) => (
        <ApprovalCard key={a.id} approval={a} compact />
      ))}
    </div>
  );
}