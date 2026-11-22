//! NEXUS Canvas: servicio de pizarras infinitas.
//! Validación del documento, plantillas con contenido real, duplicado con
//! ids nuevos, export/import JSON y auditoría de las operaciones de gestión.
//! El deshacer/rehacer de edición vive en el frontend (snapshots), por eso
//! el autoguardado del documento no genera eventos de actividad.

use std::collections::{HashMap, HashSet};

use nexus_core::{
    new_id, now_str, Canvas, CanvasDocument, CanvasEdge, CanvasElement, CanvasElementType,
    CanvasSummary, CanvasTemplateInfo, CreateCanvas, Id, NexusError, Result, UpdateCanvas,
    CANVAS_DOCUMENT_VERSION, CANVAS_MAX_EDGES, CANVAS_MAX_ELEMENTS,
};
use nexus_db::{repos, Db};
use serde_json::{json, Value};

use crate::validate;

pub const CANVAS_TEMPLATES: &[(&str, &str, &str)] = &[
    (
        "mind_map",
        "Mapa mental",
        "Idea central con cuatro ramas listas para desarrollar.",
    ),
    (
        "flowchart",
        "Diagrama de flujo",
        "Inicio, pasos, decisión y fin conectados con flechas.",
    ),
    (
        "brainstorming",
        "Lluvia de ideas",
        "Tablero de notas adhesivas de colores para idear en grupo.",
    ),
    (
        "concept_map",
        "Mapa conceptual",
        "Conceptos enlazados con relaciones etiquetadas.",
    ),
    (
        "architecture",
        "Diagrama de arquitectura",
        "Capas de un sistema: cliente, API, servicios y datos.",
    ),
    (
        "freeform",
        "Lienzo libre",
        "Pizarra vacía para empezar desde cero.",
    ),
];

#[derive(Clone)]
pub struct CanvasService {
    pub db: Db,
}

impl CanvasService {
    pub async fn list(&self) -> Result<Vec<CanvasSummary>> {
        repos::canvas::list(&self.db).await
    }

    pub async fn get(&self, id: &str) -> Result<Canvas> {
        repos::canvas::get(&self.db, id).await
    }

    pub async fn create(&self, data: CreateCanvas, actor: &str) -> Result<Canvas> {
        let name = validate::title(&data.name, "el nombre del canvas")?;
        let description = validate::long_text(&data.description, "la descripción", 2_000)?;
        let document = match data.document {
            Some(doc) => doc,
            None => match data.template.as_deref() {
                Some(t) => template_document(t)?,
                None => CanvasDocument::default(),
            },
        };
        let document = validate_document(document)?;
        let now = now_str();
        let canvas = Canvas {
            id: new_id(),
            name,
            description,
            document,
            created_at: now.clone(),
            updated_at: now,
        };
        let canvas = repos::canvas::create(&self.db, &canvas).await?;
        self.audit(
            "created",
            &canvas.id,
            &format!("Canvas creado: {}", canvas.name),
            actor,
        )
        .await?;
        Ok(canvas)
    }

    pub async fn update(&self, id: &str, patch: UpdateCanvas, actor: &str) -> Result<Canvas> {
        let name = match &patch.name {
            Some(n) => Some(validate::title(n, "el nombre del canvas")?),
            None => None,
        };
        if let Some(d) = &patch.description {
            validate::long_text(d, "la descripción", 2_000)?;
        }
        let canvas =
            repos::canvas::update_meta(&self.db, id, name.as_deref(), patch.description.as_deref())
                .await?;
        self.audit(
            "updated",
            id,
            &format!("Canvas actualizado: {}", canvas.name),
            actor,
        )
        .await?;
        Ok(canvas)
    }

    pub async fn delete(&self, id: &str, actor: &str) -> Result<()> {
        let canvas = repos::canvas::get(&self.db, id).await?;
        repos::canvas::soft_delete(&self.db, id).await?;
        repos::activity::record(
            &self.db,
            "deleted",
            "canvas",
            Some(id),
            &format!("Canvas eliminado: {}", canvas.name),
            actor,
            Some(serde_json::json!({"op": "restore", "entity": "canvas", "id": id})),
        )
        .await?;
        Ok(())
    }

