import { create } from "zustand";

interface UiState {
  quickCaptureOpen: boolean;
  setQuickCaptureOpen: (open: boolean) => void;
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  quickCaptureOpen: false,
  setQuickCaptureOpen: (open) => set({ quickCaptureOpen: open }),
  sidebarCollapsed: false,
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));
