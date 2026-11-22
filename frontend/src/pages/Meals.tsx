import { BookOpen, CalendarDays, Package, ShoppingCart } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { RecipesTab } from "@/components/meals/RecipesTab";
import { WeekTab } from "@/components/meals/WeekTab";
import { ShoppingTab } from "@/components/meals/ShoppingTab";
import { PantryTab } from "@/components/meals/PantryTab";

export function MealsPage() {
  return (
    <div className="mx-auto max-w-[1280px] p-6 animate-fade-in">
      <div className="mb-4">
        <h1 className="text-lg font-semibold tracking-tight">Cocina</h1>
        <p className="text-xs text-muted-foreground">
          Tu recetario, el menú de la semana, la lista de la compra y la despensa. La IA es opcional.
        </p>
      </div>
      <Tabs defaultValue="recipes">
        <TabsList>
          <TabsTrigger value="recipes"><BookOpen /> Recetario</TabsTrigger>
          <TabsTrigger value="week"><CalendarDays /> Semana</TabsTrigger>
          <TabsTrigger value="shopping"><ShoppingCart /> Compra</TabsTrigger>
          <TabsTrigger value="pantry"><Package /> Despensa</TabsTrigger>
        </TabsList>
        <TabsContent value="recipes"><RecipesTab /></TabsContent>
        <TabsContent value="week"><WeekTab /></TabsContent>
        <TabsContent value="shopping"><ShoppingTab /></TabsContent>
        <TabsContent value="pantry"><PantryTab /></TabsContent>
      </Tabs>
    </div>
  );
}
