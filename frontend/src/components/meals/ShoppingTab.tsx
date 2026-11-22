import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  Check,
  ClipboardCopy,
  Download,
  History,
  ListChecks,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  ShoppingCart,
  Trash2,
} from "lucide-react";
import { invoke } from "@/lib/api";
import type { Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  CreateShoppingItem,
  INGREDIENT_CATEGORIES,
  ShoppingExport,
  ShoppingItem,
  ShoppingList,
  ShoppingListDetail,
  UNIT_SUGGESTIONS,
  WeekPlan,
  capitalize,
  fmtQty,
  mondayOf,
  toDateStr,
  toastMealError,
  useInvalidateMeals,
  weekLabel,
} from "./shared";

function download(filename: string, content: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

// ---------------------------------------------------------------------------
// Diálogo: generar lista desde una semana
// ---------------------------------------------------------------------------

function GenerateDialog({
  open,
  currentList,
  onClose,
  onGenerated,
}: {
  open: boolean;
  currentList: ShoppingListDetail | null;
  onClose: () => void;
  onGenerated: (id: Id) => void;
}) {
  const invalidate = useInvalidateMeals();
  const [weekStart, setWeekStart] = useState(() => mondayOf(toDateStr(new Date())));
  const [name, setName] = useState("");
  const [regenerate, setRegenerate] = useState(false);

  useEffect(() => {
    if (open) {
      setWeekStart(currentList?.list.week_start ?? mondayOf(toDateStr(new Date())));
      setName("");
      setRegenerate(false);
    }
  }, [open, currentList]);

  const preview = useQuery({
    queryKey: ["meals", "plan", weekStart],
    queryFn: () => invoke<WeekPlan>("meals.plan.get", { week_start: weekStart }),
    enabled: open,
  });

  const generate = useMutation({
    mutationFn: () =>
      invoke<ShoppingListDetail>("meals.shopping.generate", {
        week_start: weekStart,
        name: name.trim() || null,
        list_id: regenerate && currentList ? currentList.list.id : null,
      }),
    onSuccess: (d) => {
      toast.success(regenerate ? "Lista regenerada" : "Lista generada", {
        description: `${d.items.length} productos consolidados`,
      });
      invalidate();
      onGenerated(d.list.id);
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo generar la lista"),
  });

  const confirmAndGenerate = () => {
    if (regenerate && currentList?.list.has_manual_changes) {
      if (!window.confirm("La lista tiene cambios manuales. Al regenerar se rehacen los productos generados (los manuales se conservan). ¿Continuar?")) {
        return;
      }
    }
    generate.mutate();
  };

  const mealCount = preview.data?.entries.length ?? 0;

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Generar lista de la compra</DialogTitle>
          <DialogDescription>
            Consolida los ingredientes de todas las comidas de la semana, escalados por raciones.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Semana (cualquier día de esa semana)</Label>
            <Input type="date" value={weekStart} onChange={(e) => e.target.value && setWeekStart(mondayOf(e.target.value))} />
            <p className="text-[11px] text-muted-foreground">
              {weekLabel(weekStart)} · {preview.isLoading ? "…" : `${mealCount} comidas planificadas`}
            </p>
          </div>
          <div className="space-y-1.5">
            <Label>Nombre de la lista (opcional)</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder={`Compra · ${weekLabel(weekStart).toLowerCase()}`} />
          </div>
          {currentList && (
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                checked={regenerate}
                onChange={(e) => setRegenerate(e.target.checked)}
                className="h-4 w-4 accent-[var(--color-primary,#888)]"
              />
              Regenerar la lista actual «{currentList.list.name}» (conserva productos manuales)
            </label>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={confirmAndGenerate} disabled={generate.isPending || mealCount === 0}>
            {generate.isPending && <Loader2 className="animate-spin" />} Generar
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Diálogo: producto manual
// ---------------------------------------------------------------------------

function ManualItemDialog({
  listId,
  open,
  onClose,
}: {
  listId: Id | null;
  open: boolean;
  onClose: () => void;
}) {
  const invalidate = useInvalidateMeals();
  const [name, setName] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [unit, setUnit] = useState("ud");
  const [category, setCategory] = useState("otros");

  useEffect(() => {
    if (open) {
      setName("");
      setQuantity("1");
      setUnit("ud");
      setCategory("otros");
    }
  }, [open]);

  const add = useMutation({
    mutationFn: () =>
      invoke("meals.shopping.add_item", {
        list_id: listId,
        data: {
          name: name.trim(),
          quantity: Number(quantity) || 0,
          unit: unit.trim(),
          category,
        } satisfies CreateShoppingItem,
      }),
    onSuccess: () => {
      invalidate();
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo añadir el producto"),
  });

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Añadir producto manual</DialogTitle>
          <DialogDescription>Se añadirá a la lista actual y se conservará al regenerar.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Producto *</Label>
            <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="Pan, servilletas…" />
          </div>
          <div className="grid grid-cols-3 gap-2">
            <div className="space-y-1.5">
              <Label>Cantidad</Label>
              <Input type="number" min={0} step="any" value={quantity} onChange={(e) => setQuantity(e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Unidad</Label>
              <Input list="meal-units-shop" value={unit} onChange={(e) => setUnit(e.target.value)} />
              <datalist id="meal-units-shop">
                {UNIT_SUGGESTIONS.map((u) => <option key={u} value={u} />)}
              </datalist>
            </div>
            <div className="space-y-1.5">
              <Label>Categoría</Label>
              <Select value={category} onValueChange={setCategory}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {INGREDIENT_CATEGORIES.map((c) => <SelectItem key={c} value={c}>{capitalize(c)}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => add.mutate()} disabled={!name.trim() || add.isPending}>
            {add.isPending && <Loader2 className="animate-spin" />} Añadir
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Fila de producto
// ---------------------------------------------------------------------------

function ItemRow({ item }: { item: ShoppingItem }) {
  const invalidate = useInvalidateMeals();
  const [editing, setEditing] = useState(false);
  const [qty, setQty] = useState(String(item.quantity));

  const update = useMutation({
    mutationFn: (data: Record<string, unknown>) => invoke("meals.shopping.update_item", { id: item.id, data }),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo actualizar el producto"),
  });

  const del = useMutation({
    mutationFn: () => invoke("meals.shopping.delete_item", { id: item.id }),
    onSuccess: () => invalidate(),
    onError: (e) => toastMealError(e, "No se pudo eliminar el producto"),
  });

  const saveQty = () => {
    setEditing(false);
    const n = Number(qty);
    if (!Number.isFinite(n) || n < 0) {
      setQty(String(item.quantity));
      return;
    }
    if (n !== item.quantity) update.mutate({ quantity: n });
  };

  return (
    <li className="group flex items-center gap-3 border-b border-border/50 px-3 py-2 last:border-0">
      <button
        type="button"
        onClick={() => update.mutate({ checked: !item.checked })}
        title={item.checked ? "Marcar como pendiente" : "Marcar como comprado"}
        className={cn(
          "flex h-6 w-6 shrink-0 items-center justify-center rounded-md border-2 transition-colors cursor-pointer",
          item.checked ? "border-success bg-success text-white" : "border-border hover:border-ring",
        )}
      >
        {item.checked && <Check className="h-4 w-4" />}
      </button>
      <span className={cn("min-w-0 flex-1 truncate text-sm", item.checked && "text-muted-foreground line-through")}>
        {item.name}
        {item.manual && <Badge variant="outline" className="ml-2 px-1.5 py-0 text-[9px]">manual</Badge>}
      </span>
      {editing ? (
        <Input
          autoFocus
          type="number"
          min={0}
          step="any"
          value={qty}
          onChange={(e) => setQty(e.target.value)}
          onBlur={saveQty}
          onKeyDown={(e) => {
            if (e.key === "Enter") saveQty();
            if (e.key === "Escape") {
              setQty(String(item.quantity));
              setEditing(false);
            }
          }}
          className="h-7 w-20 text-xs"
        />
      ) : (
        <button
          type="button"
          onClick={() => {
            setQty(String(item.quantity));
            setEditing(true);
          }}
          className="shrink-0 rounded px-1.5 py-0.5 text-sm font-medium tabular-nums hover:bg-accent cursor-pointer"
          title="Editar cantidad"
        >
          {fmtQty(item.quantity)} <span className="text-xs font-normal text-muted-foreground">{item.unit}</span>
        </button>
      )}
      <Button
        size="icon-sm"
        variant="ghost"
        className="shrink-0 text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
        onClick={() => del.mutate()}
        title="Eliminar producto"
      >
        <Trash2 />
      </Button>
    </li>
  );
}

// ---------------------------------------------------------------------------
// Pestaña Compra
// ---------------------------------------------------------------------------

export function ShoppingTab() {
  const invalidate = useInvalidateMeals();
  const [selectedId, setSelectedId] = useState<Id | null>(null);
  const [generateOpen, setGenerateOpen] = useState(false);
  const [manualOpen, setManualOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [newName, setNewName] = useState("");

  const lists = useQuery({
    queryKey: ["meals", "shopping", "lists"],
    queryFn: () => invoke<ShoppingList[]>("meals.shopping.lists"),
  });

  const effectiveId = selectedId ?? lists.data?.[0]?.id ?? null;

  const detail = useQuery({
    queryKey: ["meals", "shopping", "list", effectiveId],
    queryFn: () => invoke<ShoppingListDetail>("meals.shopping.get", { id: effectiveId }),
    enabled: !!effectiveId,
  });

  const rename = useMutation({
    mutationFn: () => invoke("meals.shopping.rename", { id: effectiveId, name: newName.trim() }),
    onSuccess: () => {
      setRenaming(false);
      invalidate();
    },
    onError: (e) => toastMealError(e, "No se pudo renombrar"),
  });

  const del = useMutation({
    mutationFn: (id: Id) => invoke("meals.shopping.delete", { id }),
    onSuccess: () => {
      toast.success("Lista eliminada");
      setSelectedId(null);
      invalidate();
    },
    onError: (e) => toastMealError(e, "No se pudo eliminar la lista"),
  });

  const copy = useMutation({
    mutationFn: () => invoke<{ content: string }>("meals.shopping.copy_text", { id: effectiveId }),
    onSuccess: async (r) => {
      try {
        await navigator.clipboard.writeText(r.content);
        toast.success("Lista copiada al portapapeles");
      } catch {
        toast.error("No se pudo copiar", { description: "Tu entorno bloqueó el acceso al portapapeles." });
      }
    },
    onError: (e) => toastMealError(e, "No se pudo copiar la lista"),
  });

  const exportList = useMutation({
    mutationFn: (format: "txt" | "csv") =>
      invoke<ShoppingExport>("meals.shopping.export", { id: effectiveId, format }),
    onSuccess: (r, format) => {
      download(r.filename, r.content, format === "csv" ? "text/csv" : "text/plain");
      toast.success("Lista exportada", { description: r.filename });
    },
    onError: (e) => toastMealError(e, "No se pudo exportar"),
  });

  const groups = useMemo(() => {
    const map = new Map<string, ShoppingItem[]>();
    for (const item of detail.data?.items ?? []) {
      const arr = map.get(item.category) ?? [];
      arr.push(item);
      map.set(item.category, arr);
    }
    return [...map.entries()];
  }, [detail.data]);

  const items = detail.data?.items ?? [];
  const checkedCount = items.filter((i) => i.checked).length;
  const list = detail.data?.list ?? null;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        {(lists.data ?? []).length > 0 && (
          <Select value={effectiveId ?? undefined} onValueChange={setSelectedId}>
            <SelectTrigger className="h-8 w-[260px] text-xs"><SelectValue placeholder="Elegir lista" /></SelectTrigger>
            <SelectContent>
              {(lists.data ?? []).map((l) => (
                <SelectItem key={l.id} value={l.id}>{l.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <Button size="sm" variant="outline" onClick={() => setHistoryOpen(true)}>
            <History /> Historial
          </Button>
          <Button size="sm" onClick={() => setGenerateOpen(true)}>
            <RefreshCw /> Generar desde semana
          </Button>
        </div>
      </div>

      {lists.isLoading || (effectiveId && detail.isLoading) ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => <Skeleton key={i} className="h-12 rounded-lg" />)}
        </div>
      ) : !list ? (
        <EmptyState
          icon={ShoppingCart}
          title="Sin listas de la compra"
          description="Genera una lista desde una semana planificada: los ingredientes se consolidan por nombre y unidad, agrupados por categoría."
          action={<Button onClick={() => setGenerateOpen(true)}><RefreshCw /> Generar lista</Button>}
        />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2 rounded-xl border border-border bg-card px-4 py-3 shadow-sm">
            {renaming ? (
              <div className="flex items-center gap-2">
                <Input autoFocus value={newName} onChange={(e) => setNewName(e.target.value)} className="h-8 w-56 text-sm" />
                <Button size="sm" onClick={() => rename.mutate()} disabled={!newName.trim() || rename.isPending}>Guardar</Button>
                <Button size="sm" variant="ghost" onClick={() => setRenaming(false)}>Cancelar</Button>
              </div>
            ) : (
              <button
                type="button"
                className="flex items-center gap-2 text-sm font-semibold cursor-pointer hover:text-primary"
                onClick={() => {
                  setNewName(list.name);
                  setRenaming(true);
                }}
                title="Renombrar lista"
              >
                {list.name} <Pencil className="h-3 w-3 text-muted-foreground" />
              </button>
            )}
            {list.week_start && <Badge variant="secondary">{weekLabel(list.week_start)}</Badge>}
            {list.has_manual_changes && <Badge variant="warning">cambios manuales</Badge>}
            <span className="text-xs text-muted-foreground">
              {checkedCount}/{items.length} comprados
            </span>
            <div className="ml-auto flex flex-wrap items-center gap-1.5">
              <Button size="sm" variant="outline" onClick={() => setManualOpen(true)}><Plus /> Producto</Button>
              <Button size="sm" variant="outline" onClick={() => copy.mutate()} disabled={copy.isPending} title="Copiar como texto">
                <ClipboardCopy /> Copiar
              </Button>
              <Button size="sm" variant="outline" onClick={() => exportList.mutate("txt")} title="Descargar .txt">
                <Download /> TXT
              </Button>
              <Button size="sm" variant="outline" onClick={() => exportList.mutate("csv")} title="Descargar .csv">
                <Download /> CSV
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="text-destructive hover:text-destructive"
                onClick={() => {
                  if (window.confirm(`¿Eliminar la lista «${list.name}»?`)) del.mutate(list.id);
                }}
                title="Eliminar lista"
              >
                <Trash2 />
              </Button>
            </div>
          </div>

          {items.length > 0 && (
            <div className="h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-success transition-all"
                style={{ width: `${items.length ? (checkedCount / items.length) * 100 : 0}%` }}
              />
            </div>
          )}

          {items.length === 0 ? (
            <EmptyState icon={ListChecks} title="Lista vacía" description="Añade productos manualmente o regenera desde la semana." />
          ) : (
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              {groups.map(([category, groupItems]) => (
                <section key={category} className="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
                  <h3 className="border-b border-border bg-muted/40 px-3 py-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                    {capitalize(category)}
                    <span className="ml-2 font-normal">{groupItems.filter((i) => i.checked).length}/{groupItems.length}</span>
                  </h3>
                  <ul>
                    {groupItems.map((item) => <ItemRow key={item.id} item={item} />)}
                  </ul>
                </section>
              ))}
            </div>
          )}
        </>
      )}

      <GenerateDialog
        open={generateOpen}
        currentList={detail.data ?? null}
        onClose={() => setGenerateOpen(false)}
        onGenerated={(id) => setSelectedId(id)}
      />
      <ManualItemDialog listId={effectiveId} open={manualOpen} onClose={() => setManualOpen(false)} />

      <Dialog open={historyOpen} onOpenChange={setHistoryOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2"><History className="h-4 w-4 text-primary" /> Historial de listas</DialogTitle>
            <DialogDescription>Listas guardadas con nombre y fecha.</DialogDescription>
          </DialogHeader>
          <div className="max-h-80 space-y-1.5 overflow-y-auto">
            {(lists.data ?? []).map((l) => (
              <div key={l.id} className={cn("flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2", l.id === effectiveId && "border-ring/60")}>
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left cursor-pointer"
                  onClick={() => {
                    setSelectedId(l.id);
                    setHistoryOpen(false);
                  }}
                >
                  <p className="truncate text-sm font-medium">{l.name}</p>
                  <p className="text-[11px] text-muted-foreground">
                    {new Date(l.created_at).toLocaleDateString("es-ES", { day: "numeric", month: "long", year: "numeric" })}
                    {l.week_start && ` · ${weekLabel(l.week_start)}`}
                  </p>
                </button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => {
                    if (window.confirm(`¿Eliminar la lista «${l.name}»?`)) del.mutate(l.id);
                  }}
                  title="Eliminar"
                >
                  <Trash2 />
                </Button>
              </div>
            ))}
            {(lists.data ?? []).length === 0 && (
              <p className="py-4 text-center text-xs text-muted-foreground">Aún no hay listas guardadas.</p>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
