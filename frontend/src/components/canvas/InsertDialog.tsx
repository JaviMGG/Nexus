import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { CheckSquare, Database, FolderKanban, Loader2, Search, StickyNote, UtensilsCrossed } from "lucide-react";
import { invoke, ApiError } from "@/lib/api";
import type { Note, Project, Task } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EmptyState } from "@/components/ui/empty-state";
import type { CanvasElementType } from "./types";

export interface RefPick {
  type: CanvasElementType;
  refId: string;
  title: string;
  extra: Record<string, unknown>;
}

type TabKey = "task" | "project" | "note" | "database" | "recipe";

const TAB_META: Record<TabKey, { label: string; icon: typeof Search; command: string; type: CanvasElementType }> = {
  task: { label: "Tarea", icon: CheckSquare, command: "tasks.list", type: "task_ref" },
  project: { label: "Proyecto", icon: FolderKanban, command: "projects.list", type: "project_ref" },
  note: { label: "Nota", icon: StickyNote, command: "notes.list", type: "note_ref" },
  database: { label: "Base de datos", icon: Database, command: "databases.list", type: "database_ref" },
  recipe: { label: "Receta", icon: UtensilsCrossed, command: "meals.recipes.list", type: "recipe_ref" },
};

interface Row {
  id: string;
  title: string;
  subtitle?: string;
  extra: Record<string, unknown>;
}

function useRows(tab: TabKey, enabled: boolean): { rows: Row[]; isLoading: boolean; unavailable: boolean; error: string | null } {
  const meta = TAB_META[tab];
  const q = useQuery({
    queryKey: ["canvas-insert", tab],
    queryFn: () => invoke<Record<string, unknown>[]>(meta.command, tab === "task" || tab === "note" ? { filter: { limit: 200 } } : {}),
    enabled,
    retry: false,
    staleTime: 15000,
  });
  const rows = useMemo<Row[]>(() => {
    const data = q.data ?? [];
    return data.map((item) => {
      const id = String(item.id ?? "");
      switch (tab) {
        case "task": {
          const t = item as unknown as Task;
          return { id, title: t.title, subtitle: t.status === "done" ? "Completada" : t.status === "in_progress" ? "En curso" : "Pendiente", extra: { status: t.status } };
        }
        case "project": {
          const p = item as unknown as Project;
          return { id, title: p.name, subtitle: p.description || undefined, extra: { status: p.status } };
        }
        case "note": {
          const n = item as unknown as Note;
          return { id, title: n.title, subtitle: n.content.slice(0, 80) || undefined, extra: { kind: n.kind } };
        }
        default: {
          const name = typeof item.name === "string" ? item.name : typeof item.title === "string" ? item.title : id;
          const desc = typeof item.description === "string" ? item.description : undefined;
          return { id, title: name, subtitle: desc || undefined, extra: {} };
        }
      }
    });
  }, [q.data, tab]);
  const err = q.error;
  const unavailable = err instanceof ApiError && err.code === "not_found";
  return {
    rows,
    isLoading: q.isLoading && enabled,
    unavailable,
    error: !unavailable && err instanceof ApiError ? err.message : null,
  };
}

function TabBody({ tab, active, search, onPick }: { tab: TabKey; active: boolean; search: string; onPick: (pick: RefPick) => void }) {
  const { rows, isLoading, unavailable, error } = useRows(tab, active);
  const meta = TAB_META[tab];
  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-10 text-muted-foreground">
        <Loader2 className="h-5 w-5 animate-spin" />
      </div>
    );
  }
  if (unavailable) {
    return <EmptyState icon={meta.icon} title="Módulo aún no disponible" description="Este módulo de NEXUS todavía no está activo. Podrás insertar sus referencias cuando lo esté." />;
  }
  if (error) {
    return <EmptyState icon={meta.icon} title="No se pudo cargar" description={error} />;
  }
  const filtered = rows.filter((r) => r.title.toLowerCase().includes(search.toLowerCase()));
  if (filtered.length === 0) {
    return <EmptyState icon={meta.icon} title={search ? "Sin resultados" : `No hay ${meta.label.toLowerCase()}s todavía`} description={search ? "Prueba con otra búsqueda." : "Crea contenido en su módulo y vuelve aquí."} />;
  }
  return (
    <ul className="max-h-72 space-y-1 overflow-y-auto">
      {filtered.slice(0, 100).map((r) => (
        <li key={r.id}>
          <button
            className="flex w-full items-center gap-2.5 rounded-lg border border-border bg-card px-3 py-2 text-left transition-colors hover:bg-accent cursor-pointer"
            onClick={() => onPick({ type: meta.type, refId: r.id, title: r.title, extra: r.extra })}
          >
            <meta.icon className="h-4 w-4 shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-xs font-medium">{r.title}</span>
              {r.subtitle && <span className="block truncate text-[10px] text-muted-foreground">{r.subtitle}</span>}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

export function InsertDialog({
  open,
  onClose,
  onPick,
}: {
  open: boolean;
  onClose: () => void;
  onPick: (pick: RefPick) => void;
}) {
  const [tab, setTab] = useState<TabKey>("task");
  const [search, setSearch] = useState("");
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Insertar referencia</DialogTitle>
          <DialogDescription>El elemento mostrará el título real y se abrirá con doble click en su módulo.</DialogDescription>
        </DialogHeader>
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input className="h-8 pl-8 text-xs" placeholder="Buscar…" value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <Tabs value={tab} onValueChange={(v) => setTab(v as TabKey)}>
          <TabsList className="flex-wrap">
            {(Object.keys(TAB_META) as TabKey[]).map((k) => (
              <TabsTrigger key={k} value={k}>
                {(() => {
                  const Icon = TAB_META[k].icon;
                  return <Icon />;
                })()}
                {TAB_META[k].label}
              </TabsTrigger>
            ))}
          </TabsList>
          {(Object.keys(TAB_META) as TabKey[]).map((k) => (
            <TabsContent key={k} value={k}>
              <TabBody tab={k} active={open && tab === k} search={search} onPick={(p) => { onPick(p); onClose(); }} />
            </TabsContent>
          ))}
        </Tabs>
        <div className="flex justify-end">
          <Button variant="outline" size="sm" onClick={onClose}>Cerrar</Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
