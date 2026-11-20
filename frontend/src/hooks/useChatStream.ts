import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
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
            setActiveRunId(run.id);
          } else {
            setActiveRunId((prev) => (prev === run.id ? null : prev));
            if (event.type === "run_finished") {
              setToolCalls((prev) => (prev.some((c) => c.run_id === run.id) ? [] : prev));
              setApprovals((prev) => prev.filter((a) => a.run_id !== run.id));
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
