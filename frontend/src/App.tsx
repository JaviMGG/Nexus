import { useEffect } from "react";
import { HashRouter, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";
import { AppShell } from "@/components/layout/AppShell";
import { useAppEvents } from "@/hooks/useAppEvents";
import { invoke } from "@/lib/api";
import type { SettingsPayload } from "@/lib/types";
import { useThemeStore } from "@/stores/theme";
import { Dashboard } from "@/pages/Dashboard";
import { InboxPage } from "@/pages/Inbox";
import { TasksPage } from "@/pages/Tasks";
import { ProjectsPage } from "@/pages/Projects";
import { ProjectDetailPage } from "@/pages/ProjectDetail";
import { CalendarPage } from "@/pages/Calendar";
import { KnowledgePage } from "@/pages/Knowledge";
import { ChatPage } from "@/pages/Chat";
import { AgentsPage } from "@/pages/Agents";
import { ActivityPage } from "@/pages/Activity";
import { AutomationsPage } from "@/pages/Automations";
import { AboutPage } from "@/pages/About";
import { CanvasPage } from "@/pages/Canvas";
import { DatabasesPage } from "@/pages/Databases";
import { MealsPage } from "@/pages/Meals";
import { SettingsPage } from "@/pages/Settings";

export default function App() {
  useAppEvents();
  const syncFromBackend = useThemeStore((s) => s.syncFromBackend);
  const theme = useThemeStore((s) => s.theme);

  useEffect(() => {
    invoke<SettingsPayload>("settings.get")
      .then((s) => syncFromBackend(s.app.theme, s.app.palette))
      .catch(() => {});
  }, [syncFromBackend]);

  return (
    <>
      <HashRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<Dashboard />} />
            <Route path="/inbox" element={<InboxPage />} />
            <Route path="/tasks" element={<TasksPage />} />
            <Route path="/projects" element={<ProjectsPage />} />
            <Route path="/projects/:id" element={<ProjectDetailPage />} />
            <Route path="/calendar" element={<CalendarPage />} />
            <Route path="/knowledge" element={<KnowledgePage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/agents" element={<AgentsPage />} />
            <Route path="/activity" element={<ActivityPage />} />
            <Route path="/automations" element={<AutomationsPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/about" element={<AboutPage />} />
            <Route path="/canvas" element={<CanvasPage />} />
            <Route path="/canvas/:id" element={<CanvasPage />} />
            <Route path="/databases" element={<DatabasesPage />} />
            <Route path="/databases/:id" element={<DatabasesPage />} />
            <Route path="/meals" element={<MealsPage />} />
          </Route>
        </Routes>
      </HashRouter>
      <Toaster theme={theme} position="bottom-right" richColors closeButton />
    </>
  );
}
