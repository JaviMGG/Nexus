import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Archive, FolderKanban, MoreHorizontal, Pencil, Plus, Trash2 } from "lucide-react";
import { invoke } from "@/lib/api";
import type { Project, ProjectProgress } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { EmptyState } from "@/components/ui/empty-state";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ProjectDialog } from "@/components/projects/ProjectDialog";
import { PriorityBadge, ProjectStatusBadge } from "@/components/tasks/StatusBadges";
import { QueryError } from "@/components/tasks/QueryError";

export function ProjectsPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [showArchived, setShowArchived] = useState(false);
  const [dialog, setDialog] = useState<{ open: boolean; project: Project | null }>({ open: false, project: null });
  const [deleteTarget, setDeleteTarget] = useState<Project | null>(null);

  const projects = useQuery({
    queryKey: ["projects", { progress: true, include_archived: showArchived }],
    queryFn: () => invoke<ProjectProgress[]>("projects.list_with_progress", { include_archived: showArchived }),
  });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["projects"] });
    qc.invalidateQueries({ queryKey: ["dashboard"] });
    qc.invalidateQueries({ queryKey: ["tasks"] });
  };

  const archive = useMutation<Project, Error, Project>({
    mutationFn: (p) => invoke<Project>("projects.update", { id: p.id, data: { status: "archived" } }),
    onSuccess: () => {
      toast.success("Proyecto archivado");
      invalidate();
    },
    onError: (err) => toast.error("No se pudo archivar", { description: err.message }),
  });

  const remove = useMutation<void, Error, Project>({
    mutationFn: (p) => invoke<void>("projects.delete", { id: p.id }),
    onSuccess: () => {
      toast.success("Proyecto eliminado");
      setDeleteTarget(null);
      invalidate();
    },
    onError: (err) => toast.error("No se pudo eliminar", { description: err.message }),
  });

  return (
    <div className="mx-auto max-w-[1200px] p-6 animate-fade-in">
      <header className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Proyectos</h1>
          <p className="mt-1 text-sm text-muted-foreground">Tus objetivos, con progreso real.</p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Switch id="show-archived" checked={showArchived} onCheckedChange={setShowArchived} />
            <Label htmlFor="show-archived" className="text-xs text-muted-foreground cursor-pointer">
              Mostrar archivados
            </Label>
          </div>
          <Button onClick={() => setDialog({ open: true, project: null })}>
            <Plus /> Nuevo proyecto
          </Button>
        </div>
      </header>

      {projects.isLoading ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-44 rounded-xl" />
          ))}
        </div>
      ) : projects.isError ? (
        <QueryError error={projects.error} onRetry={() => projects.refetch()} />
      ) : (projects.data ?? []).length === 0 ? (
        <EmptyState
          icon={FolderKanban}
          title={showArchived ? "No hay proyectos" : "No hay proyectos activos"}
          description="Crea un proyecto para agrupar tareas y notas alrededor de un objetivo."
          action={
            <Button onClick={() => setDialog({ open: true, project: null })}>
              <Plus /> Nuevo proyecto
            </Button>
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {projects.data!.map(({ project: p, total, done, progress, overdue }) => (
            <Card
              key={p.id}
              onClick={() => navigate(`/projects/${p.id}`)}
              className="group cursor-pointer p-4 transition-all hover:border-ring/50 hover:shadow animate-slide-up"
            >
              <div className="mb-2 flex items-start justify-between gap-2">
                <h3 className="min-w-0 flex-1 truncate text-sm font-semibold">{p.name}</h3>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label="Acciones del proyecto"
                      className="opacity-0 transition-opacity group-hover:opacity-100 data-[state=open]:opacity-100"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <MoreHorizontal />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end" onClick={(e) => e.stopPropagation()}>
                    <DropdownMenuItem onClick={() => setDialog({ open: true, project: p })}>
                      <Pencil /> Editar
                    </DropdownMenuItem>
                    {p.status !== "archived" && (
                      <DropdownMenuItem disabled={archive.isPending} onClick={() => archive.mutate(p)}>
                        <Archive /> Archivar
                      </DropdownMenuItem>
                    )}
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      className="text-destructive focus:text-destructive"
                      onClick={() => setDeleteTarget(p)}
                    >
                      <Trash2 /> Eliminar
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
              {p.description ? (
                <p className="mb-3 line-clamp-2 min-h-8 text-xs text-muted-foreground">{p.description}</p>
              ) : (
                <p className="mb-3 min-h-8 text-xs text-muted-foreground/60">Sin descripción</p>
              )}
              <div className="mb-3 flex flex-wrap items-center gap-1.5">
                <ProjectStatusBadge status={p.status} />
                <PriorityBadge priority={p.priority} />
                {overdue > 0 && <Badge variant="destructive">{overdue} atrasadas</Badge>}
              </div>
              <div className="flex items-center gap-3">
                <Progress value={progress * 100} className="flex-1" />
                <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
                  {done}/{total}
                </span>
              </div>
            </Card>
          ))}
        </div>
      )}

      <ProjectDialog
        open={dialog.open}
        onOpenChange={(open) => setDialog((d) => ({ ...d, open }))}
        project={dialog.project}
      />

      <Dialog open={deleteTarget !== null} onOpenChange={(open) => !open && setDeleteTarget(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Eliminar proyecto</DialogTitle>
            <DialogDescription>
              ¿Seguro que quieres eliminar «{deleteTarget?.name}»? Podrás deshacerlo desde la actividad reciente.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)} disabled={remove.isPending}>
              Cancelar
            </Button>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              onClick={() => deleteTarget && remove.mutate(deleteTarget)}
            >
              {remove.isPending ? "Eliminando…" : "Eliminar"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
