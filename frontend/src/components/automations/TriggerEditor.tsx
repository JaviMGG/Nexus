import { useQuery } from "@tanstack/react-query";
import { invoke } from "@/lib/api";
import type {
  ScheduleSpec,
  Trigger,
  TriggerFieldKind,
  TriggerFieldSpec,
  TriggerSpec,
  TriggerValidationError,
} from "@/lib/types";
import { WEEKDAY_LABEL, cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export const DEFAULT_SCHEDULE: ScheduleSpec = {
  kind: "daily",
  time: "08:00",
  weekdays: ["mon"],
  day_of_month: 1,
  interval_minutes: 60,
  date: null,
  timezone: null,
};

function textValue(v: unknown): string {
  return typeof v === "string" ? v : v == null ? "" : String(v);
}

function rawFilterValue(v: unknown): string {
  if (v == null) return "";
  if (Array.isArray(v)) return JSON.stringify(v);
  if (typeof v === "boolean") return v ? "true" : "false";
  return String(v);
}

function parseFilterValue(kind: TriggerFieldKind, raw: string): unknown {
  const value = raw.trim();
  if (kind === "number") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : raw;
  }
  if (kind === "boolean") {
    if (value === "true") return true;
    if (value === "false") return false;
    return raw;
  }
  if (kind === "tags") {
    try {
      const parsed: unknown = JSON.parse(value);
      return Array.isArray(parsed) ? parsed : raw;
    } catch {
      return raw;
    }
  }
  return raw;
}

function FieldError({ message }: { message?: string }) {
  if (!message) return null;
  return <p className="text-[10px] text-destructive">{message}</p>;
}

