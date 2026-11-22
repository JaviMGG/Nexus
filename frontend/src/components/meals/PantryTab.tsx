import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  AlertTriangle,
  ChefHat,
  Minus,
  Package,
  Pencil,
  Plus,
  Search,
  Snowflake,
  Trash2,
  UtensilsCrossed,
} from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn, formatDate } from "@/lib/utils";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  CreatePantryItem,
  INGREDIENT_CATEGORIES,
  LOCATIONS,
  LOCATION_LABEL,
  PantryFilter,
  PantryItem,
  PantryLocation,
  PantryMatch,
  RecipeImage,
  UNIT_SUGGESTIONS,
  capitalize,
  expiryState,
  fmtQty,
  toDateStr,
  toastMealError,
  useInvalidateMeals,
} from "./shared";
import { RecipeDetailDialog } from "./RecipeDetailDialog";

const ALL = "__all__";

// ---------------------------------------------------------------------------
// Diálogo crear/editar producto de despensa
// ---------------------------------------------------------------------------

function PantryItemDialog({
  open,
  item,
  onClose,
}: {
  open: boolean;
  item: PantryItem | null;
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [name, setName] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [unit, setUnit] = useState("ud");
  const [category, setCategory] = useState("otros");
  const [location, setLocation] = useState<PantryLocation>("despensa");
  const [expiresAt, setExpiresAt] = useState("");

  useEffect(() => {
    if (open) {
      setName(item?.name ?? "");
      setQuantity(String(item?.quantity ?? 1));
      setUnit(item?.unit ?? "ud");
      setCategory(item?.category ?? "otros");
      setLocation(item?.location ?? "despensa");
      setExpiresAt(item?.expires_at ?? "");
    }
  }, [open, item]);

  const save = useMutation({
    mutationFn: () => {
      const data: CreatePantryItem = {
        name: name.trim(),
        quantity: Number(quantity) || 0,
        unit: unit.trim(),
        category,
        location,
        expires_at: expiresAt || null,
      };
      return item
        ? invoke("meals.pantry.update", { id: item.id, data })
        : invoke("meals.pantry.create", { data });
    },
    onSuccess: () => {
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo guardar el producto"),
  });

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{item ? "Editar producto" : "Añadir a la despensa"}</DialogTitle>
          <DialogDescription>
            La fecha de caducidad es opcional y solo la introduces tú: NEXUS nunca la inventa.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Producto *</Label>
            <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="Arroz, leche, huevos…" />
          </div>
          <div className="grid grid-cols-3 gap-2">
            <div className="space-y-1.5">
              <Label>Cantidad</Label>
              <Input type="number" min={0} step="any" value={quantity} onChange={(e) => setQuantity(e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Unidad</Label>
              <Input list="meal-units-pantry" value={unit} onChange={(e) => setUnit(e.target.value)} />
              <datalist id="meal-units-pantry">
                {UNIT_SUGGESTIONS.map((u) => <option key={u} value={u} />)}
              </datalist>
            </div>
            <div className="space-y-1.5">
              <Label>Ubicación</Label>
              <Select value={location} onValueChange={(v) => setLocation(v as PantryLocation)}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {LOCATIONS.map((l) => <SelectItem key={l} value={l}>{LOCATION_LABEL[l]}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <Label>Categoría</Label>
              <Select value={category} onValueChange={setCategory}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {INGREDIENT_CATEGORIES.map((c) => <SelectItem key={c} value={c}>{capitalize(c)}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1.5">
              <Label>Caduca el (opcional)</Label>
              <Input type="date" value={expiresAt} onChange={(e) => setExpiresAt(e.target.value)} />
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => save.mutate()} disabled={!name.trim() || save.isPending}>
            {item ? "Guardar cambios" : "Añadir"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Diálogo registrar consumo
// ---------------------------------------------------------------------------

function ConsumeDialog({ item, onClose }: { item: PantryItem | null; onClose: () => void }) {
  const invalidate = useInvalidateMeals();
  const [amount, setAmount] = useState("1");
  const [removeIfEmpty, setRemoveIfEmpty] = useState(true);

  useEffect(() => {
    if (item) {
      setAmount("1");
      setRemoveIfEmpty(true);
    }
  }, [item]);

  const consume = useMutation({
    mutationFn: () =>
      invoke<{ deleted: boolean }>("meals.pantry.consume", {
        id: item?.id,
        amount: Number(amount) || 0,
        remove_if_empty: removeIfEmpty,
      }),
    onSuccess: (r) => {
      toast.success(r.deleted ? "Producto consumido y agotado" : "Consumo registrado");
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo registrar el consumo"),
  });

  return (
    <Dialog open={!!item} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Consumir «{item?.name}»</DialogTitle>
          <DialogDescription>
            Tienes {item ? `${fmtQty(item.quantity)} ${item.unit}` : ""}. Se restará la cantidad indicada.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Cantidad consumida ({item?.unit})</Label>
            <Input autoFocus type="number" min={0} step="any" value={amount} onChange={(e) => setAmount(e.target.value)} />
          </div>
          <label className="flex items-center gap-2 text-xs">
            <input type="checkbox" checked={removeIfEmpty} onChange={(e) => setRemoveIfEmpty(e.target.checked)} className="h-4 w-4" />
            Eliminar el producto si la cantidad llega a 0
          </label>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => consume.mutate()} disabled={consume.isPending || !(Number(amount) > 0)}>
            Registrar consumo
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// ¿Qué puedo cocinar?
// ---------------------------------------------------------------------------

function CookMatches({ onOpenRecipe }: { onOpenRecipe: (id: Id) => void }) {
  const matches = useQuery({
    queryKey: ["meals", "pantry-matches"],
    queryFn: () => invoke<PantryMatch[]>("meals.pantry_matches"),
  });

  if (matches.isLoading) {
    return (
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {Array.from({ length: 3 }).map((_, i) => <Skeleton key={i} className="h-40 rounded-xl" />)}
      </div>
    );
  }
  const list = matches.data ?? [];
  if (list.length === 0) {
    return (
      <p className="rounded-lg border border-dashed border-border px-4 py-5 text-xs text-muted-foreground">
        Crea recetas con ingredientes para ver qué puedes cocinar con tu despensa.
      </p>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {list.map((mt) => (
        <button
          key={mt.recipe_id}
          type="button"
          onClick={() => onOpenRecipe(mt.recipe_id)}
          className="overflow-hidden rounded-xl border border-border bg-card text-left shadow-sm transition-all hover:border-ring/50 hover:shadow-md cursor-pointer"
        >
          <div className="flex items-center gap-3 p-3">
            <div className="h-14 w-18 shrink-0 overflow-hidden rounded-lg" style={{ width: 72 }}>
              <RecipeImage fileId={mt.image_file_id} alt={mt.recipe_name} iconClassName="h-5 w-5" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-semibold">{mt.recipe_name}</p>
              <div className="mt-1.5 flex items-center gap-2">
                <Progress value={mt.coverage_pct} className="h-1.5 flex-1" />
                <span
                  className={cn(
                    "text-xs font-semibold tabular-nums",
                    mt.coverage_pct >= 100 ? "text-success" : mt.coverage_pct >= 50 ? "text-warning" : "text-muted-foreground",
                  )}
                >
                  {fmtQty(mt.coverage_pct)}%
                </span>
              </div>
            </div>
          </div>
          <div className="border-t border-border/60 px-3 py-2">
            {mt.missing.length === 0 ? (
              <p className="flex items-center gap-1.5 text-xs text-success">
                <ChefHat className="h-3.5 w-3.5" /> ¡Puedes cocinarla ahora mismo!
              </p>
            ) : (
              <p className="text-xs text-muted-foreground">
                <span className="font-medium text-warning">Te falta: </span>
                {mt.missing.map((x) => `${x.name} (${fmtQty(Math.max(0, x.needed - x.have))} ${x.unit})`).join(", ")}
              </p>
            )}
          </div>
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pestaña Despensa
// ---------------------------------------------------------------------------

export function PantryTab() {
  const invalidate = useInvalidateMeals();
  const [search, setSearch] = useState("");
  const debounced = useDebouncedValue(search, 300);
  const [location, setLocation] = useState(ALL);
  const [category, setCategory] = useState(ALL);
  const [editor, setEditor] = useState<{ open: boolean; item: PantryItem | null }>({ open: false, item: null });
  const [consumeItem, setConsumeItem] = useState<PantryItem | null>(null);
  const [detailId, setDetailId] = useState<Id | null>(null);

  const filter = useMemo<PantryFilter>(() => {
    const f: PantryFilter = {};
    if (debounced.trim()) f.search = debounced.trim();
    if (location !== ALL) f.location = location as PantryLocation;
    if (category !== ALL) f.category = category;
    return f;
  }, [debounced, location, category]);

  const pantry = useQuery({
    queryKey: ["meals", "pantry", filter],
    queryFn: () => invoke<PantryItem[]>("meals.pantry.list", { filter }),
  });

  const allItems = useQuery({
    queryKey: ["meals", "pantry", "all-for-categories"],
    queryFn: () => invoke<PantryItem[]>("meals.pantry.list", {}),
  });

  const categories = useMemo(() => {
    const set = new Set<string>();
    (allItems.data ?? []).forEach((i) => set.add(i.category));
    return [...set].sort();
  }, [allItems.data]);

  const adjust = useMutation({
    mutationFn: (v: { id: Id; delta: number }) => invoke("meals.pantry.adjust", v),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo ajustar la cantidad"),
  });

  const del = useMutation({
    mutationFn: (id: Id) => invoke("meals.pantry.delete", { id }),
    onSuccess: () => {
      toast.success("Producto eliminado");
      invalidate();
    },
    onError: (e) => toastMealError(e, "No se pudo eliminar"),
  });

  const today = toDateStr(new Date());
  const items = pantry.data ?? [];
  const hasFilters = debounced.trim() !== "" || location !== ALL || category !== ALL;

  const stepFor = (unit: string) => {
    const u = unit.toLowerCase();
    if (u === "g" || u === "ml") return 100;
    if (u === "kg" || u === "l") return 0.5;
    return 1;
  };

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-52 flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar en la despensa…" className="pl-8" />
        </div>
        <Select value={location} onValueChange={setLocation}>
          <SelectTrigger className="h-8 w-[140px] text-xs"><SelectValue placeholder="Ubicación" /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Toda ubicación</SelectItem>
            {LOCATIONS.map((l) => <SelectItem key={l} value={l}>{LOCATION_LABEL[l]}</SelectItem>)}
          </SelectContent>
        </Select>
        <Select value={category} onValueChange={setCategory}>
          <SelectTrigger className="h-8 w-[140px] text-xs"><SelectValue placeholder="Categoría" /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>Toda categoría</SelectItem>
            {categories.map((c) => <SelectItem key={c} value={c}>{capitalize(c)}</SelectItem>)}
          </SelectContent>
        </Select>
        <Button onClick={() => setEditor({ open: true, item: null })}><Plus /> Añadir producto</Button>
      </div>

      {pantry.isLoading ? (
        <Skeleton className="h-64 rounded-xl" />
      ) : items.length === 0 ? (
        <EmptyState
          icon={Package}
          title={hasFilters ? "Sin resultados" : "Tu despensa está vacía"}
          description={
            hasFilters
              ? "Prueba a ajustar la búsqueda o los filtros."
              : "Registra lo que tienes en casa y descubre qué recetas puedes cocinar."
          }
          action={!hasFilters ? <Button onClick={() => setEditor({ open: true, item: null })}><Plus /> Añadir producto</Button> : undefined}
        />
      ) : (
        <div className="overflow-x-auto rounded-xl border border-border bg-card shadow-sm">
          <table className="w-full min-w-[640px] text-sm">
            <thead>
              <tr className="border-b border-border text-left text-[11px] uppercase tracking-wide text-muted-foreground">
                <th className="px-4 py-2.5 font-medium">Producto</th>
                <th className="px-3 py-2.5 font-medium">Cantidad</th>
                <th className="hidden px-3 py-2.5 font-medium md:table-cell">Categoría</th>
                <th className="px-3 py-2.5 font-medium">Ubicación</th>
                <th className="px-3 py-2.5 font-medium">Caduca</th>
                <th className="px-3 py-2.5 text-right font-medium">Acciones</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => {
                const expiry = expiryState(item.expires_at, today);
                const step = stepFor(item.unit);
                return (
                  <tr key={item.id} className="group border-b border-border/60 transition-colors last:border-0 hover:bg-muted/60">
                    <td className="px-4 py-2.5 font-medium">{item.name}</td>
                    <td className="px-3 py-2.5">
                      <div className="flex items-center gap-1">
                        <Button
                          size="icon-sm"
                          variant="outline"
                          onClick={() => adjust.mutate({ id: item.id, delta: -step })}
                          title={`Restar ${fmtQty(step)} ${item.unit}`}
                        >
                          <Minus />
                        </Button>
                        <span className="w-20 text-center text-sm tabular-nums">
                          {fmtQty(item.quantity)} <span className="text-xs text-muted-foreground">{item.unit}</span>
                        </span>
                        <Button
                          size="icon-sm"
                          variant="outline"
                          onClick={() => adjust.mutate({ id: item.id, delta: step })}
                          title={`Sumar ${fmtQty(step)} ${item.unit}`}
                        >
                          <Plus />
                        </Button>
                      </div>
                    </td>
                    <td className="hidden px-3 py-2.5 text-xs text-muted-foreground md:table-cell">{capitalize(item.category)}</td>
                    <td className="px-3 py-2.5">
                      <Badge variant="secondary">
                        {item.location === "congelador" && <Snowflake />}
                        {LOCATION_LABEL[item.location]}
                      </Badge>
                    </td>
                    <td className="px-3 py-2.5">
                      {expiry === "expired" ? (
                        <Badge variant="destructive"><AlertTriangle /> Caducado · {formatDate(item.expires_at)}</Badge>
                      ) : expiry === "soon" ? (
                        <Badge variant="warning"><AlertTriangle /> {formatDate(item.expires_at)}</Badge>
                      ) : item.expires_at ? (
                        <span className="text-xs text-muted-foreground">{formatDate(item.expires_at)}</span>
                      ) : (
                        <span className="text-xs text-muted-foreground/60">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2.5">
                      <div className="flex items-center justify-end gap-1">
                        <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setConsumeItem(item)}>
                          <UtensilsCrossed /> Consumir
                        </Button>
                        <Button size="icon-sm" variant="ghost" className="text-muted-foreground" onClick={() => setEditor({ open: true, item })} title="Editar">
                          <Pencil />
                        </Button>
                        <Button
                          size="icon-sm"
                          variant="ghost"
                          className="text-muted-foreground hover:text-destructive"
                          onClick={() => {
                            if (window.confirm(`¿Eliminar «${item.name}» de la despensa?`)) del.mutate(item.id);
                          }}
                          title="Eliminar"
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      <section className="space-y-2.5">
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <ChefHat className="h-4 w-4 text-primary" /> ¿Qué puedo cocinar?
          <span className="text-[11px] font-normal text-muted-foreground">
            Cobertura con tu despensa actual (raciones base; lo caducado no cuenta)
          </span>
        </h3>
        <CookMatches onOpenRecipe={setDetailId} />
      </section>

      <PantryItemDialog open={editor.open} item={editor.item} onClose={() => setEditor({ open: false, item: null })} />
      <ConsumeDialog item={consumeItem} onClose={() => setConsumeItem(null)} />
      <RecipeDetailDialog recipeId={detailId} onClose={() => setDetailId(null)} onEdit={() => setDetailId(null)} />
    </div>
  );
}
