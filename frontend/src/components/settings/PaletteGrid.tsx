import { Check, Moon, Sun } from "lucide-react";
import { PALETTES } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useThemeStore } from "@/stores/theme";

export function ThemeToggle() {
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  return (
    <div className="inline-flex rounded-lg border border-border bg-muted p-1">
      {(["dark", "light"] as const).map((t) => (
        <button
          key={t}
          onClick={() => setTheme(t)}
          className={cn(
            "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-all cursor-pointer",
            theme === t
              ? "bg-card text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {t === "dark" ? <Moon className="h-3.5 w-3.5" /> : <Sun className="h-3.5 w-3.5" />}
          {t === "dark" ? "Oscuro" : "Claro"}
        </button>
      ))}
    </div>
  );
}

export function PaletteGrid() {
  const palette = useThemeStore((s) => s.palette);
  const setPalette = useThemeStore((s) => s.setPalette);

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
      {PALETTES.map((p) => {
        const active = palette === p.id;
        return (
          <button
            key={p.id}
            onClick={() => setPalette(p.id)}
            aria-pressed={active}
            className={cn(
              "group relative overflow-hidden rounded-xl border bg-card p-4 text-left shadow-sm transition-all duration-200 cursor-pointer",
              active
                ? "border-ring ring-2 ring-ring/30"
                : "border-border hover:border-ring/40 hover:shadow-md",
            )}
          >
            <div className="flex items-center justify-between">
              <span
                className="h-9 w-9 rounded-full shadow-sm transition-transform duration-200 group-hover:scale-105"
                style={{ backgroundColor: p.swatch }}
              />
              <span
                className={cn(
                  "flex h-5 w-5 items-center justify-center rounded-full bg-primary text-primary-foreground transition-all duration-200",
                  active ? "scale-100 opacity-100" : "scale-50 opacity-0",
                )}
              >
                <Check className="h-3 w-3" />
              </span>
            </div>
            <p className="mt-3 text-sm font-medium">{p.name}</p>
            <div className="mt-2 flex items-center gap-1.5">
              <span className="h-1.5 w-10 rounded-full bg-primary" />
              <span className="h-1.5 w-6 rounded-full bg-accent-foreground/40" />
              <span className="h-1.5 w-4 rounded-full bg-muted-foreground/30" />
            </div>
          </button>
        );
      })}
    </div>
  );
}
