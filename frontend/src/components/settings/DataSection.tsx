import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Archive, Database, Download, FolderArchive, Loader2, RotateCcw, TriangleAlert } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

function exportFilename(): string {
  const d = new Date();
  const ymd = `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}`;
  return `nexus-export-${ymd}.json`;
}

export function DataSection() {
  const qc = useQueryClient();
  const [restorePath, setRestorePath] = useState<string | null>(null);

  const backups = useQuery({
    queryKey: ["backups"],
    queryFn: () => invoke<{ paths: string[] }>("backup.list"),
  });

  const exportJson = useMutation({
    mutationFn: () => invoke<unknown>("backup.export"),
    onSuccess: (data) => {
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = exportFilename();
      a.click();
      URL.revokeObjectURL(url);
      toast.success("Exportación descargada");
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo exportar"),
  });

  const createBackup = useMutation({
    mutationFn: () => invoke<{ path: string }>("backup.create"),
    onSuccess: (r) => {
      toast.success("Copia de seguridad creada", { description: r.path });
      qc.invalidateQueries({ queryKey: ["backups"] });
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo crear la copia"),
  });

  const restore = useMutation({
    mutationFn: () => invoke("backup.restore", { path: restorePath }),
    onSuccess: () => {
      toast.success("Datos restaurados", { description: "Se han recargado todos los datos desde la copia." });
      qc.invalidateQueries();
      setRestorePath(null);
    },
    onError: (e) => toast.error(e instanceof ApiError ? e.message : "No se pudo restaurar la copia"),
  });

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-2">
        <Button size="sm" variant="outline" onClick={() => exportJson.mutate()} disabled={exportJson.isPending}>
          {exportJson.isPending ? <Loader2 className="animate-spin" /> : <Download />}
          Exportar JSON
        </Button>
        <Button size="sm" variant="outline" onClick={() => createBackup.mutate()} disabled={createBackup.isPending}>
          {createBackup.isPending ? <Loader2 className="animate-spin" /> : <FolderArchive />}
          Crear copia
        </Button>
      </div>

      <div className="space-y-2">
        <h4 className="text-xs font-semibold text-muted-foreground">Copias de seguridad</h4>
        {backups.isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-10 w-full rounded-lg" />
            <Skeleton className="h-10 w-full rounded-lg" />
          </div>
        ) : backups.isError ? (
          <p className="text-xs text-destructive">
            {backups.error instanceof ApiError ? backups.error.message : "No se pudieron listar las copias."}
          </p>
        ) : (backups.data?.paths ?? []).length === 0 ? (
          <div className="flex items-center gap-2 rounded-lg border border-dashed border-border px-4 py-5 text-xs text-muted-foreground">
            <Archive className="h-4 w-4" />
            Todavía no hay copias de seguridad. Crea la primera con «Crear copia».
          </div>
        ) : (
          <ul className="space-y-1.5">
            {(backups.data?.paths ?? []).map((path) => (
              <li key={path} className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2 shadow-sm">
                <Database className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                <span className="min-w-0 flex-1 truncate font-mono text-[11px]" title={path}>{path}</span>
                <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setRestorePath(path)}>
                  <RotateCcw /> Restaurar
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <p className="text-[11px] text-muted-foreground">
        Todos tus datos (tareas, proyectos, notas, conversaciones y ajustes) se guardan en una base de
        datos SQLite local en tu equipo. Nada sale de tu ordenador salvo las peticiones al proveedor de IA que configures.
      </p>

      <Dialog open={restorePath != null} onOpenChange={(o) => !o && setRestorePath(null)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <TriangleAlert className="h-4 w-4" /> Restaurar copia de seguridad
            </DialogTitle>
            <DialogDescription>
              Esta acción <span className="font-semibold text-foreground">reemplazará TODOS los datos actuales</span> por
              los de la copia seleccionada. Las tareas, notas, proyectos y conversaciones creados después
              de la copia se perderán. Asegúrate de crear una copia del estado actual antes de continuar.
            </DialogDescription>
          </DialogHeader>
          <p className="break-all rounded-md bg-muted p-2 font-mono text-[11px]">{restorePath}</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRestorePath(null)}>Cancelar</Button>
            <Button variant="destructive" onClick={() => restore.mutate()} disabled={restore.isPending}>
              {restore.isPending && <Loader2 className="animate-spin" />}
              Sí, reemplazar mis datos
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
