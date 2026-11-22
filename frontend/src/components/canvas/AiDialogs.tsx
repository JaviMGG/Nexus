import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { toast } from "sonner";
import { Check, Loader2, Plus, Sparkles, X } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import type { AiExpandProposal, AiMindmapProposal, AiOrganizeProposal } from "./types";

function aiError(e: unknown): string {
  if (e instanceof ApiError && e.code === "ai_unavailable") return "Requiere proveedor de IA (Ajustes)";
  return e instanceof ApiError ? e.message : "La IA no pudo completar la propuesta";
}

export function useAiMutation<TArgs, TResult>(command: string) {
  return useMutation<TResult, unknown, TArgs>({
    mutationFn: (args) => invoke<TResult>(command, args as Record<string, unknown>),
    onError: (e) => toast.error(aiError(e)),
  });
}

// ---------------------------------------------------------------------------
// Mapa mental propuesto: preview en lista + añadir con ids nuevos
// ---------------------------------------------------------------------------

export function MindmapProposalDialog({
  proposal,
  pending,
  onClose,
  onAdd,
}: {
  proposal: AiMindmapProposal | null;
  pending: boolean;
  onClose: () => void;
  onAdd: (p: AiMindmapProposal) => void;
}) {
  return (
    <Dialog open={proposal != null || pending} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Propuesta de mapa mental</DialogTitle>
          <DialogDescription>Revisa los nodos antes de añadirlos. Nada se aplica automáticamente.</DialogDescription>
        </DialogHeader>
        {pending || !proposal ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Generando propuesta…
          </div>
        ) : (
          <>
            <ul className="max-h-72 space-y-1 overflow-y-auto">
              {proposal.elements.map((el, i) => (
                <li key={i} className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-1.5 text-xs">
                  <span className="h-3 w-3 shrink-0 rounded-full border border-border" style={{ background: el.color ?? "var(--muted)" }} />
                  <span className="min-w-0 flex-1 truncate">{el.text}</span>
                  <Badge variant="secondary" className="text-[9px]">{el.type}</Badge>
                </li>
              ))}
            </ul>
            <p className="text-[11px] text-muted-foreground">{proposal.elements.length} nodos · {proposal.edges.length} conexiones propuestas</p>
            <DialogFooter>
              <Button variant="outline" onClick={onClose}><X /> Descartar</Button>
              <Button onClick={() => { onAdd(proposal); onClose(); }}><Plus /> Añadir al canvas</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Expandir rama
// ---------------------------------------------------------------------------

export function ExpandProposalDialog({
  proposal,
  pending,
  onClose,
  onAdd,
}: {
  proposal: AiExpandProposal | null;
  pending: boolean;
  onClose: () => void;
  onAdd: (p: AiExpandProposal) => void;
}) {
  return (
    <Dialog open={proposal != null || pending} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Expandir rama con IA</DialogTitle>
          <DialogDescription>Ideas hijas propuestas para el elemento seleccionado.</DialogDescription>
        </DialogHeader>
        {pending || !proposal ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Pensando ideas…
          </div>
        ) : (
          <>
            <ul className="max-h-72 space-y-1 overflow-y-auto">
              {proposal.elements.map((el, i) => (
                <li key={i} className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-1.5 text-xs">
                  <Sparkles className="h-3.5 w-3.5 shrink-0 text-primary" />
                  <span className="min-w-0 flex-1">{el.text}</span>
                </li>
              ))}
            </ul>
            <DialogFooter>
              <Button variant="outline" onClick={onClose}><X /> Descartar</Button>
              <Button onClick={() => { onAdd(proposal); onClose(); }}><Plus /> Añadir al canvas</Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Resumen markdown
// ---------------------------------------------------------------------------

export function SummaryDialog({
  canvasId,
  open,
  onClose,
}: {
  canvasId: string;
  open: boolean;
  onClose: () => void;
}) {
  const [summary, setSummary] = useState<string | null>(null);
  const summarize = useMutation({
    mutationFn: () => invoke<{ summary: string }>("canvas.ai_summarize", { canvas_id: canvasId }),
    onSuccess: (r) => setSummary(r.summary),
    onError: (e) => {
      toast.error(aiError(e));
      onClose();
    },
  });
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (o) {
          setSummary(null);
          summarize.mutate();
        } else onClose();
      }}
    >
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Resumen del canvas</DialogTitle>
          <DialogDescription>Generado con IA a partir del contenido actual. No modifica nada.</DialogDescription>
        </DialogHeader>
        {summarize.isPending || !summary ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Resumiendo…
          </div>
        ) : (
          <div className="markdown-body max-h-96 overflow-y-auto text-sm">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{summary}</ReactMarkdown>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Organizar: conexiones propuestas, aceptar una a una o todas
// ---------------------------------------------------------------------------

export function OrganizeDialog({
  proposal,
  pending,
  titleOf,
  onClose,
  onAccept,
}: {
  proposal: AiOrganizeProposal | null;
  pending: boolean;
  titleOf: (id: string) => string;
  onClose: () => void;
  onAccept: (connections: AiOrganizeProposal["connections"]) => void;
}) {
  const [accepted, setAccepted] = useState<Set<number>>(new Set());
  const remaining = (proposal?.connections ?? []).filter((_, i) => !accepted.has(i));
  return (
    <Dialog
      open={proposal != null || pending}
      onOpenChange={(o) => {
        if (!o) {
          setAccepted(new Set());
          onClose();
        }
      }}
    >
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Conexiones propuestas por IA</DialogTitle>
          <DialogDescription>Acepta solo las que tengan sentido; se añaden como flechas editables.</DialogDescription>
        </DialogHeader>
        {pending || !proposal ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Analizando el canvas…
          </div>
        ) : proposal.connections.length === 0 ? (
          <p className="py-6 text-center text-xs text-muted-foreground">La IA no encontró conexiones nuevas que proponer.</p>
        ) : (
          <>
            <ul className="max-h-72 space-y-1.5 overflow-y-auto">
              {proposal.connections.map((c, i) => (
                <li key={i} className="rounded-lg border border-border bg-card px-3 py-2">
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 text-xs">
                      <span className="font-medium">{titleOf(c.source)}</span>
                      <span className="mx-1 text-muted-foreground">→</span>
                      <span className="font-medium">{titleOf(c.target)}</span>
                      {c.label && <Badge variant="secondary" className="ml-1.5 text-[9px]">{c.label}</Badge>}
                    </span>
                    {accepted.has(i) ? (
                      <Badge variant="success"><Check /> Añadida</Badge>
                    ) : (
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 text-[10px]"
                        onClick={() => {
                          onAccept([c]);
                          setAccepted((s) => new Set(s).add(i));
                        }}
                      >
                        <Plus /> Aceptar
                      </Button>
                    )}
                  </div>
                  {c.reason && <p className="mt-1 text-[10px] text-muted-foreground">{c.reason}</p>}
                </li>
              ))}
            </ul>
            <DialogFooter>
              <Button variant="outline" onClick={onClose}><X /> Cerrar</Button>
              <Button
                disabled={remaining.length === 0}
                onClick={() => {
                  onAccept(remaining);
                  setAccepted(new Set(proposal.connections.map((_, i) => i)));
                }}
              >
                <Check /> Aceptar todas ({remaining.length})
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
