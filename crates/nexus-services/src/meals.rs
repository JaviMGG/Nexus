//! NEXUS Meal Planner: recetas, menús semanales, compra y despensa.
//! Validación + reglas de negocio + auditoría (repos::activity::record).
//! La nutrición solo existe si el usuario la introduce: nunca se inventa.

use std::collections::BTreeMap;

use nexus_core::{
    date_plus_days, days_between, monday_of, new_id, normalize_name, now_str, round2,
    scale_quantity, today_date, unit_key, CreatePantryItem, CreateRecipe, CreateShoppingItem, Id,
    MealPlanEntry, MealSlot, MenuTemplate, MenuTemplateDetail, MissingIngredient, NexusError,
    Nutrition, PantryConsumeResult, PantryFilter, PantryItem, PantryMatch, PlanEntryInput,
    PlanEntryView, Recipe, RecipeFilter, RecipePreview, Result, ShoppingExport, ShoppingItem,
    ShoppingList, ShoppingListDetail, TemplateEntry, TemplateEntryView, UpdatePantryItem,
    UpdateRecipe, UpdateShoppingItem, WeekPlan,
};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct MealService {
    pub db: Db,
}

// ---------------------------------------------------------------------------
// Validación
// ---------------------------------------------------------------------------

fn date_str(s: &str) -> Result<String> {
    let t = s.trim();
    chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d")
        .map_err(|_| NexusError::validation(format!("fecha inválida: {s} (usa YYYY-MM-DD)")))?;
    Ok(t.to_string())
}

fn servings_valid(n: i64) -> Result<i64> {
    if !(1..=99).contains(&n) {
        return Err(NexusError::validation(
            "las raciones deben estar entre 1 y 99",
        ));
    }
    Ok(n)
}

fn minutes_valid(n: i64, field: &str) -> Result<i64> {
    if !(0..=10_080).contains(&n) {
        return Err(NexusError::validation(format!(
            "{field} debe estar entre 0 y 10080 minutos"
        )));
    }
    Ok(n)
}

fn quantity_valid(q: f64, allow_zero: bool) -> Result<f64> {
    if !q.is_finite() || q < 0.0 || (!allow_zero && q == 0.0) {
        return Err(NexusError::validation(if allow_zero {
            "la cantidad no puede ser negativa"
        } else {
            "la cantidad debe ser mayor que 0"
        }));
    }
    if q > 1_000_000.0 {
        return Err(NexusError::validation("cantidad demasiado grande"));
    }
    Ok(round2(q))
}

fn ingredient_valid(ing: &nexus_core::Ingredient) -> Result<nexus_core::Ingredient> {
    let name = validate::title(&ing.name, "el nombre del ingrediente")?;
    let unit = ing.unit.trim().to_lowercase();
    if unit.is_empty() {
        return Err(NexusError::validation(format!(
            "el ingrediente '{name}' necesita una unidad (g, ml, ud…)"
        )));
    }
    if unit.chars().count() > 20 {
        return Err(NexusError::validation("unidad demasiado larga (máx. 20)"));
    }
    let category = ing.category.trim().to_lowercase();
    Ok(nexus_core::Ingredient {
        name,
        quantity: quantity_valid(ing.quantity, false)?,
        unit,
        category: if category.is_empty() {
            nexus_core::default_category()
        } else {
            category
        },
        notes: validate::long_text(&ing.notes, "las notas del ingrediente", 500)?,
    })
}

fn steps_valid(steps: &[String]) -> Result<Vec<String>> {
    if steps.len() > 100 {
        return Err(NexusError::validation("máximo 100 pasos"));
    }
    let mut out = Vec::new();
    for s in steps {
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        out.push(validate::long_text(t, "el paso", 2_000)?);
    }
    Ok(out)
}

fn nutrition_valid(n: &Nutrition) -> Result<Nutrition> {
    for (v, name) in [
        (n.kcal, "kcal"),
        (n.protein_g, "proteínas"),
        (n.carbs_g, "carbohidratos"),
        (n.fat_g, "grasas"),
    ] {
        if !v.is_finite() || !(0.0..=100_000.0).contains(&v) {
            return Err(NexusError::validation(format!(
                "valor nutricional inválido en {name}"
            )));
        }
    }
    Ok(Nutrition {
        kcal: round2(n.kcal),
        protein_g: round2(n.protein_g),
        carbs_g: round2(n.carbs_g),
        fat_g: round2(n.fat_g),
    })
}

