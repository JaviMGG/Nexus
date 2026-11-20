import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Check, Trash2 } from "lucide-react";
import { invoke } from "@/lib/api";
import type { CalendarEvent, CreateEvent, EventColor, Project, UpdateEvent } from "@/lib/types";
import { EVENT_COLORS } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { EVENT_SWATCH_CLASS } from "./calendar-utils";

const NONE = "__none__";

interface EventDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  event?: CalendarEvent | null;
  defaultDate?: string;
}

export function EventDialog({ open, onOpenChange, event, defaultDate }: EventDialogProps) {
  const isEdit = Boolean(event);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{isEdit ? "Editar evento" : "Nuevo evento"}</DialogTitle>
          <DialogDescription>
            {isEdit ? "Modifica los detalles del evento." : "Planifica un evento en tu calendario."}
          </DialogDescription>
        </DialogHeader>
        {open && (
          <EventForm
            key={event?.id ?? `new-${defaultDate ?? ""}`}
            event={event}
            defaultDate={defaultDate}
            onDone={() => onOpenChange(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function EventForm({
  event,
  defaultDate,
  onDone,
}: {
  event?: CalendarEvent | null;
  defaultDate?: string;
  onDone: () => void;
}) {
  const qc = useQueryClient();
  const isEdit = Boolean(event);
  const [title, setTitle] = useState(event?.title ?? "");
  const [description, setDescription] = useState(event?.description ?? "");
  const [date, setDate] = useState(event?.date ?? defaultDate ?? "");
  const [allDay, setAllDay] = useState(event?.all_day ?? false);
  const [startTime, setStartTime] = useState(event?.start_time ?? "");
  const [endTime, setEndTime] = useState(event?.end_time ?? "");
  const [color, setColor] = useState<EventColor>(event?.color ?? "primary");
  const [location, setLocation] = useState(event?.location ?? "");
  const [projectId, setProjectId] = useState<string>(event?.project_id ?? NONE);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const projects = useQuery({
    queryKey: ["projects", { include_archived: false }],
    queryFn: () => invoke<Project[]>("projects.list", { include_archived: false }),
  });

  const invalidate = () => {
    ["events", "dashboard"].forEach((k) => qc.invalidateQueries({ queryKey: [k] }));
  };

  const create = useMutation<CalendarEvent, Error, CreateEvent>({
    mutationFn: (data) => invoke<CalendarEvent>("events.create", { data }),
    onSuccess: () => {
      toast.success("Evento creado");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo crear el evento", { description: err.message }),
  });

  const update = useMutation<CalendarEvent, Error, UpdateEvent>({
    mutationFn: (data) => invoke<CalendarEvent>("events.update", { id: event!.id, data }),
    onSuccess: () => {
      toast.success("Evento guardado");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo guardar el evento", { description: err.message }),
  });

  const remove = useMutation<{ deleted: boolean }, Error, void>({
    mutationFn: () => invoke<{ deleted: boolean }>("events.delete", { id: event!.id }),
    onSuccess: () => {
      toast.success("Evento eliminado");
      invalidate();
      onDone();
    },
    onError: (err) => toast.error("No se pudo eliminar el evento", { description: err.message }),
  });

  const pending = create.isPending || update.isPending || remove.isPending;

  const submit = () => {
    const t = title.trim();
    if (!t) {
      toast.error("El título es obligatorio");
      return;
    }
    if (!date) {
      toast.error("La fecha es obligatoria");
      return;
    }
    const pid = projectId === NONE ? null : projectId;
    const times = allDay
      ? { start_time: null, end_time: null }
      : { start_time: startTime || null, end_time: endTime || null };
    if (isEdit) {
      update.mutate({
        title: t,
        description,
        date,
        all_day: allDay,
        ...times,
        color,
        location: location.trim() || null,
        project_id: pid,
      });
    } else {
      create.mutate({
        title: t,
        description,
        date,
        all_day: allDay,
        ...times,
        color,
        location: location.trim(),
        project_id: pid,
      });
    }
  };

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <Label htmlFor="event-title">Título</Label>
        <Input
          id="event-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="¿Qué va a pasar?"
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submit();
            }
          }}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="event-desc">Descripción</Label>
        <Textarea
          id="event-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Detalles opcionales…"
          rows={2}
        />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <Label htmlFor="event-date">Fecha</Label>
          <Input id="event-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
        <div className="flex items-end pb-2">
          <div className="flex items-center gap-2">
            <Switch id="event-all-day" checked={allDay} onCheckedChange={setAllDay} />
            <Label htmlFor="event-all-day" className="cursor-pointer">
              Todo el día
            </Label>
          </div>
        </div>
        {!allDay && (
          <>
            <div className="space-y-1.5">
              <Label htmlFor="event-start">Hora de inicio</Label>
              <Input
                id="event-start"
                type="time"
                value={startTime}
                onChange={(e) => setStartTime(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="event-end">Hora de fin</Label>
              <Input
                id="event-end"
                type="time"
                value={endTime}
                onChange={(e) => setEndTime(e.target.value)}
              />
            </div>
          </>
        )}
        <div className="space-y-1.5">
          <Label htmlFor="event-location">Ubicación</Label>
          <Input
            id="event-location"
            value={location}
            onChange={(e) => setLocation(e.target.value)}
            placeholder="p. ej. Sala 2, Meet…"
          />
        </div>
        <div className="space-y-1.5">
          <Label>Proyecto</Label>
          <Select value={projectId} onValueChange={setProjectId} disabled={projects.isLoading}>
            <SelectTrigger>
              <SelectValue placeholder="Ninguno" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NONE}>Ninguno</SelectItem>
              {(projects.data ?? []).map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="space-y-1.5">
        <Label>Color</Label>
        <div className="flex items-center gap-2">
          {EVENT_COLORS.map((c) => (
            <button
              key={c.id}
              type="button"
              title={c.label}
              aria-label={`Color ${c.label}`}
              onClick={() => setColor(c.id)}
              className={cn(
                "flex size-7 items-center justify-center rounded-full transition-all cursor-pointer hover:scale-110",
                EVENT_SWATCH_CLASS[c.id],
                color === c.id ? "ring-2 ring-ring ring-offset-2 ring-offset-popover" : "opacity-70",
              )}
            >
              {color === c.id && <Check className="size-3.5" />}
            </button>
          ))}
        </div>
      </div>
      {isEdit && confirmDelete && (
        <div className="flex items-center justify-between gap-3 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 animate-fade-in">
          <p className="text-xs text-destructive">¿Eliminar este evento? Podrás restaurarlo desde Actividad.</p>
          <div className="flex shrink-0 gap-2">
            <Button variant="ghost" size="sm" onClick={() => setConfirmDelete(false)} disabled={pending}>
              Cancelar
            </Button>
            <Button variant="destructive" size="sm" onClick={() => remove.mutate()} disabled={pending}>
              {remove.isPending ? "Eliminando…" : "Eliminar"}
            </Button>
          </div>
        </div>
      )}
      <DialogFooter className={cn(isEdit && !confirmDelete && "sm:justify-between")}>
        {isEdit && !confirmDelete && (
          <Button
            variant="ghost"
            className="text-destructive hover:text-destructive"
            onClick={() => setConfirmDelete(true)}
            disabled={pending}
          >
            <Trash2 /> Eliminar
          </Button>
        )}
        <div className="flex gap-2">
          <Button variant="outline" onClick={onDone} disabled={pending}>
            Cancelar
          </Button>
          <Button onClick={submit} disabled={pending || !title.trim() || !date}>
            {pending ? "Guardando…" : isEdit ? "Guardar cambios" : "Crear evento"}
          </Button>
        </div>
      </DialogFooter>
    </div>
  );
}
