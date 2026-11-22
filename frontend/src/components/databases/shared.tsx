import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  ALargeSmall,
  AlignLeft,
  AppWindow,
  AtSign,
  BookOpen,
  Calculator,
  Calendar,
  CalendarClock,
  CheckSquare,
  ChevronDownCircle,
  Clapperboard,
  Cpu,
  Database,
  File,
  Gamepad2,
  Gem,
  GraduationCap,
  Hash,
  Image,
  Kanban,
  Link2,
  ListChecks,
  Phone,
  Star,
  Table2,
  Type,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { FileMeta, Id } from "@/lib/types";
import { cn } from "@/lib/utils";
import type { DbField, DbFieldKind, DbRecord, DbViewKind } from "./types";

// ---------------------------------------------------------------------------
// Etiquetas y catálogos
// ---------------------------------------------------------------------------

export const FIELD_KIND_LABEL: Record<DbFieldKind, string> = {
  text: "Texto",
  long_text: "Texto largo",
  integer: "Entero",
  decimal: "Decimal",
  date: "Fecha",
  datetime: "Fecha y hora",
  checkbox: "Casilla",
  select: "Selección",
  multiselect: "Multiselección",
  url: "URL",
  email: "Email",
  phone: "Teléfono",
  image: "Imagen",
  file: "Archivo",
  rating: "Puntuación",
  relation: "Relación",
  formula: "Fórmula",
};

export const FIELD_KIND_ICON: Record<DbFieldKind, LucideIcon> = {
  text: Type,
  long_text: AlignLeft,
  integer: Hash,
  decimal: Calculator,
  date: Calendar,
  datetime: CalendarClock,
  checkbox: CheckSquare,
  select: ChevronDownCircle,
  multiselect: ListChecks,
  url: Link2,
  email: AtSign,
  phone: Phone,
  image: Image,
  file: File,
  rating: Star,
  relation: Database,
  formula: ALargeSmall,
};

export const VIEW_KIND_LABEL: Record<DbViewKind, string> = {
  table: "Tabla",
  kanban: "Kanban",
  gallery: "Galería",
  calendar: "Calendario",
  list: "Lista",
};

export const FILTER_OP_LABEL: Record<string, string> = {
  contains: "contiene",
  not_contains: "no contiene",
  eq: "es igual a",
  ne: "es distinto de",
  gt: "mayor que",
  gte: "mayor o igual que",
  lt: "menor que",
  lte: "menor o igual que",
  is_empty: "está vacío",
  not_empty: "no está vacío",
};

export const CARDINALITY_LABEL: Record<string, string> = {
  one_to_one: "Uno a uno",
  one_to_many: "Uno a varios",
  many_to_many: "Varios a varios",
};

export function opsForKind(kind: DbFieldKind): string[] {
  switch (kind) {
    case "integer":
    case "decimal":
    case "rating":
    case "date":
    case "datetime":
      return ["eq", "ne", "gt", "gte", "lt", "lte", "is_empty", "not_empty"];
    case "checkbox":
      return ["eq", "ne", "is_empty", "not_empty"];
    case "select":
      return ["eq", "ne", "contains", "is_empty", "not_empty"];
    case "multiselect":
    case "relation":
      return ["contains", "eq", "ne", "is_empty", "not_empty"];
    default:
      return ["contains", "not_contains", "eq", "ne", "is_empty", "not_empty"];
  }
}

export function needsFilterValue(op: string): boolean {
  return op !== "is_empty" && op !== "not_empty";
}

// ---------------------------------------------------------------------------
// Colores (clases estáticas para Tailwind)
// ---------------------------------------------------------------------------

export const DB_COLORS = ["gray", "blue", "green", "yellow", "orange", "red", "purple", "pink", "cyan", "teal"] as const;

export const DB_COLOR_STYLE: Record<string, { bg: string; text: string; dot: string }> = {
  gray: { bg: "bg-muted", text: "text-muted-foreground", dot: "bg-muted-foreground" },
  blue: { bg: "bg-blue-500/15", text: "text-blue-500", dot: "bg-blue-500" },
  green: { bg: "bg-success/15", text: "text-success", dot: "bg-success" },
  yellow: { bg: "bg-yellow-500/15", text: "text-yellow-600", dot: "bg-yellow-500" },
  orange: { bg: "bg-orange-500/15", text: "text-orange-500", dot: "bg-orange-500" },
  red: { bg: "bg-destructive/15", text: "text-destructive", dot: "bg-destructive" },
  purple: { bg: "bg-purple-500/15", text: "text-purple-500", dot: "bg-purple-500" },
  pink: { bg: "bg-pink-500/15", text: "text-pink-500", dot: "bg-pink-500" },
  cyan: { bg: "bg-cyan-500/15", text: "text-cyan-500", dot: "bg-cyan-500" },
  teal: { bg: "bg-teal-500/15", text: "text-teal-500", dot: "bg-teal-500" },
};

export function dbColor(color: string) {
  return DB_COLOR_STYLE[color] ?? DB_COLOR_STYLE.blue;
}

export const OPTION_COLORS = ["gray", "blue", "green", "yellow", "orange", "red", "purple", "pink"] as const;

export const OPTION_COLOR_CLASS: Record<string, string> = {
  gray: "bg-muted text-muted-foreground",
  blue: "bg-blue-500/15 text-blue-500",
  green: "bg-success/15 text-success",
  yellow: "bg-yellow-500/15 text-yellow-600",
  orange: "bg-orange-500/15 text-orange-500",
  red: "bg-destructive/15 text-destructive",
  purple: "bg-purple-500/15 text-purple-500",
  pink: "bg-pink-500/15 text-pink-500",
};

