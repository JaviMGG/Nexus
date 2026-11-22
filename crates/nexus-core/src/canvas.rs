//! Dominio de NEXUS Canvas: pizarras infinitas con elementos, conexiones,
//! grupos y referencias vivas a otras entidades de NEXUS.
//! El documento completo se persiste como JSON versionado en SQLite.

use serde::{Deserialize, Serialize};

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

str_enum!(
    CanvasElementType,
    Sticky => "sticky",
    Text => "text",
    Rect => "rect",
    Ellipse => "ellipse",
    Line => "line",
    Arrow => "arrow",
    Image => "image",
    Link => "link",
    Card => "card",
    TaskRef => "task_ref",
    ProjectRef => "project_ref",
    NoteRef => "note_ref",
    DatabaseRef => "database_ref",
    RecipeRef => "recipe_ref",
    Document => "document",
);

str_enum!(CanvasEdgeStyle, Solid => "solid", Dashed => "dashed");

#[allow(clippy::derivable_impls)]
impl Default for CanvasEdgeStyle {
    fn default() -> Self {
        Self::Solid
    }
}

pub const CANVAS_DOCUMENT_VERSION: i64 = 1;
pub const CANVAS_MAX_ELEMENTS: usize = 5_000;
pub const CANVAS_MAX_EDGES: usize = 10_000;

/// Elemento del lienzo. `content` depende del tipo:
/// sticky/text/rect/ellipse/card → texto (card: {title, body});
/// image/document → {file_id, name, mime}; link → {url, title};
/// *_ref → {ref_id, title, ...campos cacheados del origen}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasElement {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub rotation: Option<f64>,
    #[serde(default)]
    pub z: i64,
    #[serde(default)]
    pub group_id: Option<Id>,
    /// Colores/estilo libre: background, border, text, font_size, align…
    #[serde(default)]
    pub style: serde_json::Value,
    #[serde(default)]
    pub content: serde_json::Value,
}

/// Conexión entre dos elementos (flecha por defecto; `arrow=false` → línea).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEdge {
    pub id: Id,
    pub source: Id,
    pub target: Id,
    #[serde(default)]
    pub source_handle: Option<String>,
    #[serde(default)]
    pub target_handle: Option<String>,
    #[serde(default = "default_true")]
    pub arrow: bool,
    #[serde(default)]
    pub style: CanvasEdgeStyle,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub label: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasViewport {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

fn default_zoom() -> f64 {
    1.0
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasDocument {
    #[serde(default = "default_version")]
    pub version: i64,
    #[serde(default)]
    pub elements: Vec<CanvasElement>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub viewport: CanvasViewport,
}

fn default_version() -> i64 {
    CANVAS_DOCUMENT_VERSION
}

impl Default for CanvasDocument {
    fn default() -> Self {
        Self {
            version: CANVAS_DOCUMENT_VERSION,
            elements: Vec::new(),
            edges: Vec::new(),
            viewport: CanvasViewport::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub document: CanvasDocument,
    pub created_at: String,
    pub updated_at: String,
}

/// Resumen para la lista de canvases (sin el documento completo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSummary {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub element_count: i64,
    pub edge_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCanvas {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// mind_map | flowchart | brainstorming | concept_map | architecture | freeform
    #[serde(default)]
    pub template: Option<String>,
    pub document: Option<CanvasDocument>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCanvas {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasTemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub element_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_kind_roundtrip() {
        for s in [
            "sticky",
            "text",
            "rect",
            "ellipse",
            "line",
            "arrow",
            "image",
            "link",
            "card",
            "task_ref",
            "project_ref",
            "note_ref",
            "database_ref",
            "recipe_ref",
            "document",
        ] {
            let k = CanvasElementType::parse(s).unwrap();
            assert_eq!(k.as_str(), s);
        }
        assert!(CanvasElementType::parse("nope").is_none());
    }
}
