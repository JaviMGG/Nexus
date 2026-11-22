//! Escritura real en NEXUS Database: crear y actualizar registros de bases
//! de datos personalizadas. Los valores se validan en el servicio según el
//! tipo de cada campo; las mutaciones se auditan con actor `agent:<id>`.

use std::sync::Arc;

use nexus_core::{CreateDbRecord, NexusError, Result, Risk, UpdateDbRecord};
use serde_json::{json, Value};

use crate::builtins::{define_tool, req_str, schema, val};
use crate::{Tool, ToolContext};

define_tool!(DatabasesRecordsCreate, "databases.records.create", Risk::Write,
    "Crea un registro real en una base de datos personalizada. Consulta antes databases.get para conocer los ids y tipos de campo; los valores se validan según el tipo de cada campo.",
    schema(json!({
        "database_id": { "type": "string" },
        "values": { "type": "object", "description": "mapa id_de_campo → valor; consulta databases.get para conocer los ids y tipos de campo" }
    }), &["database_id", "values"]),
    |args, ctx| {
        let database_id = req_str(&args, "database_id")?;
        let values = args.get("values").and_then(Value::as_object).cloned()
            .ok_or_else(|| NexusError::validation("falta 'values' (objeto)"))?;
        val(ctx.services.databases
            .create_record(&database_id, CreateDbRecord { values }, &ctx.actor())
            .await?)
    }
);

define_tool!(DatabasesRecordsUpdate, "databases.records.update", Risk::Write,
    "Actualiza un registro existente de una base de datos personalizada. Consulta antes databases.get para los ids de campo y databases.records para los ids de registro; los valores se validan según el tipo de cada campo.",
    schema(json!({
        "database_id": { "type": "string" },
        "record_id": { "type": "string" },
        "values": { "type": "object", "description": "solo las claves presentes se reemplazan; null borra el valor" }
    }), &["database_id", "record_id", "values"]),
    |args, ctx| {
        let database_id = req_str(&args, "database_id")?;
        let record_id = req_str(&args, "record_id")?;
        let values = args.get("values").and_then(Value::as_object).cloned()
            .ok_or_else(|| NexusError::validation("falta 'values' (objeto)"))?;
        val(ctx.services.databases
            .update_record(&database_id, &record_id, UpdateDbRecord { values }, &ctx.actor())
            .await?)
    }
);

pub(crate) fn tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(DatabasesRecordsCreate),
        Arc::new(DatabasesRecordsUpdate),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_args;
    use nexus_core::{CreateDatabase, CreateDbField, DbFieldConfig, DbFieldKind, RecordQuery};
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

    async fn base_con_campo_texto(ctx: &ToolContext) -> (String, String) {
        let db = ctx
            .services
            .databases
            .create(
                CreateDatabase {
                    name: "Contactos".into(),
                    description: "base de prueba".into(),
                    icon: "database".into(),
                    color: "blue".into(),
                },
                "user",
            )
            .await
            .unwrap();
        let field = ctx
            .services
            .databases
            .create_field(
                &db.id,
                CreateDbField {
                    name: "Nombre".into(),
                    kind: DbFieldKind::Text,
                    config: DbFieldConfig::default(),
                    position: None,
                },
                "user",
            )
            .await
            .unwrap();
        (db.id, field.id)
    }

    fn tool(name: &str) -> Arc<dyn Tool> {
        tools()
            .into_iter()
            .find(|t| t.spec().name == name)
            .unwrap()
    }

    #[tokio::test]
    async fn crear_registro_y_consultarlo() {
        let ctx = ctx().await;
        let (db_id, field_id) = base_con_campo_texto(&ctx).await;
        let create = tool("databases.records.create");
        let args = json!({"database_id": db_id, "values": { field_id.clone(): "Ada" }});
        validate_args(&create.spec(), &args).unwrap();
        let created = create.execute(args, &ctx).await.unwrap();
        assert_eq!(created["values"][&field_id], "Ada");

        let page = ctx
            .services
            .databases
            .query_records(&db_id, RecordQuery::default())
            .await
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.records[0].id, created["id"].as_str().unwrap());
        assert_eq!(page.records[0].values.get(&field_id), Some(&json!("Ada")));
    }

    #[tokio::test]
    async fn actualizar_registro_cambia_el_valor() {
        let ctx = ctx().await;
        let (db_id, field_id) = base_con_campo_texto(&ctx).await;
        let record = ctx
            .services
            .databases
            .create_record(
                &db_id,
                CreateDbRecord {
                    values: [(field_id.clone(), json!("Ada"))].into_iter().collect(),
                },
                "user",
            )
            .await
            .unwrap();

        let update = tool("databases.records.update");
        let args = json!({
            "database_id": db_id,
            "record_id": record.id,
            "values": { field_id.clone(): "Grace" }
        });
        validate_args(&update.spec(), &args).unwrap();
        let updated = update.execute(args, &ctx).await.unwrap();
        assert_eq!(updated["id"], json!(record.id));
        assert_eq!(updated["values"][&field_id], "Grace");

        let page = ctx
            .services
            .databases
            .query_records(&db_id, RecordQuery::default())
            .await
            .unwrap();
        assert_eq!(page.records[0].values.get(&field_id), Some(&json!("Grace")));
    }

    #[tokio::test]
    async fn validate_args_rechaza_argumentos_incompletos() {
        let ctx = ctx().await;
        let create = tool("databases.records.create");
        let update = tool("databases.records.update");

        for spec in [create.spec(), update.spec()] {
            let err = validate_args(&spec, &json!({})).unwrap_err();
            assert!(matches!(err, NexusError::Validation(_)));
            let err = validate_args(&spec, &json!({"database_id": "db"})).unwrap_err();
            assert!(matches!(err, NexusError::Validation(_)));
            let err = validate_args(&spec, &json!({"values": {}})).unwrap_err();
            assert!(matches!(err, NexusError::Validation(_)));
        }

        let err = create
            .execute(json!({"database_id": "db"}), &ctx)
            .await
            .unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
        let err = update
            .execute(json!({"database_id": "db", "record_id": "r"}), &ctx)
            .await
            .unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
    }

    #[test]
    fn ambas_tools_son_de_escritura() {
        for t in tools() {
            assert_eq!(t.risk(), Risk::Write);
        }
    }
}