    /// Duplica el canvas regenerando todos los ids (elementos, aristas y grupos).
    pub async fn duplicate(&self, id: &str, actor: &str) -> Result<Canvas> {
        let source = repos::canvas::get(&self.db, id).await?;
        let mut doc = source.document.clone();
        remap_ids(&mut doc);
        let now = now_str();
        let canvas = Canvas {
            id: new_id(),
            name: format!("{} (copia)", source.name),
            description: source.description,
            document: doc,
            created_at: now.clone(),
            updated_at: now,
        };
        let canvas = repos::canvas::create(&self.db, &canvas).await?;
        self.audit(
            "created",
            &canvas.id,
            &format!("Canvas duplicado: {}", canvas.name),
            actor,
        )
        .await?;
        Ok(canvas)
    }

    /// Autoguardado del documento completo. Sin auditoría: la edición continua
    /// no debe inundar el registro de actividad (undo vive en el frontend).
    pub async fn save_document(&self, id: &str, document: CanvasDocument) -> Result<Canvas> {
        let document = validate_document(document)?;
        repos::canvas::save_document(&self.db, id, &document).await
    }

    pub fn templates(&self) -> Vec<CanvasTemplateInfo> {
        CANVAS_TEMPLATES
            .iter()
            .map(|(id, name, description)| {
                let count = template_document(id)
                    .map(|d| d.elements.len() as i64)
                    .unwrap_or(0);
                CanvasTemplateInfo {
                    id: id.to_string(),
                    name: name.to_string(),
                    description: description.to_string(),
                    element_count: count,
                }
            })
            .collect()
    }

    /// Exportación JSON portable: documento + metadatos, con formato identificable.
    pub async fn export_json(&self, id: &str) -> Result<Value> {
        let canvas = repos::canvas::get(&self.db, id).await?;
        Ok(json!({
            "format": "nexus.canvas",
            "version": CANVAS_DOCUMENT_VERSION,
            "name": canvas.name,
            "description": canvas.description,
            "document": canvas.document,
        }))
    }

    /// Importa un export JSON (o un documento suelto) como canvas nuevo.
    pub async fn import_json(
        &self,
        data: Value,
        name: Option<String>,
        actor: &str,
    ) -> Result<Canvas> {
        if !data.is_object() {
            return Err(NexusError::validation("el JSON debe ser un objeto"));
        }
        let (document, default_name, description) = if data.get("document").is_some() {
            if data.get("format").and_then(Value::as_str) != Some("nexus.canvas") {
                return Err(NexusError::validation(
                    "el archivo no es una exportación de NEXUS Canvas",
                ));
            }
            let doc: CanvasDocument = serde_json::from_value(data["document"].clone())
                .map_err(|e| NexusError::validation(format!("documento inválido: {e}")))?;
            (
                doc,
                data.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Canvas importado")
                    .to_string(),
                data.get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            )
        } else {
            if data.get("elements").is_none() && data.get("edges").is_none() {
                return Err(NexusError::validation(
                    "el JSON no contiene un documento de canvas (faltan elements/edges)",
                ));
            }
            let doc: CanvasDocument = serde_json::from_value(data)
                .map_err(|e| NexusError::validation(format!("documento inválido: {e}")))?;
            (doc, "Canvas importado".to_string(), String::new())
        };
        let name = match name {
            Some(n) if !n.trim().is_empty() => n,
            _ => default_name,
        };
        self.create(
            CreateCanvas {
                name,
                description,
                template: None,
                document: Some(document),
            },
            actor,
        )
        .await
    }

