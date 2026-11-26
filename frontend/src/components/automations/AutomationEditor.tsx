import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ArrowLeft, ChevronDown, FlaskConical, Loader2, Save, SlidersHorizontal } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type {
  Automation,
  AutomationFlow,
  AutomationMode,
  Condition,
  CreateAutomation,
  ExecutionPolicy,
  ModeInfo,
  TriggerEntry,
  TriggerSpec,
} from "@/lib/types";
import { AUTOMATION_MODE_DESCRIPTION, AUTOMATION_MODE_LABEL, cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { FlowCanvas } from "./FlowCanvas";
import { AutomationIcon, ModeBadge } from "./shared";

function emptyDraft(): CreateAutomation {
  return {
    name: "",
    description: "",
    icon: "zap",
    enabled: false,
    mode: "local",
    triggers: [],
    conditions: null,
    flow: { version: 1, steps: [], entry: null },
    permissions: { write: "auto" },
    policy: "skip_if_active",
  };
}

function fromAutomation(a: Automation): CreateAutomation {
  return {
    name: a.name,
    description: a.description,
    icon: a.icon,
    enabled: a.enabled && (a.triggers?.length ?? 0) > 0,
    mode: a.mode,
    triggers: a.triggers ?? [],
    conditions: a.conditions,
    flow: a.flow,
    permissions: a.permissions,
    limits: a.limits,
    policy: a.policy,
    template_id: a.template_id,
  };
}

const ICON_CHOICES = ["zap", "sunrise", "sunset", "alarm-clock", "lightbulb", "rocket", "hourglass", "calendar-check", "target", "copy", "search", "workflow", "bot", "timer", "book-open"];

/** Editor a página completa: barra de herramientas + lienzo de nodos a ancho completo. */
export function AutomationEditor({
  automation,
  onClose,
}: {
  automation: Automation | null;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [draft, setDraft] = useState<CreateAutomation>(() => (automation ? fromAutomation(automation) : emptyDraft()));
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [showLimits, setShowLimits] = useState(false);

  const triggerCatalog = useQuery({
    queryKey: ["automation-triggers"],
    queryFn: () => invoke<TriggerSpec[]>("automations.triggers"),
    staleTime: Infinity,
  });
  const modeInfo = useQuery({
    queryKey: ["automation-mode-info", draft.mode],
    queryFn: () => invoke<ModeInfo>("automations.mode_info", { mode: draft.mode }),
  });

  const payload = (): CreateAutomation => ({
    ...draft,
    enabled: draft.triggers.length > 0 ? (draft.enabled ?? false) : false,
  });

  const save = useMutation({
    mutationFn: () =>
      automation
        ? invoke<Automation>("automations.update", { id: automation.id, data: payload() })
        : invoke<Automation>("automations.create", { data: payload() }),
    onSuccess: (a) => {
      toast.success(automation ? "Automatización actualizada" : `Automatización «${a.name}» creada`);
      qc.invalidateQueries({ queryKey: ["automations"] });
      qc.invalidateQueries({ queryKey: ["automation-overview"] });
      onClose();
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo guardar"),
  });

  const testRun = useMutation({
    mutationFn: async () => {
      let id = automation?.id;
      if (!id) {
        const created = await invoke<Automation>("automations.create", { data: { ...payload(), enabled: false } });
        id = created.id;
        qc.invalidateQueries({ queryKey: ["automations"] });
      } else {
        await invoke("automations.update", { id, data: { ...payload(), enabled: false } });
      }
      return invoke("automations.run", { id, test: true });
    },
    onSuccess: () => {
      toast.success("Ejecución de prueba lanzada", { description: "No modifica datos. Mira el resultado en Historial." });
      qc.invalidateQueries({ queryKey: ["automation-executions"] });
      onClose();
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo ejecutar la prueba"),
  });

  const firstSchedule = draft.triggers.find((t) => t.trigger.type === "schedule");
  const scheduleTimezone = firstSchedule && firstSchedule.trigger.type === "schedule"
    ? firstSchedule.trigger.spec.timezone ?? ""
    : "";
  const setTimezone = (value: string) =>
    setDraft((d) => ({
      ...d,
      triggers: d.triggers.map((t) => t.trigger.type === "schedule"
        ? { ...t, trigger: { ...t.trigger, spec: { ...t.trigger.spec, timezone: value || null } } }
        : t),
    }));

  const limits = draft.limits ?? {};
  const setLimit = (key: string, value: number) =>
    setDraft((d) => ({ ...d, limits: { ...(d.limits ?? {}), [key]: value } }));

  const canSave = draft.name.trim().length > 0 && !save.isPending;
  const info = modeInfo.data;

  const conditionJson = useMemo(() => (draft.conditions ? JSON.stringify(draft.conditions) : ""), [draft.conditions]);
  const [condText, setCondText] = useState(conditionJson);
  useEffect(() => setCondText(conditionJson), [conditionJson]);
  const condError = useMemo(() => {
    if (!condText.trim()) return null;
    try {
      JSON.parse(condText);
      return null;
    } catch {
      return "JSON inválido";
    }
  }, [condText]);

  return (
    <div className="space-y-3 animate-fade-in">
      {/* Barra de herramientas (waybar): identidad + acciones */}
      <div className="rounded-xl border border-border bg-card shadow-sm">
        <div className="flex flex-wrap items-center gap-2 px-3 py-2">
          <Button variant="ghost" size="sm" className="shrink-0" onClick={onClose}>
            <ArrowLeft /> Volver
          </Button>
          <div className="mx-1 h-6 w-px shrink-0 bg-border" />
          <Select value={draft.icon || "zap"} onValueChange={(v) => setDraft((d) => ({ ...d, icon: v }))}>
            <SelectTrigger className="h-8 w-[64px] shrink-0 px-2 text-xs" aria-label="Icono">
              <span className="flex items-center gap-1.5">
                <AutomationIcon icon={draft.icon || "zap"} />
              </span>
            </SelectTrigger>
            <SelectContent>
              {ICON_CHOICES.map((i) => (
                <SelectItem key={i} value={i}>
                  <span className="flex items-center gap-2"><AutomationIcon icon={i} /> {i}</span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Input
            className="h-8 min-w-[160px] flex-1 text-sm font-medium"
            value={draft.name}
            onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
            placeholder={automation ? automation.name : "Nombre de la automatización"}
          />
          <div className="ml-auto flex shrink-0 flex-wrap items-center gap-2">
            <div className="flex items-center gap-1.5" title="Activar al guardar">
              <span className="text-[11px] text-muted-foreground">Activa</span>
              <Switch
                checked={draft.enabled ?? false}
                onCheckedChange={(v) => setDraft((d) => ({ ...d, enabled: v }))}
                disabled={draft.triggers.length === 0}
              />
            </div>
            <Button
              variant={optionsOpen ? "secondary" : "outline"}
              size="sm"
              onClick={() => setOptionsOpen((v) => !v)}
              title="Más opciones: descripción, zona horaria, permisos, condición raíz, límites"
            >
              <SlidersHorizontal /> Opciones <ChevronDown className={cn("transition-transform", optionsOpen && "rotate-180")} />
            </Button>
            <Button variant="outline" size="sm" onClick={() => testRun.mutate()} disabled={!canSave || testRun.isPending}>
              {testRun.isPending ? <Loader2 className="animate-spin" /> : <FlaskConical />}
              Probar
            </Button>
            <Button size="sm" onClick={() => save.mutate()} disabled={!canSave || !!condError}>
              {save.isPending ? <Loader2 className="animate-spin" /> : <Save />}
              Guardar
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2 border-t border-border/60 px-3 py-2">
          <Select value={draft.mode ?? "local"} onValueChange={(v) => setDraft((d) => ({ ...d, mode: v as AutomationMode }))}>
            <SelectTrigger className="h-8 w-[104px] shrink-0 text-xs" aria-label="Modo de ejecución">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {(["local", "ai", "hybrid"] as AutomationMode[]).map((m) => (
                <SelectItem key={m} value={m} className="text-xs">
                  {AUTOMATION_MODE_LABEL[m]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {info && (
            <Badge variant={info.requires_ai ? (info.ai_available ? "success" : "warning") : "success"} className="shrink-0">
              {info.requires_ai ? (info.ai_available ? "IA disponible" : "IA no disponible") : "Offline"}
            </Badge>
          )}
          <span className="text-[11px] text-muted-foreground">
            Añade disparadores desde la barra del lienzo y configúralos al seleccionar su nodo.
          </span>
        </div>
        {draft.triggers.length === 0 && (
          <div className="border-t border-border/60 px-3 py-2 text-[11px] text-muted-foreground">
            Borrador manual: añade un disparador para activarlo
          </div>
        )}
      </div>

      {/* Panel de opciones desplegable */}
      {optionsOpen && (
        <div className="grid grid-cols-1 gap-3 rounded-xl border border-border bg-card p-3 shadow-sm animate-fade-in md:grid-cols-2 xl:grid-cols-4">
          <div className="space-y-1">
            <Label className="text-[11px]">Descripción</Label>
            <Input className="h-8 text-xs" value={draft.description ?? ""} onChange={(e) => setDraft((d) => ({ ...d, description: e.target.value }))} placeholder="Para qué sirve" />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Zona horaria de los disparadores temporales (IANA)</Label>
            <Input className="h-8 text-xs" placeholder="Europe/Madrid" value={scheduleTimezone} onChange={(e) => setTimezone(e.target.value)} disabled={!firstSchedule} />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Ejecuciones simultáneas</Label>
            <Select value={draft.policy ?? "skip_if_active"} onValueChange={(v) => setDraft((d) => ({ ...d, policy: v as ExecutionPolicy }))}>
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="skip_if_active" className="text-xs">Omitir si hay una activa</SelectItem>
                <SelectItem value="cooldown" className="text-xs">Enfriamiento entre ejecuciones</SelectItem>
                <SelectItem value="always" className="text-xs">Permitir siempre</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Modificaciones locales</Label>
            <Select
              value={draft.permissions?.write ?? "auto"}
              onValueChange={(v) => setDraft((d) => ({ ...d, permissions: { write: v as "auto" | "confirm" } }))}
            >
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="auto" className="text-xs">Automáticas (registradas)</SelectItem>
                <SelectItem value="confirm" className="text-xs">Pedir autorización</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1 md:col-span-2">
            <Label className="text-[11px]">Condición raíz de activación (JSON, opcional)</Label>
            <Textarea
              className="min-h-[40px] font-mono text-[11px]"
              placeholder='{"type":"metric","metric":"open_tasks","op":"gt","value":0}'
              value={condText}
              onChange={(e) => {
                setCondText(e.target.value);
                if (!e.target.value.trim()) {
                  setDraft((d) => ({ ...d, conditions: null }));
                } else {
                  try {
                    const c = JSON.parse(e.target.value) as Condition;
                    setDraft((d) => ({ ...d, conditions: c }));
                  } catch {
                    /* se valida al guardar */
                  }
                }
              }}
            />
            {condError && <p className="text-[11px] text-destructive">{condError}</p>}
          </div>
          <div className="md:col-span-2 xl:col-span-2">
            <button className="text-xs text-muted-foreground underline-offset-2 hover:underline cursor-pointer" onClick={() => setShowLimits((v) => !v)}>
              {showLimits ? "Ocultar límites avanzados" : "Límites avanzados (tokens, llamadas, timeout, reintentos…)"}
            </button>
            {showLimits && (
              <div className="mt-2 grid grid-cols-3 gap-2">
                {([
                  ["max_tokens", "Tokens máx.", 20000],
                  ["max_agent_calls", "Llamadas a agentes", 3],
                  ["max_steps", "Pasos máx.", 100],
                  ["timeout_secs", "Timeout (s)", 300],
                  ["retries", "Reintentos", 0],
                  ["cooldown_minutes", "Enfriamiento (min)", 0],
                ] as const).map(([key, label, def]) => (
                  <div key={key} className="space-y-0.5">
                    <Label className="text-[10px]">{label}</Label>
                    <Input
                      type="number"
                      min={0}
                      className="h-7 text-xs"
                      value={(limits as Record<string, number>)[key] ?? def}
                      onChange={(e) => setLimit(key, Number(e.target.value))}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
          <div className="md:col-span-2 xl:col-span-4">
            <p className="text-[11px] text-muted-foreground">{AUTOMATION_MODE_DESCRIPTION[draft.mode ?? "local"]}</p>
          </div>
        </div>
      )}

      {/* Lienzo a ancho completo */}
      <FlowCanvas
        flow={draft.flow ?? { version: 1, steps: [], entry: null }}
        triggers={draft.triggers}
        triggerSpecs={triggerCatalog.data ?? []}
        onChange={(f: AutomationFlow) => setDraft((d) => ({ ...d, flow: f }))}
        onTriggersChange={(triggers: TriggerEntry[]) => setDraft((d) => ({
          ...d,
          triggers,
          enabled: triggers.length > 0 ? d.enabled : false,
        }))}
        height={640}
      />
    </div>
  );
}
