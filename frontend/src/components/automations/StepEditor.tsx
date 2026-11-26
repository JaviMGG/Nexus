import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@/lib/api";
import type {
  ActionSpec,
  AgentDef,
  Automation,
  CompareOp,
  Condition,
  FlowStep,
  MetricSpec,
  Note,
  Project,
  Task,
} from "@/lib/types";
import { AGENT_LABEL, cn } from "@/lib/utils";
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

const YMD_RE = /^\d{4}-\d{2}-\d{2}$/;
const RFC3339_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;

function isValidDueDate(raw: string): boolean {
  if (YMD_RE.test(raw)) {
    const parsed = new Date(`${raw}T00:00:00Z`);
    return !Number.isNaN(parsed.getTime()) && parsed.toISOString().slice(0, 10) === raw;
  }
  return RFC3339_RE.test(raw) && !Number.isNaN(Date.parse(raw));
}

function dueDateError(raw: string): string | null {
  const value = raw.trim();
  if (!value || (value.includes("{{") && value.includes("}}"))) return null;
  return isValidDueDate(value) ? null : "Usa YYYY-MM-DD o RFC3339";
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

type RefKind = "task" | "project" | "note" | "automation";

const REF_TOKEN: Record<RefKind, { id: string; label: string } | null> = {
  task: { id: "task", label: "La tarea del evento que disparó la automatización" },
  project: { id: "project", label: "El proyecto del evento que disparó la automatización" },
  note: { id: "note", label: "La nota del evento que disparó la automatización" },
  automation: null,
};

interface RefOption {
  id: string;
  title: string;
}

function useRefOptions(kind: RefKind, enabled: boolean): { options: RefOption[]; isLoading: boolean } {
  const tasks = useQuery({
    queryKey: ["tasks", {}],
    queryFn: () => invoke<Task[]>("tasks.list", { filter: {} }),
    enabled: enabled && kind === "task",
    staleTime: 15000,
  });
  const projects = useQuery({
    queryKey: ["projects", { include_archived: true }],
    queryFn: () => invoke<Project[]>("projects.list", { include_archived: true }),
    enabled: enabled && kind === "project",
    staleTime: 15000,
  });
  const notes = useQuery({
    queryKey: ["notes", {}],
    queryFn: () => invoke<Note[]>("notes.list", { filter: {} }),
    enabled: enabled && kind === "note",
    staleTime: 15000,
  });
  const automations = useQuery({
    queryKey: ["automations"],
    queryFn: () => invoke<Automation[]>("automations.list"),
    enabled: enabled && kind === "automation",
    staleTime: 15000,
  });
  switch (kind) {
    case "task":
      return { options: (tasks.data ?? []).map((t) => ({ id: t.id, title: t.title })), isLoading: tasks.isLoading };
    case "project":
      return { options: (projects.data ?? []).map((p) => ({ id: p.id, title: p.name })), isLoading: projects.isLoading };
    case "note":
      return { options: (notes.data ?? []).map((n) => ({ id: n.id, title: n.title })), isLoading: notes.isLoading };
    case "automation":
      return { options: (automations.data ?? []).map((a) => ({ id: a.id, title: a.name })), isLoading: automations.isLoading };
  }
}

function RefInput({
  kind,
  value,
  placeholder,
  onChange,
}: {
  kind: RefKind;
  value: string;
  placeholder?: string;
  onChange: (v: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [text, setText] = useState("");
  const { options, isLoading } = useRefOptions(kind, true);
  const token = REF_TOKEN[kind];
  const selected = options.find((o) => o.id === value);
  const display = open
    ? text
    : value === ""
      ? ""
      : token?.id === value
        ? token.label
        : (selected?.title ?? value);
  const q = (open ? text : "").trim().toLowerCase();
  const suggestions = useMemo(() => {
    const matches = options
      .filter((o) => !q || o.title.toLowerCase().includes(q) || o.id.toLowerCase().includes(q))
      .slice(0, 8);
    if (token && (!q || token.id.includes(q) || token.label.toLowerCase().includes(q))) {
      return [{ id: token.id, title: token.label }, ...matches];
    }
    return matches;
  }, [options, q, token]);

  const pick = (id: string) => {
    onChange(id);
    setOpen(false);
  };

  return (
    <div className="relative">
      <Input
        className="h-8 text-xs"
        value={display}
        title={selected ? `id: ${value}` : undefined}
        placeholder={placeholder ?? "Escribe para buscar por nombre…"}
        onChange={(e) => {
          setText(e.target.value);
          onChange(e.target.value);
          setOpen(true);
          setActive(0);
        }}
        onFocus={() => { setText(""); setOpen(true); setActive(0); }}
        onBlur={() => {
          if (text.trim() !== "") {
            const match = options.find((o) => o.title.toLowerCase() === text.trim().toLowerCase());
            onChange(match ? match.id : text.trim());
          }
          setOpen(false);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setOpen(false);
            return;
          }
          if (!open || suggestions.length === 0) return;
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setActive((i) => (i + 1) % suggestions.length);
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((i) => (i - 1 + suggestions.length) % suggestions.length);
          } else if (e.key === "Enter") {
            e.preventDefault();
            pick(suggestions[Math.min(active, suggestions.length - 1)].id);
          }
        }}
      />
      {open && (suggestions.length > 0 || isLoading) && (
        <ul className="absolute left-0 right-0 top-full z-30 mt-1 max-h-44 overflow-y-auto rounded-md border border-border bg-card py-1 shadow-lg">
          {isLoading && suggestions.length === 0 && (
            <li className="px-2 py-1.5 text-[11px] text-muted-foreground">Cargando…</li>
          )}
          {suggestions.map((s, i) => (
            <li key={s.id}>
              <button
                type="button"
                className={cn(
                  "block w-full truncate px-2 py-1.5 text-left text-xs cursor-pointer",
                  i === active ? "bg-accent" : "hover:bg-accent/60",
                )}
                onMouseDown={(e) => e.preventDefault()}
                onMouseEnter={() => setActive(i)}
                onClick={() => pick(s.id)}
              >
                {token?.id === s.id ? `«${s.id}» — ${s.title}` : s.title}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

const PARAM_FIELDS: Record<string, { key: string; label: string; kind?: "textarea" | "number"; ref?: RefKind; placeholder?: string }[]> = {
  "tasks.create": [
    { key: "title", label: "Título", placeholder: "admite {{title}}" },
    { key: "description", label: "Descripción", kind: "textarea" },
    { key: "priority", label: "Prioridad (low/medium/high/urgent)" },
    { key: "due_date", label: "Fecha límite (YYYY-MM-DD o RFC3339)" },
    { key: "tags", label: "Etiquetas (JSON array)" },
  ],
  "tasks.create_subtask": [
    { key: "title", label: "Título" },
    { key: "task_id", label: "Tarea padre (id o \"task\")", ref: "task" },
    { key: "due_date", label: "Fecha límite (YYYY-MM-DD o RFC3339)" },
  ],
  "projects.add_task": [
    { key: "title", label: "Título" },
    { key: "project_id", label: "Proyecto (id o \"project\")", ref: "project" },
    { key: "due_date", label: "Fecha límite (YYYY-MM-DD o RFC3339)" },
  ],
  "tasks.update": [
    { key: "task_id", label: "Tarea (id o \"task\")", ref: "task" },
    { key: "title", label: "Nuevo título" },
    { key: "description", label: "Nueva descripción", kind: "textarea" },
    { key: "due_date", label: "Fecha límite (YYYY-MM-DD o RFC3339)" },
  ],
  "tasks.complete": [{ key: "task_id", label: "Tarea (id o \"task\")", ref: "task" }],
  "tasks.delete": [{ key: "task_id", label: "Tarea (id o \"task\")", ref: "task" }],
  "tasks.set_priority": [
    { key: "task_id", label: "Tarea (id o \"task\")", ref: "task" },
    { key: "priority", label: "Prioridad (low/medium/high/urgent)" },
  ],
  "tasks.add_tags": [
    { key: "task_id", label: "Tarea (id o \"task\")", ref: "task" },
    { key: "add_tags", label: "Etiquetas (JSON array)" },
  ],
  "tasks.reorder": [
    { key: "task_id", label: "Tarea (id o \"task\")", ref: "task" },
    { key: "status", label: "Estado (todo/in_progress/done/cancelled)" },
    { key: "position", label: "Posición", kind: "number" },
  ],
  "projects.create": [
    { key: "name", label: "Nombre" },
    { key: "description", label: "Descripción", kind: "textarea" },
    { key: "goal", label: "Objetivo" },
  ],
  "projects.update_description": [
    { key: "project_id", label: "Proyecto (id o \"project\")", ref: "project" },
    { key: "description", label: "Descripción", kind: "textarea" },
  ],
  "projects.update_status": [
    { key: "project_id", label: "Proyecto (id o \"project\")", ref: "project" },
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
    { key: "note_id", label: "Nota (id o \"note\")", ref: "note" },
    { key: "title", label: "Título" },
    { key: "content", label: "Contenido", kind: "textarea" },
  ],
  "notes.append": [
    { key: "note_id", label: "Nota (id o \"note\")", ref: "note" },
    { key: "append_content", label: "Texto a añadir", kind: "textarea" },
  ],
  "notes.classify": [
    { key: "note_id", label: "Nota (id o \"note\")", ref: "note" },
    { key: "kind", label: "Tipo (note/idea/document/link)" },
  ],
  "notes.add_tags": [
    { key: "note_id", label: "Nota (id o \"note\")", ref: "note" },
    { key: "add_tags", label: "Etiquetas (JSON array)" },
  ],
  "notes.link_project": [
    { key: "note_id", label: "Nota (id o \"note\")", ref: "note" },
    { key: "project_id", label: "Proyecto (id o \"project\")", ref: "project" },
  ],
  "notes.delete": [{ key: "note_id", label: "Nota (id o \"note\")", ref: "note" }],
  "mail.send": [
    { key: "account", label: "Cuenta (id/nombre o {{account_id}})" },
    { key: "to", label: "Destinatario" },
    { key: "subject", label: "Asunto", placeholder: "admite {{campo}}" },
    { key: "body", label: "Cuerpo", kind: "textarea", placeholder: "admite {{campo}}" },
  ],
  "mail.reply": [
    { key: "account", label: "Cuenta (id/nombre o {{account_id}})" },
    { key: "body", label: "Cuerpo", kind: "textarea", placeholder: "admite {{campo}}" },
    { key: "in_reply_to", label: "Responder a (message-id)", placeholder: "{{message_id}}" },
  ],
  "system.run_automation": [{ key: "automation_id", label: "Automatización (solo modo local)", ref: "automation" }],
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
      {fields.map((f) => {
        const value = valueToStr(params[f.key] ?? "");
        const error = f.key === "due_date" ? dueDateError(value) : null;
        return (
          <div key={f.key} className="space-y-0.5">
            <Label className="text-[11px] text-muted-foreground">{f.label}</Label>
            {f.kind === "textarea" ? (
              <Textarea
                className="min-h-[48px] text-xs"
                value={value}
                placeholder={f.placeholder}
                onChange={(e) => set(f.key, e.target.value, f.kind)}
              />
            ) : f.ref ? (
              <RefInput
                kind={f.ref}
                value={value}
                placeholder={f.placeholder}
                onChange={(v) => set(f.key, v)}
              />
            ) : (
              <Input
                className={cn("h-8 text-xs", error && "border-destructive")}
                type={f.kind === "number" ? "number" : "text"}
                value={value}
                placeholder={f.placeholder}
                onChange={(e) => set(f.key, e.target.value, f.kind)}
              />
            )}
            {error && <p className="text-[10px] text-destructive">{error}</p>}
          </div>
        );
      })}
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
