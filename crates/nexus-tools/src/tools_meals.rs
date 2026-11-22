//! Herramientas de escritura del Meal Planner: crear recetas, generar
//! listas de la compra, añadir artículos y ajustar la despensa.

use std::sync::Arc;

use nexus_core::{CreateRecipe, CreateShoppingItem, NexusError, Result, Risk};
use serde_json::{json, Value};

use crate::builtins::{define_tool, dto, enum_prop, req_str, schema, val};
use crate::{Tool, ToolContext};

define_tool!(
    MealsRecipeCreate,
    "meals.recipe_create",
    Risk::Write,
    "Crea una receta real en el recetario. Los ingredientes llevan cantidad y unidad reales (g, ml, ud…); no se estiman datos nutricionales.",
    schema(
        json!({
            "name": { "type": "string" },
            "description": { "type": "string" },
            "servings": { "type": "integer" },
            "prep_time_min": { "type": "integer" },
            "cook_time_min": { "type": "integer" },
            "difficulty": enum_prop(&["facil", "media", "dificil"]),
            "categories": { "type": "array", "items": { "type": "string" } },
            "tags": { "type": "array", "items": { "type": "string" } },
            "ingredients": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "quantity": { "type": "number" },
                        "unit": { "type": "string" },
                        "category": { "type": "string" },
                        "notes": { "type": "string" }
                    },
                    "required": ["name", "quantity", "unit"]
                }
            },
            "steps": { "type": "array", "items": { "type": "string" } }
        }),
        &["name"]
    ),
    |args, ctx| {
        let mut args = args;
        if let Some(obj) = args.as_object_mut() {
            obj.remove("nutrition");
            obj.remove("image_file_id");
            obj.remove("favorite");
        }
        let data: CreateRecipe = dto(args)?;
        val(ctx.services.meals.create_recipe(data, &ctx.actor()).await?)
    }
);

define_tool!(
    MealsShoppingGenerate,
    "meals.shopping_generate",
    Risk::Write,
    "Genera la lista de la compra consolidando los ingredientes del menú semanal. La semana se normaliza al lunes.",
    schema(
        json!({
            "week_start": { "type": "string", "description": "YYYY-MM-DD de cualquier día de la semana" },
            "name": { "type": "string" },
            "list_id": { "type": "string", "description": "si se indica, regenera esa lista existente conservando artículos manuales" }
        }),
        &["week_start"]
    ),
    |args, ctx| {
        let week_start = req_str(&args, "week_start")?;
        let name = args.get("name").and_then(Value::as_str);
        let list_id = args.get("list_id").and_then(Value::as_str);
        val(ctx
            .services
            .meals
            .generate_shopping_list(&week_start, name, list_id, &ctx.actor())
            .await?)
    }
);

define_tool!(
    MealsShoppingAddItem,
    "meals.shopping_add_item",
    Risk::Write,
    "Añade un artículo manual a una lista de la compra existente. Cantidad por defecto 1, unidad 'ud', categoría 'otros'.",
    schema(
        json!({
            "list_id": { "type": "string" },
            "name": { "type": "string" },
            "quantity": { "type": "number" },
            "unit": { "type": "string" },
            "category": { "type": "string" }
        }),
        &["list_id", "name"]
    ),
    |args, ctx| {
        let list_id = req_str(&args, "list_id")?;
        let mut args = args;
        if let Some(obj) = args.as_object_mut() {
            obj.remove("list_id");
        }
        let data: CreateShoppingItem = dto(args)?;
        val(ctx
            .services
            .meals
            .add_shopping_item(&list_id, data, &ctx.actor())
            .await?)
    }
);

define_tool!(
    MealsPantryAdjust,
    "meals.pantry_adjust",
    Risk::Write,
    "Ajusta la cantidad de un artículo de la despensa sumando un delta (positivo o negativo). La cantidad nunca baja de 0.",
    schema(
        json!({
            "id": { "type": "string" },
            "delta": { "type": "number", "description": "cantidad a sumar (positivo) o restar (negativo); nunca baja de 0" }
        }),
        &["id", "delta"]
    ),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let delta = args
            .get("delta")
            .and_then(Value::as_f64)
            .ok_or_else(|| NexusError::validation("falta 'delta' (número)"))?;
        val(ctx.services.meals.adjust_pantry(&id, delta, &ctx.actor()).await?)
    }
);

