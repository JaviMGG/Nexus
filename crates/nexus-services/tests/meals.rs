//! Tests del módulo NEXUS Meal Planner (recetas, planner, menús, compra, despensa).

use nexus_core::*;
use nexus_db::Db;
use nexus_services::meals::MealService;
use nexus_services::Services;

async fn setup() -> MealService {
    Services::new(Db::connect_memory().await.unwrap()).meals
}

fn ing(name: &str, quantity: f64, unit: &str, category: &str) -> Ingredient {
    Ingredient {
        name: name.into(),
        quantity,
        unit: unit.into(),
        category: category.into(),
        notes: String::new(),
    }
}

fn tortilla_data() -> CreateRecipe {
    CreateRecipe {
        name: "Tortilla de patatas".into(),
        description: "Clásica española".into(),
        servings: Some(2),
        prep_time_min: Some(10),
        cook_time_min: Some(20),
        difficulty: Some(RecipeDifficulty::Media),
        categories: vec!["cena".into(), "española".into()],
        tags: vec!["huevo".into()],
        ingredients: vec![
            ing("Patata", 300.0, "g", "verduras"),
            ing("Huevo", 3.0, "ud", "lácteos"),
            ing("Aceite de oliva", 2.0, "cda", "despensa"),
        ],
        steps: vec![
            "Pelar y cortar las patatas".into(),
            "Freír".into(),
            "Cuajar".into(),
        ],
        ..Default::default()
    }
}

async fn create_tortilla(m: &MealService) -> Recipe {
    m.create_recipe(tortilla_data(), "user").await.unwrap()
}

// ---------------------------------------------------------------------------
// Recetario
// ---------------------------------------------------------------------------

#[tokio::test]
async fn recipe_crud() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    assert_eq!(r.name, "Tortilla de patatas");
    assert_eq!(r.servings, 2);
    assert_eq!(r.ingredients.len(), 3);
    assert!(r.nutrition.is_none(), "la nutrición nunca se inventa");

    let updated = m
        .update_recipe(
            &r.id,
            UpdateRecipe {
                name: Some("Tortilla con cebolla".into()),
                servings: Some(4),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Tortilla con cebolla");
    assert_eq!(updated.servings, 4);

    let fetched = m.get_recipe(&r.id).await.unwrap();
    assert_eq!(fetched.steps.len(), 3);

    m.delete_recipe(&r.id, "user").await.unwrap();
    assert!(m.get_recipe(&r.id).await.is_err());
    assert!(m
        .list_recipes(RecipeFilter::default())
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn recipe_validations() {
    let m = setup().await;

    let mut data = tortilla_data();
    data.servings = Some(0);
    let err = m.create_recipe(data, "user").await.unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));

    let mut data = tortilla_data();
    data.ingredients[0].quantity = -5.0;
    assert!(matches!(
        m.create_recipe(data, "user").await.unwrap_err(),
        NexusError::Validation(_)
    ));

    let mut data = tortilla_data();
    data.ingredients[0].unit = "   ".into();
    assert!(matches!(
        m.create_recipe(data, "user").await.unwrap_err(),
        NexusError::Validation(_)
    ));

    let mut data = tortilla_data();
    data.name = "  ".into();
    assert!(matches!(
        m.create_recipe(data, "user").await.unwrap_err(),
        NexusError::Validation(_)
    ));

    let mut data = tortilla_data();
    data.prep_time_min = Some(-1);
    assert!(matches!(
        m.create_recipe(data, "user").await.unwrap_err(),
        NexusError::Validation(_)
    ));

    // Nutrición introducida por el usuario sí se guarda.
    let mut data = tortilla_data();
    data.nutrition = Some(Nutrition {
        kcal: 350.0,
        protein_g: 12.0,
        carbs_g: 30.0,
        fat_g: 20.0,
    });
    let r = m.create_recipe(data, "user").await.unwrap();
    assert_eq!(r.nutrition.unwrap().kcal, 350.0);
}

#[tokio::test]
async fn recipe_duplicate() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    let copy = m.duplicate_recipe(&r.id, "user").await.unwrap();
    assert_ne!(copy.id, r.id);
    assert_eq!(copy.name, "Tortilla de patatas (copia)");
    assert_eq!(copy.ingredients, r.ingredients);
    assert_eq!(copy.servings, r.servings);
    assert_eq!(
        m.list_recipes(RecipeFilter::default()).await.unwrap().len(),
        2
    );
}

