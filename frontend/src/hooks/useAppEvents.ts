import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { subscribeEvents } from "@/lib/events";
import { invoke } from "@/lib/api";
import type { AppEvent } from "@/lib/types";
import { AGENT_LABEL } from "@/lib/utils";

const ENTITY_KEYS: Record<string, string[]> = {
  task: ["tasks", "dashboard", "projects"],
  project: ["projects", "dashboard", "tasks"],
  note: ["notes", "dashboard", "projects"],
  event: ["events", "dashboard"],
  inbox: ["inbox", "dashboard"],
  run: ["runs", "dashboard", "activity"],
  settings: ["settings"],
};

export function useAppEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    return subscribeEvents((event: AppEvent) => {
      switch (event.type) {
        case "data_changed": {
          const keys = ENTITY_KEYS[event.entity] ?? [event.entity];
          keys.forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
          break;
        }
        case "run_started":
        case "run_updated":
        case "run_finished":
          qc.invalidateQueries({ queryKey: ["runs"] });
          qc.invalidateQueries({ queryKey: ["run", event.run.id] });
          qc.invalidateQueries({ queryKey: ["dashboard"] });
          if (event.type === "run_finished") {
            // Los agentes pueden haber modificado datos: refrescar entidades.
            ["tasks", "notes", "projects", "inbox", "activity"].forEach((k) =>
              qc.invalidateQueries({ queryKey: [k] }),
            );
          }
          if (event.type === "run_finished" && event.run.status === "failed") {
            toast.error(`${AGENT_LABEL[event.run.agent_id] ?? event.run.agent_id}: ejecución fallida`, {
              description: event.run.error ?? undefined,
            });
          }
          break;
        case "message_created":
          qc.invalidateQueries({ queryKey: ["messages", event.message.conversation_id] });
          qc.invalidateQueries({ queryKey: ["conversations"] });
          break;
        case "approval_requested": {
          const a = event.approval;
          toast.warning("Acción pendiente de autorización", {
            description: `${a.tool_name} · riesgo ${a.risk}`,
            duration: 15000,
            action: {
              label: "Revisar",
              onClick: () => {
                window.location.hash = "#/activity";
              },
            },
          });
          qc.invalidateQueries({ queryKey: ["approvals"] });
          break;
        }
        case "approval_resolved":
          qc.invalidateQueries({ queryKey: ["approvals"] });
          qc.invalidateQueries({ queryKey: ["runs"] });
          break;
        case "ai_status_changed":
          qc.setQueryData(["ai-status"], event.status);
          qc.invalidateQueries({ queryKey: ["settings"] });
          break;
        case "tool_started":
        case "tool_finished":
          qc.invalidateQueries({ queryKey: ["run", event.call.run_id] });
          break;
        case "message_delta":
          // El chat se suscribe directamente a los deltas.
          break;
      }
    });
  }, [qc]);
}

export async function approveAction(id: string) {
  await invoke("agents.approve", { approval_id: id });
}

export async function rejectAction(id: string) {
  await invoke("agents.reject", { approval_id: id });
}
