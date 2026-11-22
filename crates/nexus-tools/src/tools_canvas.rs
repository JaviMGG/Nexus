//! Herramientas de escritura para NEXUS Canvas: crear pizarras, añadir
//! tarjetas sin perder el resto del documento y reemplazar el documento
//! completo (autoguardado validado).

use std::sync::Arc;

use nexus_core::{CanvasDocument, CanvasElement, CreateCanvas, NexusError, Result, Risk};
use serde_json::{json, Value};

use crate::builtins::{define_tool, dto, enum_prop, req_str, schema, val};
use crate::{Tool, ToolContext};

define_tool!(
    CanvasCreate,
    "canvas.create",
    Risk::Write,
    "Crea un canvas (pizarra visual) real con nombre y descripción. Opcionalmente parte de una plantilla (mind_map, flowchart, brainstorming, concept_map, architecture, freeform).",
    schema(
        json!({
            "name": { "type": "string" },
            "description": { "type": "string" },
            "template": enum_prop(&[
                "mind_map",
                "flowchart",
                "brainstorming",
                "concept_map",
                "architecture",
                "freeform"
            ])
        }),
        &["name"]
    ),
    |args, ctx| {
        let name = req_str(&args, "name")?;
        let description = args
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let template = args
            .get("template")
            .and_then(Value::as_str)
            .map(str::to_string);
        let data = CreateCanvas {
            name,
            description,
            template,
            document: None,
        };
        val(ctx.services.canvas.create(data, &ctx.actor()).await?)
    }
);

define_tool!(
    CanvasAddCard,
    "canvas.add_card",
    Risk::Write,
    "Añade una tarjeta (título y cuerpo) a un canvas existente sin perder el resto de elementos. Sin x/y se coloca debajo del último elemento.",
    schema(
        json!({
            "canvas_id": { "type": "string" },
            "title": { "type": "string" },
            "body": { "type": "string" },
            "x": { "type": "number" },
            "y": { "type": "number" }
        }),
        &["canvas_id", "title"]
    ),
    |args, ctx| {
        let canvas_id = req_str(&args, "canvas_id")?;
        let title = req_str(&args, "title")?;
        let body = args.get("body").and_then(Value::as_str).unwrap_or("");
        let canvas = ctx.services.canvas.get(&canvas_id).await?;
        let mut document = canvas.document;
        let bottom = document
            .elements
            .iter()
            .map(|e| e.y + e.h)
            .fold(0.0_f64, f64::max);
        let max_z = document.elements.iter().map(|e| e.z).max().unwrap_or(0);
        let x = args.get("x").and_then(Value::as_f64).unwrap_or(40.0);
        let default_y = if document.elements.is_empty() {
            40.0
        } else {
            bottom + 40.0
        };
        let y = args.get("y").and_then(Value::as_f64).unwrap_or(default_y);
        let element = CanvasElement {
            id: nexus_core::new_id(),
            kind: "card".into(),
            x,
            y,
            w: 260.0,
            h: 160.0,
            rotation: None,
            z: max_z + 1,
            group_id: None,
            style: json!({}),
            content: json!({ "title": title, "body": body }),
        };
        let element_id = element.id.clone();
        document.elements.push(element);
        ctx.services.canvas.save_document(&canvas_id, document).await?;
        val(json!({ "canvas_id": canvas_id, "element_id": element_id, "title": title }))
    }
);

define_tool!(
    CanvasSaveDocument,
    "canvas.save_document",
    Risk::Write,
    "REEMPLAZA el documento COMPLETO de un canvas (elementos y conexiones). Lee antes canvas.get para no perder elementos existentes: lo que omitas se borra.",
    schema(
        json!({
            "canvas_id": { "type": "string" },
            "document": {
                "type": "object",
                "description": "documento completo {version, elements:[{id,type,x,y,w,h,content,style}], edges:[]}; lee antes canvas.get para no perder elementos"
            }
        }),
        &["canvas_id", "document"]
    ),
    |args, ctx| {
        let canvas_id = req_str(&args, "canvas_id")?;
        let raw = args
            .get("document")
            .filter(|v| v.is_object())
            .cloned()
            .ok_or_else(|| {
                NexusError::validation(
                    "falta el campo obligatorio 'document' (objeto con el documento completo)",
                )
            })?;
        let document: CanvasDocument = dto(raw)?;
        val(ctx.services.canvas.save_document(&canvas_id, document).await?)
    }
);