pub(crate) fn tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(MealsRecipeCreate),
        Arc::new(MealsShoppingGenerate),
        Arc::new(MealsShoppingAddItem),
        Arc::new(MealsPantryAdjust),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_args, ToolRegistry};
    use nexus_core::CreatePantryItem;
    use nexus_db::Db;
    use nexus_services::Services;

    async fn ctx() -> ToolContext {
        ToolContext {
            services: Services::new(Db::connect_memory().await.unwrap()),
            run_id: "run-test".into(),
            agent_id: "executor".into(),
            automation: None,
        }
    }

    #[tokio::test]
    async fn recipe_create_con_ingredientes_reales() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("meals.recipe_create").unwrap();
        let args = json!({
            "name": "Tortilla de patatas",
            "servings": 4,
            "prep_time_min": 15,
            "cook_time_min": 20,
            "difficulty": "media",
            "ingredients": [
                { "name": "Patata", "quantity": 600, "unit": "g" },
                { "name": "Huevo", "quantity": 5, "unit": "ud" },
                { "name": "Aceite de oliva", "quantity": 100, "unit": "ml", "category": "aceites" }
            ],
            "steps": ["Pelar y freír las patatas", "Batir los huevos", "Cuajar la tortilla"],
            "nutrition": { "kcal": 999, "protein_g": 1, "carbs_g": 1, "fat_g": 1 }
        });
        validate_args(&create.spec(), &args).unwrap();
        let created = create.execute(args, &ctx).await.unwrap();
        assert_eq!(created["name"], "Tortilla de patatas");
        assert!(created["nutrition"].is_null());

        let recipe = ctx
            .services
            .meals
            .get_recipe(created["id"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(recipe.ingredients.len(), 3);
        assert_eq!(recipe.ingredients[0].name, "Patata");
        assert_eq!(recipe.ingredients[0].quantity, 600.0);
        assert_eq!(recipe.ingredients[0].unit, "g");
        assert_eq!(recipe.ingredients[2].category, "aceites");
        assert!(recipe.nutrition.is_none());
    }

    #[tokio::test]
    async fn shopping_generate_y_add_item() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let generate = registry.get("meals.shopping_generate").unwrap();
        let args = json!({ "week_start": "2026-11-18", "name": "Compra semanal" });
        validate_args(&generate.spec(), &args).unwrap();
        let generated = generate.execute(args, &ctx).await.unwrap();
        let list_id = generated["list"]["id"].as_str().unwrap().to_string();
        assert_eq!(generated["list"]["week_start"], "2026-11-16");
        assert_eq!(generated["items"].as_array().unwrap().len(), 0);

        let add = registry.get("meals.shopping_add_item").unwrap();
        let args = json!({ "list_id": list_id, "name": "Pan", "quantity": 2, "unit": "ud" });
        validate_args(&add.spec(), &args).unwrap();
        let item = add.execute(args, &ctx).await.unwrap();
        assert_eq!(item["name"], "Pan");
        assert_eq!(item["quantity"], 2.0);
        assert_eq!(item["manual"], true);

        let detail = ctx.services.meals.get_shopping_list(&list_id).await.unwrap();
        assert_eq!(detail.items.len(), 1);
        assert_eq!(detail.items[0].name, "Pan");
    }

    #[tokio::test]
    async fn pantry_adjust_nunca_baja_de_cero() {
        let ctx = ctx().await;
        let item = ctx
            .services
            .meals
            .create_pantry_item(
                CreatePantryItem {
                    name: "Arroz".into(),
                    quantity: Some(2.0),
                    unit: Some("kg".into()),
                    ..Default::default()
                },
                "user",
            )
            .await
            .unwrap();
        let registry = ToolRegistry::with_defaults();
        let adjust = registry.get("meals.pantry_adjust").unwrap();
        let args = json!({ "id": item.id, "delta": -1 });
        validate_args(&adjust.spec(), &args).unwrap();
        let out = adjust.execute(args, &ctx).await.unwrap();
        assert_eq!(out["quantity"], 1.0);

        let out = adjust
            .execute(json!({ "id": item.id, "delta": -5 }), &ctx)
            .await
            .unwrap();
        assert_eq!(out["quantity"], 0.0);
    }

    #[tokio::test]
    async fn validate_args_rechaza_campos_obligatorios() {
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("meals.recipe_create").unwrap();
        let err = validate_args(&create.spec(), &json!({})).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));

        let adjust = registry.get("meals.pantry_adjust").unwrap();
        let err = validate_args(&adjust.spec(), &json!({ "id": "x" })).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
    }

    #[test]
    fn todas_las_tools_son_write() {
        for tool in tools() {
            assert_eq!(tool.risk(), Risk::Write);
        }
    }
}