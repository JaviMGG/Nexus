import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@/lib/api";
import { subscribeEvents } from "@/lib/events";
import type { AgentRun, Approval, Id, RunStatus, ToolCall } from "@/lib/types";

const ACTIVE_STATUSES: RunStatus[] = ["pending", "running", "waiting_approval"];

export function isActiveRun(run: AgentRun): boolean {
  return ACTIVE_STATUSES.includes(run.status);
}

export interface ChatStreamState {
  streamingText: string;
  streamingAgentId: string | null;
  activeRun: AgentRun | null;
  toolCalls: ToolCall[];
  approvals: Approval[];
}

export function useChatStream(conversationId: Id | null): ChatStreamState {
  const qc = useQueryClient();
  const convRef = useRef(conversationId);
  convRef.current = conversationId;
  const runsRef = useRef<Map<Id, AgentRun>>(new Map());

  const [streamingText, setStreamingText] = useState("");
  const [streamingAgentId, setStreamingAgentId] = useState<string | null>(null);
  const [activeRunId, setActiveRunId] = useState<Id | null>(null);
  const [toolCalls, setToolCalls] = useState<ToolCall[]>([]);
  const [approvals, setApprovals] = useState<Approval[]>([]);

  const activeRunIdRef = useRef<Id | null>(null);
  activeRunIdRef.current = activeRunId;

  useEffect(() => {
    setStreamingText("");
    setStreamingAgentId(null);
    setActiveRunId(null);
    setToolCalls([]);
    setApprovals([]);
    if (conversationId == null) return;
    let cancelled = false;
    (async () => {
      try {
        const runs = await invoke<AgentRun[]>("agents.runs", { limit: 100 });
        if (cancelled || convRef.current !== conversationId) return;
        const mine = runs.filter((r) => r.conversation_id === conversationId);
        for (const r of mine) runsRef.current.set(r.id, r);
        const active = mine.filter(isActiveRun);
        const root = active.find((r) => r.parent_run_id == null) ?? active[0] ?? null;
        if (root) setActiveRunId(root.id);
        if (active.length > 0) {
          const details = await Promise.all(
            active.map((r) => invoke<{ approvals: Approval[]; tool_calls: ToolCall[] }>("agents.run_detail", { id: r.id }).catch(() => null)),
          );
          if (cancelled || convRef.current !== conversationId) return;
          const pending = details.flatMap((d) => d?.approvals ?? []).filter((a) => a.status === "pending");
          setApprovals((prev) => {
            const seen = new Set(prev.map((a) => a.id));
            return [...prev, ...pending.filter((a) => !seen.has(a.id))];
          });
          if (root) {
            const calls = details.flatMap((d) => d?.tool_calls ?? []).filter((c) => c.status === "pending" || c.status === "pending_approval");
            setToolCalls((prev) => {
              const seen = new Set(prev.map((c) => c.id));
              return [...prev, ...calls.filter((c) => !seen.has(c.id))];
            });
          }
        }
      } catch {
        // sin conexión o backend antiguo: el stream en vivo sigue funcionando
      }
    })();
    return () => { cancelled = true; };
  }, [conversationId]);

  useEffect(() => {
    return subscribeEvents((event) => {
      switch (event.type) {
        case "run_started":
        case "run_updated":
        case "run_finished": {
          const run = event.run;
          runsRef.current.set(run.id, run);
          const belongs =
            run.conversation_id != null && run.conversation_id === convRef.current;
          if (!belongs) break;
          if (isActiveRun(run)) {
            // El run raíz manda; un hijo no debe sustituirlo en la barra de estado.
            setActiveRunId((prev) => {
              if (prev == null) return run.id;
              const current = runsRef.current.get(prev);
              if (current && current.parent_run_id == null && run.parent_run_id != null) return prev;
              return run.id;
            });
          } else {
            if (event.type === "run_finished") {
              setToolCalls((prev) => prev.filter((c) => c.run_id !== run.id));
              setApprovals((prev) => prev.filter((a) => a.run_id !== run.id));
              setActiveRunId((prev) => {
                if (prev === run.id) return null;
                const current = prev ? runsRef.current.get(prev) : null;
                // Si el activo era un hijo de este run, vuelve al padre si sigue vivo.
                if (current && current.parent_run_id === run.id) {
                  const parent = runsRef.current.get(run.id);
                  return parent && isActiveRun(parent) ? parent.id : null;
                }
                return prev;
              });
            } else {
              setActiveRunId((prev) => (prev === run.id ? null : prev));
            }
          }
          break;
        }
        case "message_delta": {
          if (event.conversation_id !== convRef.current) break;
          const run = runsRef.current.get(event.run_id);
          if (run) setStreamingAgentId(run.agent_id);
          setStreamingText((prev) => prev + event.delta);
          break;
        }
        case "message_created": {
          if (event.message.conversation_id !== convRef.current) break;
          setStreamingText("");
          setStreamingAgentId(null);
          qc.invalidateQueries({ queryKey: ["messages", event.message.conversation_id] });
          break;
        }
        case "tool_started":
        case "tool_finished": {
          const call = event.call;
          const run = runsRef.current.get(call.run_id);
          const belongs =
            call.run_id === activeRunIdRef.current ||
            (run != null && run.conversation_id === convRef.current);
          if (!belongs) break;
          setToolCalls((prev) => {
            const idx = prev.findIndex((c) => c.id === call.id);
            if (idx === -1) return [...prev, call];
            const next = [...prev];
            next[idx] = call;
            return next;
          });
          break;
        }
        case "approval_requested": {
          const run = runsRef.current.get(event.approval.run_id);
          const belongs =
            event.approval.run_id === activeRunIdRef.current ||
            (run != null && run.conversation_id === convRef.current);
          if (!belongs) break;
          setApprovals((prev) =>
            prev.some((a) => a.id === event.approval.id) ? prev : [...prev, event.approval],
          );
          break;
        }
        case "approval_resolved": {
          setApprovals((prev) => prev.filter((a) => a.id !== event.approval.id));
          break;
        }
        default:
          break;
      }
    });
  }, [qc]);

  const activeRun = activeRunId ? runsRef.current.get(activeRunId) ?? null : null;

  return { streamingText, streamingAgentId, activeRun, toolCalls, approvals };
}