export function optionClass(color: string): string {
  return OPTION_COLOR_CLASS[color] ?? OPTION_COLOR_CLASS.gray;
}

// ---------------------------------------------------------------------------
// Iconos de bases (nombres lucide)
// ---------------------------------------------------------------------------

export const DB_ICONS: Record<string, LucideIcon> = {
  database: Database,
  "gamepad-2": Gamepad2,
  clapperboard: Clapperboard,
  "book-open": BookOpen,
  cpu: Cpu,
  "graduation-cap": GraduationCap,
  gem: Gem,
  "app-window": AppWindow,
  kanban: Kanban,
  table: Table2,
  star: Star,
  image: Image,
  calendar: Calendar,
};

export function DbIcon({ icon, className }: { icon: string; className?: string }) {
  const Icon = DB_ICONS[icon] ?? Database;
  return <Icon className={cn("h-4 w-4", className)} />;
}

// ---------------------------------------------------------------------------
// Valores de registro
// ---------------------------------------------------------------------------

export function titleFieldOf(fields: DbField[]): DbField | undefined {
  return fields.find((f) => f.kind === "text") ?? fields.find((f) => f.kind === "long_text");
}

export function recordTitle(record: DbRecord, fields: DbField[]): string {
  const tf = titleFieldOf(fields);
  const v = tf ? record.values[tf.id] : undefined;
  if (typeof v === "string" && v.trim()) return v;
  for (const f of fields) {
    const x = record.values[f.id];
    if (typeof x === "string" && x.trim()) return x;
  }
  return "(sin título)";
}

export function fieldValue(record: DbRecord, field: DbField): unknown {
  return field.kind === "formula" ? record.computed[field.id] : record.values[field.id];
}

export function isEmptyValue(v: unknown): boolean {
  return v === null || v === undefined || v === "" || (Array.isArray(v) && v.length === 0);
}

export function displayValue(record: DbRecord, field: DbField): string {
  const v = fieldValue(record, field);
  if (isEmptyValue(v)) return "";
  switch (field.kind) {
    case "select": {
      const opt = (field.config.options ?? []).find((o) => o.id === v);
      return opt?.label ?? String(v);
    }
    case "multiselect":
      return (v as string[])
        .map((id) => (field.config.options ?? []).find((o) => o.id === id)?.label ?? id)
        .join(", ");
    case "relation":
      return (record.relation_titles[field.id] ?? []).map((c) => c.title).join(", ");
    case "checkbox":
      return v ? "Sí" : "No";
    case "rating":
      return `${v} / ${field.config.max_rating ?? 5}`;
    case "image":
    case "file":
      return "Adjunto";
    default:
      return String(v);
  }
}

// ---------------------------------------------------------------------------
// Adjuntos (files.save / files.get)
// ---------------------------------------------------------------------------

const MAX_UPLOAD_BYTES = 25 * 1024 * 1024;

export async function uploadAttachment(file: File): Promise<FileMeta> {
  if (file.size > MAX_UPLOAD_BYTES) {
    throw new ApiError("validation", "El archivo supera el máximo de 25 MB.");
  }
  const dataBase64 = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      resolve(result.slice(result.indexOf(",") + 1));
    };
    reader.onerror = () => reject(new ApiError("internal", "No se pudo leer el archivo"));
    reader.readAsDataURL(file);
  });
  return invoke<FileMeta>("files.save", {
    name: file.name,
    mime: file.type || "application/octet-stream",
    data_base64: dataBase64,
  });
}

const imageCache = new Map<Id, string>();

export async function fetchAttachmentUrl(fileId: Id): Promise<string> {
  const cached = imageCache.get(fileId);
  if (cached) return cached;
  const res = await invoke<{ meta: FileMeta; data_base64: string }>("files.get", { id: fileId });
  const url = `data:${res.meta.mime};base64,${res.data_base64}`;
  imageCache.set(fileId, url);
  return url;
}

export function useAttachmentUrl(fileId: Id | null | undefined) {
  return useQuery({
    queryKey: ["db-attachment", fileId],
    queryFn: () => fetchAttachmentUrl(fileId as Id),
    enabled: !!fileId,
    staleTime: Infinity,
    gcTime: Infinity,
    retry: 1,
  });
}

export function downloadText(filename: string, mime: string, content: string) {
  const blob = new Blob([content], { type: `${mime};charset=utf-8` });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}

// ---------------------------------------------------------------------------
// Errores y caché
// ---------------------------------------------------------------------------

export function toastDbError(e: unknown, fallback: string) {
  if (e instanceof ApiError && e.code === "ai_unavailable") {
    toast.info("Esta función necesita un proveedor de IA", {
      description: "Configúralo en Ajustes → Inteligencia Artificial. El resto del módulo funciona sin IA.",
    });
    return;
  }
  toast.error(fallback, { description: e instanceof ApiError ? e.message : undefined });
}

export function useInvalidateDatabases() {
  const qc = useQueryClient();
  return useCallback(() => qc.invalidateQueries({ queryKey: ["databases"] }), [qc]);
}

/** Chip de opción select con su color. */
export function OptionBadge({ label, color, className }: { label: string; color: string; className?: string }) {
  return (
    <span className={cn("inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium", optionClass(color), className)}>
      {label}
    </span>
  );
}
