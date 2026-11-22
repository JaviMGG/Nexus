import { useEffect, useRef, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { ImagePlus, Loader2, Plus, Sparkles, Trash2, X } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import {
  CreateRecipe,
  DIFFICULTY_LABEL,
  INGREDIENT_CATEGORIES,
  Ingredient,
  Nutrition,
  Recipe,
  RecipeDifficulty,
  RecipeImage,
  UNIT_SUGGESTIONS,
  capitalize,
  toastMealError,
  uploadRecipeImage,
  useInvalidateMeals,
} from "./shared";

interface FormState {
  name: string;
  description: string;
  image_file_id: Id | null;
  servings: string;
  prep_time_min: string;
  cook_time_min: string;
  difficulty: RecipeDifficulty;
  categories: string;
  tags: string;
  ingredients: Ingredient[];
  steps: string[];
  nutritionEnabled: boolean;
  nutrition: { kcal: string; protein_g: string; carbs_g: string; fat_g: string };
}

function emptyIngredient(): Ingredient {
  return { name: "", quantity: 1, unit: "ud", category: "otros", notes: "" };
}

function fromRecipe(r: Recipe | CreateRecipe | null): FormState {
  const n = r && "nutrition" in r ? r.nutrition : null;
  return {
    name: r?.name ?? "",
    description: r?.description ?? "",
    image_file_id: r?.image_file_id ?? null,
    servings: String(r?.servings ?? 2),
    prep_time_min: String(r?.prep_time_min ?? 0),
    cook_time_min: String(r?.cook_time_min ?? 0),
    difficulty: r?.difficulty ?? "facil",
    categories: (r?.categories ?? []).join(", "),
    tags: (r?.tags ?? []).join(", "),
    ingredients: r?.ingredients?.length ? r.ingredients.map((i) => ({ ...i })) : [emptyIngredient()],
    steps: r?.steps?.length ? [...r.steps] : [""],
    nutritionEnabled: !!n,
    nutrition: {
      kcal: n ? String(n.kcal) : "",
      protein_g: n ? String(n.protein_g) : "",
      carbs_g: n ? String(n.carbs_g) : "",
      fat_g: n ? String(n.fat_g) : "",
    },
  };
}

function toPayload(f: FormState): CreateRecipe {
  const nutrition: Nutrition | null = f.nutritionEnabled
    ? {
        kcal: Number(f.nutrition.kcal) || 0,
        protein_g: Number(f.nutrition.protein_g) || 0,
        carbs_g: Number(f.nutrition.carbs_g) || 0,
        fat_g: Number(f.nutrition.fat_g) || 0,
      }
    : null;
  return {
    name: f.name.trim(),
    description: f.description.trim(),
    image_file_id: f.image_file_id,
    servings: Number(f.servings) || 1,
    prep_time_min: Number(f.prep_time_min) || 0,
    cook_time_min: Number(f.cook_time_min) || 0,
    difficulty: f.difficulty,
    categories: f.categories.split(",").map((s) => s.trim()).filter(Boolean),
    tags: f.tags.split(",").map((s) => s.trim()).filter(Boolean),
    ingredients: f.ingredients
      .filter((i) => i.name.trim())
      .map((i) => ({ ...i, name: i.name.trim(), quantity: Number(i.quantity) || 0 })),
    steps: f.steps.map((s) => s.trim()).filter(Boolean),
    nutrition,
  };
}

export function RecipeDialog({
  open,
  recipe,
  proposal,
  onClose,
  onSaved,
}: {
  open: boolean;
  recipe: Recipe | null;
  proposal?: CreateRecipe | null;
  onClose: () => void;
  onSaved?: (r: Recipe) => void;
}) {
  const invalidate = useInvalidateMeals();
  const [form, setForm] = useState<FormState>(() => fromRecipe(recipe ?? proposal ?? null));
  const [uploading, setUploading] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) setForm(fromRecipe(recipe ?? proposal ?? null));
  }, [open, recipe, proposal]);

  const set = <K extends keyof FormState>(key: K, value: FormState[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const setIngredient = (idx: number, patch: Partial<Ingredient>) =>
    setForm((f) => ({
      ...f,
      ingredients: f.ingredients.map((i, n) => (n === idx ? { ...i, ...patch } : i)),
    }));

  const save = useMutation({
    mutationFn: () => {
      const data = toPayload(form);
      return recipe
        ? invoke<Recipe>("meals.recipes.update", { id: recipe.id, data })
        : invoke<Recipe>("meals.recipes.create", { data });
    },
    onSuccess: (r) => {
      invalidate();
      onSaved?.(r);
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo guardar la receta"),
  });

  const onPickImage = async (file: File | undefined) => {
    if (!file) return;
    setUploading(true);
    try {
      const id = await uploadRecipeImage(file);
      set("image_file_id", id);
    } catch (e) {
      toastMealError(e, "No se pudo subir la imagen");
    } finally {
      setUploading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{recipe ? "Editar receta" : "Nueva receta"}</DialogTitle>
          <DialogDescription>
            {proposal && !recipe
              ? "Propuesta de IA: revisa y edita todo antes de guardar. Nada se guarda sin tu confirmación."
              : "Los campos de nutrición son opcionales: rellénalos solo si conoces los valores reales."}
          </DialogDescription>
        </DialogHeader>

        {proposal && !recipe && (
          <div className="flex items-center gap-2 rounded-lg border border-warning/40 bg-warning/10 px-3 py-2 text-xs text-warning">
            <Sparkles className="h-4 w-4 shrink-0" />
            Borrador generado por IA — editable. La nutrición nunca se estima automáticamente.
          </div>
        )}

        <div className="space-y-4">
          <div className="flex gap-4">
            <button
              type="button"
              onClick={() => fileRef.current?.click()}
              className="group relative h-28 w-40 shrink-0 overflow-hidden rounded-lg border border-border cursor-pointer"
              title="Cambiar imagen"
            >
              <RecipeImage fileId={form.image_file_id} alt="Imagen de la receta" />
              <span className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
                {uploading ? <Loader2 className="h-5 w-5 animate-spin text-white" /> : <ImagePlus className="h-5 w-5 text-white" />}
              </span>
            </button>
            <input
              ref={fileRef}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={(e) => void onPickImage(e.target.files?.[0])}
            />
            <div className="flex-1 space-y-3">
              <div className="space-y-1.5">
                <Label>Nombre *</Label>
                <Input value={form.name} onChange={(e) => set("name", e.target.value)} placeholder="Tortilla de patatas" />
              </div>
              <div className="space-y-1.5">
                <Label>Descripción</Label>
                <Textarea
                  value={form.description}
                  onChange={(e) => set("description", e.target.value)}
                  placeholder="Breve descripción de la receta"
                  className="min-h-[52px]"
                />
              </div>
              {form.image_file_id && (
                <Button size="sm" variant="ghost" className="h-6 text-xs text-muted-foreground" onClick={() => set("image_file_id", null)}>
                  <X /> Quitar imagen
                </Button>
              )}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <div className="space-y-1.5">
              <Label>Raciones *</Label>
              <Input type="number" min={1} max={99} value={form.servings} onChange={(e) => set("servings", e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Preparación (min)</Label>
              <Input type="number" min={0} value={form.prep_time_min} onChange={(e) => set("prep_time_min", e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Cocción (min)</Label>
              <Input type="number" min={0} value={form.cook_time_min} onChange={(e) => set("cook_time_min", e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Dificultad</Label>
              <Select value={form.difficulty} onValueChange={(v) => set("difficulty", v as RecipeDifficulty)}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {(Object.keys(DIFFICULTY_LABEL) as RecipeDifficulty[]).map((d) => (
                    <SelectItem key={d} value={d}>{DIFFICULTY_LABEL[d]}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label>Categorías (separadas por comas)</Label>
              <Input value={form.categories} onChange={(e) => set("categories", e.target.value)} placeholder="cena, española" />
            </div>
            <div className="space-y-1.5">
              <Label>Etiquetas (separadas por comas)</Label>
              <Input value={form.tags} onChange={(e) => set("tags", e.target.value)} placeholder="rápida, horno" />
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>Ingredientes (cantidades para {form.servings || "?"} raciones)</Label>
              <Button size="sm" variant="outline" onClick={() => set("ingredients", [...form.ingredients, emptyIngredient()])}>
                <Plus /> Añadir
              </Button>
            </div>
            <datalist id="meal-units">
              {UNIT_SUGGESTIONS.map((u) => <option key={u} value={u} />)}
            </datalist>
            <div className="space-y-2">
              {form.ingredients.map((ing, idx) => (
                <div key={idx} className="grid grid-cols-[1fr_72px_72px_110px_32px] items-center gap-1.5">
                  <Input value={ing.name} placeholder="Ingrediente" onChange={(e) => setIngredient(idx, { name: e.target.value })} />
                  <Input
                    type="number"
                    min={0}
                    step="any"
                    value={ing.quantity}
                    onChange={(e) => setIngredient(idx, { quantity: Number(e.target.value) })}
                    title="Cantidad"
                  />
                  <Input list="meal-units" value={ing.unit} placeholder="ud" onChange={(e) => setIngredient(idx, { unit: e.target.value })} title="Unidad" />
                  <Select value={ing.category} onValueChange={(v) => setIngredient(idx, { category: v })}>
                    <SelectTrigger className="h-9"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      {INGREDIENT_CATEGORIES.map((c) => (
                        <SelectItem key={c} value={c}>{capitalize(c)}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    className="text-muted-foreground hover:text-destructive"
                    onClick={() => set("ingredients", form.ingredients.filter((_, n) => n !== idx))}
                    title="Quitar ingrediente"
                  >
                    <Trash2 />
                  </Button>
                </div>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>Pasos (en orden)</Label>
              <Button size="sm" variant="outline" onClick={() => set("steps", [...form.steps, ""])}>
                <Plus /> Añadir paso
              </Button>
            </div>
            <div className="space-y-2">
              {form.steps.map((step, idx) => (
                <div key={idx} className="flex items-start gap-2">
                  <span className="mt-2 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/15 text-[11px] font-semibold text-primary">
                    {idx + 1}
                  </span>
                  <Textarea
                    value={step}
                    onChange={(e) => set("steps", form.steps.map((s, n) => (n === idx ? e.target.value : s)))}
                    placeholder={`Paso ${idx + 1}`}
                    className="min-h-[44px] flex-1"
                  />
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    className="mt-1 text-muted-foreground hover:text-destructive"
                    onClick={() => set("steps", form.steps.filter((_, n) => n !== idx))}
                    title="Quitar paso"
                  >
                    <Trash2 />
                  </Button>
                </div>
              ))}
            </div>
          </div>

          <div className="rounded-lg border border-border p-3">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs font-medium">Nutrición por ración (opcional)</p>
                <p className="text-[11px] text-muted-foreground">
                  Introduce solo valores que conozcas. NEXUS nunca los inventa ni los estima.
                </p>
              </div>
              <Button
                size="sm"
                variant={form.nutritionEnabled ? "secondary" : "outline"}
                onClick={() => set("nutritionEnabled", !form.nutritionEnabled)}
              >
                {form.nutritionEnabled ? "Quitar" : "Añadir"}
              </Button>
            </div>
            {form.nutritionEnabled && (
              <div className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-4">
                {([
                  ["kcal", "Kcal"],
                  ["protein_g", "Proteínas (g)"],
                  ["carbs_g", "Carbohidratos (g)"],
                  ["fat_g", "Grasas (g)"],
                ] as const).map(([key, label]) => (
                  <div key={key} className="space-y-1.5">
                    <Label>{label}</Label>
                    <Input
                      type="number"
                      min={0}
                      step="any"
                      value={form.nutrition[key]}
                      onChange={(e) => set("nutrition", { ...form.nutrition, [key]: e.target.value })}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>

          {form.categories.trim() && (
            <div className="flex flex-wrap gap-1.5">
              {form.categories.split(",").map((c) => c.trim()).filter(Boolean).map((c) => (
                <Badge key={c} variant="secondary">{c}</Badge>
              ))}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => save.mutate()} disabled={save.isPending || !form.name.trim()}>
            {save.isPending && <Loader2 className="animate-spin" />}
            {recipe ? "Guardar cambios" : "Crear receta"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
