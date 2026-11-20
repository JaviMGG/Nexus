import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { toast } from "sonner";
import {
  ArrowUp,
  ChevronDown,
  ChevronRight,
  Info,
  Loader2,
  MessageSquarePlus,
  MessagesSquare,
  MoreHorizontal,
  Pencil,
  Sparkles,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type {
  AgentDef,
  AgentRun,
  ChatMessage,
  ChatSendResult,
  Conversation,
  Id,
  ToolCall,
} from "@/lib/types";
import { AGENT_LABEL, cn, formatDateTime, relativeTime } from "@/lib/utils";
import { useChatStream } from "@/hooks/useChatStream";
import { useAiStatus } from "@/components/layout/AiStatusPill";
import { ApprovalCard } from "@/components/chat/ApprovalCard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { EmptyState } from "@/components/ui/empty-state";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";

const SUGGESTIONS = [
  "Organízame la semana que viene",
  "¿Qué proyectos tengo atrasados?",
  "Resume mis notas sobre Rust",
];

function agentLabel(id: string | null | undefined): string {
  if (!id) return "NEXUS";
  return AGENT_LABEL[id] ?? id;
}

function ToolMessage({ message }: { message: ChatMessage }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="max-w-[85%]">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] text-muted-foreground transition-colors hover:bg-muted cursor-pointer"
      >
        {open ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        <Wrench className="h-3 w-3" />
        {message.role === "tool" ? "Resultado de herramienta" : "Mensaje de sistema"}
      </button>
      {open && (
        <pre className="mt-1 max-h-48 overflow-auto rounded-md bg-muted p-2 font-mono text-[11px] leading-relaxed animate-fade-in">
          {message.content}
        </pre>
      )}
    </div>
  );
}

function ToolChip({ call }: { call: ToolCall }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-border bg-card px-2 py-0.5 text-[11px] text-muted-foreground">
      {call.status === "pending" || call.status === "pending_approval" ? (
        <Loader2 className="h-3 w-3 animate-spin text-primary" />
      ) : call.status === "ok" ? (
        <span className="text-success">✓</span>
      ) : (
        <span className="text-destructive">✕</span>
      )}
      {call.tool_name}
    </span>
  );
}

