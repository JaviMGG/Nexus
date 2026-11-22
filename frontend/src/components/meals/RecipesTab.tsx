import { useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { Clock, CookingPot, Copy, Heart, Plus, Search, Sparkles, Users } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  CreateRecipe,
  DIFFICULTY_LABEL,
  DIFFICULTY_VARIANT,
  Recipe,
  RecipeDifficulty,
  RecipeFilter,
  RecipeImage,
  fmtMinutes,
  toastMealError,
  useInvalidateMeals,
} from "./shared";
import { RecipeDialog } from "./RecipeDialog";
import { RecipeDetailDialog } from "./RecipeDetailDialog";
import { AiRecipeDialog } from "./AiRecipeDialog";

const ALL = "__all__";

function RecipeCard({
  recipe,
  onOpen,
  onToggleFavorite,
  onDuplicate,
}: {
  recipe: Recipe;
  onOpen: () => void;
  onToggleFavorite: () => void;
  onDuplicate: () => void;
}) {
  return (
    <div
      className="group cursor-pointer overflow-hidden rounded-xl border border-border bg-card shadow-sm transition-all hover:border-ring/50 hover:shadow-md animate-fade-in"
      onClick={onOpen}
    >
      <div className="relative h-36">
        <RecipeImage fileId={recipe.image_file_id} alt={recipe.name} iconClassName="h-10 w-10" />
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onToggleFavorite();
          }}
          title={recipe.favorite ? "Quitar de favoritas" : "Marcar como favorita"}
          className="absolute right-2 top-2 flex h-8 w-8 items-center justify-center rounded-full bg-black/35 backdrop-blur-sm transition-transform hover:scale-110 cursor-pointer"
        >
          <Heart className={cn("h-4 w-4", recipe.favorite ? "fill-destructive text-destructive" : "text-white")} />
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onDuplicate();
          }}
          title="Duplicar receta"
          className="absolute right-2 top-12 flex h-8 w-8 items-center justify-center rounded-full bg-black/35 opacity-0 backdrop-blur-sm transition-opacity hover:scale-110 group-hover:opacity-100 cursor-pointer"
        >
          <Copy className="h-3.5 w-3.5 text-white" />
        </button>
      </div>
      <div className="space-y-2 p-3">
        <p className="truncate text-sm font-semibold leading-snug">{recipe.name}</p>
        <div className="flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
          <Badge variant={DIFFICULTY_VARIANT[recipe.difficulty]}>{DIFFICULTY_LABEL[recipe.difficulty]}</Badge>
          <span className="flex items-center gap-1"><Clock className="h-3 w-3" />{fmtMinutes(recipe.prep_time_min + recipe.cook_time_min)}</span>
          <span className="flex items-center gap-1"><Users className="h-3 w-3" />{recipe.servings}</span>
        </div>
        {recipe.categories.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {recipe.categories.slice(0, 3).map((c) => (
              <Badge key={c} variant="secondary" className="px-1.5 py-0 text-[10px]">{c}</Badge>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export function RecipesTab() {
  const invalidate = useInvalidateMeals();
  const [search, setSearch] = useState("");
  const debounced = useDebouncedValue(search, 300);
  const [category, setCategory] = useState(ALL);
  const [difficulty, setDifficulty] = useState(ALL);
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [maxTime, setMaxTime] = useState(ALL);
  const [sort, setSort] = useState<"name" | "time" | "recent">("recent");

  const [detailId, setDetailId] = useState<Id | null>(null);
  const [editor, setEditor] = useState<{ open: boolean; recipe: Recipe | null; proposal: CreateRecipe | null }>({
    open: false,
    recipe: null,
    proposal: null,
  });
  const [aiOpen, setAiOpen] = useState(false);

  const filter = useMemo<RecipeFilter>(() => {
    const f: RecipeFilter = { sort };
    if (debounced.trim()) f.search = debounced.trim();
    if (category !== ALL) f.category = category;
    if (difficulty !== ALL) f.difficulty = difficulty as RecipeDifficulty;
    if (favoritesOnly) f.favorites_only = true;
    if (maxTime !== ALL) f.max_time_min = Number(maxTime);
    return f;
  }, [debounced, category, difficulty, favoritesOnly, maxTime, sort]);

  const recipes = useQuery({
    queryKey: ["meals", "recipes", filter],
    queryFn: () => invoke<Recipe[]>("meals.recipes.list", { filter }),
  });

  const allRecipes = useQuery({
    queryKey: ["meals", "recipes", "all-for-categories"],
    queryFn: () => invoke<Recipe[]>("meals.recipes.list", {}),
  });

  const categories = useMemo(() => {
    const set = new Set<string>();
    (allRecipes.data ?? []).forEach((r) => r.categories.forEach((c) => set.add(c)));
    return [...set].sort();
  }, [allRecipes.data]);

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

  const hasFilters = debounced.trim() !== "" || category !== ALL || difficulty !== ALL || favoritesOnly || maxTime !== ALL;
  const list = recipes.data ?? [];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-52 flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Buscar por nombre o ingrediente…"
            className="pl-8"
          />
        </div>
        <Button variant="outline" onClick={() => setAiOpen(true)}>
          <Sparkles /> Receta con IA
        </Button>
        <Button onClick={() => setEditor({ open: true, recipe: null, proposal: null })}>
          <Plus /> Nueva receta
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Select value={category} onValueChange={setCategory}>
          <SelectTrigger className="h-8 w-[150px] text-xs"><SelectValue placeholder="Categoría" /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Todas las categorías</SelectItem>
            {categories.map((c) => <SelectItem key={c} value={c}>{c}</SelectItem>)}
          </SelectContent>
        </Select>
        <Select value={difficulty} onValueChange={setDifficulty}>
          <SelectTrigger className="h-8 w-[130px] text-xs"><SelectValue placeholder="Dificultad" /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Toda dificultad</SelectItem>
            {(Object.keys(DIFFICULTY_LABEL) as RecipeDifficulty[]).map((d) => (
              <SelectItem key={d} value={d}>{DIFFICULTY_LABEL[d]}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={maxTime} onValueChange={setMaxTime}>
          <SelectTrigger className="h-8 w-[140px] text-xs"><SelectValue placeholder="Tiempo máx." /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Sin límite de tiempo</SelectItem>
            <SelectItem value="15">Hasta 15 min</SelectItem>
            <SelectItem value="30">Hasta 30 min</SelectItem>
            <SelectItem value="60">Hasta 1 h</SelectItem>
            <SelectItem value="120">Hasta 2 h</SelectItem>
          </SelectContent>
        </Select>
        <Select value={sort} onValueChange={(v) => setSort(v as typeof sort)}>
          <SelectTrigger className="h-8 w-[130px] text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="recent">Más recientes</SelectItem>
            <SelectItem value="name">Por nombre</SelectItem>
            <SelectItem value="time">Por tiempo</SelectItem>
          </SelectContent>
        </Select>
        <Button
          size="sm"
          variant={favoritesOnly ? "default" : "outline"}
          className="h-8"
          onClick={() => setFavoritesOnly((v) => !v)}
        >
          <Heart className={cn(favoritesOnly && "fill-current")} /> Favoritas
        </Button>
      </div>

      {recipes.isLoading ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-56 rounded-xl" />
          ))}
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          icon={CookingPot}
          title={hasFilters ? "Sin resultados" : "Tu recetario está vacío"}
          description={
            hasFilters
              ? "Prueba a ajustar la búsqueda o los filtros."
              : "Crea tu primera receta o pide una propuesta a la IA (opcional)."
          }
          action={
            !hasFilters ? (
              <Button onClick={() => setEditor({ open: true, recipe: null, proposal: null })}>
                <Plus /> Nueva receta
              </Button>
            ) : undefined
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {list.map((r) => (
            <RecipeCard
              key={r.id}
              recipe={r}
              onOpen={() => setDetailId(r.id)}
              onToggleFavorite={() => fav.mutate(r.id)}
              onDuplicate={() => duplicate.mutate(r.id)}
            />
          ))}
        </div>
      )}

      <RecipeDetailDialog
        recipeId={detailId}
        onClose={() => setDetailId(null)}
        onEdit={(r) => {
          setDetailId(null);
          setEditor({ open: true, recipe: r, proposal: null });
        }}
      />
      <RecipeDialog
        open={editor.open}
        recipe={editor.recipe}
        proposal={editor.proposal}
        onClose={() => setEditor({ open: false, recipe: null, proposal: null })}
        onSaved={(r) => {
          toast.success(editor.recipe ? "Receta actualizada" : "Receta creada", { description: r.name });
        }}
      />
      <AiRecipeDialog
        open={aiOpen}
        onClose={() => setAiOpen(false)}
        onProposal={(data) => setEditor({ open: true, recipe: null, proposal: data })}
      />
    </div>
  );
}