export function TriggerEditor({ trigger, onChange }: { trigger: Trigger; onChange: (t: Trigger) => void }) {
  const catalog = useQuery({
    queryKey: ["automation-triggers"],
    queryFn: () => invoke<TriggerSpec[]>("automations.triggers"),
    staleTime: Infinity,
  });

  const validation = useQuery({
    queryKey: ["automation-trigger-validation", trigger],
    queryFn: () =>
      invoke<{ valid: boolean; errors: TriggerValidationError[] }>("automations.validate_trigger", {
        trigger,
      }),
    retry: false,
  });

  const errors = validation.data?.errors ?? [];
  const errorFor = (field: string) => errors.find((e) => e.field === field)?.message;

  const eventSpecs = (catalog.data ?? []).filter((s) => s.kind === "event");
  if (trigger.type === "event" && !eventSpecs.some((s) => s.id === trigger.event)) {
    eventSpecs.push({ id: trigger.event, name: trigger.event, description: "", kind: "event", fields: [] });
  }
  const eventSpec = trigger.type === "event" ? eventSpecs.find((s) => s.id === trigger.event) : undefined;
  const fields = eventSpec?.fields ?? [];

  const spec = trigger.type === "schedule" ? trigger.spec : DEFAULT_SCHEDULE;
  const setSpec = (patch: Partial<ScheduleSpec>) =>
    onChange({ type: "schedule", spec: { ...spec, ...patch } });

  const setFilter = (key: string, raw: string, kind?: TriggerFieldKind) => {
    if (trigger.type !== "event") return;
    const filter = { ...(trigger.filter ?? {}) };
    if (raw.trim() === "") delete filter[key];
    else filter[key] = kind ? parseFilterValue(kind, raw) : raw;
    onChange({ ...trigger, filter });
  };

  const renderFilterField = (field: TriggerFieldSpec) => {
    if (trigger.type !== "event") return null;
    const raw = rawFilterValue(trigger.filter?.[field.key]);
    const message = errorFor(`filter.${field.key}`);
    const common = {
      className: "h-8 text-xs",
      onChange: (e: React.ChangeEvent<HTMLInputElement>) => setFilter(field.key, e.target.value, field.kind),
    };

    return (
      <div key={field.key} className="space-y-0.5">
        <Label className="text-[11px]">{field.label}</Label>
        {field.kind === "enum" ? (
          <Select
            value={raw || "__none__"}
            onValueChange={(v) => setFilter(field.key, v === "__none__" ? "" : v, field.kind)}
          >
            <SelectTrigger className="h-8 w-full text-xs" aria-label={field.label}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__none__" className="text-xs">—</SelectItem>
              {field.options.map((option) => (
                <SelectItem key={option} value={option} className="text-xs">{option}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : field.kind === "boolean" ? (
          <Select
            value={raw || "__none__"}
            onValueChange={(v) => setFilter(field.key, v === "__none__" ? "" : v, field.kind)}
          >
            <SelectTrigger className="h-8 w-full text-xs" aria-label={field.label}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__none__" className="text-xs">—</SelectItem>
              <SelectItem value="true" className="text-xs">true</SelectItem>
              <SelectItem value="false" className="text-xs">false</SelectItem>
            </SelectContent>
          </Select>
        ) : (
          <Input
            {...common}
            placeholder={field.kind === "tags" ? '["a","b"]' : field.hint}
            aria-label={field.label}
          />
        )}
        {field.hint && <p className="text-[10px] text-muted-foreground">{field.hint}</p>}
        <FieldError message={message} />
        {(field.kind === "text" || field.kind === "id") && (
          <>
            <Input
              className="h-8 text-xs"
              placeholder={`${field.label} (contiene)`}
              value={textValue(trigger.filter?.[`${field.key}~`])}
              onChange={(e) => setFilter(`${field.key}~`, e.target.value)}
              aria-label={`${field.label} contiene`}
            />
            <FieldError message={errorFor(`filter.${field.key}~`)} />
          </>
        )}
      </div>
    );
  };

  return (
    <div className="space-y-2.5">
      <div className="space-y-1">
        <Label className="text-[11px]">Tipo de disparador</Label>
        <Select
          value={trigger.type === "schedule" ? `schedule:${spec.kind}` : `event:${trigger.event}`}
          onValueChange={(v) => {
            if (v.startsWith("schedule:")) {
              onChange({ type: "schedule", spec: { ...spec, kind: v.slice(9) as ScheduleSpec["kind"] } });
            } else {
              onChange({ type: "event", event: v.slice(6), filter: {} });
            }
          }}
        >
          <SelectTrigger className="h-8 w-full text-xs" aria-label="Disparador"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="schedule:daily" className="text-xs">Todos los días a una hora</SelectItem>
            <SelectItem value="schedule:weekly" className="text-xs">Días de la semana concretos</SelectItem>
            <SelectItem value="schedule:monthly" className="text-xs">Una vez al mes</SelectItem>
            <SelectItem value="schedule:interval" className="text-xs">Cada cierto intervalo</SelectItem>
            <SelectItem value="schedule:once" className="text-xs">Una vez en una fecha</SelectItem>
            {eventSpecs.map((t) => (
              <SelectItem key={t.id} value={`event:${t.id}`} className="text-xs">{t.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <FieldError message={errorFor("event") ?? errorFor("spec.kind")} />
      </div>

      {trigger.type === "schedule" && (
        <div className="space-y-2">
          {spec.kind !== "interval" && (
            <div className="space-y-0.5">
              <Label className="text-[11px]">Hora (HH:MM)</Label>
              <Input className="h-8 text-xs" placeholder="08:00" value={spec.time ?? ""} onChange={(e) => setSpec({ time: e.target.value || null })} aria-label="Hora" />
              <FieldError message={errorFor("spec.time")} />
            </div>
          )}
          {spec.kind === "weekly" && (
            <div className="space-y-0.5">
              <Label className="text-[11px]">Días de la semana</Label>
              <div className="flex gap-0.5">
                {Object.entries(WEEKDAY_LABEL).map(([id, label]) => {
                  const active = spec.weekdays.includes(id);
                  return (
                    <button
                      key={id}
                      type="button"
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
              <FieldError message={errorFor("spec.weekdays")} />
            </div>
          )}
          {spec.kind === "monthly" && (
            <div className="space-y-0.5">
              <Label className="text-[11px]">Día del mes (1-31)</Label>
              <Input type="number" min={1} max={31} className="h-8 w-20 text-xs" value={spec.day_of_month ?? ""} onChange={(e) => setSpec({ day_of_month: e.target.value === "" ? null : Number(e.target.value) })} aria-label="Día del mes" />
              <FieldError message={errorFor("spec.day_of_month")} />
            </div>
          )}
          {spec.kind === "interval" && (
            <div className="space-y-0.5">
              <Label className="text-[11px]">Intervalo (mínimo 5 min)</Label>
              <div className="flex items-center gap-1">
                <Input type="number" min={5} className="h-8 w-20 text-xs" value={spec.interval_minutes ?? ""} onChange={(e) => setSpec({ interval_minutes: e.target.value === "" ? null : Number(e.target.value) })} aria-label="Intervalo en minutos" />
                <span className="text-[11px] text-muted-foreground">min</span>
              </div>
              <FieldError message={errorFor("spec.interval_minutes")} />
            </div>
          )}
          {spec.kind === "once" && (
            <div className="space-y-0.5">
              <Label className="text-[11px]">Fecha (YYYY-MM-DD)</Label>
              <Input className="h-8 text-xs" placeholder="2026-12-31" value={spec.date ?? ""} onChange={(e) => setSpec({ date: e.target.value || null })} aria-label="Fecha" />
              <FieldError message={errorFor("spec.date")} />
            </div>
          )}
          <div className="space-y-0.5">
            <Label className="text-[11px]">Zona horaria (IANA, vacío = local)</Label>
            <Input className="h-8 text-xs" placeholder="Europe/Madrid" value={spec.timezone ?? ""} onChange={(e) => setSpec({ timezone: e.target.value || null })} aria-label="Zona horaria" />
            <FieldError message={errorFor("spec.timezone")} />
          </div>
        </div>
      )}

      {trigger.type === "event" && (
        <div className="space-y-2">
          <FieldError message={errorFor("filter")} />
          {fields.length > 0 ? fields.map(renderFilterField) : (
            <p className="text-[11px] text-muted-foreground">Sin filtros configurables</p>
          )}
        </div>
      )}
    </div>
  );
}
