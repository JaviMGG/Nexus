import { Check, Palette } from "lucide-react";
import { PALETTES } from "@/lib/types";
import { useThemeStore } from "@/stores/theme";
import { cn } from "@/lib/utils";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export function PaletteSwitcher({ compact = false }: { compact?: boolean }) {
  const palette = useThemeStore((s) => s.palette);
  const setPalette = useThemeStore((s) => s.setPalette);
  const current = PALETTES.find((p) => p.id === palette) ?? PALETTES[0];

  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <button
              className={cn(
                "flex items-center gap-2 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-sidebar-accent hover:text-foreground transition-colors cursor-pointer outline-none",
                compact && "h-7 w-7 justify-center rounded-md p-0",
              )}
            >
              <span className="flex h-4 w-4 items-center justify-center">
                <span className="h-3 w-3 rounded-full border border-black/20" style={{ background: current.swatch }} />
              </span>
              {!compact && <span>Paleta</span>}
              {!compact && <Palette className="h-3 w-3 ml-auto opacity-50" />}
            </button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent side="top">Cambiar paleta de colores</TooltipContent>
      </Tooltip>
      <DropdownMenuContent side="top" align="start" className="w-52 max-h-[60vh] overflow-y-auto">
        <DropdownMenuLabel>Básicas</DropdownMenuLabel>
        {PALETTES.filter((p) => p.group === "basicas").map((p) => (
          <DropdownMenuItem key={p.id} onClick={() => setPalette(p.id)}>
            <span className="h-3.5 w-3.5 rounded-full border border-black/20" style={{ background: p.swatch }} />
            <span className="flex-1">{p.name}</span>
            {palette === p.id && <Check className="h-3.5 w-3.5 text-primary" />}
          </DropdownMenuItem>
        ))}
        <DropdownMenuLabel>Avanzados</DropdownMenuLabel>
        {PALETTES.filter((p) => p.group === "avanzados").map((p) => (
          <DropdownMenuItem key={p.id} onClick={() => setPalette(p.id)}>
            <span className="h-3.5 w-3.5 rounded-full border border-black/20" style={{ background: p.swatch }} />
            <span className="flex-1">{p.name}</span>
            {palette === p.id && <Check className="h-3.5 w-3.5 text-primary" />}
          </DropdownMenuItem>
        ))}
        <DropdownMenuLabel>Otros</DropdownMenuLabel>
        {PALETTES.filter((p) => p.group === "otros").map((p) => (
          <DropdownMenuItem key={p.id} onClick={() => setPalette(p.id)}>
            <span className="h-3.5 w-3.5 rounded-full border border-black/20" style={{ background: p.swatch }} />
            <span className="flex-1">{p.name}</span>
            {palette === p.id && <Check className="h-3.5 w-3.5 text-primary" />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
