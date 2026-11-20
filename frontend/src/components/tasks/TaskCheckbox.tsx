import { Check } from "lucide-react";
import { cn } from "@/lib/utils";

export function TaskCheckbox({
  checked,
  onChange,
  className,
  disabled,
}: {
  checked: boolean;
  onChange: () => void;
  className?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={checked ? "Marcar como pendiente" : "Completar tarea"}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange();
      }}
      className={cn(
        "flex size-4 shrink-0 items-center justify-center rounded-full border transition-all duration-150 cursor-pointer",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        checked
          ? "border-primary bg-primary text-primary-foreground"
          : "border-muted-foreground/40 hover:border-primary hover:bg-primary/10",
        disabled && "opacity-50 pointer-events-none",
        className,
      )}
    >
      {checked && <Check className="size-3" strokeWidth={3} />}
    </button>
  );
}