async fn check_image(db: &Db, file_id: &Option<Id>) -> Result<()> {
    if let Some(id) = file_id {
        repos::files::get(db, id).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Recetario
// ---------------------------------------------------------------------------

impl MealService {
    pub async fn list_recipes(&self, f: RecipeFilter) -> Result<Vec<Recipe>> {
        repos::meals::list_recipes(&self.db, &f).await
    }

    pub async fn get_recipe(&self, id: &str) -> Result<Recipe> {
        repos::meals::get_recipe(&self.db, id).await
    }

    pub async fn recipe_preview(&self, id: &str) -> Result<RecipePreview> {
        let r = self.get_recipe(id).await?;
        Ok(RecipePreview {
            total_time: r.total_time_min(),
            id: r.id,
            name: r.name,
            image_file_id: r.image_file_id,
            difficulty: r.difficulty,
        })
    }

    async fn validate_create(&self, data: &mut CreateRecipe) -> Result<()> {
        data.name = validate::title(&data.name, "el nombre de la receta")?;
        data.description = validate::long_text(&data.description, "la descripción", 5_000)?;
        data.servings = Some(servings_valid(data.servings.unwrap_or(2))?);
        data.prep_time_min = Some(minutes_valid(
            data.prep_time_min.unwrap_or(0),
            "el tiempo de preparación",
        )?);
        data.cook_time_min = Some(minutes_valid(
            data.cook_time_min.unwrap_or(0),
            "el tiempo de cocción",
        )?);
        data.categories = validate::tags(&data.categories)?;
        data.tags = validate::tags(&data.tags)?;
        if data.ingredients.len() > 100 {
            return Err(NexusError::validation("máximo 100 ingredientes"));
        }
        data.ingredients = data
            .ingredients
            .iter()
            .map(ingredient_valid)
            .collect::<Result<_>>()?;
        data.steps = steps_valid(&data.steps)?;
        if let Some(n) = &data.nutrition {
            data.nutrition = Some(nutrition_valid(n)?);
        }
        check_image(&self.db, &data.image_file_id).await?;
        Ok(())
    }

    pub async fn create_recipe(&self, mut data: CreateRecipe, actor: &str) -> Result<Recipe> {
        self.validate_create(&mut data).await?;
        let recipe =
            repos::meals::create_recipe(&self.db, &repos::meals::recipe_from(new_id(), data))
                .await?;
        self.audit(
            "created",
            "recipe",
            Some(&recipe.id),
            &format!("Receta creada: {}", recipe.name),
            actor,
            Some(serde_json::json!({"op": "meals_delete_recipe", "id": recipe.id})),
        )
        .await?;
        Ok(recipe)
    }

    pub async fn update_recipe(
        &self,
        id: &str,
        mut patch: UpdateRecipe,
        actor: &str,
    ) -> Result<Recipe> {
        let before = self.get_recipe(id).await?;
        if let Some(n) = &patch.name {
            patch.name = Some(validate::title(n, "el nombre de la receta")?);
        }
        if let Some(d) = &patch.description {
            validate::long_text(d, "la descripción", 5_000)?;
        }
        if let Some(s) = patch.servings {
            patch.servings = Some(servings_valid(s)?);
        }
        if let Some(t) = patch.prep_time_min {
            patch.prep_time_min = Some(minutes_valid(t, "el tiempo de preparación")?);
        }
        if let Some(t) = patch.cook_time_min {
            patch.cook_time_min = Some(minutes_valid(t, "el tiempo de cocción")?);
        }
        if let Some(c) = &patch.categories {
            patch.categories = Some(validate::tags(c)?);
        }
        if let Some(t) = &patch.tags {
            patch.tags = Some(validate::tags(t)?);
        }
        if let Some(ings) = &patch.ingredients {
            if ings.len() > 100 {
                return Err(NexusError::validation("máximo 100 ingredientes"));
            }
            patch.ingredients = Some(
                ings.iter()
                    .map(ingredient_valid)
                    .collect::<Result<Vec<_>>>()?,
            );
        }
        if let Some(steps) = &patch.steps {
            patch.steps = Some(steps_valid(steps)?);
        }
        if let Some(Some(n)) = &patch.nutrition {
            patch.nutrition = Some(Some(nutrition_valid(n)?));
        }
        if let Some(f) = &patch.image_file_id {
            check_image(&self.db, f).await?;
        }
        let recipe = repos::meals::update_recipe(&self.db, id, patch).await?;
        self.audit(
            "updated",
            "recipe",
            Some(id),
            &format!("Receta actualizada: {}", recipe.name),
            actor,
            Some(serde_json::json!({"op": "meals_restore_recipe", "recipe": before})),
        )
        .await?;
        Ok(recipe)
    }

    pub async fn delete_recipe(&self, id: &str, actor: &str) -> Result<()> {
        let recipe = self.get_recipe(id).await?;
        repos::meals::soft_delete_recipe(&self.db, id).await?;
        self.audit(
            "deleted",
            "recipe",
            Some(id),
            &format!("Receta eliminada: {}", recipe.name),
            actor,
            Some(serde_json::json!({"op": "meals_restore_recipe", "recipe": recipe})),
        )
        .await?;
        Ok(())
    }

    /// Restaura una receta borrada (usado por el deshacer de auditoría).
    pub async fn restore_recipe(&self, recipe: Recipe) -> Result<Recipe> {
        sqlx_restore_recipe(&self.db, &recipe).await
    }

    pub async fn duplicate_recipe(&self, id: &str, actor: &str) -> Result<Recipe> {
        let base = self.get_recipe(id).await?;
        let data = CreateRecipe {
            name: format!("{} (copia)", base.name),
            description: base.description,
            image_file_id: base.image_file_id,
            servings: Some(base.servings),
            prep_time_min: Some(base.prep_time_min),
            cook_time_min: Some(base.cook_time_min),
            difficulty: Some(base.difficulty),
            categories: base.categories,
            tags: base.tags,
            ingredients: base.ingredients,
            steps: base.steps,
            nutrition: base.nutrition,
            favorite: Some(false),
        };
        self.create_recipe(data, actor).await
    }

    pub async fn toggle_favorite(&self, id: &str, actor: &str) -> Result<Recipe> {
        let r = self.get_recipe(id).await?;
        let recipe = repos::meals::update_recipe(
            &self.db,
            id,
            UpdateRecipe {
                favorite: Some(!r.favorite),
                ..Default::default()
            },
        )
        .await?;
        self.audit(
            "updated",
            "recipe",
            Some(id),
            &format!(
                "{}: {}",
                if recipe.favorite {
                    "Añadida a favoritas"
                } else {
                    "Quitada de favoritas"
                },
                recipe.name
            ),
            actor,
            None,
        )
        .await?;
        Ok(recipe)
    }

    // -------------------------------------------------------------------------
    // Planificador semanal
    // -------------------------------------------------------------------------

    pub async fn week_plan(&self, date: &str) -> Result<WeekPlan> {
        let week_start = monday_of(date)?;
        let entries = repos::meals::list_plan_entries(&self.db, &week_start).await?;
        let mut views = Vec::with_capacity(entries.len());
        for entry in entries {
            let recipe = repos::meals::get_recipe(&self.db, &entry.recipe_id).await?;
            views.push(PlanEntryView {
                recipe_name: recipe.name.clone(),
                recipe_image_file_id: recipe.image_file_id.clone(),
                recipe_difficulty: recipe.difficulty,
                total_time_min: recipe.total_time_min(),
                entry,
            });
        }
        Ok(WeekPlan {
            week_start,
            entries: views,
        })
    }

    async fn check_date_in_week(&self, date: &str, week_start: &str) -> Result<String> {
        let d = date_str(date)?;
        if monday_of(&d)? != week_start {
            return Err(NexusError::validation(
                "la fecha no pertenece a la semana indicada",
            ));
        }
        Ok(d)
    }

    pub async fn add_plan_entry(
        &self,
        date: &str,
        slot: MealSlot,
        recipe_id: &str,
        servings: i64,
        actor: &str,
    ) -> Result<MealPlanEntry> {
        let d = date_str(date)?;
        let week_start = monday_of(&d)?;
        let recipe = self.get_recipe(recipe_id).await?;
        let servings = servings_valid(servings)?;
        let now = now_str();
        let entry = MealPlanEntry {
            id: new_id(),
            week_start,
            date: d,
            slot,
            recipe_id: recipe.id.clone(),
            servings,
            position: repos::meals::next_plan_position(&self.db, &date_str(date)?, slot).await?,
            created_at: now.clone(),
            updated_at: now,
        };
        let entry = repos::meals::create_plan_entry(&self.db, &entry).await?;
        self.audit(
            "created",
            "meal_plan",
            Some(&entry.id),
            &format!(
                "Comida planificada: {} ({}, {} raciones)",
                recipe.name, entry.date, entry.servings
            ),
            actor,
            Some(serde_json::json!({"op": "meals_delete_plan_entry", "id": entry.id})),
        )
        .await?;
        Ok(entry)
    }

    /// Actualiza raciones y/o mueve la entrada a otro día/slot.
    pub async fn update_plan_entry(
        &self,
        id: &str,
        date: Option<&str>,
        slot: Option<MealSlot>,
        servings: Option<i64>,
        actor: &str,
    ) -> Result<MealPlanEntry> {
        let before = repos::meals::get_plan_entry(&self.db, id).await?;
        let mut new_date = None;
        let mut new_week = None;
        if let Some(d) = date {
            let d = date_str(d)?;
            new_week = Some(monday_of(&d)?);
            new_date = Some(d);
        }
        let servings = match servings {
            Some(s) => Some(servings_valid(s)?),
            None => None,
        };
        let mut entry = repos::meals::update_plan_entry(
            &self.db,
            id,
            new_date.as_deref(),
            slot,
            servings,
            new_week.as_deref(),
        )
        .await?;
        if new_date.is_some() || slot.is_some() {
            let pos = repos::meals::next_plan_position(&self.db, &entry.date, entry.slot).await?;
            entry = repos::meals::update_plan_entry(&self.db, id, None, None, None, None).await?;
            sqlx::query("UPDATE meal_plan_entries SET position = ? WHERE id = ?")
                .bind(pos)
                .bind(id)
                .execute(&self.db.pool)
                .await
                .map_err(nexus_db::db_err)?;
            entry.position = pos;
        }
        self.audit(
            "updated",
            "meal_plan",
            Some(id),
            "Comida del plan actualizada",
            actor,
            Some(serde_json::json!({"op": "meals_restore_plan_entry", "entry": before})),
        )
        .await?;
        Ok(entry)
    }

    pub async fn move_plan_entry(
        &self,
        id: &str,
        date: &str,
        slot: MealSlot,
        actor: &str,
    ) -> Result<MealPlanEntry> {
        self.update_plan_entry(id, Some(date), Some(slot), None, actor)
            .await
    }

    pub async fn duplicate_plan_entry(
        &self,
        id: &str,
        date: &str,
        slot: MealSlot,
        actor: &str,
    ) -> Result<MealPlanEntry> {
        let e = repos::meals::get_plan_entry(&self.db, id).await?;
        self.add_plan_entry(date, slot, &e.recipe_id, e.servings, actor)
            .await
    }

    pub async fn delete_plan_entry(&self, id: &str, actor: &str) -> Result<()> {
        let entry = repos::meals::get_plan_entry(&self.db, id).await?;
        repos::meals::delete_plan_entry(&self.db, id).await?;
        self.audit(
            "deleted",
            "meal_plan",
            Some(id),
            "Comida eliminada del plan",
            actor,
            Some(serde_json::json!({"op": "meals_restore_plan_entry", "entry": entry})),
        )
        .await?;
        Ok(())
    }

    /// Restaura una entrada del plan tal cual estaba (deshacer).
    pub async fn restore_plan_entry(&self, entry: MealPlanEntry) -> Result<MealPlanEntry> {
        repos::meals::create_plan_entry(&self.db, &entry).await
    }

    /// Aplica un conjunto de entradas a una semana. `replace` vacía la semana
    /// antes (la confirmación la hace la UI). Devuelve las entradas creadas.
    pub async fn apply_plan_entries(
        &self,
        week_start: &str,
        entries: Vec<PlanEntryInput>,
        replace: bool,
        actor: &str,
        summary: &str,
    ) -> Result<Vec<MealPlanEntry>> {
        let week_start = monday_of(week_start)?;
        if entries.len() > 35 * 10 {
            return Err(NexusError::validation("demasiadas comidas en la propuesta"));
        }
        let mut validated = Vec::with_capacity(entries.len());
        for input in entries {
            let d = self.check_date_in_week(&input.date, &week_start).await?;
            self.get_recipe(&input.recipe_id).await?;
            validated.push((d, input));
        }
        if replace {
            repos::meals::clear_plan_week(&self.db, &week_start).await?;
        }
        let mut created = Vec::with_capacity(validated.len());
        for (d, input) in validated {
            let now = now_str();
            let entry = MealPlanEntry {
                id: new_id(),
                week_start: week_start.clone(),
                date: d.clone(),
                slot: input.slot,
                recipe_id: input.recipe_id,
                servings: servings_valid(input.servings)?,
                position: repos::meals::next_plan_position(&self.db, &d, input.slot).await?,
                created_at: now.clone(),
                updated_at: now,
            };
            created.push(repos::meals::create_plan_entry(&self.db, &entry).await?);
        }
        self.audit(
            "created",
            "meal_plan",
            None,
            &format!("{summary} (semana del {week_start})"),
            actor,
            None,
        )
        .await?;
        Ok(created)
    }

    // -------------------------------------------------------------------------
    // Menús personalizados (plantillas)
    // -------------------------------------------------------------------------

    pub async fn save_menu_template(
        &self,
        name: &str,
        week_start: &str,
        actor: &str,
    ) -> Result<MenuTemplate> {
        let name = validate::title(name, "el nombre del menú")?;
        let week_start = monday_of(week_start)?;
        let entries = repos::meals::list_plan_entries(&self.db, &week_start).await?;
        if entries.is_empty() {
            return Err(NexusError::validation(
                "la semana está vacía: planifica alguna comida antes de guardar el menú",
            ));
        }
        let mut tpl_entries = Vec::with_capacity(entries.len());
        for e in entries {
            tpl_entries.push(TemplateEntry {
                day_offset: days_between(&week_start, &e.date)?,
                slot: e.slot,
                recipe_id: e.recipe_id,
                servings: e.servings,
            });
        }
        let now = now_str();
        let template = MenuTemplate {
            id: new_id(),
            name,
            entries: tpl_entries,
            created_at: now.clone(),
            updated_at: now,
        };
        let template = repos::meals::create_menu_template(&self.db, &template).await?;
        self.audit(
            "created",
            "menu_template",
            Some(&template.id),
            &format!("Menú guardado: {}", template.name),
            actor,
            Some(serde_json::json!({"op": "meals_delete_menu_template", "id": template.id})),
        )
        .await?;
        Ok(template)
    }

    pub async fn list_menu_templates(&self) -> Result<Vec<MenuTemplate>> {
        repos::meals::list_menu_templates(&self.db).await
    }

    pub async fn get_menu_template(&self, id: &str) -> Result<MenuTemplateDetail> {
        let t = repos::meals::get_menu_template(&self.db, id).await?;
        let mut entries = Vec::with_capacity(t.entries.len());
        for entry in t.entries {
            let recipe_name = repos::meals::get_recipe(&self.db, &entry.recipe_id)
                .await
                .ok()
                .map(|r| r.name);
            entries.push(TemplateEntryView { entry, recipe_name });
        }
        Ok(MenuTemplateDetail {
            id: t.id,
            name: t.name,
            entries,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
    }

    pub async fn rename_menu_template(
        &self,
        id: &str,
        name: &str,
        actor: &str,
    ) -> Result<MenuTemplate> {
        let name = validate::title(name, "el nombre del menú")?;
        let t = repos::meals::rename_menu_template(&self.db, id, &name).await?;
        self.audit(
            "updated",
            "menu_template",
            Some(id),
            &format!("Menú renombrado: {}", t.name),
            actor,
            None,
        )
        .await?;
        Ok(t)
    }

    pub async fn delete_menu_template(&self, id: &str, actor: &str) -> Result<()> {
        let t = repos::meals::get_menu_template(&self.db, id).await?;
        repos::meals::delete_menu_template(&self.db, id).await?;
        self.audit(
            "deleted",
            "menu_template",
            Some(id),
            &format!("Menú eliminado: {}", t.name),
            actor,
            None,
        )
        .await?;
        Ok(())
    }

    /// Aplica un menú plantilla a otra semana (sobrescribe; confirma la UI).
    /// Las recetas eliminadas se omiten y se reportan en `skipped`.
    pub async fn apply_menu_template(
        &self,
        id: &str,
        week_start: &str,
        actor: &str,
    ) -> Result<(Vec<MealPlanEntry>, Vec<String>)> {
        let t = repos::meals::get_menu_template(&self.db, id).await?;
        let week_start = monday_of(week_start)?;
        let mut inputs = Vec::new();
        let mut skipped = Vec::new();
        for entry in &t.entries {
            if !(0..=6).contains(&entry.day_offset) {
                continue;
            }
            match repos::meals::get_recipe(&self.db, &entry.recipe_id).await {
                Ok(_) => inputs.push(PlanEntryInput {
                    date: date_plus_days(&week_start, entry.day_offset)?,
                    slot: entry.slot,
                    recipe_id: entry.recipe_id.clone(),
                    servings: entry.servings,
                }),
                Err(_) => skipped.push(entry.recipe_id.clone()),
            }
        }
        let created = self
            .apply_plan_entries(
                &week_start,
                inputs,
                true,
                actor,
                &format!("Menú aplicado: {}", t.name),
            )
            .await?;
        Ok((created, skipped))
    }

    // -------------------------------------------------------------------------
    // Lista de la compra
    // -------------------------------------------------------------------------

    /// Consolida los ingredientes de una semana: mismo nombre normalizado +
    /// misma unidad compatible → suma; unidades distintas → líneas separadas.
    /// Nunca convierte unidades ni mezcla productos distintos.
    pub async fn consolidate_week(&self, week_start: &str) -> Result<Vec<ConsolidatedItem>> {
        let week_start = monday_of(week_start)?;
        let entries = repos::meals::list_plan_entries(&self.db, &week_start).await?;
        let mut groups: BTreeMap<String, ConsolidatedItem> = BTreeMap::new();
        for entry in entries {
            let recipe = self.get_recipe(&entry.recipe_id).await?;
            for ing in &recipe.ingredients {
                let name_key = normalize_name(&ing.name);
                let uk = unit_key(&ing.unit);
                let key = format!("{name_key}|{uk}");
                let scaled = scale_quantity(ing.quantity, recipe.servings, entry.servings);
                let item = groups.entry(key).or_insert_with(|| ConsolidatedItem {
                    name: ing.name.clone(),
                    name_key: name_key.clone(),
                    quantity: 0.0,
                    unit: ing.unit.clone(),
                    category: ing.category.clone(),
                });
                item.quantity = round2(item.quantity + scaled);
            }
        }
        Ok(groups.into_values().collect())
    }

    pub async fn generate_shopping_list(
        &self,
        week_start: &str,
        name: Option<&str>,
        list_id: Option<&str>,
        actor: &str,
    ) -> Result<ShoppingListDetail> {
        let week_start = monday_of(week_start)?;
        let consolidated = self.consolidate_week(&week_start).await?;
        let list = match list_id {
            Some(id) => {
                let l = repos::meals::get_shopping_list(&self.db, id).await?;
                repos::meals::delete_generated_items(&self.db, id).await?;
                repos::meals::rename_shopping_list(&self.db, id, name.unwrap_or(&l.name)).await?
            }
            None => {
                let now = now_str();
                let l = ShoppingList {
                    id: new_id(),
                    name: name
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Compra · semana del {week_start}")),
                    week_start: Some(week_start.clone()),
                    has_manual_changes: false,
                    created_at: now.clone(),
                    updated_at: now,
                };
                repos::meals::create_shopping_list(&self.db, &l).await?
            }
        };
        let mut position = repos::meals::next_item_position(&self.db, &list.id).await?;
        for c in consolidated {
            let now = now_str();
            let item = ShoppingItem {
                id: new_id(),
                list_id: list.id.clone(),
                name: c.name,
                name_key: c.name_key,
                quantity: c.quantity,
                unit: c.unit,
                category: c.category,
                checked: false,
                manual: false,
                position,
                created_at: now.clone(),
                updated_at: now,
            };
            repos::meals::add_shopping_item(&self.db, &item).await?;
            position += 1000;
        }
        let detail = self.get_shopping_list(&list.id).await?;
        self.audit(
            "created",
            "shopping_list",
            Some(&list.id),
            &format!("Lista de la compra generada: {}", detail.list.name),
            actor,
            Some(serde_json::json!({"op": "meals_delete_shopping_list", "id": list.id})),
        )
        .await?;
        Ok(detail)
    }

    pub async fn list_shopping_lists(&self) -> Result<Vec<ShoppingList>> {
        repos::meals::list_shopping_lists(&self.db).await
    }

    pub async fn get_shopping_list(&self, id: &str) -> Result<ShoppingListDetail> {
        let list = repos::meals::get_shopping_list(&self.db, id).await?;
        let items = repos::meals::list_shopping_items(&self.db, id).await?;
        Ok(ShoppingListDetail { list, items })
    }

    pub async fn rename_shopping_list(&self, id: &str, name: &str) -> Result<ShoppingList> {
        let name = validate::title(name, "el nombre de la lista")?;
        repos::meals::rename_shopping_list(&self.db, id, &name).await
    }

    pub async fn delete_shopping_list(&self, id: &str, actor: &str) -> Result<()> {
        let l = repos::meals::get_shopping_list(&self.db, id).await?;
        repos::meals::delete_shopping_list(&self.db, id).await?;
        self.audit(
            "deleted",
            "shopping_list",
            Some(id),
            &format!("Lista eliminada: {}", l.name),
            actor,
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn add_shopping_item(
        &self,
        list_id: &str,
        mut data: CreateShoppingItem,
        actor: &str,
    ) -> Result<ShoppingItem> {
        repos::meals::get_shopping_list(&self.db, list_id).await?;
        data.name = validate::title(&data.name, "el nombre del producto")?;
        data.quantity = Some(quantity_valid(data.quantity.unwrap_or(1.0), true)?);
        let unit = data
            .unit
            .unwrap_or_else(|| "ud".into())
            .trim()
            .to_lowercase();
        if unit.is_empty() {
            return Err(NexusError::validation("el producto necesita una unidad"));
        }
        data.unit = Some(unit);
        let category = data
            .category
            .unwrap_or_else(|| "otros".into())
            .trim()
            .to_lowercase();
        data.category = Some(if category.is_empty() {
            "otros".into()
        } else {
            category
        });
        let position = repos::meals::next_item_position(&self.db, list_id).await?;
        let item = repos::meals::add_shopping_item(
            &self.db,
            &repos::meals::shopping_item_from(new_id(), list_id.to_string(), data, position),
        )
        .await?;
        repos::meals::mark_list_manual(&self.db, list_id).await?;
        self.audit(
            "created",
            "shopping_list",
            Some(list_id),
            &format!("Producto añadido a la compra: {}", item.name),
            actor,
            None,
        )
        .await?;
        Ok(item)
    }

    pub async fn update_shopping_item(
        &self,
        id: &str,
        mut patch: UpdateShoppingItem,
    ) -> Result<ShoppingItem> {
        let before = repos::meals::get_shopping_item(&self.db, id).await?;
        if let Some(n) = &patch.name {
            patch.name = Some(validate::title(n, "el nombre del producto")?);
        }
        if let Some(q) = patch.quantity {
            patch.quantity = Some(quantity_valid(q, true)?);
        }
        if let Some(u) = &patch.unit {
            let u = u.trim().to_lowercase();
            if u.is_empty() {
                return Err(NexusError::validation("el producto necesita una unidad"));
            }
            patch.unit = Some(u);
        }
        let item = repos::meals::update_shopping_item(&self.db, id, patch).await?;
        if item.name != before.name
            || item.quantity != before.quantity
            || item.unit != before.unit
            || item.category != before.category
        {
            repos::meals::mark_list_manual(&self.db, &item.list_id).await?;
        }
        Ok(item)
    }

    pub async fn delete_shopping_item(&self, id: &str) -> Result<()> {
        let item = repos::meals::get_shopping_item(&self.db, id).await?;
        repos::meals::delete_shopping_item(&self.db, id).await?;
        repos::meals::mark_list_manual(&self.db, &item.list_id).await?;
        Ok(())
    }

    /// Exporta la lista como texto plano agrupado por categoría o CSV.
    pub async fn export_shopping_list(&self, id: &str, format: &str) -> Result<ShoppingExport> {
        let detail = self.get_shopping_list(id).await?;
        let slug: String = detail
            .list
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let slug = slug.trim_matches('-').to_string();
        match format {
            "csv" => {
                let mut wtr = csv::Writer::from_writer(Vec::new());
                wtr.write_record(["categoria", "producto", "cantidad", "unidad", "comprado"])
                    .map_err(|e| NexusError::Internal(e.to_string()))?;
                for item in &detail.items {
                    let checked = if item.checked { "sí" } else { "no" };
                    wtr.write_record([
                        item.category.as_str(),
                        item.name.as_str(),
                        &fmt_qty(item.quantity),
                        item.unit.as_str(),
                        checked,
                    ])
                    .map_err(|e| NexusError::Internal(e.to_string()))?;
                }
                let bytes = wtr
                    .into_inner()
                    .map_err(|e| NexusError::Internal(e.to_string()))?;
                Ok(ShoppingExport {
                    filename: format!("{slug}.csv"),
                    content: String::from_utf8(bytes)
                        .map_err(|e| NexusError::Internal(e.to_string()))?,
                })
            }
            _ => Ok(ShoppingExport {
                filename: format!("{slug}.txt"),
                content: shopping_plain_text(&detail),
            }),
        }
    }

    // -------------------------------------------------------------------------
    // Despensa
    // -------------------------------------------------------------------------

    pub async fn list_pantry(&self, f: PantryFilter) -> Result<Vec<PantryItem>> {
        repos::meals::list_pantry(&self.db, &f).await
    }

    async fn validate_pantry(
        &self,
        name: &str,
        quantity: Option<f64>,
        unit: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<()> {
        validate::title(name, "el nombre del producto")?;
        quantity_valid(quantity.unwrap_or(0.0), true)?;
        if let Some(u) = unit {
            if u.trim().is_empty() {
                return Err(NexusError::validation("el producto necesita una unidad"));
            }
        }
        if let Some(d) = expires_at {
            date_str(d)?;
        }
        Ok(())
    }

    pub async fn create_pantry_item(
        &self,
        mut data: CreatePantryItem,
        actor: &str,
    ) -> Result<PantryItem> {
        self.validate_pantry(
            &data.name,
            data.quantity,
            data.unit.as_deref(),
            data.expires_at.as_deref(),
        )
        .await?;
        data.name = data.name.trim().to_string();
        data.quantity = Some(quantity_valid(data.quantity.unwrap_or(0.0), true)?);
        if let Some(u) = &data.unit {
            data.unit = Some(u.trim().to_lowercase());
        }
        if let Some(c) = &data.category {
            let c = c.trim().to_lowercase();
            data.category = Some(if c.is_empty() { "otros".into() } else { c });
        }
        let item = repos::meals::create_pantry_item(
            &self.db,
            &repos::meals::pantry_item_from(new_id(), data),
        )
        .await?;
        self.audit(
            "created",
            "pantry",
            Some(&item.id),
            &format!("Despensa: añadido {}", item.name),
            actor,
            Some(serde_json::json!({"op": "meals_delete_pantry_item", "id": item.id})),
        )
        .await?;
        Ok(item)
    }

    pub async fn update_pantry_item(
        &self,
        id: &str,
        mut patch: UpdatePantryItem,
        actor: &str,
    ) -> Result<PantryItem> {
        let before = repos::meals::get_pantry_item(&self.db, id).await?;
        if let Some(n) = &patch.name {
            validate::title(n, "el nombre del producto")?;
            patch.name = Some(n.trim().to_string());
        }
        if let Some(q) = patch.quantity {
            patch.quantity = Some(quantity_valid(q, true)?);
        }
        if let Some(u) = &patch.unit {
            let u = u.trim().to_lowercase();
            if u.is_empty() {
                return Err(NexusError::validation("el producto necesita una unidad"));
            }
            patch.unit = Some(u);
        }
        if let Some(Some(d)) = &patch.expires_at {
            patch.expires_at = Some(Some(date_str(d)?));
        }
        let item = repos::meals::update_pantry_item(&self.db, id, patch).await?;
        self.audit(
            "updated",
            "pantry",
            Some(id),
            &format!("Despensa: actualizado {}", item.name),
            actor,
            Some(serde_json::json!({"op": "meals_restore_pantry_item", "item": before})),
        )
        .await?;
        Ok(item)
    }

    /// Ajuste rápido (+/-) de cantidad. Nunca baja de 0.
    pub async fn adjust_pantry(&self, id: &str, delta: f64, actor: &str) -> Result<PantryItem> {
        if !delta.is_finite() {
            return Err(NexusError::validation("ajuste inválido"));
        }
        let item = repos::meals::get_pantry_item(&self.db, id).await?;
        let new_qty = round2((item.quantity + delta).max(0.0));
        self.update_pantry_item(
            id,
            UpdatePantryItem {
                quantity: Some(new_qty),
                ..Default::default()
            },
            actor,
        )
        .await
    }

    /// Registra consumo: resta cantidad; si llega a 0 y `remove_if_empty`,
    /// elimina el producto.
    pub async fn consume_pantry(
        &self,
        id: &str,
        amount: f64,
        remove_if_empty: bool,
        actor: &str,
    ) -> Result<PantryConsumeResult> {
        let amount = quantity_valid(amount, false)?;
        let item = repos::meals::get_pantry_item(&self.db, id).await?;
        let remaining = round2((item.quantity - amount).max(0.0));
        if remaining <= 0.0 && remove_if_empty {
            repos::meals::delete_pantry_item(&self.db, id).await?;
            self.audit(
                "deleted",
                "pantry",
                Some(id),
                &format!("Despensa: consumido y agotado {}", item.name),
                actor,
                Some(serde_json::json!({"op": "meals_restore_pantry_item", "item": item})),
            )
            .await?;
            return Ok(PantryConsumeResult {
                deleted: true,
                item: None,
            });
        }
        let updated = self
            .update_pantry_item(
                id,
                UpdatePantryItem {
                    quantity: Some(remaining),
                    ..Default::default()
                },
                actor,
            )
            .await?;
        Ok(PantryConsumeResult {
            deleted: false,
            item: Some(updated),
        })
    }

    pub async fn delete_pantry_item(&self, id: &str, actor: &str) -> Result<()> {
        let item = repos::meals::get_pantry_item(&self.db, id).await?;
        repos::meals::delete_pantry_item(&self.db, id).await?;
        self.audit(
            "deleted",
            "pantry",
            Some(id),
            &format!("Despensa: eliminado {}", item.name),
            actor,
            Some(serde_json::json!({"op": "meals_restore_pantry_item", "item": item})),
        )
        .await?;
        Ok(())
    }

    pub async fn restore_pantry_item(&self, item: PantryItem) -> Result<PantryItem> {
        repos::meals::create_pantry_item(&self.db, &item).await
    }

    /// "¿Qué puedo cocinar?": cobertura de cada receta con la despensa actual
    /// sobre las raciones base. Orden: mayor % de cobertura primero.
    pub async fn pantry_matches(&self) -> Result<Vec<PantryMatch>> {
        let recipes = repos::meals::list_recipes(&self.db, &RecipeFilter::default()).await?;
        let pantry = repos::meals::list_pantry(&self.db, &PantryFilter::default()).await?;
        let today = today_date();
        let mut stock: BTreeMap<String, f64> = BTreeMap::new();
        for item in &pantry {
            if item.quantity <= 0.0 {
                continue;
            }
            if let Some(exp) = &item.expires_at {
                if exp.as_str() < today.as_str() {
                    continue;
                }
            }
            let key = format!("{}|{}", item.name_key, unit_key(&item.unit));
            *stock.entry(key).or_insert(0.0) += item.quantity;
        }
        let mut matches = Vec::new();
        for recipe in recipes {
            if recipe.ingredients.is_empty() {
                continue;
            }
            let mut needed: BTreeMap<String, (String, String, f64)> = BTreeMap::new();
            for ing in &recipe.ingredients {
                let key = format!("{}|{}", normalize_name(&ing.name), unit_key(&ing.unit));
                let e = needed
                    .entry(key)
                    .or_insert_with(|| (ing.name.clone(), ing.unit.clone(), 0.0));
                e.2 = round2(e.2 + ing.quantity);
            }
            let total = needed.len();
            let mut covered = Vec::new();
            let mut missing = Vec::new();
            for (key, (name, unit, qty)) in needed {
                let have = round2(stock.get(&key).copied().unwrap_or(0.0));
                if have + 1e-9 >= qty {
                    covered.push(name);
                } else {
                    missing.push(MissingIngredient {
                        name,
                        needed: qty,
                        have,
                        unit,
                    });
                }
            }
            let coverage_pct = round2(covered.len() as f64 / total as f64 * 100.0);
            matches.push(PantryMatch {
                recipe_id: recipe.id.clone(),
                recipe_name: recipe.name.clone(),
                image_file_id: recipe.image_file_id.clone(),
                coverage_pct,
                covered,
                missing,
            });
        }
        matches.sort_by(|a, b| {
            b.coverage_pct
                .partial_cmp(&a.coverage_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.recipe_name
                        .to_lowercase()
                        .cmp(&b.recipe_name.to_lowercase())
                })
        });
        Ok(matches)
    }

    // -------------------------------------------------------------------------
    // IA opcional: solo propuestas; nunca escribe sin confirmación del usuario
    // -------------------------------------------------------------------------

    /// Convierte una propuesta JSON de la IA en CreateRecipe saneado.
    /// La nutrición se descarta SIEMPRE: no se aceptan valores inventados.
    pub fn sanitize_ai_recipe(value: &serde_json::Value) -> Result<CreateRecipe> {
        let mut data: CreateRecipe = serde_json::from_value(value.clone())
            .map_err(|e| NexusError::Ai(format!("la propuesta de receta no es válida: {e}")))?;
        data.nutrition = None;
        data.image_file_id = None;
        data.favorite = Some(false);
        Ok(data)
    }

    /// Valida una propuesta de semana de la IA: solo recetas EXISTENTES.
    pub async fn sanitize_ai_week(
        &self,
        week_start: &str,
        value: &serde_json::Value,
    ) -> Result<Vec<PlanEntryInput>> {
        let week_start = monday_of(week_start)?;
        let entries = value
            .get("entries")
            .and_then(|v| v.as_array())
            .ok_or_else(|| NexusError::Ai("la propuesta no contiene 'entries'".into()))?;
        let mut out = Vec::new();
        for e in entries {
            let day_offset = e
                .get("day_offset")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| NexusError::Ai("entrada sin day_offset".into()))?;
            if !(0..=6).contains(&day_offset) {
                return Err(NexusError::Ai("day_offset fuera de 0..6".into()));
            }
            let slot_str = e
                .get("slot")
                .and_then(|v| v.as_str())
                .ok_or_else(|| NexusError::Ai("entrada sin slot".into()))?;
            let slot = MealSlot::parse(slot_str)
                .ok_or_else(|| NexusError::Ai(format!("slot desconocido: {slot_str}")))?;
            let recipe_id = e
                .get("recipe_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| NexusError::Ai("entrada sin recipe_id".into()))?;
            self.get_recipe(recipe_id).await.map_err(|_| {
                NexusError::Ai(format!("la IA propuso una receta inexistente: {recipe_id}"))
            })?;
            let servings = e.get("servings").and_then(|v| v.as_i64()).unwrap_or(2);
            out.push(PlanEntryInput {
                date: date_plus_days(&week_start, day_offset)?,
                slot,
                recipe_id: recipe_id.to_string(),
                servings: servings_valid(servings)?,
            });
        }
        if out.is_empty() {
            return Err(NexusError::Ai("la propuesta está vacía".into()));
        }
        Ok(out)
    }

    async fn audit(
        &self,
        kind: &str,
        entity_type: &str,
        entity_id: Option<&str>,
        summary: &str,
        actor: &str,
        undo: Option<serde_json::Value>,
    ) -> Result<()> {
        repos::activity::record(&self.db, kind, entity_type, entity_id, summary, actor, undo)
            .await?;
        Ok(())
    }
}

/// Ítem consolidado de la lista de la compra (nombre + unidad compatible).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsolidatedItem {
    pub name: String,
    pub name_key: String,
    pub quantity: f64,
    pub unit: String,
    pub category: String,
}

fn fmt_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q as i64)
    } else {
        format!("{q:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// Texto plano de la lista agrupado por categoría (portapapeles y .txt).
pub fn shopping_plain_text(detail: &ShoppingListDetail) -> String {
    let mut by_cat: BTreeMap<String, Vec<&ShoppingItem>> = BTreeMap::new();
    for item in &detail.items {
        by_cat.entry(item.category.clone()).or_default().push(item);
    }
    let mut out = format!("{}\n", detail.list.name);
    if let Some(w) = &detail.list.week_start {
        out.push_str(&format!("Semana del {w}\n"));
    }
    for (cat, items) in by_cat {
        let title = cat
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 {
                    c.to_uppercase().to_string()
                } else {
                    c.to_string()
                }
            })
            .collect::<String>();
        out.push_str(&format!("\n{title}\n"));
        for item in items {
            out.push_str(&format!(
                "[{}] {} — {} {}\n",
                if item.checked { "x" } else { " " },
                item.name,
                fmt_qty(item.quantity),
                item.unit
            ));
        }
    }
    out
}

async fn sqlx_restore_recipe(db: &Db, r: &Recipe) -> Result<Recipe> {
    use nexus_db::DbResultExt;
    let res = sqlx::query(
        "UPDATE recipes SET name=?, description=?, image_file_id=?, servings=?, prep_time_min=?, cook_time_min=?, difficulty=?, categories=?, tags=?, ingredients_json=?, steps_json=?, nutrition_json=?, favorite=?, updated_at=?, deleted_at=NULL WHERE id=?",
    )
    .bind(&r.name)
    .bind(&r.description)
    .bind(&r.image_file_id)
    .bind(r.servings)
    .bind(r.prep_time_min)
    .bind(r.cook_time_min)
    .bind(r.difficulty.as_str())
    .bind(serde_json::to_string(&r.categories).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&r.tags).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&r.ingredients).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&r.steps).unwrap_or_else(|_| "[]".into()))
    .bind(r.nutrition.as_ref().map(|n| serde_json::to_string(n).unwrap_or_default()))
    .bind(r.favorite as i64)
    .bind(now_str())
    .bind(&r.id)
    .execute(&db.pool)
    .await
    .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("receta", &r.id));
    }
    repos::meals::get_recipe(db, &r.id).await
}
