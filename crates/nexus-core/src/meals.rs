//! Dominio de NEXUS Meal Planner: recetario, planificador semanal,
//! menús plantilla, listas de la compra y despensa.
//! Las fechas de dominio son strings "YYYY-MM-DD"; las marcas de tiempo
//! de auditoría usan `now_str()` (RFC3339). La nutrición es OPCIONAL y
//! solo la introduce el usuario: nunca se inventa ni se estima por defecto.

use serde::{Deserialize, Serialize};

use crate::error::{NexusError, Result};
use crate::types::Id;

macro_rules! str_enum {
    ($name:ident, $($variant:ident => $s:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
            pub fn all() -> &'static [&'static str] {
                &[$($s),+]
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

str_enum!(RecipeDifficulty, Facil => "facil", Media => "media", Dificil => "dificil");
str_enum!(MealSlot, Desayuno => "desayuno", Almuerzo => "almuerzo", Comida => "comida", Merienda => "merienda", Cena => "cena");
str_enum!(PantryLocation, Despensa => "despensa", Nevera => "nevera", Congelador => "congelador", Otro => "otro");
str_enum!(RecipeSort, Name => "name", Time => "time", Recent => "recent");

#[allow(clippy::derivable_impls)]
impl Default for RecipeDifficulty {
    fn default() -> Self {
        Self::Facil
    }
}
#[allow(clippy::derivable_impls)]
impl Default for PantryLocation {
    fn default() -> Self {
        Self::Despensa
    }
}
#[allow(clippy::derivable_impls)]
impl Default for RecipeSort {
    fn default() -> Self {
        Self::Recent
    }
}

// ---------------------------------------------------------------------------
// Utilidades de dominio (compartidas por servicio, repos y tests)
// ---------------------------------------------------------------------------

/// Normaliza un nombre de ingrediente/producto: minúsculas + trim.
pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Clave de unidad compatible. Solo agrupa alias de la MISMA unidad
/// (g/gr/gramos…); nunca convierte entre unidades distintas (g ≠ kg).
pub fn unit_key(unit: &str) -> String {
    let u = unit.trim().to_lowercase();
    match u.as_str() {
        "g" | "gr" | "grs" | "gramo" | "gramos" => "g",
        "kg" | "kgs" | "kilo" | "kilos" | "kilogramo" | "kilogramos" => "kg",
        "ml" | "mililitro" | "mililitros" => "ml",
        "l" | "litro" | "litros" => "l",
        "ud" | "uds" | "u" | "unidad" | "unidades" => "ud",
        "cda" | "cucharada" | "cucharadas" => "cda",
        "cdta" | "cucharadita" | "cucharaditas" => "cdta",
        "taza" | "tazas" => "taza",
        "pizca" | "pizcas" => "pizca",
        other => other,
    }
    .to_string()
}

/// Redondeo sensato a 2 decimales para cantidades escaladas.
pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Escala una cantidad de `base` raciones a `target` raciones.
pub fn scale_quantity(quantity: f64, base: i64, target: i64) -> f64 {
    if base <= 0 {
        return quantity;
    }
    round2(quantity * target as f64 / base as f64)
}

/// Lunes de la semana que contiene `date` (YYYY-MM-DD). Ancla del planner.
pub fn monday_of(date: &str) -> Result<String> {
    use chrono::Datelike;
    let d = chrono::NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .map_err(|_| NexusError::validation(format!("fecha inválida: {date} (usa YYYY-MM-DD)")))?;
    let monday = d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64);
    Ok(monday.format("%Y-%m-%d").to_string())
}

/// Suma días a una fecha YYYY-MM-DD.
pub fn date_plus_days(date: &str, days: i64) -> Result<String> {
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| NexusError::validation(format!("fecha inválida: {date}")))?;
    Ok((d + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string())
}

/// Días entre dos fechas YYYY-MM-DD (b - a).
pub fn days_between(a: &str, b: &str) -> Result<i64> {
    let da = chrono::NaiveDate::parse_from_str(a, "%Y-%m-%d")
        .map_err(|_| NexusError::validation(format!("fecha inválida: {a}")))?;
    let db = chrono::NaiveDate::parse_from_str(b, "%Y-%m-%d")
        .map_err(|_| NexusError::validation(format!("fecha inválida: {b}")))?;
    Ok((db - da).num_days())
}

pub fn today_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

// ---------------------------------------------------------------------------
// Recetas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ingredient {
    pub name: String,
    pub quantity: f64,
    pub unit: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub notes: String,
}

pub fn default_category() -> String {
    "otros".to_string()
}

/// Valores por ración introducidos por el usuario. Nunca estimados por NEXUS.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Nutrition {
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub image_file_id: Option<Id>,
    pub servings: i64,
    pub prep_time_min: i64,
    pub cook_time_min: i64,
    pub difficulty: RecipeDifficulty,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub ingredients: Vec<Ingredient>,
    pub steps: Vec<String>,
    pub nutrition: Option<Nutrition>,
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Recipe {
    pub fn total_time_min(&self) -> i64 {
        self.prep_time_min + self.cook_time_min
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateRecipe {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub image_file_id: Option<Id>,
    pub servings: Option<i64>,
    pub prep_time_min: Option<i64>,
    pub cook_time_min: Option<i64>,
    pub difficulty: Option<RecipeDifficulty>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ingredients: Vec<Ingredient>,
    #[serde(default)]
    pub steps: Vec<String>,
    pub nutrition: Option<Nutrition>,
    pub favorite: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateRecipe {
    pub name: Option<String>,
    pub description: Option<String>,
    pub image_file_id: Option<Option<Id>>,
    pub servings: Option<i64>,
    pub prep_time_min: Option<i64>,
    pub cook_time_min: Option<i64>,
    pub difficulty: Option<RecipeDifficulty>,
    pub categories: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub ingredients: Option<Vec<Ingredient>>,
    pub steps: Option<Vec<String>>,
    pub nutrition: Option<Option<Nutrition>>,
    pub favorite: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecipeFilter {
    /// Busca por nombre o ingrediente.
    pub search: Option<String>,
    pub category: Option<String>,
    pub difficulty: Option<RecipeDifficulty>,
    pub favorites_only: Option<bool>,
    /// Tiempo total máximo (preparación + cocción) en minutos.
    pub max_time_min: Option<i64>,
    pub sort: Option<RecipeSort>,
}

/// Vista ligera para referencias (Canvas recipe_ref, tarjetas del planner).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipePreview {
    pub id: Id,
    pub name: String,
    pub image_file_id: Option<Id>,
    pub difficulty: RecipeDifficulty,
    pub total_time: i64,
}

// ---------------------------------------------------------------------------
// Planificador semanal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealPlanEntry {
    pub id: Id,
    /// Lunes de la semana, YYYY-MM-DD.
    pub week_start: String,
    /// Día concreto dentro de la semana, YYYY-MM-DD.
    pub date: String,
    pub slot: MealSlot,
    pub recipe_id: Id,
    pub servings: i64,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntryView {
    #[serde(flatten)]
    pub entry: MealPlanEntry,
    pub recipe_name: String,
    pub recipe_image_file_id: Option<Id>,
    pub recipe_difficulty: RecipeDifficulty,
    pub total_time_min: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekPlan {
    pub week_start: String,
    pub entries: Vec<PlanEntryView>,
}

/// Entrada de un menú plantilla: día relativo al lunes (0..6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub day_offset: i64,
    pub slot: MealSlot,
    pub recipe_id: Id,
    pub servings: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntryView {
    #[serde(flatten)]
    pub entry: TemplateEntry,
    /// None si la receta fue eliminada después de guardar la plantilla.
    pub recipe_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuTemplate {
    pub id: Id,
    pub name: String,
    pub entries: Vec<TemplateEntry>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuTemplateDetail {
    pub id: Id,
    pub name: String,
    pub entries: Vec<TemplateEntryView>,
    pub created_at: String,
    pub updated_at: String,
}

/// Entrada propuesta para aplicar en bloque (IA o menús).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntryInput {
    pub date: String,
    pub slot: MealSlot,
    pub recipe_id: Id,
    pub servings: i64,
}

// ---------------------------------------------------------------------------
// Lista de la compra
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingList {
    pub id: Id,
    pub name: String,
    pub week_start: Option<String>,
    pub has_manual_changes: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingItem {
    pub id: Id,
    pub list_id: Id,
    pub name: String,
    pub name_key: String,
    pub quantity: f64,
    pub unit: String,
    pub category: String,
    pub checked: bool,
    pub manual: bool,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingListDetail {
    pub list: ShoppingList,
    pub items: Vec<ShoppingItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateShoppingItem {
    pub name: String,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateShoppingItem {
    pub name: Option<String>,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub checked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingExport {
    pub filename: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Despensa
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PantryItem {
    pub id: Id,
    pub name: String,
    pub name_key: String,
    pub quantity: f64,
    pub unit: String,
    pub category: String,
    /// Fecha de caducidad YYYY-MM-DD introducida por el usuario (opcional).
    pub expires_at: Option<String>,
    pub location: PantryLocation,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePantryItem {
    pub name: String,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub expires_at: Option<String>,
    pub location: Option<PantryLocation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePantryItem {
    pub name: Option<String>,
    pub quantity: Option<f64>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub expires_at: Option<Option<String>>,
    pub location: Option<PantryLocation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PantryFilter {
    pub search: Option<String>,
    pub location: Option<PantryLocation>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PantryConsumeResult {
    pub deleted: bool,
    pub item: Option<PantryItem>,
}

/// Ingrediente sin cobertura suficiente en despensa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingIngredient {
    pub name: String,
    pub needed: f64,
    pub have: f64,
    pub unit: String,
}

/// "¿Qué puedo cocinar?": cobertura de una receta con la despensa actual,
/// calculada sobre las raciones base de la receta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PantryMatch {
    pub recipe_id: Id,
    pub recipe_name: String,
    pub image_file_id: Option<Id>,
    pub coverage_pct: f64,
    pub covered: Vec<String>,
    pub missing: Vec<MissingIngredient>,
}
