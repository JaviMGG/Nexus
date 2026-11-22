import { useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  BookOpen,
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Coffee,
  Cookie,
  Copy,
  FileText,
  Loader2,
  Moon,
  MoreHorizontal,
  Plus,
  Search,
  Sparkles,
  Sun,
  Sunrise,
  Trash2,
  UtensilsCrossed,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import {
  MenuTemplate,
  MenuTemplateDetail,
  MealSlot,
  PlanEntryInput,
  PlanEntryView,
  Recipe,
  RecipeImage,
  SLOT_LABEL,
  SLOTS,
  WeekPlan,
  addDays,
  dayHeader,
  mondayOf,
  toDateStr,
  toastMealError,
  useInvalidateMeals,
  weekLabel,
} from "./shared";
import { RecipeDetailDialog } from "./RecipeDetailDialog";

const SLOT_ICON: Record<MealSlot, LucideIcon> = {
  desayuno: Coffee,
  almuerzo: Sunrise,
  comida: Sun,
  merienda: Cookie,
  cena: Moon,
};

const DAY_NAMES = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];

type DragPayload = { kind: "recipe"; recipeId: Id } | { kind: "entry"; entryId: Id };

function setDrag(e: React.DragEvent, payload: DragPayload) {
  e.dataTransfer.setData("application/x-nexus-meal", JSON.stringify(payload));
  e.dataTransfer.effectAllowed = "copyMove";
}

