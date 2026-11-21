import { useLocation } from "react-router-dom";
import { PanelLeft, Plus } from "lucide-react";
import { useUiStore } from "@/stores/ui";
import { Button } from "@/components/ui/button";

const TITLES: Record<string, string> = {
  "/": "Inicio",
  "/inbox": "Bandeja de entrada",
  "/tasks": "Tareas",
  "/projects": "Proyectos",
  "/calendar": "Calendario",
  "/knowledge": "Conocimiento",
  "/chat": "Chat IA",
  "/agents": "Agentes",
  "/activity": "Actividad",
  "/automations": "Automatizaciones",
  "/settings": "Ajustes",
  "/about": "Acerca de",
};

export function Topbar() {
  const { pathname } = useLocation();
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const setQuickCaptureOpen = useUiStore((s) => s.setQuickCaptureOpen);
  const title = TITLES[pathname] ?? (pathname.startsWith("/projects/") ? "Proyecto" : pathname.startsWith("/chat") ? "Chat IA" : "NEXUS");

  return (
    <header className="flex h-14 shrink-0 items-center gap-3 border-b border-border px-4">
      <Button variant="ghost" size="icon-sm" onClick={toggleSidebar} aria-label="Alternar sidebar">
        <PanelLeft />
      </Button>
      <h1 className="text-sm font-semibold">{title}</h1>
      <div className="ml-auto flex items-center gap-2">
        <Button variant="outline" size="sm" onClick={() => setQuickCaptureOpen(true)}>
          <Plus />
          Captura rápida
          <kbd className="ml-1 rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">⌘K</kbd>
        </Button>
      </div>
    </header>
  );
}
