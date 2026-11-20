import { NavLink } from "react-router-dom";
import {
  Activity,
  BookOpen,
  Bot,
  CalendarDays,
  CheckSquare,
  FolderKanban,
  Inbox,
  LayoutDashboard,
  MessagesSquare,
  Moon,
  Settings,
  Sun,
  Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useThemeStore } from "@/stores/theme";
import { useUiStore } from "@/stores/ui";
import { AiStatusPill } from "./AiStatusPill";
import { PaletteSwitcher } from "./PaletteSwitcher";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

const NAV = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/inbox", label: "Inbox", icon: Inbox },
  { to: "/tasks", label: "Tareas", icon: CheckSquare },
  { to: "/projects", label: "Proyectos", icon: FolderKanban },
  { to: "/calendar", label: "Calendario", icon: CalendarDays },
  { to: "/knowledge", label: "Knowledge Base", icon: BookOpen },
  { to: "/chat", label: "Chat IA", icon: MessagesSquare },
  { to: "/agents", label: "Agentes", icon: Bot },
  { to: "/activity", label: "Actividad", icon: Activity },
  { to: "/settings", label: "Ajustes", icon: Settings },
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
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-primary shadow-sm">
          <Zap className="h-4 w-4 text-primary-foreground" />
        </div>
        {!collapsed && (
          <span className="text-sm font-bold tracking-[0.18em] text-foreground">NEXUS</span>
        )}
      </div>

      <nav className="flex-1 overflow-y-auto px-2.5 py-2 space-y-0.5">
        {NAV.map(({ to, label, icon: Icon }) => (
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
