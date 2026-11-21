import { Check, Moon, Sun } from "lucide-react";
import { PALETTES, type PaletteDef } from "@/lib/types";
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

function PaletteCard({ p, active, onSelect }: { p: PaletteDef; active: boolean; onSelect: () => void }) {
  const [bg, card, primary, accent] = p.preview;
  return (
    <button
      onClick={onSelect}
      aria-pressed={active}
      className={cn(
        "group relative overflow-hidden rounded-xl border bg-card p-4 text-left shadow-sm transition-all duration-200 cursor-pointer",
        active
          ? "border-ring ring-2 ring-ring/30"
          : "border-border hover:border-ring/40 hover:shadow-md",
      )}
    >
      <div
        className="relative h-16 overflow-hidden rounded-lg border border-black/10 p-2 transition-transform duration-200 group-hover:scale-[1.02]"
        style={{ backgroundColor: bg }}
      >
        <div className="flex h-full gap-1.5">
          <div className="flex w-1/4 flex-col gap-1 rounded-md p-1" style={{ backgroundColor: card }}>
            <span className="h-1 w-full rounded-full" style={{ backgroundColor: primary }} />
            <span className="h-1 w-2/3 rounded-full opacity-60" style={{ backgroundColor: accent }} />
            <span className="h-1 w-1/2 rounded-full opacity-30" style={{ backgroundColor: accent }} />
          </div>
          <div className="flex flex-1 flex-col gap-1 rounded-md p-1.5" style={{ backgroundColor: card }}>
            <span className="h-1.5 w-1/3 rounded-full" style={{ backgroundColor: primary }} />
            <span className="h-1 w-4/5 rounded-full opacity-50" style={{ backgroundColor: accent }} />
            <span className="h-1 w-3/5 rounded-full opacity-30" style={{ backgroundColor: accent }} />
            <span className="mt-auto h-2.5 w-8 rounded-sm" style={{ backgroundColor: primary }} />
          </div>
        </div>
        <span
          className={cn(
            "absolute right-1.5 top-1.5 flex h-5 w-5 items-center justify-center rounded-full shadow transition-all duration-200",
            active ? "scale-100 opacity-100" : "scale-50 opacity-0",
          )}
          style={{ backgroundColor: primary }}
        >
          <Check className="h-3 w-3" style={{ color: bg }} />
        </span>
      </div>
      <div className="mt-3 flex items-center gap-2">
        <span className="h-3 w-3 shrink-0 rounded-full border border-black/20" style={{ backgroundColor: p.swatch }} />
        <p className="text-sm font-medium">{p.name}</p>
      </div>
      {p.description && <p className="mt-0.5 text-[11px] leading-snug text-muted-foreground">{p.description}</p>}
    </button>
  );
}

export function PaletteGrid() {
  const palette = useThemeStore((s) => s.palette);
  const setPalette = useThemeStore((s) => s.setPalette);
  const basicas = PALETTES.filter((p) => p.group === "basicas");
  const avanzados = PALETTES.filter((p) => p.group === "avanzados");
  const otros = PALETTES.filter((p) => p.group === "otros");

  return (
    <div className="space-y-5">
      <div>
        <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Básicas</p>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
          {basicas.map((p) => (
            <PaletteCard key={p.id} p={p} active={palette === p.id} onSelect={() => setPalette(p.id)} />
          ))}
        </div>
      </div>
      <div>
        <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          Avanzados · temas completos
        </p>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {avanzados.map((p) => (
            <PaletteCard key={p.id} p={p} active={palette === p.id} onSelect={() => setPalette(p.id)} />
          ))}
        </div>
      </div>
      <div>
        <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">Otros</p>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {otros.map((p) => (
            <PaletteCard key={p.id} p={p} active={palette === p.id} onSelect={() => setPalette(p.id)} />
          ))}
        </div>
      </div>
    </div>
  );
}
