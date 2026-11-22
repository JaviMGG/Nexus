//! Dominio de NEXUS Database: bases de datos personalizadas con campos
//! dinámicos, registros, vistas guardadas, filtros, relaciones, fórmulas,
//! import/export y plantillas. Todo es serializable a JSON.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

str_enum!(DbFieldKind,
    Text => "text",
    LongText => "long_text",
    Integer => "integer",
    Decimal => "decimal",
    Date => "date",
    Datetime => "datetime",
    Checkbox => "checkbox",
    Select => "select",
    Multiselect => "multiselect",
    Url => "url",
    Email => "email",
    Phone => "phone",
    Image => "image",
    File => "file",
    Rating => "rating",
    Relation => "relation",
    Formula => "formula",
);

str_enum!(DbViewKind,
    Table => "table",
    Kanban => "kanban",
    Gallery => "gallery",
    Calendar => "calendar",
    List => "list",
);

str_enum!(DbCardinality, OneToOne => "one_to_one", OneToMany => "one_to_many", ManyToMany => "many_to_many");

str_enum!(DbFilterOp,
    Contains => "contains",
    NotContains => "not_contains",
    Eq => "eq",
    Ne => "ne",
    Gt => "gt",
    Gte => "gte",
    Lt => "lt",
    Lte => "lte",
    IsEmpty => "is_empty",
    NotEmpty => "not_empty",
);

str_enum!(DbSortDir, Asc => "asc", Desc => "desc");

#[allow(clippy::derivable_impls)]
impl Default for DbViewKind {
    fn default() -> Self {
        Self::Table
    }
}
#[allow(clippy::derivable_impls)]
impl Default for DbCardinality {
    fn default() -> Self {
        Self::ManyToMany
    }
}

// ---------------------------------------------------------------------------
// Configuración por tipo de campo (JSON tolerante: claves opcionales)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DbFieldConfig {
    /// Opciones de select/multiselect.
    pub options: Vec<DbSelectOption>,
    /// Base destino de un campo relation.
    pub target_database_id: Option<Id>,
    pub cardinality: Option<DbCardinality>,
    /// Expresión de un campo formula, p. ej. "ROUND({Precio} * 1.21, 2)".
    pub expression: Option<String>,
    /// Máximo del rating (por defecto 5, rango permitido 1-10).
    pub max_rating: Option<i64>,
    /// Decimales para decimal (solo informativo en UI).
    pub decimals: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSelectOption {
    pub id: Id,
    pub label: String,
    /// Color de la paleta: gray, blue, green, yellow, orange, red, purple, pink.
    #[serde(default = "default_option_color")]
    pub color: String,
}

fn default_option_color() -> String {
    "gray".into()
}

impl DbFieldKind {
    /// ¿El valor se almacena? Las fórmulas se calculan al leer.
    pub fn is_stored(&self) -> bool {
        !matches!(self, Self::Formula)
    }
}

