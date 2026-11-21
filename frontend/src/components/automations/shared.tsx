import {
  AlarmClock,
  BookOpen,
  Bot,
  CalendarCheck,
  Copy,
  Cpu,
  GitBranch,
  Hourglass,
  Lightbulb,
  Rocket,
  Search,
  Sunrise,
  Sunset,
  Target,
  Timer,
  Workflow,
  Zap,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AutomationMode, ExecutionStatus, StepStatus } from "@/lib/types";
import { AUTOMATION_MODE_LABEL, EXECUTION_STATUS_LABEL, cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export const AUTOMATION_ICONS: Record<string, LucideIcon> = {
  zap: Zap,
  sunrise: Sunrise,
  sunset: Sunset,
  "alarm-clock": AlarmClock,
  lightbulb: Lightbulb,
  rocket: Rocket,
  hourglass: Hourglass,
  "calendar-check": CalendarCheck,
  target: Target,
  copy: Copy,
  search: Search,
  workflow: Workflow,
  bot: Bot,
  timer: Timer,
  "book-open": BookOpen,
};

export function AutomationIcon({ icon, className }: { icon: string; className?: string }) {
  const Icon = AUTOMATION_ICONS[icon] ?? Zap;
  return <Icon className={cn("h-4 w-4", className)} />;
}

export const EXEC_BADGE_VARIANT: Record<ExecutionStatus, "default" | "secondary" | "success" | "destructive" | "outline" | "warning"> = {
  pending: "secondary",
  running: "default",
  waiting_approval: "warning",
  completed: "success",
  failed: "destructive",
  cancelled: "outline",
};

export function ExecutionBadge({ status, className }: { status: ExecutionStatus; className?: string }) {
  return (
    <Badge variant={EXEC_BADGE_VARIANT[status]} className={cn(status === "running" && "animate-pulse", className)}>
      {EXECUTION_STATUS_LABEL[status]}
    </Badge>
  );
}

export const STEP_BADGE_VARIANT: Record<StepStatus, "default" | "secondary" | "success" | "destructive" | "outline" | "warning"> = {
  ok: "success",
  error: "destructive",
  skipped: "outline",
  deferred: "warning",
  waiting_approval: "warning",
  cancelled: "outline",
};

const MODE_STYLE: Record<AutomationMode, string> = {
  local: "bg-success/15 text-success",
  ai: "bg-primary/15 text-primary",
  hybrid: "bg-warning/15 text-warning",
};

const MODE_ICON: Record<AutomationMode, LucideIcon> = {
  local: Cpu,
  ai: Bot,
  hybrid: GitBranch,
};

const MODE_HINT: Record<AutomationMode, string> = {
  local: "Sin IA · funciona offline · no consume tokens",
  ai: "Requiere proveedor de IA y conexión",
  hybrid: "Local primero; IA solo cuando es necesaria",
};

export function ModeBadge({ mode, className }: { mode: AutomationMode; className?: string }) {
  const Icon = MODE_ICON[mode];
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className={cn("inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium", MODE_STYLE[mode], className)}>
          <Icon className="h-3 w-3" />
          {AUTOMATION_MODE_LABEL[mode]}
        </span>
      </TooltipTrigger>
      <TooltipContent>{MODE_HINT[mode]}</TooltipContent>
    </Tooltip>
  );
}

export function executionDuration(ms: number | null): string {
  if (ms == null) return "—";
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(1)} s`;
}
