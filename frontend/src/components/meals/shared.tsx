import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { invoke, ApiError } from "@/lib/api";
import type { FileMeta, Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import { CookingPot } from "lucide-react";

export interface Recipe {
  id: Id;
  name: string;
  description: string;
  image_file_id: Id | null;
  servings: number;
  prep_time_min: number;
  cook_time_min: number;
  difficulty: RecipeDifficulty;
  categories: string[];
  tags: string[];
  ingredients: Ingredient[];
  steps: string[];
  nutrition: Nutrition | null;
  favorite: boolean;
  created_at: string;
  updated_at: string;
}

export type RecipeDifficulty = "facil" | "media" | "dificil";
export type MealSlot = "desayuno" | "almuerzo" | "comida" | "merienda" | "cena";
export type PantryLocation = "despensa" | "nevera" | "congelador" | "otro";

export interface Ingredient {
  name: string;
  quantity: number;
  unit: string;
  category: string;
  notes: string;
}

export interface Nutrition {
  kcal: number;
  protein_g: number;
  carbs_g: number;
  fat_g: number;
}

export interface CreateRecipe {
  name: string;
  description?: string;
  image_file_id?: Id | null;
  servings?: number;
  prep_time_min?: number;
  cook_time_min?: number;
  difficulty?: RecipeDifficulty;
  categories?: string[];
  tags?: string[];
  ingredients?: Ingredient[];
  steps?: string[];
  nutrition?: Nutrition | null;
  favorite?: boolean;
}

export type UpdateRecipe = Partial<Omit<CreateRecipe, "nutrition" | "image_file_id">> & {
  nutrition?: Nutrition | null;
  image_file_id?: Id | null;
};

export interface RecipeFilter {
  search?: string;
  category?: string;
  difficulty?: RecipeDifficulty;
  favorites_only?: boolean;
  max_time_min?: number;
  sort?: "name" | "time" | "recent";
}

export interface RecipePreview {
  id: Id;
  name: string;
  image_file_id: Id | null;
  difficulty: RecipeDifficulty;
  total_time: number;
}

export interface MealPlanEntry {
  id: Id;
  week_start: string;
  date: string;
  slot: MealSlot;
  recipe_id: Id;
  servings: number;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface PlanEntryView extends MealPlanEntry {
  recipe_name: string;
  recipe_image_file_id: Id | null;
  recipe_difficulty: RecipeDifficulty;
  total_time_min: number;
}

export interface WeekPlan {
  week_start: string;
  entries: PlanEntryView[];
}

export interface TemplateEntry {
  day_offset: number;
  slot: MealSlot;
  recipe_id: Id;
  servings: number;
}

export interface MenuTemplate {
  id: Id;
  name: string;
  entries: TemplateEntry[];
  created_at: string;
  updated_at: string;
}

export interface TemplateEntryView extends TemplateEntry {
  recipe_name: string | null;
}

export interface MenuTemplateDetail {
  id: Id;
  name: string;
  entries: TemplateEntryView[];
  created_at: string;
  updated_at: string;
}

export interface PlanEntryInput {
  date: string;
  slot: MealSlot;
  recipe_id: Id;
  servings: number;
}

export interface ShoppingList {
  id: Id;
  name: string;
  week_start: string | null;
  has_manual_changes: boolean;
  created_at: string;
  updated_at: string;
}

export interface ShoppingItem {
  id: Id;
  list_id: Id;
  name: string;
  name_key: string;
  quantity: number;
  unit: string;
  category: string;
  checked: boolean;
  manual: boolean;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface ShoppingListDetail {
  list: ShoppingList;
  items: ShoppingItem[];
}

export interface CreateShoppingItem {
  name: string;
  quantity?: number;
  unit?: string;
  category?: string;
}

export interface UpdateShoppingItem {
  name?: string;
  quantity?: number;
  unit?: string;
  category?: string;
  checked?: boolean;
}

export interface ShoppingExport {
  filename: string;
  content: string;
}

export interface PantryItem {
  id: Id;
  name: string;
  name_key: string;
  quantity: number;
  unit: string;
  category: string;
  expires_at: string | null;
  location: PantryLocation;
  created_at: string;
  updated_at: string;
}

export interface CreatePantryItem {
  name: string;
  quantity?: number;
  unit?: string;
  category?: string;
  expires_at?: string | null;
  location?: PantryLocation;
}

export type UpdatePantryItem = Partial<Omit<CreatePantryItem, "expires_at">> & {
  expires_at?: string | null;
};

export interface PantryFilter {
  search?: string;
  location?: PantryLocation;
  category?: string;
}

export interface PantryConsumeResult {
  deleted: boolean;
  item: PantryItem | null;
}

export interface MissingIngredient {
  name: string;
  needed: number;
  have: number;
  unit: string;
}

export interface PantryMatch {
  recipe_id: Id;
  recipe_name: string;
  image_file_id: Id | null;
  coverage_pct: number;
  covered: string[];
  missing: MissingIngredient[];
}

// ---------------------------------------------------------------------------
// Etiquetas y helpers de dominio
// ---------------------------------------------------------------------------

export const DIFFICULTY_LABEL: Record<RecipeDifficulty, string> = {
  facil: "Fácil",
  media: "Media",
  dificil: "Difícil",
};

export const DIFFICULTY_VARIANT: Record<RecipeDifficulty, "success" | "warning" | "destructive"> = {
  facil: "success",
  media: "warning",
  dificil: "destructive",
};

export const SLOT_LABEL: Record<MealSlot, string> = {
  desayuno: "Desayuno",
  almuerzo: "Almuerzo",
  comida: "Comida",
  merienda: "Merienda",
  cena: "Cena",
};

export const SLOTS: MealSlot[] = ["desayuno", "almuerzo", "comida", "merienda", "cena"];

export const LOCATION_LABEL: Record<PantryLocation, string> = {
  despensa: "Despensa",
  nevera: "Nevera",
  congelador: "Congelador",
  otro: "Otro",
};

export const LOCATIONS: PantryLocation[] = ["despensa", "nevera", "congelador", "otro"];

export const INGREDIENT_CATEGORIES = [
  "verduras",
  "frutas",
  "carne",
  "pescado",
  "lácteos",
  "huevos",
  "despensa",
  "panadería",
  "congelados",
  "bebidas",
  "especias",
  "otros",
];

export const UNIT_SUGGESTIONS = ["g", "kg", "ml", "l", "ud", "cda", "cdta", "taza", "pizca"];

export function round2(x: number): number {
  return Math.round(x * 100) / 100;
}

export function scaleQuantity(quantity: number, base: number, target: number): number {
  if (base <= 0) return quantity;
  return round2((quantity * target) / base);
}

export function fmtQty(q: number): string {
  const r = round2(q);
  return Number.isInteger(r) ? String(r) : String(r).replace(/\.?0+$/, "");
}

export function fmtMinutes(min: number): string {
  if (min <= 0) return "0 min";
  if (min < 60) return `${min} min`;
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** Lunes de la semana que contiene `date` (YYYY-MM-DD), en hora local. */
export function mondayOf(date: string): string {
  const d = new Date(date + "T00:00:00");
  const day = (d.getDay() + 6) % 7;
  d.setDate(d.getDate() - day);
  return toDateStr(d);
}

export function toDateStr(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function addDays(date: string, days: number): string {
  const d = new Date(date + "T00:00:00");
  d.setDate(d.getDate() + days);
  return toDateStr(d);
}

export function weekLabel(weekStart: string): string {
  const start = new Date(weekStart + "T00:00:00");
  const end = new Date(weekStart + "T00:00:00");
  end.setDate(end.getDate() + 6);
  const opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "short" };
  return `Semana del ${start.toLocaleDateString("es-ES", opts)} al ${end.toLocaleDateString("es-ES", opts)}`;
}

export function dayHeader(date: string): { name: string; num: string } {
  const d = new Date(date + "T00:00:00");
  return {
    name: d.toLocaleDateString("es-ES", { weekday: "short" }).replace(".", ""),
    num: d.toLocaleDateString("es-ES", { day: "numeric", month: "short" }),
  };
}

/** Estado de caducidad SOLO con fechas del usuario: null si no hay fecha. */
export function expiryState(expiresAt: string | null, today: string): "expired" | "soon" | "ok" | null {
  if (!expiresAt) return null;
  if (expiresAt < today) return "expired";
  const diff = (new Date(expiresAt + "T00:00:00").getTime() - new Date(today + "T00:00:00").getTime()) / 86400000;
  if (diff <= 3) return "soon";
  return "ok";
}

export function capitalize(s: string): string {
  return s.length === 0 ? s : s[0].toUpperCase() + s.slice(1);
}

/** Muestra un toast claro; si falta proveedor de IA lo indica expresamente. */
export function toastMealError(e: unknown, fallback: string) {
  if (e instanceof ApiError && e.code === "ai_unavailable") {
    toast.error("Requiere proveedor de IA (Ajustes)", {
      description: "Configura un proveedor en Ajustes → Inteligencia Artificial. Todo lo demás funciona sin IA.",
    });
    return;
  }
  toast.error(fallback, { description: e instanceof ApiError ? e.message : undefined });
}

// ---------------------------------------------------------------------------
// Imágenes de receta (files.get con caché de data URL)
// ---------------------------------------------------------------------------

const imageCache = new Map<Id, string>();

export async function fetchRecipeImage(fileId: Id): Promise<string> {
  const cached = imageCache.get(fileId);
  if (cached) return cached;
  const res = await invoke<{ meta: FileMeta; data_base64: string }>("files.get", { id: fileId });
  const url = `data:${res.meta.mime};base64,${res.data_base64}`;
  imageCache.set(fileId, url);
  return url;
}

export function useRecipeImage(fileId: Id | null | undefined) {
  return useQuery({
    queryKey: ["meal-image", fileId],
    queryFn: () => fetchRecipeImage(fileId as Id),
    enabled: !!fileId,
    staleTime: Infinity,
    gcTime: Infinity,
    retry: 1,
  });
}

/** Sube una imagen con files.save y devuelve el id de archivo. */
export async function uploadRecipeImage(file: File): Promise<Id> {
  const dataBase64 = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      resolve(result.slice(result.indexOf(",") + 1));
    };
    reader.onerror = () => reject(new Error("No se pudo leer la imagen"));
    reader.readAsDataURL(file);
  });
  const meta = await invoke<FileMeta>("files.save", {
    name: file.name,
    mime: file.type || "image/jpeg",
    data_base64: dataBase64,
  });
  return meta.id;
}

// ---------------------------------------------------------------------------
// Componente de imagen con placeholder bonito
// ---------------------------------------------------------------------------

export function RecipeImage({
  fileId,
  alt,
  className,
  iconClassName,
}: {
  fileId: Id | null | undefined;
  alt: string;
  className?: string;
  iconClassName?: string;
}) {
  const img = useRecipeImage(fileId);
  const placeholder = (
    <div
      className={cn(
        "flex h-full w-full items-center justify-center bg-gradient-to-br from-primary/15 via-muted to-warning/10",
        className,
      )}
    >
      <CookingPot className={cn("h-8 w-8 text-muted-foreground/60", iconClassName)} />
    </div>
  );
  if (!fileId || img.isError) return placeholder;
  if (img.isLoading || !img.data) return placeholder;
  return (
    <img
      src={img.data}
      alt={alt}
      loading="lazy"
      className={cn("h-full w-full object-cover", className)}
    />
  );
}

/** Hook de invalidación de todas las queries del módulo. */
export function useInvalidateMeals() {
  const qc = useQueryClient();
  return useCallback(() => qc.invalidateQueries({ queryKey: ["meals"] }), [qc]);
}
