//! Comandos del módulo NEXUS Canvas: CRUD, plantillas, documento, export/import
//! e IA opcional (siempre como propuesta revisable, nunca aplicada directamente).

use nexus_core::{CanvasDocument, NexusError, Result, UpdateCanvas};
use serde_json::{json, Value};

use crate::ai_json::ask_json;
use crate::context::AppContext;
use crate::dispatch::{arg, ok, opt_arg};

pub async fn dispatch(ctx: &AppContext, command: &str, payload: Value) -> Result<Value> {
    let s = &ctx.services;
    match command {
        "list" => ok(s.canvas.list().await?),
        "get" => ok(s.canvas.get(&arg::<String>(&payload, "id")?).await?),
        "create" => {
            let c = s.canvas.create(arg(&payload, "data")?, "user").await?;
            ctx.data_changed("canvas");
            ok(c)
        }
        "update" | "rename" => {
            let id: String = arg(&payload, "id")?;
            let patch: UpdateCanvas = if command == "rename" {
                UpdateCanvas {
                    name: Some(arg(&payload, "name")?),
                    description: opt_arg(&payload, "description")?,
                }
            } else {
                arg(&payload, "data")?
            };
            let c = s.canvas.update(&id, patch, "user").await?;
            ctx.data_changed("canvas");
            ok(c)
        }
        "delete" => {
            s.canvas
                .delete(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("canvas");
            ok(json!({"deleted": true}))
        }
        "duplicate" => {
            let c = s
                .canvas
                .duplicate(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("canvas");
            ok(c)
        }
        "save_document" => {
            let id: String = arg(&payload, "id")?;
            let doc: CanvasDocument = arg(&payload, "document")?;
            let c = s.canvas.save_document(&id, doc).await?;
            ctx.data_changed("canvas");
            ok(c)
        }
        "templates" => ok(s.canvas.templates()),
        "template_document" => {
            let id: String = arg(&payload, "id")?;
            ok(nexus_services::canvas::template_document(&id)?)
        }
        "export_json" => ok(s
            .canvas
            .export_json(&arg::<String>(&payload, "id")?)
            .await?),
        "import_json" => {
            let data: Value = arg(&payload, "data")?;
            let name: Option<String> = opt_arg(&payload, "name")?;
            let c = s.canvas.import_json(data, name, "user").await?;
            ctx.data_changed("canvas");
            ok(c)
        }

        // ------------------------------------------------------------- IA opcional
        "ai_mindmap" => {
            let description: String = arg(&payload, "description")?;
            if description.trim().is_empty() {
                return Err(NexusError::validation("describe el tema del mapa mental"));
            }
            let v = ask_json(
                ctx,
                AI_MINDMAP_SYSTEM,
                &format!("Tema del mapa mental: {description}"),
                2500,
            )
            .await?;
            ok(parse_proposal(v)?)
        }
        "ai_expand" => {
            let canvas_id: String = arg(&payload, "canvas_id")?;
            let element_id: String = arg(&payload, "element_id")?;
            let canvas = s.canvas.get(&canvas_id).await?;
            let el = canvas
                .document
                .elements
                .iter()
                .find(|e| e.id == element_id)
                .ok_or_else(|| NexusError::not_found("elemento", &element_id))?;
            let branch = element_text(el);
            let context: Vec<String> = canvas
                .document
                .elements
                .iter()
                .take(60)
                .map(element_text)
                .filter(|t| !t.is_empty())
                .collect();
            let v = ask_json(
                ctx,
                AI_EXPAND_SYSTEM,
                &format!(
                    "Rama a expandir: {branch}\nContexto del canvas: {}",
                    context.join(" | ")
                ),
                1500,
            )
            .await?;
            let nodes = v
                .get("nodes")
                .and_then(Value::as_array)
                .ok_or_else(|| NexusError::Ai("la IA no devolvió nodos".into()))?;
            let mut elements = Vec::new();
            for n in nodes.iter().take(12) {
                let text = n
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| n.get("text").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_default();
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                elements.push(json!({
                    "type": "sticky",
                    "text": text,
                    "color": "#dbeafe",
                }));
            }
            if elements.is_empty() {
                return Err(NexusError::Ai("la IA no propuso ninguna rama nueva".into()));
            }
            ok(json!({
                "parent_id": element_id,
                "parent_x": el.x,
                "parent_y": el.y,
                "parent_w": el.w,
                "parent_h": el.h,
                "elements": elements,
            }))
        }
        "ai_summarize" => {
            let canvas_id: String = arg(&payload, "canvas_id")?;
            let canvas = s.canvas.get(&canvas_id).await?;
            let texts: Vec<String> = canvas
                .document
                .elements
                .iter()
                .take(200)
                .map(|e| format!("[{}] {}", e.kind, element_text(e)))
                .filter(|t| !t.ends_with("] "))
                .collect();
            if texts.is_empty() {
                return Err(NexusError::validation(
                    "el canvas está vacío: nada que resumir",
                ));
            }
            let labels: Vec<String> = canvas
                .document
                .edges
                .iter()
                .filter(|e| !e.label.trim().is_empty())
                .map(|e| e.label.clone())
                .collect();
            let v = ask_json(
                ctx,
                "Eres un asistente que resume pizarras visuales en español. Devuelve SOLO un JSON {\"summary\": \"...\"} donde summary es markdown breve (títulos, listas) con las ideas clave, agrupaciones y posibles siguientes pasos.",
                &format!(
                    "Canvas «{}». Elementos:\n{}\nRelaciones etiquetadas: {}",
                    canvas.name,
                    texts.join("\n"),
                    labels.join(", ")
                ),
                1500,
            )
            .await?;
            let summary = v
                .get("summary")
                .and_then(Value::as_str)
                .ok_or_else(|| NexusError::Ai("la IA no devolvió un resumen".into()))?;
            ok(json!({"summary": summary}))
        }
        "ai_organize" => {
            let canvas_id: String = arg(&payload, "canvas_id")?;
            let canvas = s.canvas.get(&canvas_id).await?;
            if canvas.document.elements.len() < 2 {
                return Err(NexusError::validation(
                    "hacen falta al menos dos elementos para proponer conexiones",
                ));
            }
            let listing: Vec<Value> = canvas
                .document
                .elements
                .iter()
                .take(80)
                .map(|e| json!({"id": e.id, "type": e.kind, "text": element_text(e)}))
                .collect();
            let existing: Vec<Value> = canvas
                .document
                .edges
                .iter()
                .map(|e| json!({"source": e.source, "target": e.target}))
                .collect();
            let v = ask_json(
                ctx,
                "Eres un asistente que organiza pizarras. Devuelve SOLO un JSON {\"connections\": [{\"source\": \"id\", \"target\": \"id\", \"label\": \"relación corta\", \"reason\": \"por qué\"}]}. Usa exclusivamente los ids dados; no repitas conexiones existentes ni conectes un elemento consigo mismo. Máximo 10 conexiones, en español.",
                &format!(
                    "Elementos: {}\nConexiones existentes: {}",
                    Value::Array(listing),
                    Value::Array(existing)
                ),
                1500,
            )
            .await?;
            let conns = v
                .get("connections")
                .and_then(Value::as_array)
                .ok_or_else(|| NexusError::Ai("la IA no devolvió conexiones".into()))?;
            let ids: std::collections::HashSet<&str> = canvas
                .document
                .elements
                .iter()
                .map(|e| e.id.as_str())
                .collect();
            let pairs: std::collections::HashSet<(String, String)> = canvas
                .document
                .edges
                .iter()
                .flat_map(|e| {
                    [
                        (e.source.clone(), e.target.clone()),
                        (e.target.clone(), e.source.clone()),
                    ]
                })
                .collect();
            let mut out = Vec::new();
            for c in conns.iter().take(10) {
                let source = c.get("source").and_then(Value::as_str).unwrap_or("");
                let target = c.get("target").and_then(Value::as_str).unwrap_or("");
                if !ids.contains(source) || !ids.contains(target) || source == target {
                    continue;
                }
                if pairs.contains(&(source.to_string(), target.to_string())) {
                    continue;
                }
                out.push(json!({
                    "source": source,
                    "target": target,
                    "label": c.get("label").and_then(Value::as_str).unwrap_or(""),
                    "reason": c.get("reason").and_then(Value::as_str).unwrap_or(""),
                }));
            }
            ok(json!({"connections": out}))
        }

        other => Err(NexusError::NotFound(format!(
            "comando desconocido: canvas.{other}"
        ))),
    }
}

const AI_MINDMAP_SYSTEM: &str = "Eres un asistente que crea mapas mentales en español. Devuelve SOLO un JSON {\"elements\": [{\"type\": \"sticky\"|\"text\"|\"rect\"|\"ellipse\", \"text\": \"...\", \"color\": \"#hex opcional\"}], \"edges\": [{\"source\": índice, \"target\": índice, \"label\": \"opcional\"}]}. El primer elemento es la idea central (ellipse); entre 6 y 16 elementos, textos cortos, jerarquía clara.";

const AI_EXPAND_SYSTEM: &str = "Eres un asistente que expande ramas de mapas mentales en español. Devuelve SOLO un JSON {\"nodes\": [\"texto corto\", ...]} con entre 3 y 6 ideas hijas nuevas, concretas y sin repetir el contexto.";

fn element_text(el: &nexus_core::CanvasElement) -> String {
    let c = &el.content;
    let get = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("");
    match el.kind.as_str() {
        "card" => format!("{}: {}", get("title"), get("body"))
            .trim_matches(':')
            .trim()
            .to_string(),
        "link" => format!("{} ({})", get("title"), get("url")),
        "image" | "document" => get("name").to_string(),
        "task_ref" | "project_ref" | "note_ref" | "database_ref" | "recipe_ref" => {
            get("title").to_string()
        }
        _ => get("text").to_string(),
    }
}

/// Normaliza la propuesta de mapa mental: tipos válidos, índices de aristas
/// dentro de rango y textos no vacíos. Nunca toca el canvas: solo propuesta.
fn parse_proposal(v: Value) -> Result<Value> {
    let elements = v
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| NexusError::Ai("la IA no devolvió elementos".into()))?;
    let mut clean_elements = Vec::new();
    for e in elements.iter().take(24) {
        let kind = e.get("type").and_then(Value::as_str).unwrap_or("sticky");
        let kind = match kind {
            "sticky" | "text" | "rect" | "ellipse" | "card" => kind,
            _ => "sticky",
        };
        let text = e.get("text").and_then(Value::as_str).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let mut item = json!({"type": kind, "text": text});
        if let Some(color) = e.get("color").and_then(Value::as_str) {
            if color.starts_with('#') && color.len() <= 9 {
                item["color"] = json!(color);
            }
        }
        clean_elements.push(item);
    }
    if clean_elements.is_empty() {
        return Err(NexusError::Ai("la propuesta está vacía".into()));
    }
    let mut clean_edges = Vec::new();
    if let Some(edges) = v.get("edges").and_then(Value::as_array) {
        for e in edges.iter().take(48) {
            let source = e.get("source").and_then(Value::as_i64);
            let target = e.get("target").and_then(Value::as_i64);
            if let (Some(s), Some(t)) = (source, target) {
                let n = clean_elements.len() as i64;
                if s >= 0 && t >= 0 && s < n && t < n && s != t {
                    clean_edges.push(json!({
                        "source": s,
                        "target": t,
                        "label": e.get("label").and_then(Value::as_str).unwrap_or(""),
                    }));
                }
            }
        }
    }
    Ok(json!({"elements": clean_elements, "edges": clean_edges}))
}
