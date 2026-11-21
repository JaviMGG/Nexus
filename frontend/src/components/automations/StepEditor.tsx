import { useQuery } from "@tanstack/react-query";
import { invoke } from "@/lib/api";
import type {
  ActionSpec,
  AgentDef,
  CompareOp,
  Condition,
  FlowStep,
  MetricSpec,
} from "@/lib/types";
import { AGENT_LABEL } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

export const OPS: { id: CompareOp; label: string }[] = [
  { id: "eq", label: "=" },
  { id: "ne", label: "≠" },
  { id: "gt", label: ">" },
  { id: "gte", label: "≥" },
  { id: "lt", label: "<" },
  { id: "lte", label: "≤" },
  { id: "contains", label: "contiene" },
];

export function parseValue(raw: string): unknown {
  const t = raw.trim();
  if (t === "true") return true;
  if (t === "false") return false;
  if (t !== "" && !isNaN(Number(t))) return Number(t);
  try {
    return JSON.parse(t);
  } catch {
    return raw;
  }
}

export function valueToStr(v: unknown): string {
  if (typeof v === "string") return v;
  return JSON.stringify(v ?? "");
}

export function useCatalogs() {
  const actions = useQuery({ queryKey: ["automation-actions"], queryFn: () => invoke<ActionSpec[]>("automations.actions"), staleTime: Infinity });
  const agents = useQuery({ queryKey: ["agents"], queryFn: () => invoke<AgentDef[]>("agents.list"), staleTime: Infinity });
  const metrics = useQuery({ queryKey: ["automation-metrics"], queryFn: () => invoke<MetricSpec[]>("automations.metrics"), staleTime: Infinity });
  return { actions: actions.data ?? [], agents: agents.data ?? [], metrics: metrics.data ?? [] };
}

