import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso.length === 10 ? iso + "T00:00:00" : iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("es-ES", { day: "numeric", month: "short" });
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-ES", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

export function relativeTime(iso: string): string {
  const d = new Date(iso);
  const diff = Date.now() - d.getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "ahora";
  if (mins < 60) return `hace ${mins} min`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `hace ${hours} h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `hace ${days} d`;
  return formatDate(iso);
}

export function todayStr(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function isOverdue(due: string | null, status: string): boolean {
  if (!due || status === "done" || status === "cancelled") return false;
  return due.slice(0, 10) < todayStr();
}

export function greeting(): string {
  const h = new Date().getHours();
  if (h < 6) return "Buenas noches";
  if (h < 13) return "Buenos días";
  if (h < 20) return "Buenas tardes";
  return "Buenas noches";
}

export const PRIORITY_LABEL: Record<string, string> = {
  low: "Baja",
  medium: "Media",
  high: "Alta",
  urgent: "Urgente",
};

export const STATUS_LABEL: Record<string, string> = {
  todo: "Pendiente",
  in_progress: "En curso",
  done: "Hecha",
  cancelled: "Cancelada",
};

export const PROJECT_STATUS_LABEL: Record<string, string> = {
  active: "Activo",
  paused: "Pausado",
  completed: "Completado",
  archived: "Archivado",
};

export const NOTE_KIND_LABEL: Record<string, string> = {
  note: "Nota",
  idea: "Idea",
  document: "Documento",
  link: "Enlace",
};

export const RUN_STATUS_LABEL: Record<string, string> = {
  pending: "Pendiente",
  running: "Ejecutando",
  waiting_approval: "Esperando aprobación",
  completed: "Completada",
  failed: "Fallida",
  cancelled: "Cancelada",
};

export const AGENT_LABEL: Record<string, string> = {
  orchestrator: "NEXUS Orchestrator",
  planner: "Planner",
  project_manager: "Project Manager",
  knowledge: "Knowledge",
  researcher: "Researcher",
  executor: "Executor",
};