function getDrag(e: React.DragEvent): DragPayload | null {
  try {
    const raw = e.dataTransfer.getData("application/x-nexus-meal");
    return raw ? (JSON.parse(raw) as DragPayload) : null;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Diálogo: añadir receta a una celda
// ---------------------------------------------------------------------------

function AddMealDialog({
  target,
  recipes,
  onClose,
}: {
  target: { date: string; slot: MealSlot } | null;
  recipes: Recipe[];
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [search, setSearch] = useState("");
  const [servings, setServings] = useState("2");

  const add = useMutation({
    mutationFn: (recipeId: Id) =>
      invoke("meals.plan.add", {
        date: target?.date,
        slot: target?.slot,
        recipe_id: recipeId,
        servings: Number(servings) || 2,
      }),
    onSuccess: () => {
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo añadir la comida"),
  });

  const filtered = recipes.filter(
    (r) =>
      r.name.toLowerCase().includes(search.toLowerCase()) ||
      r.ingredients.some((i) => i.name.toLowerCase().includes(search.toLowerCase())),
  );

  return (
    <Dialog open={!!target} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Añadir comida</DialogTitle>
          <DialogDescription>
            {target && `${SLOT_LABEL[target.slot]} · ${new Date(target.date + "T00:00:00").toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" })}`}
          </DialogDescription>
        </DialogHeader>
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input autoFocus value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar receta…" className="pl-8" />
          </div>
          <div className="w-24">
            <Input type="number" min={1} max={99} value={servings} onChange={(e) => setServings(e.target.value)} title="Raciones" />
          </div>
        </div>
        <div className="max-h-72 space-y-1 overflow-y-auto">
          {filtered.length === 0 ? (
            <p className="py-6 text-center text-xs text-muted-foreground">
              {recipes.length === 0 ? "Tu recetario está vacío: crea recetas en la pestaña Recetario." : "Sin resultados."}
            </p>
          ) : (
            filtered.map((r) => (
              <button
                key={r.id}
                type="button"
                disabled={add.isPending}
                onClick={() => add.mutate(r.id)}
                className="flex w-full items-center gap-2.5 rounded-lg border border-border bg-card px-2.5 py-2 text-left transition-colors hover:bg-accent cursor-pointer"
              >
                <div className="h-9 w-12 shrink-0 overflow-hidden rounded-md">
                  <RecipeImage fileId={r.image_file_id} alt={r.name} iconClassName="h-4 w-4" />
                </div>
                <span className="min-w-0 flex-1 truncate text-sm">{r.name}</span>
                <span className="text-[11px] text-muted-foreground">{r.servings} rac.</span>
              </button>
            ))
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Diálogo: duplicar comida a otro día/slot
// ---------------------------------------------------------------------------

function DuplicateEntryDialog({
  entry,
  weekStart,
  onClose,
}: {
  entry: PlanEntryView | null;
  weekStart: string;
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [day, setDay] = useState("0");
  const [slot, setSlot] = useState<MealSlot>("comida");

  const dup = useMutation({
    mutationFn: () =>
      invoke("meals.plan.duplicate", {
        id: entry?.id,
        date: addDays(weekStart, Number(day)),
        slot,
      }),
    onSuccess: () => {
      toast.success("Comida duplicada");
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo duplicar la comida"),
  });

  return (
    <Dialog open={!!entry} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Duplicar «{entry?.recipe_name}»</DialogTitle>
          <DialogDescription>Elige el día y momento de destino (esta semana).</DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1.5">
            <Label>Día</Label>
            <Select value={day} onValueChange={setDay}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {DAY_NAMES.map((d, i) => (
                  <SelectItem key={i} value={String(i)}>{d}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label>Momento</Label>
            <Select value={slot} onValueChange={(v) => setSlot(v as MealSlot)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                {SLOTS.map((s) => <SelectItem key={s} value={s}>{SLOT_LABEL[s]}</SelectItem>)}
              </SelectContent>
            </Select>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => dup.mutate()} disabled={dup.isPending}>
            {dup.isPending && <Loader2 className="animate-spin" />} Duplicar
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// IA: propuesta de semana y resumen
// ---------------------------------------------------------------------------

function AiWeekDialog({
  open,
  weekStart,
  recipes,
  onClose,
}: {
  open: boolean;
  weekStart: string;
  recipes: Recipe[];
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [preferences, setPreferences] = useState("");
  const [proposal, setProposal] = useState<PlanEntryInput[] | null>(null);
  const [servings, setServings] = useState<Record<number, string>>({});

  const ask = useMutation({
    mutationFn: () =>
      invoke<{ week_start: string; entries: PlanEntryInput[] }>("meals.ai_week", {
        week_start: weekStart,
        preferences: preferences.trim() || null,
      }),
    onSuccess: (r) => {
      setProposal(r.entries);
      setServings({});
    },
    onError: (e) => toastMealError(e, "No se pudo generar el menú"),
  });

  const apply = useMutation({
    mutationFn: (entries: PlanEntryInput[]) =>
      invoke("meals.plan.apply", { week_start: weekStart, entries, replace: true }),
    onSuccess: () => {
      toast.success("Menú aplicado a la semana");
      invalidate();
      setProposal(null);
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo aplicar el menú"),
  });

  const recipeName = (id: Id) => recipes.find((r) => r.id === id)?.name ?? "Receta";

  const editedEntries = (proposal ?? []).map((e, i) => ({
    ...e,
    servings: Number(servings[i]) || e.servings,
  }));

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) { setProposal(null); onClose(); } }}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-4 w-4 text-primary" /> Menú semanal con IA
          </DialogTitle>
          <DialogDescription>
            Propuesta editable con recetas de tu recetario. Aplicarla sustituye el contenido de {weekLabel(weekStart).toLowerCase()}.
          </DialogDescription>
        </DialogHeader>
        {!proposal ? (
          <>
            <div className="space-y-1.5">
              <Label>Preferencias (opcional)</Label>
              <Textarea
                value={preferences}
                onChange={(e) => setPreferences(e.target.value)}
                placeholder="cenas ligeras, sin repetir plato, aprovechar el pescado…"
              />
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={onClose}>Cancelar</Button>
              <Button onClick={() => ask.mutate()} disabled={ask.isPending || recipes.length === 0}>
                {ask.isPending ? <Loader2 className="animate-spin" /> : <Sparkles />}
                {ask.isPending ? "Generando…" : "Proponer menú"}
              </Button>
            </DialogFooter>
            {recipes.length === 0 && (
              <p className="text-xs text-warning">Crea primero algunas recetas en el recetario.</p>
            )}
          </>
        ) : (
          <>
            <div className="max-h-80 space-y-1.5 overflow-y-auto">
              {editedEntries.map((e, i) => (
                <div key={i} className="flex items-center gap-2 rounded-lg border border-border bg-card px-2.5 py-1.5">
                  <span className="w-20 shrink-0 text-[11px] text-muted-foreground">
                    {DAY_NAMES[(new Date(e.date + "T00:00:00").getDay() + 6) % 7]}
                  </span>
                  <Badge variant="secondary" className="shrink-0">{SLOT_LABEL[e.slot]}</Badge>
                  <span className="min-w-0 flex-1 truncate text-sm">{recipeName(e.recipe_id)}</span>
                  <Input
                    type="number"
                    min={1}
                    max={99}
                    className="h-7 w-16 text-xs"
                    value={servings[i] ?? String(e.servings)}
                    onChange={(ev) => setServings((s) => ({ ...s, [i]: ev.target.value }))}
                    title="Raciones"
                  />
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    className="text-muted-foreground hover:text-destructive"
                    onClick={() => setProposal((p) => (p ?? []).filter((_, n) => n !== i))}
                    title="Quitar de la propuesta"
                  >
                    <Trash2 />
                  </Button>
                </div>
              ))}
              {editedEntries.length === 0 && (
                <p className="py-4 text-center text-xs text-muted-foreground">Propuesta vacía: vuelve a generar.</p>
              )}
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => setProposal(null)}>Volver</Button>
              <Button
                onClick={() => {
                  if (window.confirm(`¿Aplicar esta propuesta? Se sustituirán las comidas de ${weekLabel(weekStart).toLowerCase()}.`)) {
                    apply.mutate(editedEntries);
                  }
                }}
                disabled={apply.isPending || editedEntries.length === 0}
              >
                {apply.isPending && <Loader2 className="animate-spin" />} Aplicar a la semana
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

function AiSummaryDialog({ open, weekStart, onClose }: { open: boolean; weekStart: string; onClose: () => void }) {
  const [markdown, setMarkdown] = useState<string | null>(null);
  const ask = useMutation({
    mutationFn: () => invoke<{ markdown: string }>("meals.ai_summary", { week_start: weekStart }),
    onSuccess: (r) => setMarkdown(r.markdown),
    onError: (e) => {
      toastMealError(e, "No se pudo generar el resumen");
      onClose();
    },
  });

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) { setMarkdown(null); onClose(); } }}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FileText className="h-4 w-4 text-primary" /> Resumen de la semana
          </DialogTitle>
          <DialogDescription>{weekLabel(weekStart)}</DialogDescription>
        </DialogHeader>
        {!markdown && !ask.isPending && (
          <DialogFooter>
            <Button onClick={() => ask.mutate()}>
              <Sparkles /> Generar resumen con IA
            </Button>
          </DialogFooter>
        )}
        {ask.isPending && (
          <div className="flex items-center gap-2 py-6 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Analizando tu semana…
          </div>
        )}
        {markdown && (
          <div className="markdown-body rounded-lg border border-border bg-card p-4">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{markdown}</ReactMarkdown>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Menús personalizados
// ---------------------------------------------------------------------------

function MenusDialog({
  open,
  weekStart,
  onClose,
}: {
  open: boolean;
  weekStart: string;
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [name, setName] = useState("");
  const [detailId, setDetailId] = useState<Id | null>(null);

  const menus = useQuery({
    queryKey: ["meals", "menus"],
    queryFn: () => invoke<MenuTemplate[]>("meals.menus.list"),
    enabled: open,
  });

  const detail = useQuery({
    queryKey: ["meals", "menus", detailId],
    queryFn: () => invoke<MenuTemplateDetail>("meals.menus.get", { id: detailId }),
    enabled: !!detailId,
  });

  const save = useMutation({
    mutationFn: () => invoke("meals.menus.save", { name: name.trim(), week_start: weekStart }),
    onSuccess: () => {
      toast.success("Menú guardado", { description: "Podrás aplicarlo a cualquier semana." });
      setName("");
      invalidate();
    },
    onError: (e) => toastMealError(e, "No se pudo guardar el menú"),
  });

  const apply = useMutation({
    mutationFn: (id: Id) => invoke<{ skipped_recipe_ids: Id[] }>("meals.menus.apply", { id, week_start: weekStart }),
    onSuccess: (r) => {
      toast.success("Menú aplicado", {
        description: r.skipped_recipe_ids.length > 0
          ? `${r.skipped_recipe_ids.length} recetas ya no existen y se omitieron.`
          : undefined,
      });
      invalidate();
      setDetailId(null);
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo aplicar el menú"),
  });

  const del = useMutation({
    mutationFn: (id: Id) => invoke("meals.menus.delete", { id }),
    onSuccess: () => {
      toast.success("Menú eliminado");
      invalidate();
    },
    onError: (e) => toastMealError(e, "No se pudo eliminar el menú"),
  });

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <BookOpen className="h-4 w-4 text-primary" /> Menús personalizados
          </DialogTitle>
          <DialogDescription>
            Guarda la semana actual como plantilla y aplícala cuando quieras.
          </DialogDescription>
        </DialogHeader>

        <div className="flex items-end gap-2">
          <div className="flex-1 space-y-1.5">
            <Label>Guardar {weekLabel(weekStart).toLowerCase()}</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Nombre del menú (p. ej. Menú de invierno)" />
          </div>
          <Button onClick={() => save.mutate()} disabled={!name.trim() || save.isPending}>
            {save.isPending && <Loader2 className="animate-spin" />} Guardar
          </Button>
        </div>

        <div className="space-y-1.5">
          <Label>Menús guardados</Label>
          {menus.isLoading ? (
            <Skeleton className="h-20 rounded-lg" />
          ) : (menus.data ?? []).length === 0 ? (
            <p className="rounded-lg border border-dashed border-border px-3 py-4 text-xs text-muted-foreground">
              Aún no has guardado ningún menú.
            </p>
          ) : (
            (menus.data ?? []).map((t) => (
              <div key={t.id} className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2">
                <button type="button" className="min-w-0 flex-1 text-left cursor-pointer" onClick={() => setDetailId(t.id)}>
                  <p className="truncate text-sm font-medium">{t.name}</p>
                  <p className="text-[11px] text-muted-foreground">{t.entries.length} comidas</p>
                </button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    if (window.confirm(`¿Aplicar «${t.name}»? Se sustituirán las comidas de ${weekLabel(weekStart).toLowerCase()}.`)) {
                      apply.mutate(t.id);
                    }
                  }}
                  disabled={apply.isPending}
                >
                  Aplicar aquí
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => {
                    if (window.confirm(`¿Eliminar el menú «${t.name}»?`)) del.mutate(t.id);
                  }}
                  title="Eliminar menú"
                >
                  <Trash2 />
                </Button>
              </div>
            ))
          )}
        </div>

        <Dialog open={!!detailId} onOpenChange={(o) => !o && setDetailId(null)}>
          <DialogContent className="max-w-md">
            <DialogHeader>
              <DialogTitle>{detail.data?.name ?? "Menú"}</DialogTitle>
              <DialogDescription>Contenido de la plantilla.</DialogDescription>
            </DialogHeader>
            <div className="max-h-72 space-y-1 overflow-y-auto">
              {(detail.data?.entries ?? []).map((e, i) => (
                <div key={i} className="flex items-center gap-2 rounded-md border border-border/60 px-2.5 py-1.5 text-sm">
                  <span className="w-20 shrink-0 text-[11px] text-muted-foreground">{DAY_NAMES[e.day_offset] ?? `Día ${e.day_offset}`}</span>
                  <Badge variant="secondary" className="shrink-0">{SLOT_LABEL[e.slot]}</Badge>
                  <span className={cn("min-w-0 flex-1 truncate", !e.recipe_name && "text-destructive")}>
                    {e.recipe_name ?? "Receta eliminada"}
                  </span>
                  <span className="text-[11px] text-muted-foreground">{e.servings} rac.</span>
                </div>
              ))}
            </div>
          </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Celda de comida
// ---------------------------------------------------------------------------

function MealChip({
  entry,
  onServings,
  onDuplicate,
  onDelete,
  onOpenRecipe,
}: {
  entry: PlanEntryView;
  onServings: (servings: number) => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onOpenRecipe: () => void;
}) {
  return (
    <div
      draggable
      onDragStart={(e) => setDrag(e, { kind: "entry", entryId: entry.id })}
      className="group/chip cursor-grab rounded-md border border-border bg-card px-1.5 py-1 shadow-sm transition-colors hover:border-ring/50 active:cursor-grabbing"
    >
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={onOpenRecipe}
          className="min-w-0 flex-1 truncate text-left text-[11px] font-medium leading-tight cursor-pointer hover:text-primary"
          title={entry.recipe_name}
        >
          {entry.recipe_name}
        </button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              className="shrink-0 rounded p-0.5 text-muted-foreground opacity-0 transition-opacity hover:bg-accent group-hover/chip:opacity-100 cursor-pointer"
              title="Opciones"
            >
              <MoreHorizontal className="h-3 w-3" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="text-xs">
            <DropdownMenuLabel className="text-[11px]">Raciones</DropdownMenuLabel>
            <div className="flex items-center gap-1 px-2 py-1">
              <Button size="icon-sm" variant="outline" onClick={() => onServings(Math.max(1, entry.servings - 1))}>−</Button>
              <span className="w-8 text-center text-xs tabular-nums">{entry.servings}</span>
              <Button size="icon-sm" variant="outline" onClick={() => onServings(Math.min(99, entry.servings + 1))}>+</Button>
            </div>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={onDuplicate}><Copy /> Duplicar en…</DropdownMenuItem>
            <DropdownMenuItem onClick={onOpenRecipe}><UtensilsCrossed /> Ver receta</DropdownMenuItem>
            <DropdownMenuItem className="text-destructive focus:text-destructive" onClick={onDelete}>
              <Trash2 /> Eliminar
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <span className="text-[10px] text-muted-foreground">{entry.servings} rac.</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pestaña Semana
// ---------------------------------------------------------------------------

export function WeekTab() {
  const invalidate = useInvalidateMeals();
  const [weekStart, setWeekStart] = useState(() => mondayOf(toDateStr(new Date())));
  const [dragOver, setDragOver] = useState<string | null>(null);
  const [addTarget, setAddTarget] = useState<{ date: string; slot: MealSlot } | null>(null);
  const [dupEntry, setDupEntry] = useState<PlanEntryView | null>(null);
  const [detailId, setDetailId] = useState<Id | null>(null);
  const [menusOpen, setMenusOpen] = useState(false);
  const [aiWeekOpen, setAiWeekOpen] = useState(false);
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [stripSearch, setStripSearch] = useState("");

  const plan = useQuery({
    queryKey: ["meals", "plan", weekStart],
    queryFn: () => invoke<WeekPlan>("meals.plan.get", { week_start: weekStart }),
  });

  const recipes = useQuery({
    queryKey: ["meals", "recipes", "strip"],
    queryFn: () => invoke<Recipe[]>("meals.recipes.list", { filter: { sort: "name" } }),
  });

  const addEntry = useMutation({
    mutationFn: (v: { date: string; slot: MealSlot; recipeId: Id }) =>
      invoke("meals.plan.add", { date: v.date, slot: v.slot, recipe_id: v.recipeId, servings: 2 }),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo añadir la comida"),
  });

  const moveEntry = useMutation({
    mutationFn: (v: { id: Id; date: string; slot: MealSlot }) =>
      invoke("meals.plan.move", v),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo mover la comida"),
  });

  const setServings = useMutation({
    mutationFn: (v: { id: Id; servings: number }) => invoke("meals.plan.update", v),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudieron cambiar las raciones"),
  });

  const deleteEntry = useMutation({
    mutationFn: (id: Id) => invoke("meals.plan.delete", { id }),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo eliminar la comida"),
  });

  const days = useMemo(() => Array.from({ length: 7 }, (_, i) => addDays(weekStart, i)), [weekStart]);
  const entries = plan.data?.entries ?? [];
  const today = toDateStr(new Date());

  const byCell = useMemo(() => {
    const map = new Map<string, PlanEntryView[]>();
    for (const e of entries) {
      const key = `${e.date}|${e.slot}`;
      const arr = map.get(key) ?? [];
      arr.push(e);
      map.set(key, arr);
    }
    return map;
  }, [entries]);

  const handleDrop = (e: React.DragEvent, date: string, slot: MealSlot) => {
    e.preventDefault();
    setDragOver(null);
    const payload = getDrag(e);
    if (!payload) return;
    if (payload.kind === "recipe") {
      addEntry.mutate({ date, slot, recipeId: payload.recipeId });
    } else {
      const entry = entries.find((x) => x.id === payload.entryId);
      if (entry && (entry.date !== date || entry.slot !== slot)) {
        moveEntry.mutate({ id: payload.entryId, date, slot });
      }
    }
  };

  const stripRecipes = (recipes.data ?? []).filter((r) =>
    r.name.toLowerCase().includes(stripSearch.toLowerCase()),
  );

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1">
          <Button size="icon-sm" variant="outline" onClick={() => setWeekStart(addDays(weekStart, -7))} title="Semana anterior">
            <ChevronLeft />
          </Button>
          <Button size="sm" variant="outline" onClick={() => setWeekStart(mondayOf(today))}>Hoy</Button>
          <Button size="icon-sm" variant="outline" onClick={() => setWeekStart(addDays(weekStart, 7))} title="Semana siguiente">
            <ChevronRight />
          </Button>
        </div>
        <span className="flex items-center gap-1.5 text-sm font-semibold">
          <CalendarDays className="h-4 w-4 text-primary" /> {weekLabel(weekStart)}
        </span>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <Button size="sm" variant="outline" onClick={() => setSummaryOpen(true)}>
            <FileText /> Resumen IA
          </Button>
          <Button size="sm" variant="outline" onClick={() => setAiWeekOpen(true)}>
            <Sparkles /> Menú con IA
          </Button>
          <Button size="sm" variant="outline" onClick={() => setMenusOpen(true)}>
            <BookOpen /> Menús
          </Button>
        </div>
      </div>

      <div className="flex gap-3">
        {/* Tira lateral de recetas: origen del drag & drop */}
        <aside className="hidden w-48 shrink-0 lg:block">
          <div className="sticky top-2 space-y-2 rounded-xl border border-border bg-muted/40 p-2.5">
            <p className="px-0.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              Recetas · arrastra al plan
            </p>
            <div className="relative">
              <Search className="absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={stripSearch}
                onChange={(e) => setStripSearch(e.target.value)}
                placeholder="Buscar…"
                className="h-7 pl-7 text-xs"
              />
            </div>
            <div className="max-h-[520px] space-y-1.5 overflow-y-auto pr-0.5">
              {stripRecipes.map((r) => (
                <div
                  key={r.id}
                  draggable
                  onDragStart={(e) => setDrag(e, { kind: "recipe", recipeId: r.id })}
                  className="flex cursor-grab items-center gap-2 rounded-lg border border-border bg-card p-1.5 shadow-sm transition-colors hover:border-ring/50 active:cursor-grabbing"
                  title={`Arrastra «${r.name}» a una celda`}
                >
                  <div className="h-8 w-10 shrink-0 overflow-hidden rounded">
                    <RecipeImage fileId={r.image_file_id} alt={r.name} iconClassName="h-3.5 w-3.5" />
                  </div>
                  <span className="min-w-0 flex-1 truncate text-[11px] font-medium">{r.name}</span>
                </div>
              ))}
              {stripRecipes.length === 0 && (
                <p className="px-1 py-3 text-center text-[11px] text-muted-foreground">
                  {(recipes.data ?? []).length === 0 ? "Crea recetas en el recetario." : "Sin resultados."}
                </p>
              )}
            </div>
          </div>
        </aside>

        {/* Grid semanal */}
        <div className="min-w-0 flex-1 overflow-x-auto">
          {plan.isLoading ? (
            <Skeleton className="h-96 w-full rounded-xl" />
          ) : (
            <div className="grid min-w-[760px] grid-cols-[88px_repeat(7,minmax(96px,1fr))] overflow-hidden rounded-xl border border-border bg-card shadow-sm">
              <div className="border-b border-r border-border bg-muted/40 p-2" />
              {days.map((d) => {
                const h = dayHeader(d);
                const isToday = d === today;
                return (
                  <div
                    key={d}
                    className={cn(
                      "border-b border-r border-border p-2 text-center last:border-r-0",
                      isToday ? "bg-primary/15" : "bg-muted/40",
                    )}
                  >
                    <p className={cn("text-[11px] font-semibold uppercase", isToday && "text-primary")}>{h.name}</p>
                    <p className="text-[10px] text-muted-foreground">{h.num}</p>
                  </div>
                );
              })}
              {SLOTS.map((slot) => {
                const Icon = SLOT_ICON[slot];
                return (
                  <div key={slot} className="contents">
                    <div className="flex items-center gap-1.5 border-b border-r border-border bg-muted/30 px-2 py-2">
                      <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                      <span className="text-[11px] font-medium">{SLOT_LABEL[slot]}</span>
                    </div>
                    {days.map((date) => {
                      const key = `${date}|${slot}`;
                      const cellEntries = byCell.get(key) ?? [];
                      return (
                        <div
                          key={key}
                          onDragOver={(e) => {
                            e.preventDefault();
                            setDragOver(key);
                          }}
                          onDragLeave={() => setDragOver((s) => (s === key ? null : s))}
                          onDrop={(e) => handleDrop(e, date, slot)}
                          className={cn(
                            "group/cell min-h-[74px] space-y-1 border-b border-r border-border p-1 transition-colors last:border-r-0",
                            dragOver === key && "bg-primary/10 ring-2 ring-inset ring-ring",
                          )}
                        >
                          {cellEntries.map((entry) => (
                            <MealChip
                              key={entry.id}
                              entry={entry}
                              onServings={(servings) => setServings.mutate({ id: entry.id, servings })}
                              onDuplicate={() => setDupEntry(entry)}
                              onDelete={() => deleteEntry.mutate(entry.id)}
                              onOpenRecipe={() => setDetailId(entry.recipe_id)}
                            />
                          ))}
                          <button
                            type="button"
                            onClick={() => setAddTarget({ date, slot })}
                            className={cn(
                              "flex w-full items-center justify-center rounded-md border border-dashed border-border py-1 text-muted-foreground opacity-0 transition-opacity hover:border-ring/60 hover:text-foreground group-hover/cell:opacity-100 cursor-pointer",
                              cellEntries.length === 0 && "opacity-40",
                            )}
                            title="Añadir comida"
                          >
                            <Plus className="h-3 w-3" />
                          </button>
                        </div>
                      );
                    })}
                  </div>
                );
              })}
            </div>
          )}
          {entries.length === 0 && !plan.isLoading && (
            <p className="mt-2 text-center text-xs text-muted-foreground">
              Semana vacía: arrastra recetas desde la tira lateral (o usa el botón + de cada celda).
            </p>
          )}
        </div>
      </div>

      <AddMealDialog target={addTarget} recipes={recipes.data ?? []} onClose={() => setAddTarget(null)} />
      <DuplicateEntryDialog entry={dupEntry} weekStart={weekStart} onClose={() => setDupEntry(null)} />
      <RecipeDetailDialog recipeId={detailId} onClose={() => setDetailId(null)} onEdit={() => setDetailId(null)} />
      <MenusDialog open={menusOpen} weekStart={weekStart} onClose={() => setMenusOpen(false)} />
      <AiWeekDialog open={aiWeekOpen} weekStart={weekStart} recipes={recipes.data ?? []} onClose={() => setAiWeekOpen(false)} />
      <AiSummaryDialog open={summaryOpen} weekStart={weekStart} onClose={() => setSummaryOpen(false)} />
    </div>
  );
}
