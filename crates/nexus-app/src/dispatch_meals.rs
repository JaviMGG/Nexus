//! Comandos del módulo meals: recetario, plan semanal, menús, compra,
//! despensa e IA opcional (siempre propuestas editables que confirma el usuario).

use nexus_core::{
    monday_of, MealSlot, NexusError, PantryFilter, RecipeFilter, Result, UpdatePantryItem,
    UpdateRecipe, UpdateShoppingItem,
};
use serde_json::{json, Value};

use crate::context::AppContext;
use crate::dispatch::{arg, arg_or_default, ok, opt_arg};

pub async fn dispatch(ctx: &AppContext, command: &str, payload: Value) -> Result<Value> {
    let s = &ctx.services;
    let m = &s.meals;
    match command {
        // ----------------------------------------------------------- recetario
        "recipes.list" => ok(m.list_recipes(arg_or_default(&payload, "filter")?).await?),
        "recipes.get" => ok(m.get_recipe(&arg::<String>(&payload, "id")?).await?),
        "recipes.create" => {
            let r = m.create_recipe(arg(&payload, "data")?, "user").await?;
            ctx.data_changed("meal");
            ok(r)
        }
        "recipes.update" => {
            let id: String = arg(&payload, "id")?;
            let r = m
                .update_recipe(&id, arg::<UpdateRecipe>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(r)
        }
        "recipes.delete" => {
            m.delete_recipe(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(json!({"deleted": true}))
        }
        "recipes.duplicate" => {
            let r = m
                .duplicate_recipe(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(r)
        }
        "recipes.toggle_favorite" => {
            let r = m
                .toggle_favorite(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(r)
        }
        "recipe_preview" => ok(m.recipe_preview(&arg::<String>(&payload, "id")?).await?),

        // ----------------------------------------------------------- plan semanal
        "plan.get" => {
            let date: String = arg(&payload, "week_start")?;
            ok(m.week_plan(&date).await?)
        }
        "plan.add" => {
            let e = m
                .add_plan_entry(
                    &arg::<String>(&payload, "date")?,
                    arg::<MealSlot>(&payload, "slot")?,
                    &arg::<String>(&payload, "recipe_id")?,
                    opt_arg(&payload, "servings")?.unwrap_or(2),
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(e)
        }
        "plan.update" => {
            let id: String = arg(&payload, "id")?;
            let date: Option<String> = opt_arg(&payload, "date")?;
            let slot: Option<MealSlot> = opt_arg(&payload, "slot")?;
            let servings: Option<i64> = opt_arg(&payload, "servings")?;
            let e = m
                .update_plan_entry(&id, date.as_deref(), slot, servings, "user")
                .await?;
            ctx.data_changed("meal");
            ok(e)
        }
        "plan.move" => {
            let e = m
                .move_plan_entry(
                    &arg::<String>(&payload, "id")?,
                    &arg::<String>(&payload, "date")?,
                    arg::<MealSlot>(&payload, "slot")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(e)
        }
        "plan.duplicate" => {
            let e = m
                .duplicate_plan_entry(
                    &arg::<String>(&payload, "id")?,
                    &arg::<String>(&payload, "date")?,
                    arg::<MealSlot>(&payload, "slot")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(e)
        }
        "plan.delete" => {
            m.delete_plan_entry(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(json!({"deleted": true}))
        }
        "plan.apply" => {
            let week_start: String = arg(&payload, "week_start")?;
            let entries: Vec<nexus_core::PlanEntryInput> = arg(&payload, "entries")?;
            let replace = opt_arg(&payload, "replace")?.unwrap_or(false);
            let created = m
                .apply_plan_entries(
                    &week_start,
                    entries,
                    replace,
                    "user",
                    "Menú aplicado en bloque",
                )
                .await?;
            ctx.data_changed("meal");
            ok(created)
        }

        // ----------------------------------------------------------- menús plantilla
        "menus.list" => ok(m.list_menu_templates().await?),
        "menus.get" => ok(m.get_menu_template(&arg::<String>(&payload, "id")?).await?),
        "menus.save" => {
            let t = m
                .save_menu_template(
                    &arg::<String>(&payload, "name")?,
                    &arg::<String>(&payload, "week_start")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(t)
        }
        "menus.rename" => {
            let t = m
                .rename_menu_template(
                    &arg::<String>(&payload, "id")?,
                    &arg::<String>(&payload, "name")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(t)
        }
        "menus.delete" => {
            m.delete_menu_template(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(json!({"deleted": true}))
        }
        "menus.apply" => {
            let (entries, skipped) = m
                .apply_menu_template(
                    &arg::<String>(&payload, "id")?,
                    &arg::<String>(&payload, "week_start")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(json!({"entries": entries, "skipped_recipe_ids": skipped}))
        }

        // ----------------------------------------------------------- compra
        "shopping.generate" => {
            let week_start: String = arg(&payload, "week_start")?;
            let name: Option<String> = opt_arg(&payload, "name")?;
            let list_id: Option<String> = opt_arg(&payload, "list_id")?;
            let detail = m
                .generate_shopping_list(&week_start, name.as_deref(), list_id.as_deref(), "user")
                .await?;
            ctx.data_changed("meal");
            ok(detail)
        }
        "shopping.lists" => ok(m.list_shopping_lists().await?),
        "shopping.get" => ok(m.get_shopping_list(&arg::<String>(&payload, "id")?).await?),
        "shopping.rename" => {
            let l = m
                .rename_shopping_list(
                    &arg::<String>(&payload, "id")?,
                    &arg::<String>(&payload, "name")?,
                )
                .await?;
            ctx.data_changed("meal");
            ok(l)
        }
        "shopping.delete" => {
            m.delete_shopping_list(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(json!({"deleted": true}))
        }
        "shopping.add_item" => {
            let item = m
                .add_shopping_item(
                    &arg::<String>(&payload, "list_id")?,
                    arg(&payload, "data")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(item)
        }
        "shopping.update_item" => {
            let item = m
                .update_shopping_item(
                    &arg::<String>(&payload, "id")?,
                    arg::<UpdateShoppingItem>(&payload, "data")?,
                )
                .await?;
            ctx.data_changed("meal");
            ok(item)
        }
        "shopping.delete_item" => {
            m.delete_shopping_item(&arg::<String>(&payload, "id")?)
                .await?;
            ctx.data_changed("meal");
            ok(json!({"deleted": true}))
        }
        "shopping.export" => {
            let format = opt_arg(&payload, "format")?.unwrap_or_else(|| "txt".to_string());
            ok(
                m.export_shopping_list(&arg::<String>(&payload, "id")?, &format)
                    .await?,
            )
        }
        "shopping.copy_text" => {
            let export = m
                .export_shopping_list(&arg::<String>(&payload, "id")?, "txt")
                .await?;
            ok(json!({"content": export.content}))
        }

        // ----------------------------------------------------------- despensa
        "pantry.list" => ok(m
            .list_pantry(arg_or_default::<PantryFilter>(&payload, "filter")?)
            .await?),
        "pantry.create" => {
            let item = m.create_pantry_item(arg(&payload, "data")?, "user").await?;
            ctx.data_changed("meal");
            ok(item)
        }
        "pantry.update" => {
            let item = m
                .update_pantry_item(
                    &arg::<String>(&payload, "id")?,
                    arg::<UpdatePantryItem>(&payload, "data")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(item)
        }
        "pantry.adjust" => {
            let item = m
                .adjust_pantry(
                    &arg::<String>(&payload, "id")?,
                    arg::<f64>(&payload, "delta")?,
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(item)
        }
        "pantry.consume" => {
            let result = m
                .consume_pantry(
                    &arg::<String>(&payload, "id")?,
                    arg::<f64>(&payload, "amount")?,
                    opt_arg(&payload, "remove_if_empty")?.unwrap_or(false),
                    "user",
                )
                .await?;
            ctx.data_changed("meal");
            ok(result)
        }
        "pantry.delete" => {
            m.delete_pantry_item(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("meal");
            ok(json!({"deleted": true}))
        }
        "pantry_matches" => ok(m.pantry_matches().await?),

        // ----------------------------------------------------------- IA opcional
        "ai_recipe" => {
            let ingredients: Option<String> = opt_arg(&payload, "ingredients")?;
            let description: Option<String> = opt_arg(&payload, "description")?;
            let user = match (ingredients, description) {
                (Some(i), _) if !i.trim().is_empty() => {
                    format!("Crea una receta con estos ingredientes: {i}")
                }
                (_, Some(d)) if !d.trim().is_empty() => {
                    format!("Crea una receta a partir de esta descripción: {d}")
                }
                _ => {
                    return Err(NexusError::validation(
                        "indica ingredientes o una descripción para la receta",
                    ))
                }
            };
            let system = "Eres un chef que propone recetas para NEXUS Meal Planner. \
Responde SOLO con un JSON válido con esta forma exacta: \
{\"name\": string, \"description\": string, \"servings\": int>=1, \"prep_time_min\": int>=0, \
\"cook_time_min\": int>=0, \"difficulty\": \"facil\"|\"media\"|\"dificil\", \"categories\": [string], \
\"tags\": [string], \"ingredients\": [{\"name\": string, \"quantity\": number>0, \"unit\": string \
(g, kg, ml, l, ud, cda, cdta…), \"category\": string (verduras, frutas, carne, pescado, lácteos, \
despensa, panadería, congelados, bebidas, otros), \"notes\": string}], \"steps\": [string]}. \
Todo el texto en español. NO incluyas nutrición ni valores nutricionales: está prohibido inventarlos.";
            let value = crate::ai_json::ask_json(ctx, system, &user, 2_000).await?;
            let proposal = nexus_services::meals::MealService::sanitize_ai_recipe(&value)?;
            ok(proposal)
        }
        "ai_week" => {
            let week_start: String = arg(&payload, "week_start")?;
            let week_start = monday_of(&week_start)?;
            let preferences: Option<String> = opt_arg(&payload, "preferences")?;
            let recipes = m.list_recipes(RecipeFilter::default()).await?;
            if recipes.is_empty() {
                return Err(NexusError::validation(
                    "tu recetario está vacío: crea algunas recetas antes de pedir un menú con IA",
                ));
            }
            let catalog: Vec<Value> = recipes
                .iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "name": r.name,
                        "servings": r.servings,
                        "total_time_min": r.total_time_min(),
                        "categories": r.categories,
                    })
                })
                .collect();
            let system = "Eres un planificador de menús semanales para NEXUS Meal Planner. \
Solo puedes usar recetas EXISTENTES del catálogo que te doy (usa su \"id\" exacto). \
Responde SOLO con un JSON válido: {\"entries\": [{\"day_offset\": int 0..6 (0=lunes), \
\"slot\": \"desayuno\"|\"almuerzo\"|\"comida\"|\"merienda\"|\"cena\", \"recipe_id\": string del catálogo, \
\"servings\": int>=1}]}. No inventes recetas ni ids. Varía los platos a lo largo de la semana.";
            let user = format!(
                "Catálogo de recetas: {}\nPreferencias del usuario: {}",
                Value::Array(catalog),
                preferences.unwrap_or_else(|| "menú equilibrado y variado".into())
            );
            let value = crate::ai_json::ask_json(ctx, system, &user, 2_500).await?;
            let entries = m.sanitize_ai_week(&week_start, &value).await?;
            ok(json!({"week_start": week_start, "entries": entries}))
        }
        "ai_substitute" => {
            let recipe = m.get_recipe(&arg::<String>(&payload, "recipe_id")?).await?;
            let ingredient: String = arg(&payload, "ingredient")?;
            if ingredient.trim().is_empty() {
                return Err(NexusError::validation("indica el ingrediente a sustituir"));
            }
            let others: Vec<String> = recipe
                .ingredients
                .iter()
                .map(|i| format!("{} {} {}", i.quantity, i.unit, i.name))
                .collect();
            let system = "Eres un chef ayudando con sustituciones de ingredientes. \
Responde SOLO con un JSON válido: {\"suggestions\": [string, …]}. \
Da entre 3 y 5 sustituciones concretas en español, cada una con cantidad orientativa \
y una nota breve de cómo cambia el resultado. No des información nutricional.";
            let user = format!(
                "Receta: {}\nIngredientes: {}\nQuiero sustituir: {ingredient}",
                recipe.name,
                others.join(", ")
            );
            let value = crate::ai_json::ask_json(ctx, system, &user, 800).await?;
            let suggestions = match value.get("suggestions") {
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>(),
                Some(Value::String(s)) => vec![s.clone()],
                _ => {
                    return Err(NexusError::Ai(
                        "la IA no devolvió sugerencias válidas; inténtalo de nuevo".into(),
                    ))
                }
            };
            ok(json!({"suggestions": suggestions}))
        }
        "ai_summary" => {
            let week_start: String = arg(&payload, "week_start")?;
            let plan = m.week_plan(&week_start).await?;
            if plan.entries.is_empty() {
                return Err(NexusError::validation(
                    "esa semana no tiene comidas planificadas",
                ));
            }
            let entries: Vec<Value> = plan
                .entries
                .iter()
                .map(|e| {
                    json!({
                        "date": e.entry.date,
                        "slot": e.entry.slot.as_str(),
                        "recipe": e.recipe_name,
                        "servings": e.entry.servings,
                        "total_time_min": e.total_time_min,
                    })
                })
                .collect();
            let system = "Eres el asistente de NEXUS Meal Planner. Responde SOLO con un JSON \
válido: {\"markdown\": string}. El markdown debe ser un resumen breve y cercano en español \
de la semana de comidas: platos destacados, equilibrio general y algún consejo práctico. \
NO inventes datos nutricionales ni calorías.";
            let user = format!(
                "Semana del {}. Comidas planificadas: {}",
                plan.week_start,
                Value::Array(entries)
            );
            let value = crate::ai_json::ask_json(ctx, system, &user, 1_500).await?;
            let markdown = value
                .get("markdown")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    NexusError::Ai("la IA no devolvió un resumen válido; inténtalo de nuevo".into())
                })?;
            ok(json!({"markdown": markdown}))
        }

        other => Err(NexusError::NotFound(format!(
            "comando desconocido: {other}"
        ))),
    }
}
