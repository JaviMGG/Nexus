use nexus_core::*;
use nexus_db::Db;
use nexus_services::canvas::{template_document, validate_document, CANVAS_TEMPLATES};
use nexus_services::Services;
use serde_json::json;

async fn setup() -> Services {
    Services::new(Db::connect_memory().await.unwrap())
}

fn sticky(id: &str, x: f64, y: f64, text: &str) -> CanvasElement {
    CanvasElement {
        id: id.into(),
        kind: "sticky".into(),
        x,
        y,
        w: 180.0,
        h: 160.0,
        rotation: Some(0.0),
        z: 1,
        group_id: None,
        style: json!({"background": "#fef3c7"}),
        content: json!({"text": text}),
    }
}

fn edge(id: &str, source: &str, target: &str) -> CanvasEdge {
    CanvasEdge {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        source_handle: Some("right".into()),
        target_handle: None,
        arrow: true,
        style: CanvasEdgeStyle::Solid,
        color: None,
        label: "relación".into(),
    }
}

fn sample_doc() -> CanvasDocument {
    CanvasDocument {
        version: 1,
        elements: vec![
            sticky("e1", 0.0, 0.0, "Idea"),
            sticky("e2", 300.0, 40.0, "Detalle"),
        ],
        edges: vec![edge("c1", "e1", "e2")],
        viewport: CanvasViewport {
            x: 12.5,
            y: -8.0,
            zoom: 0.75,
        },
    }
}

#[tokio::test]
async fn canvas_crud() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "  Mi pizarra  ".into(),
                description: "notas".into(),
                template: None,
                document: None,
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(c.name, "Mi pizarra");
    assert!(c.document.elements.is_empty());

    let got = s.canvas.get(&c.id).await.unwrap();
    assert_eq!(got.id, c.id);

    let up = s
        .canvas
        .update(
            &c.id,
            UpdateCanvas {
                name: Some("Renombrada".into()),
                description: None,
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(up.name, "Renombrada");
    assert_eq!(up.description, "notas");

    let list = s.canvas.list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].element_count, 0);

    let err = s
        .canvas
        .create(
            CreateCanvas {
                name: "   ".into(),
                description: String::new(),
                template: None,
                document: None,
            },
            "user",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));
}

#[tokio::test]
async fn duplicate_has_new_ids_and_keeps_links() {
    let s = setup().await;
    let mut doc = sample_doc();
    doc.elements[0].group_id = Some("g1".into());
    doc.elements[1].group_id = Some("g1".into());
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Original".into(),
                description: String::new(),
                template: None,
                document: Some(doc),
            },
            "user",
        )
        .await
        .unwrap();
    let d = s.canvas.duplicate(&c.id, "user").await.unwrap();
    assert_ne!(d.id, c.id);
    assert_eq!(d.name, "Original (copia)");
    assert_eq!(d.document.elements.len(), 2);
    let old_ids: Vec<&str> = c.document.elements.iter().map(|e| e.id.as_str()).collect();
    for el in &d.document.elements {
        assert!(!old_ids.contains(&el.id.as_str()));
    }
    assert_ne!(d.document.edges[0].id, c.document.edges[0].id);
    let new_ids: Vec<&str> = d.document.elements.iter().map(|e| e.id.as_str()).collect();
    assert!(new_ids.contains(&d.document.edges[0].source.as_str()));
    assert!(new_ids.contains(&d.document.edges[0].target.as_str()));
    assert_eq!(
        d.document.elements[0].group_id,
        d.document.elements[1].group_id
    );
    assert_ne!(d.document.elements[0].group_id.as_deref(), Some("g1"));
}

#[tokio::test]
async fn save_and_reload_document_identical() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Doc".into(),
                description: String::new(),
                template: None,
                document: None,
            },
            "user",
        )
        .await
        .unwrap();
    let doc = sample_doc();
    s.canvas.save_document(&c.id, doc.clone()).await.unwrap();
    let got = s.canvas.get(&c.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(&got.document).unwrap(),
        serde_json::to_value(&doc).unwrap()
    );
    assert_eq!(got.document.viewport.zoom, 0.75);
}

