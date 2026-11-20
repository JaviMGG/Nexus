import type { EventColor } from "@/lib/types";

export interface CalendarDay {
  key: string;
  date: Date;
  inMonth: boolean;
  isToday: boolean;
}

export function dateKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

// El grid empieza el lunes anterior (o igual) al día 1 del mes y termina el
// domingo posterior (o igual) al último día; semanas completas de 7 celdas.
export function monthGrid(year: number, month: number): CalendarDay[] {
  const first = new Date(year, month, 1);
  const mondayOffset = (first.getDay() + 6) % 7;
  const start = new Date(year, month, 1 - mondayOffset);
  const last = new Date(year, month + 1, 0);
  const sundayOffset = (7 - last.getDay()) % 7;
  const totalDays = mondayOffset + last.getDate() + sundayOffset;
  const today = dateKey(new Date());
  return Array.from({ length: totalDays }, (_, i) => {
    const date = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
    const key = dateKey(date);
    return { key, date, inMonth: date.getMonth() === month, isToday: key === today };
  });
}

export const EVENT_CHIP_CLASS: Record<EventColor, string> = {
  primary: "bg-primary/15 text-primary border-primary/30",
  success: "bg-success/15 text-success border-success/30",
  warning: "bg-warning/15 text-warning border-warning/30",
  destructive: "bg-destructive/15 text-destructive border-destructive/30",
  info: "bg-primary/10 text-accent-foreground border-primary/20",
};

export const EVENT_DOT_CLASS: Record<EventColor, string> = {
  primary: "bg-primary",
  success: "bg-success",
  warning: "bg-warning",
  destructive: "bg-destructive",
  info: "bg-accent-foreground",
};

export const EVENT_SWATCH_CLASS: Record<EventColor, string> = {
  primary: "bg-primary text-primary-foreground",
  success: "bg-success text-background",
  warning: "bg-warning text-background",
  destructive: "bg-destructive text-destructive-foreground",
  info: "bg-accent-foreground text-accent",
};
