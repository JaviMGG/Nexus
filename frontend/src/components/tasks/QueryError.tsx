import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";

export function QueryError({ error, onRetry }: { error: Error; onRetry: () => void }) {
  return (
    <EmptyState
      icon={AlertTriangle}
      title="No se pudieron cargar los datos"
      description={error.message}
      action={
        <Button variant="outline" size="sm" onClick={onRetry}>
          Reintentar
        </Button>
      }
    />
  );
}