#[tokio::test]
async fn recipe_favorite_toggle() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    assert!(!r.favorite);
    let r = m.toggle_favorite(&r.id, "user").await.unwrap();
    assert!(r.favorite);
    let favs = m
        .list_recipes(RecipeFilter {
            favorites_only: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(favs.len(), 1);
    let r = m.toggle_favorite(&r.id, "user").await.unwrap();
    assert!(!r.favorite);
    assert!(m
        .list_recipes(RecipeFilter {
            favorites_only: Some(true),
            ..Default::default()
        })
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn recipe_search_and_filters() {
    let m = setup().await;
    let tortilla = create_tortilla(&m).await;
    let mut data = tortilla_data();
    data.name = "Ensalada verde".into();
    data.difficulty = Some(RecipeDifficulty::Facil);
    data.prep_time_min = Some(10);
    data.cook_time_min = Some(0);
    data.categories = vec!["comida".into()];
    data.ingredients = vec![ing("Lechuga", 100.0, "g", "verduras")];
    let ensalada = m.create_recipe(data, "user").await.unwrap();

    // Búsqueda por nombre.
    let found = m
        .list_recipes(RecipeFilter {
            search: Some("tortilla".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, tortilla.id);

    // Búsqueda por ingrediente.
    let found = m
        .list_recipes(RecipeFilter {
            search: Some("lechuga".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, ensalada.id);

    // Filtro por categoría.
    let found = m
        .list_recipes(RecipeFilter {
            category: Some("cena".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(found.len(), 1);

    // Filtro por dificultad.
    let found = m
        .list_recipes(RecipeFilter {
            difficulty: Some(RecipeDifficulty::Facil),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(found[0].id, ensalada.id);

    // Tiempo máximo total.
    let found = m
        .list_recipes(RecipeFilter {
            max_time_min: Some(15),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, ensalada.id);

    // Orden por nombre.
    let sorted = m
        .list_recipes(RecipeFilter {
            sort: Some(RecipeSort::Name),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(sorted[0].name, "Ensalada verde");

    // Orden por tiempo.
    let sorted = m
        .list_recipes(RecipeFilter {
            sort: Some(RecipeSort::Time),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(sorted[0].id, ensalada.id);
}

#[tokio::test]
async fn proportional_scaling() {
    // 2 → 4 raciones: exactamente el doble, redondeo a 2 decimales.
    assert_eq!(scale_quantity(300.0, 2, 4), 600.0);
    assert_eq!(scale_quantity(3.0, 2, 4), 6.0);
    assert_eq!(scale_quantity(2.0, 2, 4), 4.0);
    // 2 → 3 raciones: 1.5×.
    assert_eq!(scale_quantity(300.0, 2, 3), 450.0);
    // Redondeo sensato.
    assert_eq!(scale_quantity(100.0, 3, 2), 66.67);
    assert_eq!(scale_quantity(0.33, 2, 3), 0.5);

    let m = setup().await;
    let r = create_tortilla(&m).await;
    m.add_plan_entry("2026-11-24", MealSlot::Cena, &r.id, 4, "user")
        .await
        .unwrap();
    let consolidated = m.consolidate_week("2026-11-23").await.unwrap();
    let patata = consolidated
        .iter()
        .find(|c| c.name_key == "patata")
        .unwrap();
    assert_eq!(patata.quantity, 600.0);
    assert_eq!(patata.unit, "g");
    let huevo = consolidated.iter().find(|c| c.name_key == "huevo").unwrap();
    assert_eq!(huevo.quantity, 6.0);
}

// ---------------------------------------------------------------------------
// Planificador semanal
// ---------------------------------------------------------------------------

#[tokio::test]
async fn planner_create_move_delete() {
    let m = setup().await;
    let r = create_tortilla(&m).await;

    // miércoles 2026-11-25 → semana del lunes 2026-11-23.
    let e = m
        .add_plan_entry("2026-11-25", MealSlot::Comida, &r.id, 2, "user")
        .await
        .unwrap();
    assert_eq!(e.week_start, "2026-11-23");

    let plan = m.week_plan("2026-11-25").await.unwrap();
    assert_eq!(plan.week_start, "2026-11-23");
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].recipe_name, "Tortilla de patatas");

    // Cambiar raciones inline.
    let e = m
        .update_plan_entry(&e.id, None, None, Some(6), "user")
        .await
        .unwrap();
    assert_eq!(e.servings, 6);

    // Mover a otro día/slot.
    let e = m
        .move_plan_entry(&e.id, "2026-11-27", MealSlot::Cena, "user")
        .await
        .unwrap();
    assert_eq!(e.date, "2026-11-27");
    assert_eq!(e.slot, MealSlot::Cena);
    assert_eq!(e.week_start, "2026-11-23");

    // Duplicar a otro día/slot.
    let dup = m
        .duplicate_plan_entry(&e.id, "2026-11-28", MealSlot::Comida, "user")
        .await
        .unwrap();
    assert_ne!(dup.id, e.id);
    assert_eq!(dup.servings, 6);
    assert_eq!(m.week_plan("2026-11-23").await.unwrap().entries.len(), 2);

    // Semana distinta no ve las entradas.
    assert!(m.week_plan("2026-11-30").await.unwrap().entries.is_empty());

    // Fecha de otra semana se normaliza a su propio lunes.
    let e2 = m
        .add_plan_entry("2026-12-01", MealSlot::Desayuno, &r.id, 1, "user")
        .await
        .unwrap();
    assert_eq!(e2.week_start, "2026-11-30");

    // Borrar.
    m.delete_plan_entry(&e.id, "user").await.unwrap();
    m.delete_plan_entry(&dup.id, "user").await.unwrap();
    assert!(m.week_plan("2026-11-23").await.unwrap().entries.is_empty());

    // Validaciones: raciones 0 y receta inexistente.
    assert!(m
        .add_plan_entry("2026-11-25", MealSlot::Comida, &r.id, 0, "user")
        .await
        .is_err());
    assert!(m
        .add_plan_entry("2026-11-25", MealSlot::Comida, "no-existe", 2, "user")
        .await
        .is_err());
    assert!(m
        .add_plan_entry("fecha-mala", MealSlot::Comida, &r.id, 2, "user")
        .await
        .is_err());
}

#[tokio::test]
async fn menu_template_save_and_apply() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    m.add_plan_entry("2026-11-23", MealSlot::Comida, &r.id, 2, "user")
        .await
        .unwrap();
    m.add_plan_entry("2026-11-25", MealSlot::Cena, &r.id, 4, "user")
        .await
        .unwrap();

    let tpl = m
        .save_menu_template("Mi menú", "2026-11-23", "user")
        .await
        .unwrap();
    assert_eq!(tpl.entries.len(), 2);
    assert_eq!(tpl.entries[0].day_offset, 0);
    assert_eq!(tpl.entries[1].day_offset, 2);

    // Aplicar a otra semana (sobrescribe).
    m.add_plan_entry("2026-11-30", MealSlot::Desayuno, &r.id, 1, "user")
        .await
        .unwrap();
    let (entries, skipped) = m
        .apply_menu_template(&tpl.id, "2026-11-30", "user")
        .await
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert!(skipped.is_empty());
    let plan = m.week_plan("2026-11-30").await.unwrap();
    assert_eq!(plan.entries.len(), 2, "la semana previa se sobrescribió");
    assert!(plan
        .entries
        .iter()
        .any(|e| e.entry.date == "2026-11-30" && e.entry.servings == 2));
    assert!(plan
        .entries
        .iter()
        .any(|e| e.entry.date == "2026-12-02" && e.entry.servings == 4));

    // Semana vacía → error claro.
    assert!(m
        .save_menu_template("Vacío", "2027-01-04", "user")
        .await
        .is_err());

    // Detalle con nombres y borrado.
    let detail = m.get_menu_template(&tpl.id).await.unwrap();
    assert_eq!(
        detail.entries[0].recipe_name.as_deref(),
        Some("Tortilla de patatas")
    );
    m.delete_menu_template(&tpl.id, "user").await.unwrap();
    assert!(m.list_menu_templates().await.unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Lista de la compra
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shopping_consolidation() {
    let m = setup().await;
    // Receta A: 2 raciones.
    let mut a = tortilla_data();
    a.name = "Tortilla".into();
    let a = m.create_recipe(a, "user").await.unwrap();
    // Receta B: 4 raciones, comparte patata (misma unidad) y añade leche en
    // unidades distintas (l vs ml) que NO deben combinarse.
    let b = m
        .create_recipe(
            CreateRecipe {
                name: "Puré".into(),
                servings: Some(4),
                ingredients: vec![
                    ing("patata", 400.0, "g", "verduras"),
                    ing("Leche", 0.2, "l", "lácteos"),
                ],
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let c = m
        .create_recipe(
            CreateRecipe {
                name: "Batido".into(),
                servings: Some(1),
                ingredients: vec![ing("leche", 250.0, "ml", "lácteos")],
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();

    // A ×2 raciones (base 2), B ×8 raciones (base 4 → 2×), C ×1.
    m.add_plan_entry("2026-11-23", MealSlot::Comida, &a.id, 2, "user")
        .await
        .unwrap();
    m.add_plan_entry("2026-11-24", MealSlot::Cena, &b.id, 8, "user")
        .await
        .unwrap();
    m.add_plan_entry("2026-11-25", MealSlot::Merienda, &c.id, 1, "user")
        .await
        .unwrap();

    let detail = m
        .generate_shopping_list("2026-11-23", Some("Compra semanal"), None, "user")
        .await
        .unwrap();
    let items = &detail.items;

    // Patata: 300 (A×1) + 400×2 (B×2) = 1100 g consolidados (case-insensitive).
    let patatas: Vec<_> = items.iter().filter(|i| i.name_key == "patata").collect();
    assert_eq!(patatas.len(), 1, "mismo nombre+unidad → una línea");
    assert_eq!(patatas[0].quantity, 1100.0);
    assert_eq!(patatas[0].unit, "g");
    assert_eq!(patatas[0].category, "verduras");

    // Leche: l y ml son unidades incompatibles → dos líneas, nunca suma.
    let leches: Vec<_> = items.iter().filter(|i| i.name_key == "leche").collect();
    assert_eq!(leches.len(), 2);
    let litros = leches.iter().find(|i| i.unit == "l").unwrap();
    let mililitros = leches.iter().find(|i| i.unit == "ml").unwrap();
    assert_eq!(litros.quantity, 0.4);
    assert_eq!(mililitros.quantity, 250.0);

    // Huevos escalados solo por A.
    let huevo = items.iter().find(|i| i.name_key == "huevo").unwrap();
    assert_eq!(huevo.quantity, 3.0);

    // Agrupación por categoría en el orden de salida.
    let cats: Vec<&str> = items.iter().map(|i| i.category.as_str()).collect();
    let mut sorted = cats.clone();
    sorted.sort();
    assert_eq!(cats, sorted);
    assert!(!detail.list.has_manual_changes);
}

#[tokio::test]
async fn shopping_manual_items_checked_and_regenerate() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    m.add_plan_entry("2026-11-23", MealSlot::Comida, &r.id, 2, "user")
        .await
        .unwrap();
    let detail = m
        .generate_shopping_list("2026-11-23", None, None, "user")
        .await
        .unwrap();
    let list_id = detail.list.id.clone();

    // Marcar comprado (persistente).
    let patata = detail
        .items
        .iter()
        .find(|i| i.name_key == "patata")
        .unwrap();
    m.update_shopping_item(
        &patata.id,
        UpdateShoppingItem {
            checked: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let d = m.get_shopping_list(&list_id).await.unwrap();
    assert!(d.items.iter().find(|i| i.id == patata.id).unwrap().checked);
    assert!(
        !d.list.has_manual_changes,
        "marcar comprado no es cambio manual"
    );

    // Producto manual + editar cantidad.
    let manual = m
        .add_shopping_item(
            &list_id,
            CreateShoppingItem {
                name: "Pan".into(),
                quantity: Some(2.0),
                unit: Some("ud".into()),
                category: Some("panadería".into()),
            },
            "user",
        )
        .await
        .unwrap();
    assert!(manual.manual);
    let d = m.get_shopping_list(&list_id).await.unwrap();
    assert!(d.list.has_manual_changes);

    m.update_shopping_item(
        &manual.id,
        UpdateShoppingItem {
            quantity: Some(3.0),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        m.get_shopping_list(&list_id)
            .await
            .unwrap()
            .items
            .iter()
            .find(|i| i.id == manual.id)
            .unwrap()
            .quantity,
        3.0
    );

    // Regenerar: rehace lo generado, conserva lo manual.
    let d = m
        .generate_shopping_list("2026-11-23", None, Some(&list_id), "user")
        .await
        .unwrap();
    assert!(d.items.iter().any(|i| i.id == manual.id));
    assert!(d.items.iter().any(|i| i.name_key == "patata" && !i.manual));
    // El marcado de comprado se reinicia en lo regenerado.
    assert!(
        !d.items
            .iter()
            .find(|i| i.name_key == "patata")
            .unwrap()
            .checked
    );

    // Eliminar item e historial.
    m.delete_shopping_item(&manual.id).await.unwrap();
    assert_eq!(m.list_shopping_lists().await.unwrap().len(), 1);
    m.delete_shopping_list(&list_id, "user").await.unwrap();
    assert!(m.list_shopping_lists().await.unwrap().is_empty());

    // Validaciones.
    assert!(m
        .add_shopping_item(
            &list_id,
            CreateShoppingItem {
                name: "X".into(),
                quantity: Some(-1.0),
                ..Default::default()
            },
            "user",
        )
        .await
        .is_err());
}

#[tokio::test]
async fn shopping_export() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    m.add_plan_entry("2026-11-23", MealSlot::Comida, &r.id, 2, "user")
        .await
        .unwrap();
    let detail = m
        .generate_shopping_list("2026-11-23", Some("Compra test"), None, "user")
        .await
        .unwrap();

    let txt = m
        .export_shopping_list(&detail.list.id, "txt")
        .await
        .unwrap();
    assert!(txt.filename.ends_with(".txt"));
    assert!(txt.content.contains("Compra test"));
    assert!(txt.content.contains("Verduras"));
    assert!(txt.content.contains("Patata — 300 g"));
    assert!(txt.content.contains("[ ]"));

    let csv = m
        .export_shopping_list(&detail.list.id, "csv")
        .await
        .unwrap();
    assert!(csv.filename.ends_with(".csv"));
    assert!(csv
        .content
        .starts_with("categoria,producto,cantidad,unidad,comprado"));
    assert!(csv.content.contains("Patata,300,g,no"));
}

// ---------------------------------------------------------------------------
// Despensa
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pantry_crud_adjust_consume() {
    let m = setup().await;
    let item = m
        .create_pantry_item(
            CreatePantryItem {
                name: "Arroz".into(),
                quantity: Some(1000.0),
                unit: Some("g".into()),
                category: Some("despensa".into()),
                location: Some(PantryLocation::Despensa),
                expires_at: None,
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(item.name_key, "arroz");
    assert!(item.expires_at.is_none());

    // Filtros por ubicación y categoría.
    assert_eq!(
        m.list_pantry(PantryFilter {
            location: Some(PantryLocation::Despensa),
            ..Default::default()
        })
        .await
        .unwrap()
        .len(),
        1
    );
    assert!(m
        .list_pantry(PantryFilter {
            location: Some(PantryLocation::Nevera),
            ..Default::default()
        })
        .await
        .unwrap()
        .is_empty());

    // Ajuste rápido +/- (nunca negativo).
    let item = m.adjust_pantry(&item.id, -200.0, "user").await.unwrap();
    assert_eq!(item.quantity, 800.0);
    let item = m.adjust_pantry(&item.id, -5000.0, "user").await.unwrap();
    assert_eq!(item.quantity, 0.0);
    let item = m.adjust_pantry(&item.id, 100.0, "user").await.unwrap();
    assert_eq!(item.quantity, 100.0);

    // Consumo: resta; a 0 sin eliminar.
    let res = m
        .consume_pantry(&item.id, 40.0, false, "user")
        .await
        .unwrap();
    assert!(!res.deleted);
    assert_eq!(res.item.unwrap().quantity, 60.0);

    // Consumo hasta agotar con eliminación opcional.
    let res = m
        .consume_pantry(&item.id, 60.0, true, "user")
        .await
        .unwrap();
    assert!(res.deleted);
    assert!(res.item.is_none());
    assert!(m
        .list_pantry(PantryFilter::default())
        .await
        .unwrap()
        .is_empty());

    // Update y delete + validaciones.
    let item = m
        .create_pantry_item(
            CreatePantryItem {
                name: "Yogur".into(),
                quantity: Some(4.0),
                unit: Some("ud".into()),
                location: Some(PantryLocation::Nevera),
                expires_at: Some("2026-12-01".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let item = m
        .update_pantry_item(
            &item.id,
            UpdatePantryItem {
                expires_at: Some(None),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert!(item.expires_at.is_none());
    m.delete_pantry_item(&item.id, "user").await.unwrap();

    assert!(m
        .create_pantry_item(
            CreatePantryItem {
                name: "X".into(),
                quantity: Some(-1.0),
                ..Default::default()
            },
            "user",
        )
        .await
        .is_err());
    assert!(m
        .create_pantry_item(
            CreatePantryItem {
                name: "X".into(),
                expires_at: Some("mañana".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .is_err());
}

#[tokio::test]
async fn pantry_expiry_uses_only_user_dates() {
    let m = setup().await;
    let today = today_date();
    let soon = date_plus_days(&today, 2).unwrap();
    let past = date_plus_days(&today, -1).unwrap();

    let a = m
        .create_pantry_item(
            CreatePantryItem {
                name: "Leche".into(),
                quantity: Some(1.0),
                unit: Some("l".into()),
                expires_at: Some(past.clone()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let b = m
        .create_pantry_item(
            CreatePantryItem {
                name: "Queso".into(),
                quantity: Some(1.0),
                unit: Some("ud".into()),
                expires_at: Some(soon.clone()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let c = m
        .create_pantry_item(
            CreatePantryItem {
                name: "Arroz".into(),
                quantity: Some(1.0),
                unit: Some("kg".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();

    let items = m.list_pantry(PantryFilter::default()).await.unwrap();
    let find = |id: &str| items.iter().find(|i| i.id == id).unwrap();
    // Las fechas guardadas son EXACTAMENTE las que dio el usuario.
    assert_eq!(find(&a.id).expires_at.as_deref(), Some(past.as_str()));
    assert_eq!(find(&b.id).expires_at.as_deref(), Some(soon.as_str()));
    assert!(
        find(&c.id).expires_at.is_none(),
        "sin fecha del usuario no se inventa"
    );

    // La caducidad afecta a la cobertura: lo caducado no cuenta como stock.
    let r = m
        .create_recipe(
            CreateRecipe {
                name: "Arroz con leche".into(),
                servings: Some(2),
                ingredients: vec![
                    ing("Leche", 1.0, "l", "lácteos"),
                    ing("Arroz", 1.0, "kg", "despensa"),
                ],
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let matches = m.pantry_matches().await.unwrap();
    let mt = matches.iter().find(|x| x.recipe_id == r.id).unwrap();
    assert_eq!(mt.coverage_pct, 50.0);
    assert_eq!(mt.missing.len(), 1);
    assert_eq!(mt.missing[0].name, "Leche");
}

#[tokio::test]
async fn pantry_matches_coverage_and_missing() {
    let m = setup().await;
    let completa = m
        .create_recipe(
            CreateRecipe {
                name: "Arroz blanco".into(),
                servings: Some(2),
                ingredients: vec![
                    ing("Arroz", 200.0, "g", "despensa"),
                    ing("Sal", 1.0, "pizca", "despensa"),
                ],
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let parcial = create_tortilla(&m).await;

    for (name, qty, unit) in [
        ("arroz", 500.0, "g"),
        ("sal", 5.0, "pizca"),
        ("patata", 100.0, "g"), // insuficiente: la tortilla pide 300 g
        ("huevo", 3.0, "ud"),
    ] {
        m.create_pantry_item(
            CreatePantryItem {
                name: name.into(),
                quantity: Some(qty),
                unit: Some(unit.into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    }

    let matches = m.pantry_matches().await.unwrap();
    assert_eq!(matches.len(), 2);
    // Orden: mayor cobertura primero.
    assert_eq!(matches[0].recipe_id, completa.id);
    assert_eq!(matches[0].coverage_pct, 100.0);
    assert!(matches[0].missing.is_empty());

    let t = &matches[1];
    assert_eq!(t.recipe_id, parcial.id);
    // Cubiertos: huevo y… patata NO (100 < 300), aceite falta. 1 de 3.
    assert_eq!(t.coverage_pct, 33.33);
    assert_eq!(t.covered, vec!["Huevo".to_string()]);
    assert_eq!(t.missing.len(), 2);
    let patata = t.missing.iter().find(|x| x.name == "Patata").unwrap();
    assert_eq!(patata.needed, 300.0);
    assert_eq!(patata.have, 100.0);
    assert_eq!(patata.unit, "g");

    // Unidades incompatibles no dan cobertura (g ≠ kg).
    let kg_recipe = m
        .create_recipe(
            CreateRecipe {
                name: "Saco de arroz".into(),
                servings: Some(1),
                ingredients: vec![ing("Arroz", 1.0, "kg", "despensa")],
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let matches = m.pantry_matches().await.unwrap();
    let kg = matches
        .iter()
        .find(|x| x.recipe_id == kg_recipe.id)
        .unwrap();
    assert_eq!(kg.coverage_pct, 0.0);
    assert_eq!(kg.missing[0].have, 0.0);
}

// ---------------------------------------------------------------------------
// Integración / IA (saneado de propuestas)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn recipe_preview_for_canvas() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    let p = m.recipe_preview(&r.id).await.unwrap();
    assert_eq!(p.id, r.id);
    assert_eq!(p.name, "Tortilla de patatas");
    assert_eq!(p.difficulty, RecipeDifficulty::Media);
    assert_eq!(p.total_time, 30);
    assert!(p.image_file_id.is_none());

    // La lista incluye los campos que Canvas necesita para recipe_ref.
    let list = m.list_recipes(RecipeFilter::default()).await.unwrap();
    let v = serde_json::to_value(&list[0]).unwrap();
    assert!(v.get("id").is_some());
    assert!(v.get("name").is_some());
    assert!(v.get("image_file_id").is_some());
}

#[tokio::test]
async fn ai_recipe_sanitized_nutrition_never_invented() {
    let value = serde_json::json!({
        "name": "  Pasta al pesto  ",
        "servings": 2,
        "ingredients": [{"name": "Pasta", "quantity": 200, "unit": "g", "category": "despensa"}],
        "steps": ["Hervir", "Mezclar"],
        "nutrition": {"kcal": 999, "protein_g": 99, "carbs_g": 99, "fat_g": 99},
        "favorite": true
    });
    let data = MealService::sanitize_ai_recipe(&value).unwrap();
    assert!(
        data.nutrition.is_none(),
        "la nutrición de la IA se descarta siempre"
    );
    assert_eq!(data.favorite, Some(false));
    let m = setup().await;
    let r = m.create_recipe(data, "user").await.unwrap();
    assert!(r.nutrition.is_none());
}

#[tokio::test]
async fn ai_week_only_existing_recipes() {
    let m = setup().await;
    let r = create_tortilla(&m).await;
    let proposal = serde_json::json!({
        "entries": [
            {"day_offset": 0, "slot": "comida", "recipe_id": r.id, "servings": 2},
            {"day_offset": 4, "slot": "cena", "recipe_id": r.id, "servings": 4}
        ]
    });
    let entries = m.sanitize_ai_week("2026-11-23", &proposal).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].date, "2026-11-23");
    assert_eq!(entries[1].date, "2026-11-27");

    // Aplicar la propuesta validada con reemplazo.
    m.add_plan_entry("2026-11-23", MealSlot::Desayuno, &r.id, 1, "user")
        .await
        .unwrap();
    let created = m
        .apply_plan_entries("2026-11-23", entries, true, "user", "Menú IA aplicado")
        .await
        .unwrap();
    assert_eq!(created.len(), 2);
    assert_eq!(m.week_plan("2026-11-23").await.unwrap().entries.len(), 2);

    // Id inventado → error.
    let bad = serde_json::json!({
        "entries": [{"day_offset": 0, "slot": "comida", "recipe_id": "inventado", "servings": 2}]
    });
    assert!(m.sanitize_ai_week("2026-11-23", &bad).await.is_err());
    // Slot desconocido → error.
    let bad = serde_json::json!({
        "entries": [{"day_offset": 0, "slot": "brunch", "recipe_id": r.id, "servings": 2}]
    });
    assert!(m.sanitize_ai_week("2026-11-23", &bad).await.is_err());
}

#[tokio::test]
async fn week_helpers() {
    assert_eq!(monday_of("2026-11-22").unwrap(), "2026-11-16"); // domingo
    assert_eq!(monday_of("2026-11-23").unwrap(), "2026-11-23"); // lunes
    assert_eq!(monday_of("2026-11-29").unwrap(), "2026-11-23"); // domingo siguiente
    assert_eq!(date_plus_days("2026-11-23", 6).unwrap(), "2026-11-29");
    assert_eq!(days_between("2026-11-23", "2026-11-29").unwrap(), 6);
    assert!(monday_of("2026-13-40").is_err());
    // Alias de la misma unidad se agrupan; distintas no.
    assert_eq!(unit_key("gramos"), unit_key("g"));
    assert_eq!(unit_key("Unidades"), unit_key("ud"));
    assert_ne!(unit_key("g"), unit_key("kg"));
    assert_ne!(unit_key("ml"), unit_key("l"));
}
