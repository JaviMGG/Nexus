import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Loader2, Sparkles } from "lucide-react";
import { invoke } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { CreateRecipe, toastMealError } from "./shared";

/** Pide una propuesta de receta a la IA; el resultado abre el editor editable. */
export function AiRecipeDialog({
  open,
  onClose,
  onProposal,
}: {
  open: boolean;
  onClose: () => void;
  onProposal: (data: CreateRecipe) => void;
}) {
  const [ingredients, setIngredients] = useState("");
  const [description, setDescription] = useState("");

  const ask = useMutation({
    mutationFn: () =>
      invoke<CreateRecipe>("meals.ai_recipe", {
        ingredients: ingredients.trim() || null,
        description: description.trim() || null,
      }),
    onSuccess: (data) => {
      onProposal(data);
      onClose();
    },
    onError: (e) => toastMealError(e, "No se pudo generar la propuesta"),
  });

  const canAsk = ingredients.trim().length > 0 || description.trim().length > 0;

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-4 w-4 text-primary" /> Receta con IA
          </DialogTitle>
          <DialogDescription>
            La IA propone un borrador que podrás editar antes de guardar. Nunca inventa valores nutricionales.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>Ingredientes disponibles</Label>
            <Textarea
              value={ingredients}
              onChange={(e) => setIngredients(e.target.value)}
              placeholder="huevos, patatas, cebolla…"
            />
          </div>
          <div className="space-y-1.5">
            <Label>O describe la receta que quieres</Label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="una cena ligera de invierno para 2 personas…"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Cancelar</Button>
          <Button onClick={() => ask.mutate()} disabled={!canAsk || ask.isPending}>
            {ask.isPending ? <Loader2 className="animate-spin" /> : <Sparkles />}
            {ask.isPending ? "Generando…" : "Proponer receta"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