#[tokio::test]
async fn invalid_element_type_rejected() {
    let s = setup().await;
    let mut doc = sample_doc();
    doc.elements[0].kind = "banana".into();
    let err = s
        .canvas
        .create(
            CreateCanvas {
                name: "X".into(),
                description: String::new(),
                template: None,
                document: Some(doc),
            },
            "user",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));
}

#[tokio::test]
async fn edge_to_missing_element_rejected() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "X".into(),
                description: String::new(),
                template: None,
                document: None,
            },
            "user",
        )
        .await
        .unwrap();
    let mut doc = sample_doc();
    doc.edges[0].target = "fantasma".into();
    let err = s.canvas.save_document(&c.id, doc).await.unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));

    let mut doc2 = sample_doc();
    doc2.elements[1].id = doc2.elements[0].id.clone();
    let err = validate_document(doc2).unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));
}

#[tokio::test]
async fn all_templates_are_valid_documents() {
    for (id, _, _) in CANVAS_TEMPLATES {
        let doc = template_document(id).unwrap();
        validate_document(doc.clone()).unwrap_or_else(|e| panic!("plantilla {id}: {e}"));
        if *id != "freeform" {
            assert!(!doc.elements.is_empty(), "plantilla {id} vacía");
        }
        if matches!(
            *id,
            "mind_map" | "flowchart" | "concept_map" | "architecture"
        ) {
            assert!(!doc.edges.is_empty(), "plantilla {id} sin conexiones");
        }
    }
    assert!(template_document("inexistente").is_err());

    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Mapa".into(),
                description: String::new(),
                template: Some("mind_map".into()),
                document: None,
            },
            "user",
        )
        .await
        .unwrap();
    assert!(c.document.elements.len() >= 5);
    let got = s.canvas.get(&c.id).await.unwrap();
    assert_eq!(got.document.elements.len(), c.document.elements.len());
}

#[tokio::test]
async fn export_json_structure() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Export".into(),
                description: "desc".into(),
                template: None,
                document: Some(sample_doc()),
            },
            "user",
        )
        .await
        .unwrap();
    let v = s.canvas.export_json(&c.id).await.unwrap();
    assert_eq!(v["format"], "nexus.canvas");
    assert_eq!(v["version"], 1);
    assert_eq!(v["name"], "Export");
    assert_eq!(v["description"], "desc");
    assert_eq!(v["document"]["elements"].as_array().unwrap().len(), 2);
    assert_eq!(v["document"]["edges"].as_array().unwrap().len(), 1);
    assert_eq!(v["document"]["viewport"]["zoom"], 0.75);
}

#[tokio::test]
async fn import_json_valid_creates_new_canvas() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Base".into(),
                description: String::new(),
                template: None,
                document: Some(sample_doc()),
            },
            "user",
        )
        .await
        .unwrap();
    let exported = s.canvas.export_json(&c.id).await.unwrap();
    let imported = s.canvas.import_json(exported, None, "user").await.unwrap();
    assert_ne!(imported.id, c.id);
    assert_eq!(imported.name, "Base");
    assert_eq!(
        serde_json::to_value(&imported.document).unwrap(),
        serde_json::to_value(&c.document).unwrap()
    );

    // Documento suelto también es aceptado, con nombre explícito.
    let raw = serde_json::to_value(sample_doc()).unwrap();
    let imported2 = s
        .canvas
        .import_json(raw, Some("Desde archivo".into()), "user")
        .await
        .unwrap();
    assert_eq!(imported2.name, "Desde archivo");
    assert_eq!(s.canvas.list().await.unwrap().len(), 3);
}

#[tokio::test]
async fn import_json_invalid_rejected() {
    let s = setup().await;
    // No es ni export ni documento.
    let err = s
        .canvas
        .import_json(json!({"hola": 1}), None, "user")
        .await
        .unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));

    let err = s
        .canvas
        .import_json(
            json!({"format": "otro", "document": sample_doc()}),
            None,
            "user",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));

    let mut bad = sample_doc();
    bad.edges[0].source = "no-existe".into();
    let err = s
        .canvas
        .import_json(
            json!({"format": "nexus.canvas", "name": "Malo", "document": bad}),
            None,
            "user",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));

    let mut bad_version = sample_doc();
    bad_version.version = 99;
    let err = s
        .canvas
        .import_json(
            json!({"format": "nexus.canvas", "document": bad_version}),
            None,
            "user",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));
}

