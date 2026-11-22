//! Tests de integración de NEXUS Database (servicio real + SQLite en memoria).

use nexus_core::*;
use nexus_db::Db;
use nexus_services::databases::DatabaseService;
use nexus_services::Services;
use serde_json::{json, Map, Value};

async fn setup() -> Services {
    Services::new(Db::connect_memory().await.unwrap())
}

async fn make_db(s: &Services, name: &str) -> Database {
    s.databases
        .create(
            CreateDatabase {
                name: name.into(),
                description: "base de prueba".into(),
                icon: "database".into(),
                color: "blue".into(),
            },
            "user",
        )
        .await
        .unwrap()
}

async fn field(
    s: &Services,
    db_id: &str,
    name: &str,
    kind: DbFieldKind,
    config: DbFieldConfig,
) -> DbField {
    s.databases
        .create_field(
            db_id,
            CreateDbField {
                name: name.into(),
                kind,
                config,
                position: None,
            },
            "user",
        )
        .await
        .unwrap()
}

fn vals(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

async fn record(s: &Services, db_id: &str, values: Map<String, Value>) -> DbRecord {
    s.databases
        .create_record(db_id, CreateDbRecord { values }, "user")
        .await
        .unwrap()
}

fn select_config(options: &[(&str, &str, &str)]) -> DbFieldConfig {
    DbFieldConfig {
        options: options
            .iter()
            .map(|(id, label, color)| DbSelectOption {
                id: id.to_string(),
                label: label.to_string(),
                color: color.to_string(),
            })
            .collect(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 1. CRUD de bases + detalle + referencia cruzada
// ---------------------------------------------------------------------------

#[tokio::test]
async fn database_crud_and_cross_reference() {
    let s = setup().await;
    let d = make_db(&s, "Videojuegos").await;
    assert_eq!(d.name, "Videojuegos");

    let list = s.databases.list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].record_count, 0);
    assert_eq!(list[0].view_count, 1); // vista tabla por defecto

    let updated = s
        .databases
        .update(
            &d.id,
            UpdateDatabase {
                name: Some("Juegos".into()),
                color: Some("purple".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Juegos");
    assert_eq!(updated.color, "purple");

    let f = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    record(&s, &d.id, vals(&[(&f.id, json!("Hades"))])).await;

    // databases.get devuelve lo que otros módulos necesitan para la referencia.
    let detail = s.databases.get(&d.id).await.unwrap();
    assert_eq!(detail.summary.database.name, "Juegos");
    assert_eq!(detail.summary.database.icon, "database");
    assert_eq!(detail.summary.record_count, 1);
    assert_eq!(detail.fields.len(), 1);

    s.databases.delete(&d.id, "user").await.unwrap();
    assert!(s.databases.list().await.unwrap().is_empty());
    assert_eq!(
        s.databases.get(&d.id).await.unwrap_err().code(),
        "not_found"
    );
}

// ---------------------------------------------------------------------------
// 2. Validación del nombre de la base
// ---------------------------------------------------------------------------

#[tokio::test]
async fn database_name_validation() {
    let s = setup().await;
    let err = s
        .databases
        .create(
            CreateDatabase {
                name: "   ".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    let err = s
        .databases
        .create(
            CreateDatabase {
                name: "X".into(),
                color: "no-color".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

// ---------------------------------------------------------------------------
// 3. Campos: crear, editar, nombre duplicado, config inválida
// ---------------------------------------------------------------------------

#[tokio::test]
async fn field_crud_and_validation() {
    let s = setup().await;
    let d = make_db(&s, "Cosas").await;
    let f = field(
        &s,
        &d.id,
        "Nombre",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    assert_eq!(f.position, 0);

    let f2 = s
        .databases
        .update_field(
            &f.id,
            UpdateDbField {
                name: Some("Título".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(f2.name, "Título");

    let err = s
        .databases
        .create_field(
            &d.id,
            CreateDbField {
                name: "título".into(),
                kind: DbFieldKind::Text,
                config: DbFieldConfig::default(),
                position: None,
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    // relation sin destino → error
    let err = s
        .databases
        .create_field(
            &d.id,
            CreateDbField {
                name: "Rel".into(),
                kind: DbFieldKind::Relation,
                config: DbFieldConfig::default(),
                position: None,
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    // fórmula con sintaxis inválida → error
    let err = s
        .databases
        .create_field(
            &d.id,
            CreateDbField {
                name: "Mala".into(),
                kind: DbFieldKind::Formula,
                config: DbFieldConfig {
                    expression: Some("2 +".into()),
                    ..Default::default()
                },
                position: None,
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    s.databases.delete_field(&f.id, "user").await.unwrap();
    assert!(s.databases.get(&d.id).await.unwrap().fields.is_empty());
}

// ---------------------------------------------------------------------------
// 4. Reordenar campos
// ---------------------------------------------------------------------------

#[tokio::test]
async fn field_reorder() {
    let s = setup().await;
    let d = make_db(&s, "Orden").await;
    let a = field(&s, &d.id, "A", DbFieldKind::Text, DbFieldConfig::default()).await;
    let b = field(
        &s,
        &d.id,
        "B",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    let c = field(
        &s,
        &d.id,
        "C",
        DbFieldKind::Checkbox,
        DbFieldConfig::default(),
    )
    .await;

    let reordered = s
        .databases
        .reorder_fields(&d.id, vec![c.id.clone(), a.id.clone(), b.id.clone()])
        .await
        .unwrap();
    assert_eq!(
        reordered
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        vec!["C", "A", "B"]
    );

    // Lista incompleta → error
    let err = s
        .databases
        .reorder_fields(&d.id, vec![a.id.clone()])
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

// ---------------------------------------------------------------------------
// 5. Borrar campo NO pierde datos (soft-delete)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn field_soft_delete_preserves_values() {
    let s = setup().await;
    let d = make_db(&s, "Datos").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let n = field(
        &s,
        &d.id,
        "Nota",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    let r = record(
        &s,
        &d.id,
        vals(&[(&t.id, json!("Item")), (&n.id, json!(9))]),
    )
    .await;

    s.databases.delete_field(&n.id, "user").await.unwrap();
    let fetched = s.databases.get_record(&d.id, &r.id).await.unwrap();
    // El valor sigue almacenado aunque el campo ya no se muestre.
    assert_eq!(fetched.values.get(&n.id), Some(&json!(9)));

    // Un campo nuevo con el mismo nombre no hereda los valores del borrado.
    let n2 = field(
        &s,
        &d.id,
        "Nota",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    assert_ne!(n2.id, n.id);
    let fetched = s.databases.get_record(&d.id, &r.id).await.unwrap();
    assert!(fetched.values.get(&n2.id).is_none());
}

// ---------------------------------------------------------------------------
// 6. Validación de valores por tipo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn record_value_validation_by_type() {
    let s = setup().await;
    let d = make_db(&s, "Tipos").await;
    let email = field(
        &s,
        &d.id,
        "Email",
        DbFieldKind::Email,
        DbFieldConfig::default(),
    )
    .await;
    let rating = field(
        &s,
        &d.id,
        "Nota",
        DbFieldKind::Rating,
        DbFieldConfig::default(),
    )
    .await;
    let sel = field(
        &s,
        &d.id,
        "Estado",
        DbFieldKind::Select,
        select_config(&[("a", "Activo", "green")]),
    )
    .await;
    let date = field(
        &s,
        &d.id,
        "Fecha",
        DbFieldKind::Date,
        DbFieldConfig::default(),
    )
    .await;
    let url = field(&s, &d.id, "Web", DbFieldKind::Url, DbFieldConfig::default()).await;
    let int = field(
        &s,
        &d.id,
        "Cantidad",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;

    let bad = |values: Map<String, Value>| {
        let s = &s;
        let d = d.id.clone();
        async move {
            s.databases
                .create_record(&d, CreateDbRecord { values }, "user")
                .await
                .unwrap_err()
                .code()
                .to_string()
        }
    };

    assert_eq!(
        bad(vals(&[(&email.id, json!("no-es-email"))])).await,
        "validation"
    );
    assert_eq!(bad(vals(&[(&rating.id, json!(6))])).await, "validation");
    assert_eq!(bad(vals(&[(&rating.id, json!(0))])).await, "validation");
    assert_eq!(
        bad(vals(&[(&sel.id, json!("inexistente"))])).await,
        "validation"
    );
    assert_eq!(
        bad(vals(&[(&date.id, json!("31/12/2025"))])).await,
        "validation"
    );
    assert_eq!(
        bad(vals(&[(&url.id, json!("ftp://x"))])).await,
        "validation"
    );
    assert_eq!(bad(vals(&[(&int.id, json!("abc"))])).await, "validation");
    assert_eq!(
        bad(vals(&[("campo-fantasma", json!(1))])).await,
        "validation"
    );

    // Valores buenos (con normalización: integer desde string).
    let r = record(
        &s,
        &d.id,
        vals(&[
            (&email.id, json!("ada@example.com")),
            (&rating.id, json!(4)),
            (&sel.id, json!("a")),
            (&date.id, json!("2025-12-31")),
            (&url.id, json!("https://example.com")),
            (&int.id, json!("42")),
        ]),
    )
    .await;
    assert_eq!(r.values.get(&int.id), Some(&json!(42)));
}

// ---------------------------------------------------------------------------
// 7. Escribir un campo fórmula → rechazado; unknown update
// ---------------------------------------------------------------------------

#[tokio::test]
async fn formula_field_is_read_only_and_record_update_merges() {
    let s = setup().await;
    let d = make_db(&s, "Calc").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let p = field(
        &s,
        &d.id,
        "Precio",
        DbFieldKind::Decimal,
        DbFieldConfig::default(),
    )
    .await;
    let f = field(
        &s,
        &d.id,
        "Con IVA",
        DbFieldKind::Formula,
        DbFieldConfig {
            expression: Some("ROUND({Precio} * 1.21, 2)".into()),
            ..Default::default()
        },
    )
    .await;

    let err = s
        .databases
        .create_record(
            &d.id,
            CreateDbRecord {
                values: vals(&[(&f.id, json!(100))]),
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    let r = record(
        &s,
        &d.id,
        vals(&[(&t.id, json!("Silla")), (&p.id, json!(100.0))]),
    )
    .await;
    assert_eq!(r.computed.get(&f.id), Some(&json!(121.0)));

    // Update parcial: solo cambia Precio, Título se conserva.
    let updated = s
        .databases
        .update_record(
            &d.id,
            &r.id,
            UpdateDbRecord {
                values: vals(&[(&p.id, json!(200.0))]),
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(updated.values.get(&t.id), Some(&json!("Silla")));
    assert_eq!(updated.computed.get(&f.id), Some(&json!(242.0)));

    // null borra el valor
    let cleared = s
        .databases
        .update_record(
            &d.id,
            &r.id,
            UpdateDbRecord {
                values: vals(&[(&p.id, Value::Null)]),
            },
            "user",
        )
        .await
        .unwrap();
    assert!(cleared.values.get(&p.id).is_none());
    // Fórmula con operando vacío → null + error legible.
    assert_eq!(cleared.computed.get(&f.id), Some(&Value::Null));
    assert!(cleared.formula_errors.get(&f.id).is_some());
}

// ---------------------------------------------------------------------------
// 8. Fórmulas: IF, concatenación, referencia, error de campo inexistente
// ---------------------------------------------------------------------------

#[tokio::test]
async fn formula_engine_end_to_end() {
    let s = setup().await;
    let d = make_db(&s, "Fórmulas").await;
    let nombre = field(
        &s,
        &d.id,
        "Nombre",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let nota = field(
        &s,
        &d.id,
        "Nota",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    let etiqueta = field(
        &s,
        &d.id,
        "Etiqueta",
        DbFieldKind::Formula,
        DbFieldConfig {
            expression: Some("UPPER({Nombre}) & \": \" & IF({Nota} >= 5, \"OK\", \"KO\")".into()),
            ..Default::default()
        },
    )
    .await;
    let r = record(
        &s,
        &d.id,
        vals(&[(&nombre.id, json!("ada")), (&nota.id, json!(7))]),
    )
    .await;
    assert_eq!(r.computed.get(&etiqueta.id), Some(&json!("ADA: OK")));

    let r2 = record(
        &s,
        &d.id,
        vals(&[(&nombre.id, json!("bob")), (&nota.id, json!(3))]),
    )
    .await;
    assert_eq!(r2.computed.get(&etiqueta.id), Some(&json!("BOB: KO")));

    // DAYS entre fechas
    let ini = field(
        &s,
        &d.id,
        "Inicio",
        DbFieldKind::Date,
        DbFieldConfig::default(),
    )
    .await;
    let fin = field(
        &s,
        &d.id,
        "Fin",
        DbFieldKind::Date,
        DbFieldConfig::default(),
    )
    .await;
    let dias = field(
        &s,
        &d.id,
        "Duración",
        DbFieldKind::Formula,
        DbFieldConfig {
            expression: Some("DAYS({Inicio}, {Fin})".into()),
            ..Default::default()
        },
    )
    .await;
    let r3 = record(
        &s,
        &d.id,
        vals(&[
            (&ini.id, json!("2025-01-01")),
            (&fin.id, json!("2025-02-01")),
        ]),
    )
    .await;
    assert_eq!(r3.computed.get(&dias.id), Some(&json!(31.0)));
}

// ---------------------------------------------------------------------------
// 9. Filtros AND/OR + ordenación multi-campo + búsqueda
// ---------------------------------------------------------------------------

#[tokio::test]
async fn filters_and_or_sorting_and_search() {
    let s = setup().await;
    let d = make_db(&s, "Filtros").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let n = field(
        &s,
        &d.id,
        "Nota",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    let sel = field(
        &s,
        &d.id,
        "Estado",
        DbFieldKind::Select,
        select_config(&[("a", "Activo", "green"), ("b", "Archivado", "gray")]),
    )
    .await;

    record(
        &s,
        &d.id,
        vals(&[
            (&t.id, json!("Alfa")),
            (&n.id, json!(3)),
            (&sel.id, json!("a")),
        ]),
    )
    .await;
    record(
        &s,
        &d.id,
        vals(&[
            (&t.id, json!("Beta")),
            (&n.id, json!(8)),
            (&sel.id, json!("b")),
        ]),
    )
    .await;
    record(
        &s,
        &d.id,
        vals(&[
            (&t.id, json!("Gamma")),
            (&n.id, json!(8)),
            (&sel.id, json!("a")),
        ]),
    )
    .await;
    record(
        &s,
        &d.id,
        vals(&[(&t.id, json!("Delta sin nota")), (&sel.id, json!("b"))]),
    )
    .await;

    // AND: nota >= 5 y estado = a → solo Gamma
    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "and".into(),
                    filters: vec![
                        DbFilter {
                            field_id: n.id.clone(),
                            op: DbFilterOp::Gte,
                            value: json!(5),
                        },
                        DbFilter {
                            field_id: sel.id.clone(),
                            op: DbFilterOp::Eq,
                            value: json!("a"),
                        },
                    ],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.records[0].values.get(&t.id), Some(&json!("Gamma")));

    // OR: nota < 5 o estado = b → Alfa, Beta, Delta
    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "or".into(),
                    filters: vec![
                        DbFilter {
                            field_id: n.id.clone(),
                            op: DbFilterOp::Lt,
                            value: json!(5),
                        },
                        DbFilter {
                            field_id: sel.id.clone(),
                            op: DbFilterOp::Eq,
                            value: json!("b"),
                        },
                    ],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 3);

    // is_empty sobre la nota → Delta
    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "and".into(),
                    filters: vec![DbFilter {
                        field_id: n.id.clone(),
                        op: DbFilterOp::IsEmpty,
                        value: Value::Null,
                    }],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(
        page.records[0].values.get(&t.id),
        Some(&json!("Delta sin nota"))
    );

    // Orden multi-campo: nota desc (vacíos al final), título asc
    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                sorts: vec![
                    DbSort {
                        field_id: n.id.clone(),
                        dir: DbSortDir::Desc,
                    },
                    DbSort {
                        field_id: t.id.clone(),
                        dir: DbSortDir::Asc,
                    },
                ],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let titles: Vec<&str> = page
        .records
        .iter()
        .map(|r| r.values.get(&t.id).unwrap().as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Beta", "Gamma", "Alfa", "Delta sin nota"]);

    // Búsqueda de texto libre
    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                search: Some("gam".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);

    // contains sobre select usa etiquetas almacenadas (id)
    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "and".into(),
                    filters: vec![DbFilter {
                        field_id: sel.id.clone(),
                        op: DbFilterOp::Contains,
                        value: json!("b"),
                    }],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 2); // ids que contienen "b": solo "b" (Beta, Delta)

    // Filtrar por fórmula → error claro (nunca SQL del usuario)
    let fx = field(
        &s,
        &d.id,
        "Doble",
        DbFieldKind::Formula,
        DbFieldConfig {
            expression: Some("{Nota} * 2".into()),
            ..Default::default()
        },
    )
    .await;
    let err = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "and".into(),
                    filters: vec![DbFilter {
                        field_id: fx.id.clone(),
                        op: DbFilterOp::Gt,
                        value: json!(1),
                    }],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

// ---------------------------------------------------------------------------
// 10. Ordenar por fórmula (paginación en memoria)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sort_by_formula_field() {
    let s = setup().await;
    let d = make_db(&s, "OrdenFormula").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let n = field(
        &s,
        &d.id,
        "Base",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    let f = field(
        &s,
        &d.id,
        "Doble",
        DbFieldKind::Formula,
        DbFieldConfig {
            expression: Some("{Base} * 2".into()),
            ..Default::default()
        },
    )
    .await;
    record(&s, &d.id, vals(&[(&t.id, json!("uno")), (&n.id, json!(1))])).await;
    record(
        &s,
        &d.id,
        vals(&[(&t.id, json!("tres")), (&n.id, json!(3))]),
    )
    .await;
    record(&s, &d.id, vals(&[(&t.id, json!("dos")), (&n.id, json!(2))])).await;

    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                sorts: vec![DbSort {
                    field_id: f.id.clone(),
                    dir: DbSortDir::Desc,
                }],
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.records.len(), 2);
    assert_eq!(page.records[0].values.get(&t.id), Some(&json!("tres")));
    assert_eq!(page.records[1].values.get(&t.id), Some(&json!("dos")));
}

// ---------------------------------------------------------------------------
// 11. Relaciones: crear, leer títulos, cardinalidad, destino inexistente
// ---------------------------------------------------------------------------

#[tokio::test]
async fn relations_create_read_and_cardinality() {
    let s = setup().await;
    let autores = make_db(&s, "Autores").await;
    let at = field(
        &s,
        &autores.id,
        "Nombre",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let a1 = record(
        &s,
        &autores.id,
        vals(&[(&at.id, json!("Ursula K. Le Guin"))]),
    )
    .await;
    let a2 = record(&s, &autores.id, vals(&[(&at.id, json!("Ted Chiang"))])).await;

    let libros = make_db(&s, "Libros").await;
    let lt = field(
        &s,
        &libros.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let rel = field(
        &s,
        &libros.id,
        "Autores",
        DbFieldKind::Relation,
        DbFieldConfig {
            target_database_id: Some(autores.id.clone()),
            cardinality: Some(DbCardinality::ManyToMany),
            ..Default::default()
        },
    )
    .await;

    let libro = record(
        &s,
        &libros.id,
        vals(&[
            (&lt.id, json!("Antología")),
            (&rel.id, json!([a1.id, a2.id])),
        ]),
    )
    .await;
    let chips = libro
        .relation_titles
        .get(&rel.id)
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(chips.len(), 2);
    assert_eq!(chips[0].get("title").unwrap(), "Ursula K. Le Guin");

    // Destino inexistente → error
    let err = s
        .databases
        .create_record(
            &libros.id,
            CreateDbRecord {
                values: vals(&[(&rel.id, json!(["id-fantasma"]))]),
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    // Cardinalidad 1:1 → máximo un id
    let rel1 = field(
        &s,
        &libros.id,
        "Autor principal",
        DbFieldKind::Relation,
        DbFieldConfig {
            target_database_id: Some(autores.id.clone()),
            cardinality: Some(DbCardinality::OneToOne),
            ..Default::default()
        },
    )
    .await;
    let err = s
        .databases
        .create_record(
            &libros.id,
            CreateDbRecord {
                values: vals(&[(&rel1.id, json!([a1.id, a2.id]))]),
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    // record_preview para navegación cruzada
    let preview = s
        .databases
        .record_preview(&autores.id, &a1.id)
        .await
        .unwrap();
    assert_eq!(preview.title, "Ursula K. Le Guin");
    assert_eq!(preview.database_name, "Autores");
}

// ---------------------------------------------------------------------------
// 12. Al borrar un registro se limpian las referencias entrantes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deleting_record_cleans_references() {
    let s = setup().await;
    let tags = make_db(&s, "Etiquetas").await;
    let tt = field(
        &s,
        &tags.id,
        "Nombre",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let t1 = record(&s, &tags.id, vals(&[(&tt.id, json!("urgente"))])).await;
    let t2 = record(&s, &tags.id, vals(&[(&tt.id, json!("casa"))])).await;

    let tareas = make_db(&s, "Tareas").await;
    let nt = field(
        &s,
        &tareas.id,
        "Tarea",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let rel = field(
        &s,
        &tareas.id,
        "Etiquetas",
        DbFieldKind::Relation,
        DbFieldConfig {
            target_database_id: Some(tags.id.clone()),
            ..Default::default()
        },
    )
    .await;
    let tarea = record(
        &s,
        &tareas.id,
        vals(&[(&nt.id, json!("Pintar")), (&rel.id, json!([t1.id, t2.id]))]),
    )
    .await;

    s.databases
        .delete_records(&tags.id, vec![t1.id.clone()], "user")
        .await
        .unwrap();
    let after = s.databases.get_record(&tareas.id, &tarea.id).await.unwrap();
    assert_eq!(after.values.get(&rel.id), Some(&json!([t2.id])));
    let chips = after
        .relation_titles
        .get(&rel.id)
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(chips.len(), 1);
    assert_eq!(chips[0].get("title").unwrap(), "casa");

    // Borrado masivo del resto
    let deleted = s
        .databases
        .delete_records(&tags.id, vec![t2.id.clone()], "user")
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    let after = s.databases.get_record(&tareas.id, &tarea.id).await.unwrap();
    assert_eq!(after.values.get(&rel.id), Some(&json!([])));
}

// ---------------------------------------------------------------------------
// 13. Borrar una base no rompe las demás (limpia referencias cruzadas)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deleting_database_does_not_break_others() {
    let s = setup().await;
    let a = make_db(&s, "Origen").await;
    let at = field(
        &s,
        &a.id,
        "Nombre",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let ra = record(&s, &a.id, vals(&[(&at.id, json!("Elemento"))])).await;

    let b = make_db(&s, "Destino").await;
    let bt = field(
        &s,
        &b.id,
        "Nombre",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let rel = field(
        &s,
        &b.id,
        "Ref",
        DbFieldKind::Relation,
        DbFieldConfig {
            target_database_id: Some(a.id.clone()),
            ..Default::default()
        },
    )
    .await;
    let rb = record(
        &s,
        &b.id,
        vals(&[(&bt.id, json!("Otro")), (&rel.id, json!([ra.id]))]),
    )
    .await;

    s.databases.delete(&a.id, "user").await.unwrap();

    // La base B sigue viva y el registro ya no referencia el borrado.
    let after = s.databases.get_record(&b.id, &rb.id).await.unwrap();
    assert_eq!(after.values.get(&rel.id), Some(&json!([])));
    assert_eq!(s.databases.list().await.unwrap().len(), 1);
    // Sus registros y campos siguen accesibles.
    let page = s
        .databases
        .query_records(&b.id, RecordQuery::default())
        .await
        .unwrap();
    assert_eq!(page.total, 1);
}

// ---------------------------------------------------------------------------
// 14. Vistas: CRUD + validación de configuración por tipo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn views_crud_and_config_validation() {
    let s = setup().await;
    let d = make_db(&s, "Vistas").await;
    let sel = field(
        &s,
        &d.id,
        "Estado",
        DbFieldKind::Select,
        select_config(&[("a", "A", "green")]),
    )
    .await;
    let date = field(
        &s,
        &d.id,
        "Fecha",
        DbFieldKind::Date,
        DbFieldConfig::default(),
    )
    .await;
    let img = field(
        &s,
        &d.id,
        "Foto",
        DbFieldKind::Image,
        DbFieldConfig::default(),
    )
    .await;

    let kanban = s
        .databases
        .create_view(
            &d.id,
            CreateDbView {
                name: "Tablero".into(),
                kind: DbViewKind::Kanban,
                config: DbViewConfig {
                    group_field_id: Some(sel.id.clone()),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(kanban.kind, DbViewKind::Kanban);

    let cal = s
        .databases
        .create_view(
            &d.id,
            CreateDbView {
                name: "Calendario".into(),
                kind: DbViewKind::Calendar,
                config: DbViewConfig {
                    date_field_id: Some(date.id.clone()),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap();
    let gal = s
        .databases
        .create_view(
            &d.id,
            CreateDbView {
                name: "Galería".into(),
                kind: DbViewKind::Gallery,
                config: DbViewConfig {
                    image_field_id: Some(img.id.clone()),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(s.databases.get(&d.id).await.unwrap().views.len(), 4); // + tabla por defecto

    // kanban agrupado por un campo que no es select → error
    let err = s
        .databases
        .create_view(
            &d.id,
            CreateDbView {
                name: "Mala".into(),
                kind: DbViewKind::Kanban,
                config: DbViewConfig {
                    group_field_id: Some(date.id.clone()),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    let renamed = s
        .databases
        .update_view(
            &kanban.id,
            UpdateDbView {
                name: Some("Por estado".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.name, "Por estado");

    s.databases.delete_view(&cal.id).await.unwrap();
    s.databases.delete_view(&gal.id).await.unwrap();
    assert_eq!(s.databases.get(&d.id).await.unwrap().views.len(), 2);
}

// ---------------------------------------------------------------------------
// 15. Import CSV: preview, mapeo con creación de campos, filas malas
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_csv_mapping_and_bad_rows() {
    let s = setup().await;
    let d = make_db(&s, "Importar").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;

    let csv = "nombre,nota,estado\nUno,3,Activo\nDos,9,Activo\nMalo,no-numero,Activo\n";
    let preview = DatabaseService::parse_import("csv", csv).unwrap();
    assert_eq!(preview.columns, vec!["nombre", "nota", "estado"]);
    assert_eq!(preview.total_rows, 3);

    let req = ImportRequest {
        database_id: d.id.clone(),
        format: "csv".into(),
        content: csv.into(),
        mappings: vec![
            ImportMapping {
                column: "nombre".into(),
                field_id: Some(t.id.clone()),
                new_field: None,
            },
            ImportMapping {
                column: "nota".into(),
                field_id: None,
                new_field: Some(CreateDbField {
                    name: "Nota".into(),
                    kind: DbFieldKind::Integer,
                    config: DbFieldConfig::default(),
                    position: None,
                }),
            },
            ImportMapping {
                column: "estado".into(),
                field_id: None,
                new_field: Some(CreateDbField {
                    name: "Estado".into(),
                    kind: DbFieldKind::Select,
                    config: select_config(&[("activo", "Activo", "green")]),
                    position: None,
                }),
            },
        ],
        skip_invalid: true,
    };
    let result = s.databases.import(req.clone(), "user").await.unwrap();
    assert_eq!(result.imported, 2);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.errors[0].row, 3);
    assert_eq!(result.created_fields.len(), 2);

    let page = s
        .databases
        .query_records(&d.id, RecordQuery::default())
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    let nota_field = result
        .created_fields
        .iter()
        .find(|f| f.name == "Nota")
        .unwrap();
    assert_eq!(page.records[0].values.get(&nota_field.id), Some(&json!(3)));

    // Sin skip_invalid, una fila mala aborta el import completo.
    let mut strict = req.clone();
    strict.skip_invalid = false;
    // Los campos ya existen: mapea por id para no recrearlos.
    for m in &mut strict.mappings {
        if let Some(nf) = m.new_field.take() {
            let existing = s
                .databases
                .get(&d.id)
                .await
                .unwrap()
                .fields
                .into_iter()
                .find(|f| f.name == nf.name)
                .unwrap();
            m.field_id = Some(existing.id);
        }
    }
    let err = s.databases.import(strict, "user").await.unwrap_err();
    assert_eq!(err.code(), "validation");
    assert_eq!(
        s.databases
            .query_records(&d.id, RecordQuery::default())
            .await
            .unwrap()
            .total,
        2
    );
}

// ---------------------------------------------------------------------------
// 16. Límites de import (tamaño y filas) + JSON preview
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_limits_and_json_preview() {
    let big = "x".repeat(5 * 1024 * 1024 + 1);
    let err = DatabaseService::parse_import("csv", &big).unwrap_err();
    assert_eq!(err.code(), "limit_exceeded");

    let mut many = String::from("a\n");
    for _ in 0..10_001 {
        many.push_str("1\n");
    }
    let err = DatabaseService::parse_import("csv", &many).unwrap_err();
    assert_eq!(err.code(), "limit_exceeded");

    let json = r#"[{"nombre":"Ana","edad":30},{"nombre":"Luis","edad":null,"activo":true}]"#;
    let preview = DatabaseService::parse_import("json", json).unwrap();
    assert_eq!(preview.columns, vec!["nombre", "edad", "activo"]);
    assert_eq!(preview.total_rows, 2);
    assert_eq!(preview.rows[1][2], "true");

    let err = DatabaseService::parse_import("xml", "<x/>").unwrap_err();
    assert_eq!(err.code(), "validation");
}

// ---------------------------------------------------------------------------
// 17. Export CSV/JSON roundtrip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn export_csv_json_roundtrip() {
    let s = setup().await;
    let d = make_db(&s, "Exportar").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    let n = field(
        &s,
        &d.id,
        "Nota",
        DbFieldKind::Integer,
        DbFieldConfig::default(),
    )
    .await;
    let sel = field(
        &s,
        &d.id,
        "Estado",
        DbFieldKind::Select,
        select_config(&[("a", "Activo", "green")]),
    )
    .await;
    record(
        &s,
        &d.id,
        vals(&[
            (&t.id, json!("Primero, con coma")),
            (&n.id, json!(7)),
            (&sel.id, json!("a")),
        ]),
    )
    .await;
    record(
        &s,
        &d.id,
        vals(&[(&t.id, json!("Segundo")), (&n.id, json!(2))]),
    )
    .await;

    let csv = s.databases.export(&d.id, "csv", None).await.unwrap();
    assert_eq!(csv.row_count, 2);
    assert!(csv.content.contains("Activo")); // etiqueta, no id

    // Roundtrip CSV → base nueva
    let d2 = make_db(&s, "Reimportada").await;
    let preview = DatabaseService::parse_import("csv", &csv.content).unwrap();
    assert_eq!(preview.total_rows, 2);
    let mappings: Vec<ImportMapping> = preview
        .columns
        .iter()
        .map(|c| {
            let kind = match c.as_str() {
                "Nota" => DbFieldKind::Integer,
                "Estado" => DbFieldKind::Select,
                _ => DbFieldKind::Text,
            };
            let config = if c == "Estado" {
                select_config(&[("a", "Activo", "green")])
            } else {
                DbFieldConfig::default()
            };
            ImportMapping {
                column: c.clone(),
                field_id: None,
                new_field: Some(CreateDbField {
                    name: c.clone(),
                    kind,
                    config,
                    position: None,
                }),
            }
        })
        .collect();
    let result = s
        .databases
        .import(
            ImportRequest {
                database_id: d2.id.clone(),
                format: "csv".into(),
                content: csv.content,
                mappings,
                skip_invalid: false,
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(result.imported, 2);
    let page = s
        .databases
        .query_records(&d2.id, RecordQuery::default())
        .await
        .unwrap();
    let titles: Vec<&str> = page
        .records
        .iter()
        .map(|r| {
            r.values
                .get(&result.created_fields[0].id)
                .unwrap()
                .as_str()
                .unwrap()
        })
        .collect();
    assert!(titles.contains(&"Primero, con coma"));

    // JSON export
    let json_export = s.databases.export(&d.id, "json", None).await.unwrap();
    let parsed: Value = serde_json::from_str(&json_export.content).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["Título"], json!("Primero, con coma"));
    assert_eq!(parsed[0]["Nota"], json!(7));

    let err = s.databases.export(&d.id, "xlsx", None).await.unwrap_err();
    assert_eq!(err.code(), "validation");
}

// ---------------------------------------------------------------------------
// 18. Plantillas: todas instalables, con registros reales y vistas válidas
// ---------------------------------------------------------------------------

#[tokio::test]
async fn templates_all_installable() {
    let s = setup().await;
    let templates = DatabaseService::templates();
    assert_eq!(templates.len(), 8);
    for t in &templates {
        let detail = s.databases.install_template(&t.id, "user").await.unwrap();
        assert_eq!(detail.summary.database.name, t.name);
        assert!(!detail.fields.is_empty(), "plantilla {} sin campos", t.id);
        assert!(!detail.views.is_empty(), "plantilla {} sin vistas", t.id);
        assert!(t.sample_records.len() >= 2);
        let page = s
            .databases
            .query_records(&detail.summary.database.id, RecordQuery::default())
            .await
            .unwrap();
        assert_eq!(
            page.total,
            t.sample_records.len() as i64,
            "plantilla {}",
            t.id
        );
        // Las fórmulas de las plantillas evalúan sin error en los ejemplos.
        for r in &page.records {
            assert!(
                r.formula_errors.is_empty(),
                "plantilla {} con error de fórmula: {:?}",
                t.id,
                r.formula_errors
            );
        }
    }
    let err = s
        .databases
        .install_template("no-existe", "user")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

// ---------------------------------------------------------------------------
// 19. Adjuntos: image/file validan la existencia del archivo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn attachment_fields_require_existing_file() {
    let s = setup().await;
    let d = make_db(&s, "Adjuntos").await;
    let img = field(
        &s,
        &d.id,
        "Foto",
        DbFieldKind::Image,
        DbFieldConfig::default(),
    )
    .await;

    let err = s
        .databases
        .create_record(
            &d.id,
            CreateDbRecord {
                values: vals(&[(&img.id, json!("archivo-fantasma"))]),
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    let meta = s
        .files
        .save("foto.png", "image/png", &[1, 2, 3])
        .await
        .unwrap();
    let r = record(&s, &d.id, vals(&[(&img.id, json!(meta.id))])).await;
    assert_eq!(r.values.get(&img.id), Some(&json!(meta.id)));
}

// ---------------------------------------------------------------------------
// 20. IA: saneado de propuestas + duplicados locales sin proveedor
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ai_proposal_sanitizing_and_local_duplicates() {
    let proposal = DatabaseService::parse_ai_structure(&json!({
        "name": "Recetas",
        "icon": "chef-hat",
        "color": "verde-inventado",
        "fields": [
            {"name": "Nombre", "kind": "text"},
            {"name": "Dificultad", "kind": "select", "options": ["Fácil", "Media", "Difícil"]},
            {"name": "Autor", "kind": "relation", "target_database_id": "inventado"},
            {"name": "Basura", "kind": "tipo-inexistente"},
            {"name": "Nombre", "kind": "text"}
        ]
    }))
    .unwrap();
    assert_eq!(proposal.color, "blue"); // color inválido → por defecto
    assert_eq!(proposal.fields.len(), 3); // duplicado y tipo inválido descartados
    assert_eq!(proposal.fields[1].config.options.len(), 3);
    assert_eq!(proposal.fields[2].kind, DbFieldKind::Text); // relation degradada

    let err = DatabaseService::parse_ai_structure(&json!({"name": "X", "fields": []})).unwrap_err();
    assert_eq!(err.code(), "ai");

    // Duplicados locales sin IA
    let s = setup().await;
    let d = make_db(&s, "Duplicados").await;
    let t = field(
        &s,
        &d.id,
        "Título",
        DbFieldKind::Text,
        DbFieldConfig::default(),
    )
    .await;
    record(&s, &d.id, vals(&[(&t.id, json!("Dune"))])).await;
    record(&s, &d.id, vals(&[(&t.id, json!("dune"))])).await;
    record(&s, &d.id, vals(&[(&t.id, json!("Otra cosa"))])).await;
    let pairs = s.databases.find_duplicates(&d.id, None).await.unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].score, 1.0);

    // Pares de IA con ids falsos se descartan
    let pairs = s
        .databases
        .find_duplicates(
            &d.id,
            Some(json!([{"record_a": "falso", "record_b": "falso2"}])),
        )
        .await
        .unwrap();
    assert_eq!(pairs.len(), 1);
}

// ---------------------------------------------------------------------------
// 21. Multiselect: validación y filtros contains
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiselect_validation_and_filter() {
    let s = setup().await;
    let d = make_db(&s, "Multi").await;
    let m = field(
        &s,
        &d.id,
        "Etiquetas",
        DbFieldKind::Multiselect,
        select_config(&[("x", "X", "blue"), ("y", "Y", "green")]),
    )
    .await;
    let err = s
        .databases
        .create_record(
            &d.id,
            CreateDbRecord {
                values: vals(&[(&m.id, json!(["x", "z"]))]),
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    record(&s, &d.id, vals(&[(&m.id, json!(["x", "y"]))])).await;
    record(&s, &d.id, vals(&[(&m.id, json!(["y"]))])).await;

    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "and".into(),
                    filters: vec![DbFilter {
                        field_id: m.id.clone(),
                        op: DbFilterOp::Contains,
                        value: json!("x"),
                    }],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);

    let page = s
        .databases
        .query_records(
            &d.id,
            RecordQuery {
                filters: DbFilterGroup {
                    conjunction: "and".into(),
                    filters: vec![DbFilter {
                        field_id: m.id.clone(),
                        op: DbFilterOp::NotEmpty,
                        value: Value::Null,
                    }],
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 2);
}
