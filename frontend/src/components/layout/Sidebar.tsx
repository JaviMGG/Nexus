import { NavLink } from "react-router-dom";
import {
  Activity,
  BookOpen,
  Bot,
  CalendarDays,
  CheckSquare,
  FolderKanban,
  Inbox,
  Info,
  LayoutDashboard,
  MessagesSquare,
  Moon,
  Settings,
  Sun,
  Workflow,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { useThemeStore } from "@/stores/theme";
import { useUiStore } from "@/stores/ui";
import { AiStatusPill } from "./AiStatusPill";
import { PaletteSwitcher } from "./PaletteSwitcher";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
}

const NAV_GROUPS: { title: string; items: NavItem[] }[] = [
  {
    title: "General",
    items: [
      { to: "/", label: "Inicio", icon: LayoutDashboard },
      { to: "/inbox", label: "Bandeja de entrada", icon: Inbox },
    ],
  },
  {
    title: "Organización",
    items: [
      { to: "/tasks", label: "Tareas", icon: CheckSquare },
      { to: "/projects", label: "Proyectos", icon: FolderKanban },
      { to: "/calendar", label: "Calendario", icon: CalendarDays },
      { to: "/knowledge", label: "Conocimiento", icon: BookOpen },
    ],
  },
  {
    title: "Asistentes",
    items: [
      { to: "/chat", label: "Chat IA", icon: MessagesSquare },
      { to: "/agents", label: "Agentes", icon: Bot },
      { to: "/automations", label: "Automatizaciones", icon: Workflow },
    ],
  },
  {
    title: "Sistema",
    items: [
      { to: "/activity", label: "Actividad", icon: Activity },
      { to: "/about", label: "Acerca de", icon: Info },
      { to: "/settings", label: "Ajustes", icon: Settings },
    ],
  },
];

export function Sidebar() {
  const collapsed = useUiStore((s) => s.sidebarCollapsed);
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  return (
    <aside
      className={cn(
        "flex h-full flex-col border-r border-sidebar-border bg-sidebar transition-all duration-200",
        collapsed ? "w-[60px]" : "w-[228px]",
      )}
    >
      <div className={cn("flex items-center gap-2.5 px-4 h-14 shrink-0", collapsed && "justify-center px-0")}>
        <img src="/logo.png" alt="NEXUS" className="h-7 w-7 shrink-0 rounded-lg object-cover shadow-sm" />
        {!collapsed && (
          <span className="text-sm font-bold tracking-[0.18em] text-foreground">NEXUS</span>
        )}
      </div>

      <nav className="flex-1 overflow-y-auto px-2.5 py-2">
        {NAV_GROUPS.map((group, gi) => (
          <div key={group.title} className={cn(gi > 0 && "mt-3")}>
            {!collapsed ? (
              <p className="mb-1 px-2.5 text-[10px] font-semibold uppercase tracking-wider text-sidebar-foreground/50">
                {group.title}
              </p>
            ) : (
              gi > 0 && <div className="mx-2 mb-2 border-t border-sidebar-border" />
            )}
            <div className="space-y-0.5">
              {group.items.map(({ to, label, icon: Icon }) => {
                const link = (
                  <NavLink
                    key={to}
                    to={to}
                    end={to === "/"}
                    className={({ isActive }) =>
                      cn(
                        "group flex items-center gap-2.5 rounded-md px-2.5 py-2 text-[13px] font-medium transition-colors",
                        collapsed && "justify-center px-0",
                        isActive
                          ? "bg-sidebar-accent text-foreground"
                          : "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-foreground",
                      )
                    }
                  >
                    {({ isActive }) => (
                      <>
                        <Icon className={cn("h-4 w-4 shrink-0", isActive && "text-primary")} />
                        {!collapsed && <span className="truncate">{label}</span>}
                      </>
                    )}
                  </NavLink>
                );
                return collapsed ? (
                  <Tooltip key={to}>
                    <TooltipTrigger asChild>{link}</TooltipTrigger>
                    <TooltipContent side="right">{label}</TooltipContent>
                  </Tooltip>
                ) : (
                  link
                );
              })}
            </div>
          </div>
        ))}
      </nav>

      <div className={cn("border-t border-sidebar-border p-2.5 space-y-1", collapsed && "px-1.5")}>
        <div className={cn("flex items-center", collapsed ? "justify-center" : "justify-between")}>
          <AiStatusPill compact={collapsed} />
          {!collapsed && (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
                  className="rounded-md p-1.5 text-muted-foreground hover:bg-sidebar-accent hover:text-foreground transition-colors cursor-pointer"
                  aria-label="Cambiar tema"
                >
                  {theme === "dark" ? <Sun className="h-3.5 w-3.5" /> : <Moon className="h-3.5 w-3.5" />}
                </button>
              </TooltipTrigger>
              <TooltipContent side="top">{theme === "dark" ? "Modo claro" : "Modo oscuro"}</TooltipContent>
            </Tooltip>
          )}
        </div>
        <PaletteSwitcher compact={collapsed} />
      </div>
    </aside>
  );
}