// ---------------------------------------------------------------------------
// Entidades
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateDatabase {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateDatabase {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// Tarjeta del listado y referencia cruzada para otros módulos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSummary {
    #[serde(flatten)]
    pub database: Database,
    pub record_count: i64,
    pub field_count: i64,
    pub view_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbField {
    pub id: Id,
    pub database_id: Id,
    pub name: String,
    pub kind: DbFieldKind,
    pub config: DbFieldConfig,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDbField {
    pub name: String,
    pub kind: DbFieldKind,
    #[serde(default)]
    pub config: DbFieldConfig,
    pub position: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateDbField {
    pub name: Option<String>,
    pub kind: Option<DbFieldKind>,
    pub config: Option<DbFieldConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRecord {
    pub id: Id,
    pub database_id: Id,
    /// field_id -> valor almacenado (sin campos fórmula).
    pub values: serde_json::Map<String, Value>,
    /// field_id -> valor calculado de los campos fórmula.
    pub computed: serde_json::Map<String, Value>,
    /// field_id -> error legible de fórmula (valor null en computed).
    pub formula_errors: serde_json::Map<String, Value>,
    /// field_id -> [{id, title}] de las relaciones, ids huérfanos ya filtrados.
    pub relation_titles: serde_json::Map<String, Value>,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateDbRecord {
    #[serde(default)]
    pub values: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateDbRecord {
    /// Reemplaza solo las claves presentes; null borra el valor del campo.
    pub values: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbView {
    pub id: Id,
    pub database_id: Id,
    pub name: String,
    pub kind: DbViewKind,
    pub config: DbViewConfig,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDbView {
    pub name: String,
    pub kind: DbViewKind,
    #[serde(default)]
    pub config: DbViewConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateDbView {
    pub name: Option<String>,
    pub kind: Option<DbViewKind>,
    pub config: Option<DbViewConfig>,
}

/// Configuración común de las vistas guardadas. Cada renderizador usa lo suyo:
/// kanban → group_field_id (select), calendar → date_field_id,
/// gallery → image_field_id + card_field_ids, table → visible_field_ids.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DbViewConfig {
    pub visible_field_ids: Vec<Id>,
    pub group_field_id: Option<Id>,
    pub date_field_id: Option<Id>,
    pub image_field_id: Option<Id>,
    pub card_field_ids: Vec<Id>,
    pub filters: DbFilterGroup,
    pub sorts: Vec<DbSort>,
}

// ---------------------------------------------------------------------------
// Filtros y ordenación (aplicados en backend con QueryBuilder seguro)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbFilter {
    pub field_id: Id,
    pub op: DbFilterOp,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbFilterGroup {
    /// "and" | "or"
    #[serde(default = "default_conjunction")]
    pub conjunction: String,
    #[serde(default)]
    pub filters: Vec<DbFilter>,
}

fn default_conjunction() -> String {
    "and".into()
}

impl Default for DbFilterGroup {
    fn default() -> Self {
        Self {
            conjunction: default_conjunction(),
            filters: Vec::new(),
        }
    }
}

impl DbFilterGroup {
    pub fn is_or(&self) -> bool {
        self.conjunction.eq_ignore_ascii_case("or")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSort {
    pub field_id: Id,
    #[serde(default = "default_sort_dir")]
    pub dir: DbSortDir,
}

fn default_sort_dir() -> DbSortDir {
    DbSortDir::Asc
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordQuery {
    pub search: Option<String>,
    pub filters: DbFilterGroup,
    pub sorts: Vec<DbSort>,
    /// Resalta/filtra por id concreto (navegación desde chips de relación).
    pub highlight_id: Option<Id>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPage {
    pub records: Vec<DbRecord>,
    pub total: i64,
}

/// Detalle completo que consume la página de una base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDetail {
    #[serde(flatten)]
    pub summary: DatabaseSummary,
    pub fields: Vec<DbField>,
    pub views: Vec<DbView>,
}

/// Referencia ligera para otros módulos (record_preview).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPreview {
    pub record_id: Id,
    pub database_id: Id,
    pub database_name: String,
    pub title: String,
    pub icon: String,
    pub color: String,
}

// ---------------------------------------------------------------------------
// Import / Export
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub columns: Vec<String>,
    /// Primeras 50 filas (arrays alineados con columns).
    pub rows: Vec<Vec<String>>,
    pub total_rows: i64,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMapping {
    pub column: String,
    /// Campo existente destino (excluyente con new_field).
    pub field_id: Option<Id>,
    /// Crear un campo nuevo con este nombre y tipo.
    pub new_field: Option<CreateDbField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub database_id: Id,
    pub format: String,
    pub content: String,
    pub mappings: Vec<ImportMapping>,
    /// true: las filas con errores se omiten; false: aborta si hay alguna.
    #[serde(default)]
    pub skip_invalid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: i64,
    pub skipped: i64,
    pub errors: Vec<ImportRowError>,
    pub created_fields: Vec<DbField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub filename: String,
    pub mime: String,
    pub content: String,
    pub row_count: i64,
}

// ---------------------------------------------------------------------------
// Plantillas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTemplateField {
    pub name: String,
    pub kind: DbFieldKind,
    #[serde(default)]
    pub config: DbFieldConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub fields: Vec<DbTemplateField>,
    /// Registros de ejemplo reales y coherentes (nombre de campo -> valor).
    pub sample_records: Vec<serde_json::Map<String, Value>>,
    pub views: Vec<CreateDbView>,
}

// ---------------------------------------------------------------------------
// IA opcional (propuestas; nunca se aplican sin confirmación del usuario)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFieldProposal {
    pub name: String,
    pub kind: DbFieldKind,
    #[serde(default)]
    pub config: DbFieldConfig,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStructureProposal {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    pub fields: Vec<AiFieldProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatePair {
    pub record_a: Id,
    pub record_b: Id,
    pub title_a: String,
    pub title_b: String,
    pub score: f64,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Colores disponibles para bases (paleta del frontend)
// ---------------------------------------------------------------------------

pub const DATABASE_COLORS: &[&str] = &[
    "gray", "blue", "green", "yellow", "orange", "red", "purple", "pink", "cyan", "teal",
];

pub const OPTION_COLORS: &[&str] = &[
    "gray", "blue", "green", "yellow", "orange", "red", "purple", "pink",
];