    async fn audit(&self, kind: &str, id: &str, summary: &str, actor: &str) -> Result<()> {
        repos::activity::record(&self.db, kind, "canvas", Some(id), summary, actor, None).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Validación del documento
// ---------------------------------------------------------------------------

pub fn validate_document(mut doc: CanvasDocument) -> Result<CanvasDocument> {
    if doc.version != CANVAS_DOCUMENT_VERSION {
        return Err(NexusError::validation(format!(
            "versión de documento no soportada: {} (se esperaba {CANVAS_DOCUMENT_VERSION})",
            doc.version
        )));
    }
    if doc.elements.len() > CANVAS_MAX_ELEMENTS {
        return Err(NexusError::validation(format!(
            "demasiados elementos (máx. {CANVAS_MAX_ELEMENTS})"
        )));
    }
    if doc.edges.len() > CANVAS_MAX_EDGES {
        return Err(NexusError::validation(format!(
            "demasiadas conexiones (máx. {CANVAS_MAX_EDGES})"
        )));
    }
    if !doc.viewport.x.is_finite()
        || !doc.viewport.y.is_finite()
        || !doc.viewport.zoom.is_finite()
        || doc.viewport.zoom <= 0.0
        || doc.viewport.zoom > 16.0
    {
        return Err(NexusError::validation("viewport inválido"));
    }

    let mut ids: HashSet<&str> = HashSet::new();
    for el in &mut doc.elements {
        if el.id.trim().is_empty() {
            return Err(NexusError::validation("elemento sin id"));
        }
        if !ids.insert(el.id.as_str()) {
            return Err(NexusError::validation(format!(
                "id de elemento duplicado: {}",
                el.id
            )));
        }
        let kind = CanvasElementType::parse(&el.kind).ok_or_else(|| {
            NexusError::validation(format!("tipo de elemento inválido: {}", el.kind))
        })?;
        check_number(el.x, "x")?;
        check_number(el.y, "y")?;
        if !el.w.is_finite() || el.w <= 0.0 || el.w > 20_000.0 {
            return Err(NexusError::validation(format!(
                "ancho inválido en {}",
                el.id
            )));
        }
        if !el.h.is_finite() || el.h <= 0.0 || el.h > 20_000.0 {
            return Err(NexusError::validation(format!(
                "alto inválido en {}",
                el.id
            )));
        }
        if let Some(r) = el.rotation {
            check_number(r, "rotation")?;
        }
        if let Some(g) = &el.group_id {
            if g.trim().is_empty() {
                return Err(NexusError::validation("group_id vacío"));
            }
        }
        if el.style.is_null() {
            el.style = json!({});
        }
        if el.content.is_null() {
            el.content = json!({});
        }
        validate_content(kind, el)?;
    }

    let mut edge_ids: HashSet<&str> = HashSet::new();
    for edge in &doc.edges {
        if edge.id.trim().is_empty() {
            return Err(NexusError::validation("conexión sin id"));
        }
        if !edge_ids.insert(edge.id.as_str()) {
            return Err(NexusError::validation(format!(
                "id de conexión duplicado: {}",
                edge.id
            )));
        }
        if !ids.contains(edge.source.as_str()) {
            return Err(NexusError::validation(format!(
                "la conexión {} apunta a un origen inexistente: {}",
                edge.id, edge.source
            )));
        }
        if !ids.contains(edge.target.as_str()) {
            return Err(NexusError::validation(format!(
                "la conexión {} apunta a un destino inexistente: {}",
                edge.id, edge.target
            )));
        }
        if edge.label.chars().count() > 500 {
            return Err(NexusError::validation(
                "etiqueta de conexión demasiado larga (máx. 500)",
            ));
        }
    }
    Ok(doc)
}

fn check_number(v: f64, field: &str) -> Result<()> {
    if !v.is_finite() || v.abs() > 10_000_000.0 {
        return Err(NexusError::validation(format!("valor de {field} inválido")));
    }
    Ok(())
}

fn validate_content(kind: CanvasElementType, el: &CanvasElement) -> Result<()> {
    let content = &el.content;
    let require_str = |key: &str| -> Result<String> {
        content
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                NexusError::validation(format!(
                    "el elemento {} ({kind}) necesita content.{key}",
                    el.id
                ))
            })
    };
    match kind {
        CanvasElementType::Image | CanvasElementType::Document => {
            require_str("file_id")?;
        }
        CanvasElementType::Link => {
            let url = require_str("url")?;
            validate::url(&url)?;
        }
        CanvasElementType::TaskRef
        | CanvasElementType::ProjectRef
        | CanvasElementType::NoteRef
        | CanvasElementType::DatabaseRef
        | CanvasElementType::RecipeRef => {
            require_str("ref_id")?;
        }
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Duplicado / inserción con ids nuevos
// ---------------------------------------------------------------------------

/// Regenera ids de elementos, aristas y grupos manteniendo las relaciones.
pub fn remap_ids(doc: &mut CanvasDocument) {
    let mut el_map: HashMap<Id, Id> = HashMap::new();
    let mut group_map: HashMap<Id, Id> = HashMap::new();
    for el in &mut doc.elements {
        let new = new_id();
        el_map.insert(el.id.clone(), new.clone());
        el.id = new;
        if let Some(g) = &el.group_id {
            let ng = group_map.entry(g.clone()).or_insert_with(new_id);
            el.group_id = Some(ng.clone());
        }
    }
    for edge in &mut doc.edges {
        edge.id = new_id();
        if let Some(s) = el_map.get(&edge.source) {
            edge.source = s.clone();
        }
        if let Some(t) = el_map.get(&edge.target) {
            edge.target = t.clone();
        }
    }
}

// ---------------------------------------------------------------------------
// Plantillas con contenido real
// ---------------------------------------------------------------------------

pub fn template_document(id: &str) -> Result<CanvasDocument> {
    match id {
        "mind_map" => Ok(mind_map()),
        "flowchart" => Ok(flowchart()),
        "brainstorming" => Ok(brainstorming()),
        "concept_map" => Ok(concept_map()),
        "architecture" => Ok(architecture()),
        "freeform" => Ok(CanvasDocument::default()),
        other => Err(NexusError::validation(format!(
            "plantilla desconocida: {other}"
        ))),
    }
}

struct Builder {
    doc: CanvasDocument,
    z: i64,
}

impl Builder {
    fn new() -> Self {
        Self {
            doc: CanvasDocument::default(),
            z: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn el(
        &mut self,
        kind: &str,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        style: Value,
        content: Value,
    ) -> Id {
        self.z += 1;
        let id = new_id();
        self.doc.elements.push(CanvasElement {
            id: id.clone(),
            kind: kind.to_string(),
            x,
            y,
            w,
            h,
            rotation: None,
            z: self.z,
            group_id: None,
            style,
            content,
        });
        id
    }

    fn edge(&mut self, source: &str, target: &str, arrow: bool, label: &str) {
        self.doc.edges.push(CanvasEdge {
            id: new_id(),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: Some("right".into()),
            target_handle: Some("left".into()),
            arrow,
            style: Default::default(),
            color: None,
            label: label.to_string(),
        });
    }

    fn build(self) -> CanvasDocument {
        self.doc
    }
}

fn sticky_style(bg: &str) -> Value {
    json!({"background": bg, "text": "#1f2937"})
}

fn mind_map() -> CanvasDocument {
    let mut b = Builder::new();
    let center = b.el(
        "ellipse",
        0.0,
        0.0,
        240.0,
        120.0,
        json!({"background": "primary"}),
        json!({"text": "Idea central"}),
    );
    let branches = [
        ("Rama 1", -380.0, -220.0, "#fef3c7"),
        ("Rama 2", 420.0, -220.0, "#dcfce7"),
        ("Rama 3", -380.0, 200.0, "#dbeafe"),
        ("Rama 4", 420.0, 200.0, "#fce7f3"),
    ];
    for (text, x, y, color) in branches {
        let id = b.el(
            "sticky",
            x,
            y,
            190.0,
            150.0,
            sticky_style(color),
            json!({"text": text}),
        );
        b.edge(&center, &id, false, "");
    }
    b.build()
}

fn flowchart() -> CanvasDocument {
    let mut b = Builder::new();
    let start = b.el(
        "ellipse",
        0.0,
        0.0,
        180.0,
        90.0,
        json!({"background": "success"}),
        json!({"text": "Inicio"}),
    );
    let step = b.el(
        "rect",
        260.0,
        -5.0,
        200.0,
        100.0,
        json!({}),
        json!({"text": "Paso 1"}),
    );
    let decision = b.el(
        "rect",
        540.0,
        -5.0,
        200.0,
        100.0,
        json!({"background": "warning"}),
        json!({"text": "¿Condición?"}),
    );
    let ok = b.el(
        "rect",
        820.0,
        -140.0,
        200.0,
        100.0,
        json!({}),
        json!({"text": "Camino A"}),
    );
    let ko = b.el(
        "rect",
        820.0,
        130.0,
        200.0,
        100.0,
        json!({}),
        json!({"text": "Camino B"}),
    );
    let end = b.el(
        "ellipse",
        1100.0,
        0.0,
        180.0,
        90.0,
        json!({"background": "destructive"}),
        json!({"text": "Fin"}),
    );
    b.edge(&start, &step, true, "");
    b.edge(&step, &decision, true, "");
    b.edge(&decision, &ok, true, "sí");
    b.edge(&decision, &ko, true, "no");
    b.edge(&ok, &end, true, "");
    b.edge(&ko, &end, true, "");
    b.build()
}

fn brainstorming() -> CanvasDocument {
    let mut b = Builder::new();
    b.el(
        "text",
        0.0,
        -90.0,
        460.0,
        60.0,
        json!({"font_size": 24}),
        json!({"text": "Lluvia de ideas"}),
    );
    let notes = [
        ("¿Qué problema resolvemos?", "#fef3c7"),
        ("Ideas locas bienvenidas", "#dcfce7"),
        ("¿A quién ayuda?", "#dbeafe"),
        ("Primer paso pequeño", "#fce7f3"),
        ("Inspiración / referencias", "#ede9fe"),
        ("Riesgos y dudas", "#ffedd5"),
    ];
    for (i, (text, color)) in notes.iter().enumerate() {
        let x = (i % 3) as f64 * 230.0;
        let y = (i / 3) as f64 * 220.0;
        b.el(
            "sticky",
            x,
            y,
            190.0,
            180.0,
            sticky_style(color),
            json!({"text": text}),
        );
    }
    b.build()
}

fn concept_map() -> CanvasDocument {
    let mut b = Builder::new();
    let root = b.el(
        "card",
        260.0,
        0.0,
        260.0,
        130.0,
        json!({"background": "primary"}),
        json!({"title": "Concepto principal", "body": "Defínelo en una frase."}),
    );
    let a = b.el(
        "rect",
        -80.0,
        260.0,
        210.0,
        100.0,
        json!({}),
        json!({"text": "Concepto A"}),
    );
    let c = b.el(
        "rect",
        280.0,
        300.0,
        210.0,
        100.0,
        json!({}),
        json!({"text": "Concepto B"}),
    );
    let d = b.el(
        "rect",
        640.0,
        260.0,
        210.0,
        100.0,
        json!({}),
        json!({"text": "Concepto C"}),
    );
    b.edge(&root, &a, true, "incluye");
    b.edge(&root, &c, true, "se compone de");
    b.edge(&root, &d, true, "se relaciona con");
    b.edge(&a, &c, true, "influye en");
    b.build()
}

fn architecture() -> CanvasDocument {
    let mut b = Builder::new();
    let client = b.el(
        "rect",
        0.0,
        0.0,
        220.0,
        110.0,
        json!({"background": "primary"}),
        json!({"text": "Cliente (Frontend)"}),
    );
    let api = b.el(
        "rect",
        320.0,
        0.0,
        220.0,
        110.0,
        json!({}),
        json!({"text": "API / Comandos"}),
    );
    let services = b.el(
        "rect",
        640.0,
        0.0,
        220.0,
        110.0,
        json!({}),
        json!({"text": "Servicios de dominio"}),
    );
    let db = b.el(
        "rect",
        960.0,
        -120.0,
        220.0,
        110.0,
        json!({"background": "success"}),
        json!({"text": "Base de datos (SQLite)"}),
    );
    let files = b.el(
        "rect",
        960.0,
        120.0,
        220.0,
        110.0,
        json!({"background": "warning"}),
        json!({"text": "Archivos / Adjuntos"}),
    );
    let ai = b.el(
        "rect",
        640.0,
        240.0,
        220.0,
        110.0,
        json!({"background": "muted"}),
        json!({"text": "Proveedor IA (opcional)"}),
    );
    b.edge(&client, &api, true, "HTTP/JSON");
    b.edge(&api, &services, true, "");
    b.edge(&services, &db, true, "SQL");
    b.edge(&services, &files, true, "bytes");
    b.edge(&services, &ai, true, "chat");
    b.build()
}
