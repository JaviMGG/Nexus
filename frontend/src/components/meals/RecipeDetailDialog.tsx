import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  ArrowLeftRight,
  CalendarPlus,
  ChefHat,
  Clock,
  Copy,
  Heart,
  Loader2,
  Minus,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
  Users,
} from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
  CreateRecipe,
  DIFFICULTY_LABEL,
  DIFFICULTY_VARIANT,
  Ingredient,
  MealSlot,
  Recipe,
  RecipeImage,
  SLOT_LABEL,
  SLOTS,
  fmtMinutes,
  fmtQty,
  mondayOf,
  scaleQuantity,
  toastMealError,
  useInvalidateMeals,
  weekLabel,
} from "./shared";

function ScaledIngredientRow({ ing, base, target }: { ing: Ingredient; base: number; target: number }) {
  const qty = scaleQuantity(ing.quantity, base, target);
  return (
    <div className="flex items-baseline justify-between gap-3 border-b border-border/50 py-2 group-last:border-0">
      <div className="min-w-0">
        <span className="text-sm">{ing.name}</span>
        {ing.notes && <span className="ml-1.5 text-[11px] text-muted-foreground">({ing.notes})</span>}
      </div>
      <span className="shrink-0 text-sm font-medium tabular-nums">
        {fmtQty(qty)} <span className="text-xs font-normal text-muted-foreground">{ing.unit}</span>
      </span>
    </div>
  );
}

