import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { ArrowLeft, ChevronDown, FlaskConical, Loader2, Save, SlidersHorizontal, Zap } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type {
  Automation,
  AutomationFlow,
  AutomationMode,
  Condition,
  CreateAutomation,
  ExecutionPolicy,
  ModeInfo,
  ScheduleSpec,
  Trigger,
  TriggerSpec,
} from "@/lib/types";
import { AUTOMATION_MODE_DESCRIPTION, AUTOMATION_MODE_LABEL, WEEKDAY_LABEL, cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { FlowCanvas } from "./FlowCanvas";
import { AutomationIcon, ModeBadge } from "./shared";

const DEFAULT_SCHEDULE: ScheduleSpec = {
  kind: "daily",
  time: "08:00",
  weekdays: ["mon"],
  day_of_month: 1,
  interval_minutes: 60,
  date: null,
  timezone: null,
};

function emptyDraft(): CreateAutomation {
  return {
    name: "",
    description: "",
    icon: "zap",
    enabled: false,
    mode: "local",
    trigger: { type: "schedule", spec: { ...DEFAULT_SCHEDULE } },
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
    enabled: a.enabled,
    mode: a.mode,
    trigger: a.trigger,
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

  const triggers = useQuery({
    queryKey: ["automation-triggers"],
    queryFn: () => invoke<TriggerSpec[]>("automations.triggers"),
    staleTime: Infinity,
  });
  const modeInfo = useQuery({
    queryKey: ["automation-mode-info", draft.mode],
    queryFn: () => invoke<ModeInfo>("automations.mode_info", { mode: draft.mode }),
  });

  const save = useMutation({
    mutationFn: () =>
      automation
        ? invoke<Automation>("automations.update", { id: automation.id, data: draft })
        : invoke<Automation>("automations.create", { data: draft }),
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
        const created = await invoke<Automation>("automations.create", { data: { ...draft, enabled: false } });
        id = created.id;
        qc.invalidateQueries({ queryKey: ["automations"] });
      } else {
        await invoke("automations.update", { id, data: draft });
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

  const setTrigger = (t: Trigger) => setDraft((d) => ({ ...d, trigger: t }));
  const spec = draft.trigger.type === "schedule" ? draft.trigger.spec : DEFAULT_SCHEDULE;
  const setSpec = (patch: Partial<ScheduleSpec>) =>
    setTrigger({ type: "schedule", spec: { ...spec, ...patch } });

  const eventTriggers = (triggers.data ?? []).filter((t) => t.kind === "event");
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
      {/* Barra de herramientas (waybar): identidad + acciones / disparador */}
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
              <Switch checked={draft.enabled ?? false} onCheckedChange={(v) => setDraft((d) => ({ ...d, enabled: v }))} />
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
          <div className="mx-1 h-6 w-px shrink-0 bg-border" />
          <Zap className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
          <Select
            value={draft.trigger.type === "schedule" ? `schedule:${spec.kind}` : `event:${draft.trigger.event}`}
            onValueChange={(v) => {
              if (v.startsWith("schedule:")) {
                setTrigger({ type: "schedule", spec: { ...spec, kind: v.slice(9) as ScheduleSpec["kind"] } });
              } else {
                setTrigger({ type: "event", event: v.slice(6), filter: {} });
              }
            }}
          >
            <SelectTrigger className="h-8 w-[210px] shrink-0 text-xs" aria-label="Disparador"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="schedule:daily">Todos los días a una hora</SelectItem>
              <SelectItem value="schedule:weekly">Días de la semana concretos</SelectItem>
              <SelectItem value="schedule:monthly">Una vez al mes</SelectItem>
              <SelectItem value="schedule:interval">Cada cierto intervalo</SelectItem>
              <SelectItem value="schedule:once">Una vez en una fecha</SelectItem>
              {eventTriggers.map((t) => (
                <SelectItem key={t.id} value={`event:${t.id}`}>{t.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>

          {draft.trigger.type === "schedule" && (
            <>
              {spec.kind !== "interval" && (
                <Input type="time" className="h-8 w-[104px] shrink-0 text-xs" value={spec.time ?? "08:00"} onChange={(e) => setSpec({ time: e.target.value })} aria-label="Hora" />
              )}
              {spec.kind === "weekly" && (
                <div className="flex shrink-0 gap-0.5">
                  {Object.entries(WEEKDAY_LABEL).map(([id, label]) => {
                    const active = spec.weekdays.includes(id);
                    return (
                      <button
                        key={id}
                        onClick={() => setSpec({ weekdays: active ? spec.weekdays.filter((d) => d !== id) : [...spec.weekdays, id] })}
                        className={cn(
                          "h-7 w-7 rounded-md border text-[11px] font-medium transition-colors cursor-pointer",
                          active ? "border-primary bg-primary/15 text-primary" : "border-border text-muted-foreground hover:bg-muted",
                        )}
                        title={id}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              )}
              {spec.kind === "monthly" && (
                <Input type="number" min={1} max={31} className="h-8 w-16 shrink-0 text-xs" value={spec.day_of_month ?? 1} onChange={(e) => setSpec({ day_of_month: Number(e.target.value) })} aria-label="Día del mes" title="Día del mes" />
              )}
              {spec.kind === "interval" && (
                <div className="flex shrink-0 items-center gap-1">
                  <Input type="number" min={5} className="h-8 w-16 text-xs" value={spec.interval_minutes ?? 60} onChange={(e) => setSpec({ interval_minutes: Number(e.target.value) })} aria-label="Intervalo en minutos" />
                  <span className="text-[11px] text-muted-foreground">min</span>
                </div>
              )}
              {spec.kind === "once" && (
                <Input type="date" className="h-8 w-[140px] shrink-0 text-xs" value={spec.date ?? ""} onChange={(e) => setSpec({ date: e.target.value })} aria-label="Fecha" />
              )}
            </>
          )}
          {draft.trigger.type === "event" && (
            <span className="text-[11px] text-muted-foreground">
              El payload del evento llega a los pasos como {"{{title}}"}, {"{{id}}"}, etc.
            </span>
          )}
        </div>
      </div>

      {/* Panel de opciones desplegable */}
      {optionsOpen && (
        <div className="grid grid-cols-1 gap-3 rounded-xl border border-border bg-card p-3 shadow-sm animate-fade-in md:grid-cols-2 xl:grid-cols-4">
          <div className="space-y-1">
            <Label className="text-[11px]">Descripción</Label>
            <Input className="h-8 text-xs" value={draft.description ?? ""} onChange={(e) => setDraft((d) => ({ ...d, description: e.target.value }))} placeholder="Para qué sirve" />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Zona horaria (IANA, vacío = local)</Label>
            <Input className="h-8 text-xs" placeholder="Europe/Madrid" value={spec.timezone ?? ""} onChange={(e) => setSpec({ timezone: e.target.value || null })} disabled={draft.trigger.type !== "schedule"} />
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
        trigger={draft.trigger}
        onChange={(f: AutomationFlow) => setDraft((d) => ({ ...d, flow: f }))}
        height={640}
      />
    </div>
  );
}
