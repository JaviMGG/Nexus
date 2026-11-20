import { Badge } from "@/components/ui/badge";
import type { Priority, ProjectStatus, RunStatus } from "@/lib/types";
import { PRIORITY_LABEL, PROJECT_STATUS_LABEL, RUN_STATUS_LABEL } from "@/lib/utils";

export function PriorityBadge({ priority }: { priority: Priority }) {
  return <Badge variant={priority}>{PRIORITY_LABEL[priority]}</Badge>;
}

export function ProjectStatusBadge({ status }: { status: ProjectStatus }) {
  const variant =
    status === "active" ? "success" : status === "completed" ? "default" : status === "paused" ? "warning" : "secondary";
  return <Badge variant={variant}>{PROJECT_STATUS_LABEL[status]}</Badge>;
}

export function RunStatusBadge({ status }: { status: RunStatus }) {
  const variant =
    status === "completed"
      ? "success"
      : status === "failed"
        ? "destructive"
        : status === "running" || status === "waiting_approval"
          ? "warning"
          : "secondary";
  return <Badge variant={variant}>{RUN_STATUS_LABEL[status]}</Badge>;
}