function SubstituteDialog({
  recipe,
  ingredient,
  onClose,
}: {
  recipe: Recipe;
  ingredient: string | null;
  onClose: () => void;
}) {
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const ask = useMutation({
    mutationFn: () =>
      invoke<{ suggestions: string[] }>("meals.ai_substitute", {
        recipe_id: recipe.id,
        ingredient,
      }),
    onSuccess: (r) => setSuggestions(r.suggestions),
    onError: (e) => {
      toastMealError(e, "No se pudieron obtener sustituciones");
      onClose();
    },
  });

  useEffect(() => {
    if (ingredient) {
      setSuggestions([]);
      ask.mutate();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ingredient]);

  return (
    <Dialog open={!!ingredient} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ArrowLeftRight className="h-4 w-4 text-primary" /> Sustituir «{ingredient}»
          </DialogTitle>
          <DialogDescription>
            Sugerencias de IA editables: no se aplican automáticamente, ajusta la receta a mano si te convencen.
          </DialogDescription>
        </DialogHeader>
        {ask.isPending ? (
          <div className="flex items-center gap-2 py-6 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Consultando al proveedor de IA…
          </div>
        ) : (
          <div className="space-y-2">
            {suggestions.map((s, i) => (
              <div key={i} className="rounded-lg border border-border bg-card px-3 py-2 text-sm">
                {s}
              </div>
            ))}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function AddToPlanDialog({
  recipe,
  open,
  onClose,
}: {
  recipe: Recipe;
  open: boolean;
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [date, setDate] = useState(() => new Date().toISOString().slice(0, 10));
  const [slot, setSlot] = useState<MealSlot>("comida");
  const [servings, setServings] = useState(String(recipe.servings));

  const add = useMutation({
    mutationFn: () =>
      invoke("meals.plan.add", { date, slot, recipe_id: recipe.id, servings: Number(servings) || recipe.servings }),
    onSuccess: () => {
      toast.success("Comida añadida al plan", { description: weekLabel(mondayOf(date)) });
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo añadir al plan"),
  });

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Añadir al plan semanal</DialogTitle>
          <DialogDescription>Elige día, momento y raciones para «{recipe.name}».</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Día</Label>
            <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
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
          <div className="space-y-1.5">
            <Label>Raciones</Label>
            <Input type="number" min={1} max={99} value={servings} onChange={(e) => setServings(e.target.value)} />
          </div>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => add.mutate()} disabled={add.isPending || !date}>
            {add.isPending && <Loader2 className="animate-spin" />} Añadir
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export function RecipeDetailDialog({
  recipeId,
  onClose,
  onEdit,
  onDuplicateProposal,
}: {
  recipeId: Id | null;
  onClose: () => void;
  onEdit: (r: Recipe) => void;
  onDuplicateProposal?: (data: CreateRecipe) => void;
}) {
  const invalidate = useInvalidateMeals();
  const [target, setTarget] = useState<number | null>(null);
  const [substituteIng, setSubstituteIng] = useState<string | null>(null);
  const [planOpen, setPlanOpen] = useState(false);

  const recipe = useQuery({
    queryKey: ["meals", "recipe", recipeId],
    queryFn: () => invoke<Recipe>("meals.recipes.get", { id: recipeId }),
    enabled: !!recipeId,
  });

  useEffect(() => {
    setTarget(null);
    setSubstituteIng(null);
    setPlanOpen(false);
  }, [recipeId]);

  const fav = useMutation({
    mutationFn: (id: Id) => invoke<Recipe>("meals.recipes.toggle_favorite", { id }),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo cambiar el favorito"),
  });

  const duplicate = useMutation({
    mutationFn: (id: Id) => invoke<Recipe>("meals.recipes.duplicate", { id }),
    onSuccess: (r) => {
      toast.success("Receta duplicada", { description: r.name });
      invalidate();
    },
    onError: (e) => toastMealError(e, "No se pudo duplicar"),
  });

  const del = useMutation({
    mutationFn: (id: Id) => invoke("meals.recipes.delete", { id }),
    onSuccess: () => {
      toast.success("Receta eliminada");
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo eliminar"),
  });

  const r = recipe.data;
  const base = r?.servings ?? 1;
  const servings = target ?? base;

  return (
    <Dialog open={!!recipeId} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl">
        {recipe.isLoading || !r ? (
          <div className="flex items-center justify-center py-16 text-muted-foreground">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : (
          <>
            <div className="-m-6 mb-0 h-52 overflow-hidden rounded-t-xl sm:h-64">
              <RecipeImage fileId={r.image_file_id} alt={r.name} iconClassName="h-14 w-14" />
            </div>
            <DialogHeader className="mt-4">
              <div className="flex items-start justify-between gap-3 pr-6">
                <DialogTitle className="text-lg">{r.name}</DialogTitle>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  onClick={() => fav.mutate(r.id)}
                  title={r.favorite ? "Quitar de favoritas" : "Marcar como favorita"}
                >
                  <Heart className={cn("h-4 w-4", r.favorite ? "fill-destructive text-destructive" : "text-muted-foreground")} />
                </Button>
              </div>
              {r.description && <DialogDescription className="text-sm">{r.description}</DialogDescription>}
              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                <Badge variant={DIFFICULTY_VARIANT[r.difficulty]}>{DIFFICULTY_LABEL[r.difficulty]}</Badge>
                <Badge variant="outline"><Clock /> {fmtMinutes(r.prep_time_min + r.cook_time_min)}</Badge>
                <Badge variant="outline"><Users /> {r.servings} raciones base</Badge>
                {r.categories.map((c) => <Badge key={c} variant="secondary">{c}</Badge>)}
                {r.tags.map((t) => <Badge key={t} variant="outline">#{t}</Badge>)}
              </div>
            </DialogHeader>

            <div className="flex flex-wrap items-center gap-2">
              <Button size="sm" variant="outline" onClick={() => onEdit(r)}><Pencil /> Editar</Button>
              <Button size="sm" variant="outline" onClick={() => duplicate.mutate(r.id)} disabled={duplicate.isPending}>
                <Copy /> Duplicar
              </Button>
              <Button size="sm" variant="outline" onClick={() => setPlanOpen(true)}><CalendarPlus /> Añadir al plan</Button>
              {onDuplicateProposal && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    onDuplicateProposal({
                      name: `${r.name} (copia)`,
                      description: r.description,
                      servings: r.servings,
                      prep_time_min: r.prep_time_min,
                      cook_time_min: r.cook_time_min,
                      difficulty: r.difficulty,
                      categories: r.categories,
                      tags: r.tags,
                      ingredients: r.ingredients,
                      steps: r.steps,
                      nutrition: r.nutrition,
                    })
                  }
                >
                  <Sparkles /> Editar como nueva
                </Button>
              )}
              <Button
                size="sm"
                variant="ghost"
                className="text-destructive hover:text-destructive"
                onClick={() => {
                  if (window.confirm(`¿Eliminar la receta «${r.name}»? Las comidas planificadas con ella también se eliminarán.`)) {
                    del.mutate(r.id);
                  }
                }}
                disabled={del.isPending}
              >
                <Trash2 /> Eliminar
              </Button>
            </div>

            <div className="grid grid-cols-1 gap-5 sm:grid-cols-[1fr_1.2fr]">
              <div>
                <div className="mb-2 flex items-center justify-between gap-2">
                  <h3 className="text-sm font-semibold">Ingredientes</h3>
                  <div className="flex items-center gap-1.5">
                    <Button size="icon-sm" variant="outline" onClick={() => setTarget(Math.max(1, servings - 1))} title="Menos raciones">
                      <Minus />
                    </Button>
                    <Select value={String(servings)} onValueChange={(v) => setTarget(Number(v))}>
                      <SelectTrigger className="h-7 w-[104px] text-xs"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {Array.from({ length: 24 }, (_, i) => i + 1).map((n) => (
                          <SelectItem key={n} value={String(n)}>{n} {n === 1 ? "ración" : "raciones"}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <Button size="icon-sm" variant="outline" onClick={() => setTarget(Math.min(24, servings + 1))} title="Más raciones">
                      <Plus />
                    </Button>
                  </div>
                </div>
                {servings !== base && (
                  <p className="mb-1 text-[11px] text-muted-foreground">
                    Cantidades escaladas de {base} a {servings} raciones (solo vista; las unidades no se convierten).
                  </p>
                )}
                <ul>
                  {r.ingredients.map((ing, i) => (
                    <li key={i} className="group relative">
                      <ScaledIngredientRow ing={ing} base={base} target={servings} />
                      <button
                        type="button"
                        onClick={() => setSubstituteIng(ing.name)}
                        title="Sugerir sustitución con IA"
                        className="absolute right-0 top-1/2 hidden -translate-y-1/2 rounded bg-card px-1 text-muted-foreground shadow-sm group-hover:block hover:text-primary cursor-pointer"
                      >
                        <ArrowLeftRight className="h-3.5 w-3.5" />
                      </button>
                    </li>
                  ))}
                </ul>
                {r.ingredients.length === 0 && (
                  <p className="text-xs text-muted-foreground">Esta receta no tiene ingredientes registrados.</p>
                )}
              </div>

              <div>
                <h3 className="mb-2 text-sm font-semibold">Preparación</h3>
                <div className="mb-3 flex gap-3 text-[11px] text-muted-foreground">
                  <span className="flex items-center gap-1"><ChefHat className="h-3.5 w-3.5" /> Preparación: {fmtMinutes(r.prep_time_min)}</span>
                  <span className="flex items-center gap-1"><Clock className="h-3.5 w-3.5" /> Cocción: {fmtMinutes(r.cook_time_min)}</span>
                </div>
                <ol className="space-y-2.5">
                  {r.steps.map((step, i) => (
                    <li key={i} className="flex gap-2.5">
                      <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/15 text-[11px] font-semibold text-primary">
                        {i + 1}
                      </span>
                      <p className="text-sm leading-snug">{step}</p>
                    </li>
                  ))}
                </ol>
                {r.steps.length === 0 && <p className="text-xs text-muted-foreground">Sin pasos registrados.</p>}
              </div>
            </div>

            {r.nutrition && (
              <>
                <Separator />
                <div>
                  <h3 className="mb-2 text-sm font-semibold">Nutrición por ración <span className="text-[11px] font-normal text-muted-foreground">(introducida por ti)</span></h3>
                  <div className="grid grid-cols-4 gap-2">
                    {([
                      [r.nutrition.kcal, "kcal"],
                      [r.nutrition.protein_g, "proteínas (g)"],
                      [r.nutrition.carbs_g, "carbohidratos (g)"],
                      [r.nutrition.fat_g, "grasas (g)"],
                    ] as const).map(([v, label]) => (
                      <div key={label} className="rounded-lg border border-border bg-card p-2 text-center">
                        <p className="text-sm font-semibold tabular-nums">{fmtQty(v)}</p>
                        <p className="text-[10px] text-muted-foreground">{label}</p>
                      </div>
                    ))}
                  </div>
                </div>
              </>
            )}

            <AddToPlanDialog recipe={r} open={planOpen} onClose={() => setPlanOpen(false)} />
            <SubstituteDialog recipe={r} ingredient={substituteIng} onClose={() => setSubstituteIng(null)} />
          </>
        )}
        {recipe.isError && (
          <p className="py-8 text-center text-sm text-destructive">
            {recipe.error instanceof ApiError ? recipe.error.message : "No se pudo cargar la receta"}
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
