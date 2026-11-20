import { create } from "zustand";
import { persist } from "zustand/middleware";
import { invoke } from "@/lib/api";
import type { PaletteId } from "@/lib/types";

export type ThemeMode = "dark" | "light";

interface ThemeState {
  theme: ThemeMode;
  palette: PaletteId;
  setTheme: (t: ThemeMode) => void;
  setPalette: (p: PaletteId) => void;
  syncFromBackend: (theme: string, palette: string) => void;
}

function apply(theme: ThemeMode, palette: string) {
  const el = document.documentElement;
  el.classList.toggle("dark", theme === "dark");
  el.classList.toggle("light", theme === "light");
  el.dataset.palette = palette;
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: "dark",
      palette: "violet",
      setTheme: (theme) => {
        set({ theme });
        apply(theme, get().palette);
        void invoke("settings.update", { patch: { theme } }).catch(() => {});
      },
      setPalette: (palette) => {
        set({ palette });
        apply(get().theme, palette);
        void invoke("settings.update", { patch: { palette } }).catch(() => {});
      },
      syncFromBackend: (theme, palette) => {
        const t: ThemeMode = theme === "light" ? "light" : "dark";
        const p = (palette || "violet") as PaletteId;
        set({ theme: t, palette: p });
        apply(t, p);
      },
    }),
    { name: "nexus-theme" },
  ),
);
