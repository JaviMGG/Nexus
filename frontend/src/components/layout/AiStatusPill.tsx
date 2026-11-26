import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { invoke } from "@/lib/api";
import type { AiStatus } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export function useAiStatus() {
  return useQuery({
    queryKey: ["ai-status"],
    queryFn: () => invoke<AiStatus>("ai.status"),
    refetchInterval: 60_000,
    retry: 0,
    placeholderData: { state: "disconnected", provider: null, model: null, message: null } as AiStatus,
  });
}

export function AiStatusPill({ compact = false }: { compact?: boolean }) {
  const { data } = useAiStatus();
  const navigate = useNavigate();
  const state = data?.state ?? "disconnected";
  const label = state === "connected" ? "IA conectada" : state === "error" ? "Error de IA" : "Sin IA";
  const dot =
    state === "connected" ? "bg-success" : state === "error" ? "bg-destructive" : "bg-muted-foreground/50";

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={() => navigate("/settings")}
          className={cn(
            "flex items-center gap-2 rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors cursor-pointer",
            compact && "h-7 w-7 justify-center rounded-md p-0",
          )}
        >
          <span className={cn("relative flex h-2 w-2")}>
            {state === "connected" && (
              <span className={cn("absolute inline-flex h-full w-full animate-ping rounded-full opacity-40", dot)} />
            )}
            <span className={cn("relative inline-flex h-2 w-2 rounded-full", dot)} />
          </span>
          {!compact && label}
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-64">
        <p className="font-medium mb-0.5">{label}</p>
        {data?.provider && <p>Proveedor: {data.provider} · {data.model}</p>}
        {data?.message && <p className="text-muted-foreground mt-0.5">{data.message}</p>}
        {state !== "connected" && (
          <p className="text-muted-foreground mt-0.5">NEXUS funciona con normalidad en modo manual. Configura la IA en Ajustes.</p>
        )}
      </TooltipContent>
    </Tooltip>
  );
}