export function ChatPage() {
  const qc = useQueryClient();
  const { data: aiStatus } = useAiStatus();
  const [activeId, setActiveId] = useState<Id | null>(null);
  const [input, setInput] = useState("");
  const [agentId, setAgentId] = useState("orchestrator");
  const [renameTarget, setRenameTarget] = useState<Conversation | null>(null);
  const [renameTitle, setRenameTitle] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<Conversation | null>(null);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const conversations = useQuery({
    queryKey: ["conversations"],
    queryFn: () => invoke<Conversation[]>("chat.conversations"),
  });
  const agents = useQuery({
    queryKey: ["agents"],
    queryFn: () => invoke<AgentDef[]>("agents.list"),
  });
  const messages = useQuery({
    queryKey: ["messages", activeId],
    queryFn: () => invoke<ChatMessage[]>("chat.messages", { conversation_id: activeId }),
    enabled: activeId != null,
  });

  const stream = useChatStream(activeId);
  const aiConnected = aiStatus?.state === "connected";
  const activeConversation = (conversations.data ?? []).find((c) => c.id === activeId) ?? null;
  const busy = stream.activeRun != null || stream.streamingText.length > 0;

  const sortedConversations = useMemo(
    () => [...(conversations.data ?? [])].sort((a, b) => b.updated_at.localeCompare(a.updated_at)),
    [conversations.data],
  );

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages.data, stream.streamingText, stream.approvals.length, stream.activeRun?.id, activeId]);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [input]);

  const send = useMutation({
    mutationFn: () =>
      invoke<ChatSendResult>("chat.send", {
        conversation_id: activeId ?? undefined,
        agent_id: agentId,
        content: input.trim(),
      }),
    onSuccess: (r) => {
      setInput("");
      setActiveId(r.conversation_id);
      qc.invalidateQueries({ queryKey: ["conversations"] });
      qc.invalidateQueries({ queryKey: ["messages", r.conversation_id] });
      if (!r.ai_available) {
        toast.info("Mensaje guardado en modo manual", {
          description: "Configura un proveedor de IA en Ajustes para recibir respuestas automáticas.",
        });
      }
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo enviar el mensaje"),
  });

  const createConversation = useMutation({
    mutationFn: (agent: string) => invoke<Conversation>("chat.create_conversation", { agent_id: agent }),
    onSuccess: (c) => {
      qc.invalidateQueries({ queryKey: ["conversations"] });
      setActiveId(c.id);
      setAgentId(c.agent_id);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo crear la conversación"),
  });

  const rename = useMutation({
    mutationFn: () => invoke("chat.rename_conversation", { id: renameTarget!.id, title: renameTitle.trim() }),
    onSuccess: () => {
      toast.success("Conversación renombrada");
      qc.invalidateQueries({ queryKey: ["conversations"] });
      setRenameTarget(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo renombrar"),
  });

  const remove = useMutation({
    mutationFn: () => invoke("chat.delete_conversation", { id: deleteTarget!.id }),
    onSuccess: () => {
      toast.success("Conversación eliminada");
      qc.invalidateQueries({ queryKey: ["conversations"] });
      if (activeId === deleteTarget?.id) setActiveId(null);
      setDeleteTarget(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo eliminar"),
  });

  const cancelRun = useMutation({
    mutationFn: (run: AgentRun) => invoke("agents.cancel", { run_id: run.id }),
    onSuccess: () => toast.success("Ejecución cancelada"),
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo cancelar"),
  });

  const selectConversation = (c: Conversation) => {
    setActiveId(c.id);
    setAgentId(c.agent_id || "orchestrator");
  };

  const submit = () => {
    if (!input.trim() || send.isPending || busy) return;
    send.mutate();
  };

  const messageList = messages.data ?? [];

  return (
    <div className="flex h-full animate-fade-in">
      <aside className="flex w-72 shrink-0 flex-col border-r border-border bg-card/50">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-semibold">Conversaciones</h2>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button size="icon-sm" variant="ghost" aria-label="Nueva conversación">
                <MessageSquarePlus />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              <DropdownMenuLabel>Nueva conversación con…</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {(agents.data ?? []).map((a) => (
                <DropdownMenuItem
                  key={a.id}
                  onClick={() => createConversation.mutate(a.id)}
                  disabled={createConversation.isPending}
                >
                  <Sparkles className="text-primary" />
                  {a.name}
                </DropdownMenuItem>
              ))}
              {(agents.data ?? []).length === 0 && (
                <DropdownMenuItem onClick={() => createConversation.mutate("orchestrator")}>
                  <Sparkles className="text-primary" /> NEXUS Orchestrator
                </DropdownMenuItem>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {conversations.isLoading ? (
            <div className="space-y-2 p-1">
              {Array.from({ length: 5 }).map((_, i) => (
                <div key={i} className="space-y-1.5 rounded-lg p-2">
                  <Skeleton className="h-4 w-3/4" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
              ))}
            </div>
          ) : sortedConversations.length === 0 ? (
            <p className="px-3 py-8 text-center text-xs text-muted-foreground">
              Todavía no hay conversaciones. Empieza una nueva cuando quieras.
            </p>
          ) : (
            <ul className="space-y-0.5">
              {sortedConversations.map((c) => (
                <li key={c.id} className="group relative">
                  <button
                    onClick={() => selectConversation(c)}
                    className={cn(
                      "w-full rounded-lg px-3 py-2.5 pr-8 text-left transition-colors cursor-pointer",
                      activeId === c.id ? "bg-accent text-accent-foreground" : "hover:bg-muted",
                    )}
                  >
                    <span className="block truncate text-sm font-medium">{c.title || "Nueva conversación"}</span>
                    <span className="mt-0.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
                      {agentLabel(c.agent_id)}
                      <span>·</span>
                      {relativeTime(c.updated_at)}
                    </span>
                  </button>
                  <div className="absolute right-1.5 top-1/2 -translate-y-1/2 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button size="icon-sm" variant="ghost" aria-label="Opciones">
                          <MoreHorizontal />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => { setRenameTarget(c); setRenameTitle(c.title); }}>
                          <Pencil /> Renombrar
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          className="text-destructive focus:text-destructive"
                          onClick={() => setDeleteTarget(c)}
                        >
                          <Trash2 /> Eliminar
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>

      <section className="flex min-w-0 flex-1 flex-col">
        {!aiConnected && (
          <div className="flex items-center gap-2 border-b border-border bg-muted/60 px-5 py-2 text-xs text-muted-foreground">
            <Info className="h-3.5 w-3.5 shrink-0" />
            <span>
              Sin IA conectada. Puedes escribir y NEXUS te responderá en modo manual.{" "}
              <Link to="/settings" className="text-primary underline-offset-2 hover:underline">
                Configura un proveedor en Ajustes
              </Link>
              .
            </span>
          </div>
        )}

        <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto max-w-3xl space-y-4 px-6 py-6">
            {activeId == null ? (
              <EmptyState
                icon={MessagesSquare}
                title="Habla con NEXUS"
                description="Tu centro de control conversacional. Pregunta, planifica o delega trabajo en tu equipo de agentes. Funciona con o sin IA."
                className="py-16"
                action={
                  <div className="flex max-w-md flex-col gap-2">
                    {SUGGESTIONS.map((s) => (
                      <button
                        key={s}
                        onClick={() => { setInput(s); textareaRef.current?.focus(); }}
                        className="rounded-lg border border-border bg-card px-4 py-2.5 text-left text-sm text-muted-foreground shadow-sm transition-all hover:border-ring/40 hover:bg-accent hover:text-accent-foreground cursor-pointer"
                      >
                        {s}
                      </button>
                    ))}
                  </div>
                }
              />
            ) : messages.isLoading ? (
              <div className="space-y-4">
                <Skeleton className="ml-auto h-10 w-2/3 rounded-2xl" />
                <Skeleton className="h-24 w-3/4 rounded-2xl" />
                <Skeleton className="ml-auto h-10 w-1/2 rounded-2xl" />
              </div>
            ) : (
              <>
                {messageList.length === 0 && !busy && (
                  <EmptyState
                    icon={MessagesSquare}
                    title="Conversación vacía"
                    description="Escribe tu primer mensaje o prueba con una sugerencia."
                    action={
                      <div className="flex max-w-md flex-col gap-2">
                        {SUGGESTIONS.map((s) => (
                          <button
                            key={s}
                            onClick={() => { setInput(s); textareaRef.current?.focus(); }}
                            className="rounded-lg border border-border bg-card px-4 py-2.5 text-left text-sm text-muted-foreground shadow-sm transition-all hover:border-ring/40 hover:bg-accent hover:text-accent-foreground cursor-pointer"
                          >
                            {s}
                          </button>
                        ))}
                      </div>
                    }
                  />
                )}
                {messageList.map((m) =>
                  m.role === "user" ? (
                    <div key={m.id} className="flex justify-end animate-fade-in">
                      <div className="max-w-[80%] rounded-2xl rounded-br-md bg-primary px-4 py-2.5 text-sm text-primary-foreground shadow-sm">
                        <p className="whitespace-pre-wrap">{m.content}</p>
                        <p className="mt-1 text-right text-[10px] opacity-70">{formatDateTime(m.created_at)}</p>
                      </div>
                    </div>
                  ) : m.role === "assistant" ? (
                    <div key={m.id} className="flex justify-start animate-fade-in">
                      <div className="max-w-[85%]">
                        <p className="mb-1 flex items-center gap-1.5 px-1 text-[11px] text-muted-foreground">
                          <Sparkles className="h-3 w-3 text-primary" />
                          {agentLabel(m.agent_id)}
                          <span>·</span>
                          {formatDateTime(m.created_at)}
                        </p>
                        <div className="rounded-2xl rounded-bl-md border border-border bg-card px-4 py-2.5 shadow-sm">
                          <div className="markdown-body">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{m.content}</ReactMarkdown>
                          </div>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <ToolMessage key={m.id} message={m} />
                  ),
                )}
                {stream.streamingText && (
                  <div className="flex justify-start animate-fade-in">
                    <div className="max-w-[85%]">
                      <p className="mb-1 flex items-center gap-1.5 px-1 text-[11px] text-muted-foreground">
                        <Sparkles className="h-3 w-3 text-primary" />
                        {agentLabel(stream.streamingAgentId)}
                      </p>
                      <div className="rounded-2xl rounded-bl-md border border-border bg-card px-4 py-2.5 shadow-sm">
                        <div className="markdown-body">
                          <ReactMarkdown remarkPlugins={[remarkGfm]}>{stream.streamingText}</ReactMarkdown>
                        </div>
                        <span className="ml-0.5 inline-block animate-pulse text-primary">▍</span>
                      </div>
                    </div>
                  </div>
                )}
                {stream.approvals.map((a) => (
                  <ApprovalCard key={a.id} approval={a} />
                ))}
              </>
            )}
          </div>
        </div>

        <div className="border-t border-border bg-card/50 px-6 py-3">
          <div className="mx-auto max-w-3xl space-y-2">
            {stream.activeRun && (
              <div className="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-3 py-2 animate-slide-up">
                <Loader2 className="h-3.5 w-3.5 animate-spin text-primary" />
                <span className="text-xs font-medium">
                  {agentLabel(stream.activeRun.agent_id)}
                  {" · "}
                  {stream.activeRun.status === "waiting_approval" ? "esperando aprobación" : "ejecutando"}
                </span>
                {stream.toolCalls.map((c) => (
                  <ToolChip key={c.id} call={c} />
                ))}
                <Button
                  size="sm"
                  variant="outline"
                  className="ml-auto h-7 text-xs"
                  onClick={() => cancelRun.mutate(stream.activeRun!)}
                  disabled={cancelRun.isPending}
                >
                  <X /> Cancelar
                </Button>
              </div>
            )}
            <div className="flex items-end gap-2">
              <div className="min-w-0 flex-1 rounded-xl border border-input bg-card shadow-sm transition-shadow focus-within:ring-2 focus-within:ring-ring">
                <Textarea
                  ref={textareaRef}
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault();
                      submit();
                    }
                  }}
                  placeholder="Escribe a NEXUS… (Enter envía, Shift+Enter salto de línea)"
                  rows={1}
                  className="max-h-40 min-h-[44px] border-0 bg-transparent py-3 shadow-none focus-visible:ring-0"
                />
              </div>
              <Select value={agentId} onValueChange={setAgentId}>
                <SelectTrigger className="h-11 w-44 shrink-0 text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {(agents.data ?? []).map((a) => (
                    <SelectItem key={a.id} value={a.id}>{a.name}</SelectItem>
                  ))}
                  {(agents.data ?? []).length === 0 && (
                    <SelectItem value="orchestrator">NEXUS Orchestrator</SelectItem>
                  )}
                </SelectContent>
              </Select>
              <Button
                size="icon"
                className="h-11 w-11 shrink-0 rounded-xl"
                onClick={submit}
                disabled={!input.trim() || send.isPending || busy}
                aria-label="Enviar"
              >
                {send.isPending || busy ? <Loader2 className="animate-spin" /> : <ArrowUp />}
              </Button>
            </div>
            {activeConversation && (
              <p className="px-1 text-[10px] text-muted-foreground">
                {activeConversation.title || "Nueva conversación"} · {agentLabel(agentId)}
                {busy && " · respuesta en curso…"}
              </p>
            )}
          </div>
        </div>
      </section>

      <Dialog open={renameTarget != null} onOpenChange={(o) => !o && setRenameTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Renombrar conversación</DialogTitle>
            <DialogDescription>Dale un nombre que te ayude a encontrarla después.</DialogDescription>
          </DialogHeader>
          <Input
            autoFocus
            value={renameTitle}
            onChange={(e) => setRenameTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && renameTitle.trim()) rename.mutate();
            }}
            placeholder="Título…"
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setRenameTarget(null)}>Cancelar</Button>
            <Button onClick={() => rename.mutate()} disabled={!renameTitle.trim() || rename.isPending}>
              {rename.isPending && <Loader2 className="animate-spin" />}
              Guardar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteTarget != null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Eliminar conversación</DialogTitle>
            <DialogDescription>
              «{deleteTarget?.title || "Nueva conversación"}» y todos sus mensajes se eliminarán. Esta acción no se puede deshacer.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancelar</Button>
            <Button variant="destructive" onClick={() => remove.mutate()} disabled={remove.isPending}>
              {remove.isPending && <Loader2 className="animate-spin" />}
              Eliminar
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
