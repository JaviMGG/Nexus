//! Repositorio de NEXUS Meal Planner: recetas, plan semanal, menús plantilla,
//! listas de la compra y despensa. Sigue el patrón de repos/automations.rs.

use nexus_core::{
    now_str, unit_key, CreatePantryItem, CreateRecipe, CreateShoppingItem, Id, Ingredient,
    MealPlanEntry, MealSlot, MenuTemplate, NexusError, Nutrition, PantryFilter, PantryItem,
    PantryLocation, Recipe, RecipeDifficulty, RecipeFilter, RecipeSort, Result, ShoppingItem,
    ShoppingList, TemplateEntry, UpdatePantryItem, UpdateRecipe, UpdateShoppingItem,
};
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::{Db, DbResultExt};

fn json_string<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|e| NexusError::Internal(e.to_string()))
}

fn parse_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|e| NexusError::Internal(format!("json corrupto: {e}")))
}

// ---------------------------------------------------------------------------
// Recetas
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct RecipeRow {
    id: String,
    name: String,
    description: String,
    image_file_id: Option<String>,
    servings: i64,
    prep_time_min: i64,
    cook_time_min: i64,
    difficulty: String,
    categories: String,
    tags: String,
    ingredients_json: String,
    steps_json: String,
    nutrition_json: Option<String>,
    favorite: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<RecipeRow> for Recipe {
    type Error = NexusError;

    fn try_from(r: RecipeRow) -> Result<Self> {
        let nutrition = match &r.nutrition_json {
            Some(s) => Some(parse_json::<Nutrition>(s)?),
            None => None,
        };
        Ok(Self {
            id: r.id,
            name: r.name,
            description: r.description,
            image_file_id: r.image_file_id,
            servings: r.servings,
            prep_time_min: r.prep_time_min,
            cook_time_min: r.cook_time_min,
            difficulty: RecipeDifficulty::parse(&r.difficulty).unwrap_or_default(),
            categories: parse_json(&r.categories)?,
            tags: parse_json(&r.tags)?,
            ingredients: parse_json::<Vec<Ingredient>>(&r.ingredients_json)?,
            steps: parse_json(&r.steps_json)?,
            nutrition,
            favorite: r.favorite != 0,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

pub async fn create_recipe(db: &Db, r: &Recipe) -> Result<Recipe> {
    sqlx::query(
        "INSERT INTO recipes (id, name, description, image_file_id, servings, prep_time_min, cook_time_min, difficulty, categories, tags, ingredients_json, steps_json, nutrition_json, favorite, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&r.id)
    .bind(&r.name)
    .bind(&r.description)
    .bind(&r.image_file_id)
    .bind(r.servings)
    .bind(r.prep_time_min)
    .bind(r.cook_time_min)
    .bind(r.difficulty.as_str())
    .bind(json_string(&r.categories)?)
    .bind(json_string(&r.tags)?)
    .bind(json_string(&r.ingredients)?)
    .bind(json_string(&r.steps)?)
    .bind(r.nutrition.as_ref().map(json_string).transpose()?)
    .bind(r.favorite as i64)
    .bind(&r.created_at)
    .bind(&r.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_recipe(db, &r.id).await
}

pub async fn get_recipe(db: &Db, id: &str) -> Result<Recipe> {
    let row: Option<RecipeRow> =
        sqlx::query_as("SELECT * FROM recipes WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Recipe::try_from)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("receta", id))
}

pub async fn list_recipes(db: &Db, f: &RecipeFilter) -> Result<Vec<Recipe>> {
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM recipes WHERE deleted_at IS NULL");
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{}%", s.to_lowercase());
        qb.push(" AND (lower(name) LIKE ")
            .push_bind(like.clone())
            .push(" OR lower(ingredients_json) LIKE ")
            .push_bind(like)
            .push(")");
    }
    if let Some(c) = f
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        qb.push(" AND lower(categories) LIKE ")
            .push_bind(format!("%\"{}\"%", c.to_lowercase()));
    }
    if let Some(d) = &f.difficulty {
        qb.push(" AND difficulty = ").push_bind(d.as_str());
    }
    if f.favorites_only.unwrap_or(false) {
        qb.push(" AND favorite = 1");
    }
    if let Some(t) = f.max_time_min {
        qb.push(" AND (prep_time_min + cook_time_min) <= ")
            .push_bind(t);
    }
    match f.sort.unwrap_or_default() {
        RecipeSort::Name => qb.push(" ORDER BY lower(name) ASC"),
        RecipeSort::Time => {
            qb.push(" ORDER BY (prep_time_min + cook_time_min) ASC, lower(name) ASC")
        }
        RecipeSort::Recent => qb.push(" ORDER BY created_at DESC"),
    };
    let rows: Vec<RecipeRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    rows.into_iter().map(Recipe::try_from).collect()
}

pub async fn write_recipe(db: &Db, r: &Recipe) -> Result<()> {
    sqlx::query(
        "UPDATE recipes SET name=?, description=?, image_file_id=?, servings=?, prep_time_min=?, cook_time_min=?, difficulty=?, categories=?, tags=?, ingredients_json=?, steps_json=?, nutrition_json=?, favorite=?, updated_at=? WHERE id=?",
    )
    .bind(&r.name)
    .bind(&r.description)
    .bind(&r.image_file_id)
    .bind(r.servings)
    .bind(r.prep_time_min)
    .bind(r.cook_time_min)
    .bind(r.difficulty.as_str())
    .bind(json_string(&r.categories)?)
    .bind(json_string(&r.tags)?)
    .bind(json_string(&r.ingredients)?)
    .bind(json_string(&r.steps)?)
    .bind(r.nutrition.as_ref().map(json_string).transpose()?)
    .bind(r.favorite as i64)
    .bind(&r.updated_at)
    .bind(&r.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn update_recipe(db: &Db, id: &str, patch: UpdateRecipe) -> Result<Recipe> {
    let mut r = get_recipe(db, id).await?;
    if let Some(v) = patch.name {
        r.name = v;
    }
    if let Some(v) = patch.description {
        r.description = v;
    }
    if let Some(v) = patch.image_file_id {
        r.image_file_id = v;
    }
    if let Some(v) = patch.servings {
        r.servings = v;
    }
    if let Some(v) = patch.prep_time_min {
        r.prep_time_min = v;
    }
    if let Some(v) = patch.cook_time_min {
        r.cook_time_min = v;
    }
    if let Some(v) = patch.difficulty {
        r.difficulty = v;
    }
    if let Some(v) = patch.categories {
        r.categories = v;
    }
    if let Some(v) = patch.tags {
        r.tags = v;
    }
    if let Some(v) = patch.ingredients {
        r.ingredients = v;
    }
    if let Some(v) = patch.steps {
        r.steps = v;
    }
    if let Some(v) = patch.nutrition {
        r.nutrition = v;
    }
    if let Some(v) = patch.favorite {
        r.favorite = v;
    }
    r.updated_at = now_str();
    write_recipe(db, &r).await?;
    Ok(r)
}

pub async fn soft_delete_recipe(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("UPDATE recipes SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("receta", id));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Plan semanal
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct PlanEntryRow {
    id: String,
    week_start: String,
    date: String,
    slot: String,
    recipe_id: String,
    servings: i64,
    position: i64,
    created_at: String,
    updated_at: String,
}

impl From<PlanEntryRow> for MealPlanEntry {
    fn from(r: PlanEntryRow) -> Self {
        Self {
            id: r.id,
            week_start: r.week_start,
            date: r.date,
            slot: MealSlot::parse(&r.slot).unwrap_or(MealSlot::Comida),
            recipe_id: r.recipe_id,
            servings: r.servings,
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn create_plan_entry(db: &Db, e: &MealPlanEntry) -> Result<MealPlanEntry> {
    sqlx::query(
        "INSERT INTO meal_plan_entries (id, week_start, date, slot, recipe_id, servings, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&e.id)
    .bind(&e.week_start)
    .bind(&e.date)
    .bind(e.slot.as_str())
    .bind(&e.recipe_id)
    .bind(e.servings)
    .bind(e.position)
    .bind(&e.created_at)
    .bind(&e.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_plan_entry(db, &e.id).await
}

pub async fn get_plan_entry(db: &Db, id: &str) -> Result<MealPlanEntry> {
    let row: Option<PlanEntryRow> = sqlx::query_as("SELECT * FROM meal_plan_entries WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(MealPlanEntry::from)
        .ok_or_else(|| NexusError::not_found("comida planificada", id))
}

pub async fn list_plan_entries(db: &Db, week_start: &str) -> Result<Vec<MealPlanEntry>> {
    let rows: Vec<PlanEntryRow> = sqlx::query_as(
        "SELECT * FROM meal_plan_entries WHERE week_start = ? ORDER BY date ASC, slot ASC, position ASC, created_at ASC",
    )
    .bind(week_start)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(MealPlanEntry::from).collect())
}

pub async fn next_plan_position(db: &Db, date: &str, slot: MealSlot) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(position), 0) AS n FROM meal_plan_entries WHERE date = ? AND slot = ?",
    )
    .bind(date)
    .bind(slot.as_str())
    .fetch_one(&db.pool)
    .await
    .db()?;
    Ok(row.try_get::<i64, _>("n").db()? + 1000)
}

pub async fn update_plan_entry(
    db: &Db,
    id: &str,
    date: Option<&str>,
    slot: Option<MealSlot>,
    servings: Option<i64>,
    week_start: Option<&str>,
) -> Result<MealPlanEntry> {
    let mut e = get_plan_entry(db, id).await?;
    if let Some(d) = date {
        e.date = d.to_string();
    }
    if let Some(s) = slot {
        e.slot = s;
    }
    if let Some(s) = servings {
        e.servings = s;
    }
    if let Some(w) = week_start {
        e.week_start = w.to_string();
    }
    e.updated_at = now_str();
    sqlx::query(
        "UPDATE meal_plan_entries SET week_start=?, date=?, slot=?, servings=?, position=?, updated_at=? WHERE id=?",
    )
    .bind(&e.week_start)
    .bind(&e.date)
    .bind(e.slot.as_str())
    .bind(e.servings)
    .bind(e.position)
    .bind(&e.updated_at)
    .bind(&e.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(e)
}

pub async fn delete_plan_entry(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM meal_plan_entries WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("comida planificada", id));
    }
    Ok(())
}

pub async fn clear_plan_week(db: &Db, week_start: &str) -> Result<()> {
    sqlx::query("DELETE FROM meal_plan_entries WHERE week_start = ?")
        .bind(week_start)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Menús plantilla
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct MenuTemplateRow {
    id: String,
    name: String,
    entries_json: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<MenuTemplateRow> for MenuTemplate {
    type Error = NexusError;

    fn try_from(r: MenuTemplateRow) -> Result<Self> {
        Ok(Self {
            id: r.id,
            name: r.name,
            entries: parse_json::<Vec<TemplateEntry>>(&r.entries_json)?,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

pub async fn create_menu_template(db: &Db, t: &MenuTemplate) -> Result<MenuTemplate> {
    sqlx::query(
        "INSERT INTO menu_templates (id, name, entries_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&t.id)
    .bind(&t.name)
    .bind(json_string(&t.entries)?)
    .bind(&t.created_at)
    .bind(&t.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_menu_template(db, &t.id).await
}

pub async fn get_menu_template(db: &Db, id: &str) -> Result<MenuTemplate> {
    let row: Option<MenuTemplateRow> = sqlx::query_as("SELECT * FROM menu_templates WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(MenuTemplate::try_from)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("menú", id))
}

pub async fn list_menu_templates(db: &Db) -> Result<Vec<MenuTemplate>> {
    let rows: Vec<MenuTemplateRow> =
        sqlx::query_as("SELECT * FROM menu_templates ORDER BY created_at DESC")
            .fetch_all(&db.pool)
            .await
            .db()?;
    rows.into_iter().map(MenuTemplate::try_from).collect()
}

pub async fn rename_menu_template(db: &Db, id: &str, name: &str) -> Result<MenuTemplate> {
    sqlx::query("UPDATE menu_templates SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    get_menu_template(db, id).await
}

pub async fn delete_menu_template(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM menu_templates WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("menú", id));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Listas de la compra
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ShoppingListRow {
    id: String,
    name: String,
    week_start: Option<String>,
    has_manual_changes: i64,
    created_at: String,
    updated_at: String,
}

impl From<ShoppingListRow> for ShoppingList {
    fn from(r: ShoppingListRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            week_start: r.week_start,
            has_manual_changes: r.has_manual_changes != 0,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn create_shopping_list(db: &Db, l: &ShoppingList) -> Result<ShoppingList> {
    sqlx::query(
        "INSERT INTO shopping_lists (id, name, week_start, has_manual_changes, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&l.id)
    .bind(&l.name)
    .bind(&l.week_start)
    .bind(l.has_manual_changes as i64)
    .bind(&l.created_at)
    .bind(&l.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_shopping_list(db, &l.id).await
}

pub async fn get_shopping_list(db: &Db, id: &str) -> Result<ShoppingList> {
    let row: Option<ShoppingListRow> = sqlx::query_as("SELECT * FROM shopping_lists WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(ShoppingList::from)
        .ok_or_else(|| NexusError::not_found("lista de la compra", id))
}

pub async fn list_shopping_lists(db: &Db) -> Result<Vec<ShoppingList>> {
    let rows: Vec<ShoppingListRow> =
        sqlx::query_as("SELECT * FROM shopping_lists ORDER BY created_at DESC")
            .fetch_all(&db.pool)
            .await
            .db()?;
    Ok(rows.into_iter().map(ShoppingList::from).collect())
}

pub async fn rename_shopping_list(db: &Db, id: &str, name: &str) -> Result<ShoppingList> {
    sqlx::query("UPDATE shopping_lists SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    get_shopping_list(db, id).await
}

pub async fn mark_list_manual(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE shopping_lists SET has_manual_changes = 1, updated_at = ? WHERE id = ?")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn delete_shopping_list(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM shopping_lists WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("lista de la compra", id));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ShoppingItemRow {
    id: String,
    list_id: String,
    name: String,
    name_key: String,
    quantity: f64,
    unit: String,
    category: String,
    checked: i64,
    manual: i64,
    position: i64,
    created_at: String,
    updated_at: String,
}

impl From<ShoppingItemRow> for ShoppingItem {
    fn from(r: ShoppingItemRow) -> Self {
        Self {
            id: r.id,
            list_id: r.list_id,
            name: r.name,
            name_key: r.name_key,
            quantity: r.quantity,
            unit: r.unit,
            category: r.category,
            checked: r.checked != 0,
            manual: r.manual != 0,
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn add_shopping_item(db: &Db, item: &ShoppingItem) -> Result<ShoppingItem> {
    sqlx::query(
        "INSERT INTO shopping_items (id, list_id, name, name_key, quantity, unit, category, checked, manual, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&item.id)
    .bind(&item.list_id)
    .bind(&item.name)
    .bind(&item.name_key)
    .bind(item.quantity)
    .bind(&item.unit)
    .bind(&item.category)
    .bind(item.checked as i64)
    .bind(item.manual as i64)
    .bind(item.position)
    .bind(&item.created_at)
    .bind(&item.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_shopping_item(db, &item.id).await
}

pub async fn get_shopping_item(db: &Db, id: &str) -> Result<ShoppingItem> {
    let row: Option<ShoppingItemRow> = sqlx::query_as("SELECT * FROM shopping_items WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(ShoppingItem::from)
        .ok_or_else(|| NexusError::not_found("producto", id))
}

pub async fn list_shopping_items(db: &Db, list_id: &str) -> Result<Vec<ShoppingItem>> {
    let rows: Vec<ShoppingItemRow> = sqlx::query_as(
        "SELECT * FROM shopping_items WHERE list_id = ? ORDER BY lower(category) ASC, lower(name) ASC, unit ASC, position ASC",
    )
    .bind(list_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(ShoppingItem::from).collect())
}

pub async fn update_shopping_item(
    db: &Db,
    id: &str,
    patch: UpdateShoppingItem,
) -> Result<ShoppingItem> {
    let mut item = get_shopping_item(db, id).await?;
    if let Some(v) = patch.name {
        item.name = v;
        item.name_key = nexus_core::normalize_name(&item.name);
    }
    if let Some(v) = patch.quantity {
        item.quantity = v;
    }
    if let Some(v) = patch.unit {
        item.unit = v;
    }
    if let Some(v) = patch.category {
        item.category = v;
    }
    if let Some(v) = patch.checked {
        item.checked = v;
    }
    item.updated_at = now_str();
    sqlx::query(
        "UPDATE shopping_items SET name=?, name_key=?, quantity=?, unit=?, category=?, checked=?, updated_at=? WHERE id=?",
    )
    .bind(&item.name)
    .bind(&item.name_key)
    .bind(item.quantity)
    .bind(&item.unit)
    .bind(&item.category)
    .bind(item.checked as i64)
    .bind(&item.updated_at)
    .bind(&item.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(item)
}

pub async fn delete_shopping_item(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM shopping_items WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("producto", id));
    }
    Ok(())
}

pub async fn delete_generated_items(db: &Db, list_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM shopping_items WHERE list_id = ? AND manual = 0")
        .bind(list_id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn next_item_position(db: &Db, list_id: &str) -> Result<i64> {
    let row =
        sqlx::query("SELECT COALESCE(MAX(position), 0) AS n FROM shopping_items WHERE list_id = ?")
            .bind(list_id)
            .fetch_one(&db.pool)
            .await
            .db()?;
    Ok(row.try_get::<i64, _>("n").db()? + 1000)
}

/// Clave de consolidación usada también en tests: nombre normalizado + unidad.
pub fn consolidation_key(name: &str, unit: &str) -> String {
    format!("{}|{}", nexus_core::normalize_name(name), unit_key(unit))
}

// ---------------------------------------------------------------------------
// Despensa
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct PantryRow {
    id: String,
    name: String,
    name_key: String,
    quantity: f64,
    unit: String,
    category: String,
    expires_at: Option<String>,
    location: String,
    created_at: String,
    updated_at: String,
}

impl From<PantryRow> for PantryItem {
    fn from(r: PantryRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            name_key: r.name_key,
            quantity: r.quantity,
            unit: r.unit,
            category: r.category,
            expires_at: r.expires_at,
            location: PantryLocation::parse(&r.location).unwrap_or_default(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn create_pantry_item(db: &Db, item: &PantryItem) -> Result<PantryItem> {
    sqlx::query(
        "INSERT INTO pantry_items (id, name, name_key, quantity, unit, category, expires_at, location, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&item.id)
    .bind(&item.name)
    .bind(&item.name_key)
    .bind(item.quantity)
    .bind(&item.unit)
    .bind(&item.category)
    .bind(&item.expires_at)
    .bind(item.location.as_str())
    .bind(&item.created_at)
    .bind(&item.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_pantry_item(db, &item.id).await
}

pub async fn get_pantry_item(db: &Db, id: &str) -> Result<PantryItem> {
    let row: Option<PantryRow> = sqlx::query_as("SELECT * FROM pantry_items WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(PantryItem::from)
        .ok_or_else(|| NexusError::not_found("producto de despensa", id))
}

pub async fn list_pantry(db: &Db, f: &PantryFilter) -> Result<Vec<PantryItem>> {
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM pantry_items WHERE 1=1");
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        qb.push(" AND lower(name) LIKE ")
            .push_bind(format!("%{}%", s.to_lowercase()));
    }
    if let Some(l) = &f.location {
        qb.push(" AND location = ").push_bind(l.as_str());
    }
    if let Some(c) = f
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        qb.push(" AND lower(category) = ")
            .push_bind(c.to_lowercase());
    }
    qb.push(" ORDER BY lower(category) ASC, lower(name) ASC");
    let rows: Vec<PantryRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(PantryItem::from).collect())
}

pub async fn update_pantry_item(db: &Db, id: &str, patch: UpdatePantryItem) -> Result<PantryItem> {
    let mut item = get_pantry_item(db, id).await?;
    if let Some(v) = patch.name {
        item.name = v;
        item.name_key = nexus_core::normalize_name(&item.name);
    }
    if let Some(v) = patch.quantity {
        item.quantity = v;
    }
    if let Some(v) = patch.unit {
        item.unit = v;
    }
    if let Some(v) = patch.category {
        item.category = v;
    }
    if let Some(v) = patch.expires_at {
        item.expires_at = v;
    }
    if let Some(v) = patch.location {
        item.location = v;
    }
    item.updated_at = now_str();
    sqlx::query(
        "UPDATE pantry_items SET name=?, name_key=?, quantity=?, unit=?, category=?, expires_at=?, location=?, updated_at=? WHERE id=?",
    )
    .bind(&item.name)
    .bind(&item.name_key)
    .bind(item.quantity)
    .bind(&item.unit)
    .bind(&item.category)
    .bind(&item.expires_at)
    .bind(item.location.as_str())
    .bind(&item.updated_at)
    .bind(&item.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(item)
}

pub async fn delete_pantry_item(db: &Db, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM pantry_items WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    if res.rows_affected() == 0 {
        return Err(NexusError::not_found("producto de despensa", id));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DTOs de creación usados por el servicio
// ---------------------------------------------------------------------------

pub fn pantry_item_from(id: Id, data: CreatePantryItem) -> PantryItem {
    let now = now_str();
    let name = data.name;
    PantryItem {
        name_key: nexus_core::normalize_name(&name),
        id,
        name,
        quantity: data.quantity.unwrap_or(0.0),
        unit: data.unit.unwrap_or_else(|| "ud".into()),
        category: data.category.unwrap_or_else(|| "otros".into()),
        expires_at: data.expires_at,
        location: data.location.unwrap_or_default(),
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn shopping_item_from(
    id: Id,
    list_id: Id,
    data: CreateShoppingItem,
    position: i64,
) -> ShoppingItem {
    let now = now_str();
    let name = data.name;
    ShoppingItem {
        name_key: nexus_core::normalize_name(&name),
        id,
        list_id,
        name,
        quantity: data.quantity.unwrap_or(0.0),
        unit: data.unit.unwrap_or_else(|| "ud".into()),
        category: data.category.unwrap_or_else(|| "otros".into()),
        checked: false,
        manual: true,
        position,
        created_at: now.clone(),
        updated_at: now,
    }
}

pub fn recipe_from(id: Id, data: CreateRecipe) -> Recipe {
    let now = now_str();
    Recipe {
        id,
        name: data.name,
        description: data.description,
        image_file_id: data.image_file_id,
        servings: data.servings.unwrap_or(2),
        prep_time_min: data.prep_time_min.unwrap_or(0),
        cook_time_min: data.cook_time_min.unwrap_or(0),
        difficulty: data.difficulty.unwrap_or_default(),
        categories: data.categories,
        tags: data.tags,
        ingredients: data.ingredients,
        steps: data.steps,
        nutrition: data.nutrition,
        favorite: data.favorite.unwrap_or(false),
        created_at: now.clone(),
        updated_at: now,
    }
}