pub(crate) fn tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(CanvasCreate),
        Arc::new(CanvasAddCard),
        Arc::new(CanvasSaveDocument),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_args, ToolRegistry};
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
    async fn create_and_add_card_keeps_elements() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("canvas.create").unwrap();
        let args = json!({"name": "Plan del proyecto", "description": "Ideas iniciales"});
        validate_args(&create.spec(), &args).unwrap();
        let created = create.execute(args, &ctx).await.unwrap();
        let canvas_id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["name"], "Plan del proyecto");
        assert_eq!(created["document"]["elements"].as_array().unwrap().len(), 0);

        let add = registry.get("canvas.add_card").unwrap();
        let first = add
            .execute(
                json!({"canvas_id": canvas_id, "title": "Objetivo", "body": "Definir el alcance"}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(first["title"], "Objetivo");
        let second = add
            .execute(json!({"canvas_id": canvas_id, "title": "Riesgos"}), &ctx)
            .await
            .unwrap();
        assert_ne!(first["element_id"], second["element_id"]);

        let canvas = ctx.services.canvas.get(&canvas_id).await.unwrap();
        let elements = &canvas.document.elements;
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].kind, "card");
        assert_eq!(elements[0].content["title"], "Objetivo");
        assert_eq!(elements[0].content["body"], "Definir el alcance");
        assert_eq!(elements[1].content["title"], "Riesgos");
        assert_ne!(
            (elements[0].x, elements[0].y),
            (elements[1].x, elements[1].y)
        );
        assert!(elements[1].y >= elements[0].y + elements[0].h);
    }

    #[tokio::test]
    async fn save_document_replaces_whole_document() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("canvas.create").unwrap();
        let created = create
            .execute(json!({"name": "Lienzo"}), &ctx)
            .await
            .unwrap();
        let canvas_id = created["id"].as_str().unwrap().to_string();

        let add = registry.get("canvas.add_card").unwrap();
        add.execute(json!({"canvas_id": canvas_id, "title": "Vieja"}), &ctx)
            .await
            .unwrap();

        let save = registry.get("canvas.save_document").unwrap();
        let doc = json!({
            "version": 1,
            "elements": [{
                "id": "el-sticky-1",
                "type": "sticky",
                "x": 10.0,
                "y": 20.0,
                "w": 190.0,
                "h": 150.0,
                "content": {"text": "Nota nueva"},
                "style": {"background": "#fef3c7"}
            }],
            "edges": []
        });
        let args = json!({"canvas_id": canvas_id, "document": doc});
        validate_args(&save.spec(), &args).unwrap();
        save.execute(args, &ctx).await.unwrap();

        let canvas = ctx.services.canvas.get(&canvas_id).await.unwrap();
        assert_eq!(canvas.document.elements.len(), 1);
        assert_eq!(canvas.document.elements[0].id, "el-sticky-1");
        assert_eq!(canvas.document.elements[0].kind, "sticky");
        assert_eq!(canvas.document.elements[0].content["text"], "Nota nueva");
        assert!(canvas.document.edges.is_empty());
    }

    #[tokio::test]
    async fn validate_args_rejects_missing_required() {
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("canvas.create").unwrap();
        let err = validate_args(&create.spec(), &json!({})).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
        assert!(err.to_string().contains("name"));

        let add = registry.get("canvas.add_card").unwrap();
        let err = validate_args(&add.spec(), &json!({"canvas_id": "c1"})).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
        assert!(err.to_string().contains("title"));
    }

    #[test]
    fn all_canvas_write_tools_are_write_risk() {
        for tool in tools() {
            assert_eq!(tool.risk(), Risk::Write, "{}", tool.spec().name);
        }
        assert_eq!(tools().len(), 3);
    }
}