function LeafConditionEditor({
  condition,
  metrics,
  onChange,
}: {
  condition: Condition;
  metrics: MetricSpec[];
  onChange: (c: Condition) => void;
}) {
  if (condition.type === "and" || condition.type === "or" || condition.type === "not") {
    return <p className="text-xs text-muted-foreground">Condición compuesta (edítala como JSON en la condición raíz).</p>;
  }
  const isMetric = condition.type === "metric";
  return (
    <div className="grid grid-cols-1 gap-2">
      {isMetric ? (
        <Select value={condition.metric} onValueChange={(v) => onChange({ ...condition, metric: v })}>
          <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>
            {metrics.map((m) => (
              <SelectItem key={m.id} value={m.id} className="text-xs">{m.id}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      ) : (
        <Input
          className="h-8 text-xs"
          placeholder="campo del evento (p. ej. priority)"
          value={condition.field}
          onChange={(e) => onChange({ ...condition, field: e.target.value })}
        />
      )}
      <div className="grid grid-cols-[auto_1fr] gap-2">
        <Select value={condition.op} onValueChange={(v) => onChange({ ...condition, op: v as CompareOp })}>
          <SelectTrigger className="h-8 w-24 text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>
            {OPS.map((o) => (
              <SelectItem key={o.id} value={o.id} className="text-xs">{o.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Input
          className="h-8 text-xs"
          placeholder="valor"
          value={valueToStr(condition.value)}
          onChange={(e) => onChange({ ...condition, value: parseValue(e.target.value) })}
        />
      </div>
    </div>
  );
}

const PARAM_FIELDS: Record<string, { key: string; label: string; kind?: "textarea" | "number"; placeholder?: string }[]> = {
  "tasks.create": [
    { key: "title", label: "Título", placeholder: "admite {{title}}" },
    { key: "description", label: "Descripción", kind: "textarea" },
    { key: "priority", label: "Prioridad (low/medium/high/urgent)" },
    { key: "due_date", label: "Fecha límite (YYYY-MM-DD)" },
    { key: "tags", label: "Etiquetas (JSON array)" },
  ],
  "tasks.create_subtask": [
    { key: "title", label: "Título" },
    { key: "task_id", label: "Tarea padre (id o \"task\")" },
  ],
  "projects.add_task": [
    { key: "title", label: "Título" },
    { key: "project_id", label: "Proyecto (id o \"project\")" },
  ],
  "tasks.update": [
    { key: "task_id", label: "Tarea (id o \"task\")" },
    { key: "title", label: "Nuevo título" },
    { key: "description", label: "Nueva descripción", kind: "textarea" },
  ],
  "tasks.complete": [{ key: "task_id", label: "Tarea (id o \"task\")" }],
  "tasks.delete": [{ key: "task_id", label: "Tarea (id o \"task\")" }],
  "tasks.set_priority": [
    { key: "task_id", label: "Tarea (id o \"task\")" },
    { key: "priority", label: "Prioridad (low/medium/high/urgent)" },
  ],
  "tasks.add_tags": [
    { key: "task_id", label: "Tarea (id o \"task\")" },
    { key: "add_tags", label: "Etiquetas (JSON array)" },
  ],
  "tasks.reorder": [
    { key: "task_id", label: "Tarea (id o \"task\")" },
    { key: "status", label: "Estado (todo/in_progress/done/cancelled)" },
    { key: "position", label: "Posición", kind: "number" },
  ],
  "projects.create": [
    { key: "name", label: "Nombre" },
    { key: "description", label: "Descripción", kind: "textarea" },
    { key: "goal", label: "Objetivo" },
  ],
  "projects.update_description": [
    { key: "project_id", label: "Proyecto (id o \"project\")" },
    { key: "description", label: "Descripción", kind: "textarea" },
  ],
  "projects.update_status": [
    { key: "project_id", label: "Proyecto (id o \"project\")" },
    { key: "status", label: "Estado (active/paused/completed/archived)" },
  ],
  "notes.create": [
    { key: "title", label: "Título" },
    { key: "content", label: "Contenido", kind: "textarea" },
    { key: "kind", label: "Tipo (note/idea/document/link)" },
    { key: "tags", label: "Etiquetas (JSON array)" },
  ],
  "knowledge.save_summary": [
    { key: "title", label: "Título" },
    { key: "content", label: "Contenido (usa {{agent_result}})", kind: "textarea" },
    { key: "tags", label: "Etiquetas (JSON array)" },
  ],
  "system.report": [{ key: "title", label: "Título del informe" }],
  "notes.update": [
    { key: "note_id", label: "Nota (id o \"note\")" },
    { key: "title", label: "Título" },
    { key: "content", label: "Contenido", kind: "textarea" },
  ],
  "notes.append": [
    { key: "note_id", label: "Nota (id o \"note\")" },
    { key: "append_content", label: "Texto a añadir", kind: "textarea" },
  ],
  "notes.classify": [
    { key: "note_id", label: "Nota (id o \"note\")" },
    { key: "kind", label: "Tipo (note/idea/document/link)" },
  ],
  "notes.add_tags": [
    { key: "note_id", label: "Nota (id o \"note\")" },
    { key: "add_tags", label: "Etiquetas (JSON array)" },
  ],
  "notes.link_project": [
    { key: "note_id", label: "Nota (id o \"note\")" },
    { key: "project_id", label: "Proyecto (id o \"project\")" },
  ],
  "notes.delete": [{ key: "note_id", label: "Nota (id o \"note\")" }],
  "system.run_automation": [{ key: "automation_id", label: "Id de la automatización (solo modo local)" }],
};

function ActionParamsEditor({
  action,
  params,
  onChange,
}: {
  action: string;
  params: Record<string, unknown>;
  onChange: (p: Record<string, unknown>) => void;
}) {
  const fields = PARAM_FIELDS[action] ?? [];
  const set = (key: string, raw: string, kind?: string) => {
    const next = { ...params };
    if (raw.trim() === "") {
      delete next[key];
    } else if (kind === "number") {
      next[key] = Number(raw);
    } else if (key === "tags" || key === "add_tags") {
      next[key] = parseValue(raw);
    } else {
      next[key] = raw;
    }
    onChange(next);
  };
  return (
    <div className="space-y-1.5">
      {fields.map((f) => (
        <div key={f.key} className="space-y-0.5">
          <Label className="text-[11px] text-muted-foreground">{f.label}</Label>
          {f.kind === "textarea" ? (
            <Textarea
              className="min-h-[48px] text-xs"
              value={valueToStr(params[f.key] ?? "")}
              placeholder={f.placeholder}
              onChange={(e) => set(f.key, e.target.value, f.kind)}
            />
          ) : (
            <Input
              className="h-8 text-xs"
              type={f.kind === "number" ? "number" : "text"}
              value={valueToStr(params[f.key] ?? "")}
              placeholder={f.placeholder}
              onChange={(e) => set(f.key, e.target.value, f.kind)}
            />
          )}
        </div>
      ))}
      {fields.length === 0 && (
        <p className="text-[11px] text-muted-foreground">Esta acción no necesita parámetros.</p>
      )}
    </div>
  );
}

/** Formulario de configuración de un paso (sin gestión de conexiones). */
export function StepEditor({
  step,
  onChange,
}: {
  step: FlowStep;
  onChange: (s: FlowStep) => void;
}) {
  const { actions, agents, metrics } = useCatalogs();

  return (
    <div className="space-y-2.5">
      {step.type === "action" && (
        <>
          <div className="space-y-1">
            <Label className="text-[11px]">Acción</Label>
            <Select value={step.action} onValueChange={(v) => onChange({ ...step, action: v, params: {} })}>
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                {actions.map((a) => (
                  <SelectItem key={a.id} value={a.id} className="text-xs">
                    {a.name} {a.risk === "sensitive" ? "⚠" : ""}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-[11px] text-muted-foreground">
              {actions.find((a) => a.id === step.action)?.description}
            </p>
          </div>
          <ActionParamsEditor action={step.action} params={step.params} onChange={(params) => onChange({ ...step, params })} />
          <div className="space-y-1">
            <Label className="text-[11px]">Permiso</Label>
            <Select
              value={step.approval ?? "auto"}
              onValueChange={(v) => onChange({ ...step, approval: v === "auto" ? null : (v as "confirm") })}
            >
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="auto" className="text-xs">Según política</SelectItem>
                <SelectItem value="confirm" className="text-xs">Pedir autorización siempre</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </>
      )}

      {step.type === "agent" && (
        <>
          <div className="space-y-1">
            <Label className="text-[11px]">Agente</Label>
            <Select value={step.agent_id} onValueChange={(v) => onChange({ ...step, agent_id: v })}>
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                {agents.filter((a) => a.id !== "orchestrator").map((a) => (
                  <SelectItem key={a.id} value={a.id} className="text-xs">{AGENT_LABEL[a.id] ?? a.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Objetivo (admite {"{{campo}}"} del evento)</Label>
            <Textarea
              className="min-h-[64px] text-xs"
              value={step.goal}
              onChange={(e) => onChange({ ...step, goal: e.target.value })}
              placeholder="Qué debe hacer el agente…"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Permiso</Label>
            <Select
              value={step.approval ?? "auto"}
              onValueChange={(v) => onChange({ ...step, approval: v === "auto" ? null : (v as "confirm") })}
            >
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="auto" className="text-xs">Ejecutar directamente</SelectItem>
                <SelectItem value="confirm" className="text-xs">Pedir autorización siempre</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </>
      )}

      {step.type === "condition" && (
        <LeafConditionEditor condition={step.condition} metrics={metrics} onChange={(c) => onChange({ ...step, condition: c })} />
      )}

      {step.type === "delay" && (
        <div className="space-y-1">
          <Label className="text-[11px]">Segundos de espera (0–3600)</Label>
          <Input
            type="number"
            min={0}
            max={3600}
            className="h-8 w-28 text-xs"
            value={step.seconds}
            onChange={(e) => onChange({ ...step, seconds: Number(e.target.value) })}
          />
        </div>
      )}

      {step.type === "notification" && (
        <>
          <div className="space-y-1">
            <Label className="text-[11px]">Título (admite {"{{campo}}"})</Label>
            <Input
              className="h-8 text-xs"
              value={step.title}
              onChange={(e) => onChange({ ...step, title: e.target.value })}
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px]">Mensaje</Label>
            <Textarea
              className="min-h-[48px] text-xs"
              value={step.body}
              onChange={(e) => onChange({ ...step, body: e.target.value })}
            />
          </div>
        </>
      )}

      {step.type === "end" && (
        <p className="text-xs text-muted-foreground">Este nodo termina el flujo. No tiene configuración.</p>
      )}
    </div>
  );
}
