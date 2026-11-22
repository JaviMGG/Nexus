import { useEffect, useState } from "react";
import { ArrowDownToLine, ArrowUpToLine, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { CanvasElement } from "./types";
import { ELEMENT_LABEL, STICKY_COLORS, isTokenColor } from "./utils";

const TOKEN_OPTIONS = [
  { value: "__default__", label: "Predeterminado" },
  { value: "card", label: "Tarjeta" },
  { value: "primary", label: "Primario" },
  { value: "muted", label: "Suave" },
  { value: "accent", label: "Acento" },
  { value: "success", label: "Éxito" },
  { value: "warning", label: "Aviso" },
  { value: "destructive", label: "Destructivo" },
];

export interface PropertiesPanelProps {
  elements: CanvasElement[];
  onPatch: (ids: string[], patch: Partial<CanvasElement>) => void;
  onPatchStyle: (ids: string[], patch: Record<string, unknown>) => void;
  onPatchContent: (ids: string[], patch: Record<string, unknown>) => void;
  onBringFront: (ids: string[]) => void;
  onSendBack: (ids: string[]) => void;
  onClose: () => void;
}

export function PropertiesPanel({ elements, onPatch, onPatchStyle, onPatchContent, onBringFront, onSendBack, onClose }: PropertiesPanelProps) {
  const ids = elements.map((e) => e.id);
  const first = elements[0];
  const single = elements.length === 1 ? first : null;
  const [wDraft, setWDraft] = useState("");
  const [hDraft, setHDraft] = useState("");
  useEffect(() => {
    if (single) {
      setWDraft(String(Math.round(single.w)));
      setHDraft(String(Math.round(single.h)));
    }
  }, [single?.id, single?.w, single?.h]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!first) return null;
  const bg = typeof first.style.background === "string" ? first.style.background : "";
  const textColor = typeof first.style.text === "string" ? first.style.text : "";
  const borderColor = typeof first.style.border === "string" ? first.style.border : "";
  const fontSize = typeof first.style.font_size === "number" ? first.style.font_size : 14;
  const hasText = elements.some((e) => ["sticky", "text", "rect", "ellipse", "card"].includes(e.type));
  const isSticky = elements.every((e) => e.type === "sticky");

  const commitSize = () => {
    if (!single) return;
    const w = Math.max(20, Number(wDraft) || single.w);
    const h = Math.max(20, Number(hDraft) || single.h);
    if (w !== single.w || h !== single.h) onPatch(ids, { w, h });
  };

  return (
    <div className="pointer-events-auto absolute right-3 top-16 z-20 max-h-[calc(100%-6rem)] w-64 overflow-y-auto rounded-xl border border-border bg-card/95 p-3 shadow-lg backdrop-blur animate-fade-in">
      <div className="mb-2 flex items-center justify-between">
        <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {single ? ELEMENT_LABEL[single.type] : `${elements.length} elementos`}
        </h4>
        <Button size="icon-sm" variant="ghost" className="h-6 w-6" onClick={onClose} aria-label="Cerrar panel">
          <X />
        </Button>
      </div>

      <div className="space-y-3">
        <div>
          <Label className="mb-1.5 block">Color de fondo</Label>
          {isSticky && (
            <div className="mb-2 flex flex-wrap gap-1.5">
              {STICKY_COLORS.map((c) => (
                <button
                  key={c}
                  className={cn("h-6 w-6 cursor-pointer rounded-md border border-border transition-transform hover:scale-110", bg === c && "ring-2 ring-ring")}
                  style={{ background: c }}
                  title={c}
                  onClick={() => onPatchStyle(ids, { background: c, text: "#1f2937" })}
                />
              ))}
            </div>
          )}
          <Select
            value={isTokenColor(bg) ? bg : bg ? "__custom__" : "__default__"}
            onValueChange={(v) => {
              if (v === "__default__") onPatchStyle(ids, { background: null, text: null });
              else if (v !== "__custom__") onPatchStyle(ids, { background: v, text: null });
            }}
          >
            <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
            <SelectContent>
              {TOKEN_OPTIONS.map((o) => (
                <SelectItem key={o.value} value={o.value} className="text-xs">{o.label}</SelectItem>
              ))}
              {bg && !isTokenColor(bg) && <SelectItem value="__custom__" className="text-xs">Personalizado ({bg})</SelectItem>}
            </SelectContent>
          </Select>
          <div className="mt-1.5 flex items-center gap-2">
            <input
              type="color"
              className="h-7 w-9 cursor-pointer rounded border border-border bg-card"
              value={/^#[0-9a-fA-F]{6}$/.test(bg) ? bg : "#ffffff"}
              onChange={(e) => onPatchStyle(ids, { background: e.target.value, text: null })}
              title="Color personalizado"
            />
            <span className="text-[10px] text-muted-foreground">Hex personalizado</span>
          </div>
        </div>

        {hasText && (
          <div>
            <Label className="mb-1.5 block">Color de texto</Label>
            <div className="flex items-center gap-2">
              <input
                type="color"
                className="h-7 w-9 cursor-pointer rounded border border-border bg-card"
                value={/^#[0-9a-fA-F]{6}$/.test(textColor) ? textColor : "#1f2937"}
                onChange={(e) => onPatchStyle(ids, { text: e.target.value })}
              />
              <Button size="sm" variant="outline" className="h-7 text-[10px]" onClick={() => onPatchStyle(ids, { text: null })}>
                Automático
              </Button>
            </div>
          </div>
        )}

        <div>
          <Label className="mb-1.5 block">Color de borde</Label>
          <div className="flex items-center gap-2">
            <input
              type="color"
              className="h-7 w-9 cursor-pointer rounded border border-border bg-card"
              value={/^#[0-9a-fA-F]{6}$/.test(borderColor) ? borderColor : "#71717a"}
              onChange={(e) => onPatchStyle(ids, { border: e.target.value })}
            />
            <Button size="sm" variant="outline" className="h-7 text-[10px]" onClick={() => onPatchStyle(ids, { border: null })}>
              Tema
            </Button>
          </div>
        </div>

        {hasText && (
          <div className="grid grid-cols-2 gap-2">
            <div>
              <Label className="mb-1.5 block">Tamaño texto</Label>
              <Input
                type="number"
                className="h-8 text-xs"
                min={8}
                max={96}
                value={fontSize}
                onChange={(e) => onPatchStyle(ids, { font_size: Math.min(96, Math.max(8, Number(e.target.value) || 14)) })}
              />
            </div>
            <div>
              <Label className="mb-1.5 block">Alineación</Label>
              {elements.every((e) => e.type === "ellipse") ? (
                <p className="rounded-md border border-border bg-muted/40 px-2 py-1.5 text-[10px] text-muted-foreground">
                  El texto del óvalo siempre va centrado.
                </p>
              ) : (
                <Select
                  value={first.style.align === "center" || first.style.align === "right" ? first.style.align : "left"}
                  onValueChange={(v) => onPatchStyle(ids, { align: v })}
                >
                  <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="left" className="text-xs">Izquierda</SelectItem>
                    <SelectItem value="center" className="text-xs">Centro</SelectItem>
                    <SelectItem value="right" className="text-xs">Derecha</SelectItem>
                  </SelectContent>
                </Select>
              )}
            </div>
          </div>
        )}

        {single && (
          <>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label className="mb-1.5 block">Ancho</Label>
                <Input type="number" className="h-8 text-xs" value={wDraft} onChange={(e) => setWDraft(e.target.value)} onBlur={commitSize} onKeyDown={(e) => e.key === "Enter" && commitSize()} />
              </div>
              <div>
                <Label className="mb-1.5 block">Alto</Label>
                <Input type="number" className="h-8 text-xs" value={hDraft} onChange={(e) => setHDraft(e.target.value)} onBlur={commitSize} onKeyDown={(e) => e.key === "Enter" && commitSize()} />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label className="mb-1.5 block">Rotación °</Label>
                <Input
                  type="number"
                  className="h-8 text-xs"
                  value={single.rotation ?? 0}
                  onChange={(e) => onPatch(ids, { rotation: Number(e.target.value) || 0 })}
                />
              </div>
              <div>
                <Label className="mb-1.5 block">Capa (z)</Label>
                <Input
                  type="number"
                  className="h-8 text-xs"
                  value={single.z}
                  onChange={(e) => onPatch(ids, { z: Number(e.target.value) || 0 })}
                />
              </div>
            </div>
          </>
        )}

        <div className="flex gap-1.5">
          <Button size="sm" variant="outline" className="h-7 flex-1 text-[10px]" onClick={() => onBringFront(ids)}>
            <ArrowUpToLine /> Al frente
          </Button>
          <Button size="sm" variant="outline" className="h-7 flex-1 text-[10px]" onClick={() => onSendBack(ids)}>
            <ArrowDownToLine /> Al fondo
          </Button>
        </div>

        {single?.type === "link" && (
          <div className="space-y-2 border-t border-border pt-2">
            <div>
              <Label className="mb-1.5 block">URL</Label>
              <Input
                className="h-8 text-xs"
                value={typeof single.content.url === "string" ? single.content.url : ""}
                onChange={(e) => onPatchContent(ids, { url: e.target.value })}
              />
            </div>
            <div>
              <Label className="mb-1.5 block">Título</Label>
              <Input
                className="h-8 text-xs"
                value={typeof single.content.title === "string" ? single.content.title : ""}
                onChange={(e) => onPatchContent(ids, { title: e.target.value })}
              />
            </div>
          </div>
        )}

        {single?.type === "card" && (
          <div className="space-y-2 border-t border-border pt-2">
            <div>
              <Label className="mb-1.5 block">Título</Label>
              <Input
                className="h-8 text-xs"
                value={typeof single.content.title === "string" ? single.content.title : ""}
                onChange={(e) => onPatchContent(ids, { title: e.target.value })}
              />
            </div>
            <div>
              <Label className="mb-1.5 block">Cuerpo</Label>
              <Textarea
                className="text-xs"
                rows={3}
                value={typeof single.content.body === "string" ? single.content.body : ""}
                onChange={(e) => onPatchContent(ids, { body: e.target.value })}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