#[tokio::test]
async fn delete_canvas_removes_from_list_and_get() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Borrar".into(),
                description: String::new(),
                template: None,
                document: None,
            },
            "user",
        )
        .await
        .unwrap();
    s.canvas.delete(&c.id, "user").await.unwrap();
    assert!(s.canvas.list().await.unwrap().is_empty());
    let err = s.canvas.get(&c.id).await.unwrap_err();
    assert!(matches!(err, NexusError::NotFound(_)));
    let err = s.canvas.delete(&c.id, "user").await.unwrap_err();
    assert!(matches!(err, NexusError::NotFound(_)));
}

#[tokio::test]
async fn task_ref_with_real_task() {
    let s = setup().await;
    let task = s
        .tasks
        .create(
            CreateTask {
                title: "Tarea del canvas".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let mut doc = sample_doc();
    doc.elements.push(CanvasElement {
        id: "ref1".into(),
        kind: "task_ref".into(),
        x: 600.0,
        y: 0.0,
        w: 240.0,
        h: 110.0,
        rotation: None,
        z: 3,
        group_id: None,
        style: json!({}),
        content: json!({"ref_id": task.id, "title": task.title, "status": task.status}),
    });
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Refs".into(),
                description: String::new(),
                template: None,
                document: Some(doc),
            },
            "user",
        )
        .await
        .unwrap();
    let got = s.canvas.get(&c.id).await.unwrap();
    let el = got
        .document
        .elements
        .iter()
        .find(|e| e.id == "ref1")
        .unwrap();
    assert_eq!(el.content["ref_id"], json!(task.id));
    let real = s
        .tasks
        .get(el.content["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(real.task.title, "Tarea del canvas");

    // Referencia sin ref_id → error de validación.
    let mut bad = sample_doc();
    bad.elements.push(CanvasElement {
        id: "ref2".into(),
        kind: "task_ref".into(),
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
        rotation: None,
        z: 1,
        group_id: None,
        style: json!({}),
        content: json!({"title": "sin id"}),
    });
    assert!(validate_document(bad).is_err());
}

#[tokio::test]
async fn geometry_and_content_validation() {
    let mut doc = sample_doc();
    doc.elements[0].w = -5.0;
    assert!(validate_document(doc).is_err());

    let mut doc = sample_doc();
    doc.elements[0].x = f64::NAN;
    assert!(validate_document(doc).is_err());

    let mut doc = sample_doc();
    doc.viewport.zoom = 0.0;
    assert!(validate_document(doc).is_err());

    let mut doc = sample_doc();
    doc.elements.push(CanvasElement {
        id: "img".into(),
        kind: "image".into(),
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
        rotation: None,
        z: 1,
        group_id: None,
        style: json!({}),
        content: json!({}),
    });
    assert!(validate_document(doc).is_err());

    let mut doc = sample_doc();
    doc.elements.push(CanvasElement {
        id: "lnk".into(),
        kind: "link".into(),
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
        rotation: None,
        z: 1,
        group_id: None,
        style: json!({}),
        content: json!({"url": "javascript:alert(1)"}),
    });
    assert!(validate_document(doc).is_err());
}

#[tokio::test]
async fn mutations_recorded_in_activity() {
    let s = setup().await;
    let c = s
        .canvas
        .create(
            CreateCanvas {
                name: "Auditoría".into(),
                description: String::new(),
                template: None,
                document: None,
            },
            "user",
        )
        .await
        .unwrap();
    s.canvas.save_document(&c.id, sample_doc()).await.unwrap();
    s.canvas.delete(&c.id, "user").await.unwrap();
    let events = s.activity.list(50, Some("canvas"), None).await.unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"created"));
    assert!(kinds.contains(&"deleted"));
    // El autoguardado del documento NO genera auditoría (undo en frontend).
    assert!(!kinds.contains(&"updated"));
}
