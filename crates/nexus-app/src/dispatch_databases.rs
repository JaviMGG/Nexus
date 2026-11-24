//! Comandos del módulo NEXUS Database: bases, campos, registros, vistas,
//! import/export, plantillas, referencias cruzadas e IA opcional (propuestas).

use nexus_core::{
    CreateDatabase, CreateDbField, CreateDbRecord, CreateDbView, DbFieldKind, ImportRequest,
    NexusError, RecordQuery, Result, UpdateDatabase, UpdateDbField, UpdateDbRecord, UpdateDbView,
};
use nexus_services::databases::DatabaseService;
use serde_json::{json, Value};

use crate::context::AppContext;
use crate::dispatch::{arg, arg_or_default, ok, opt_arg};

const STRUCTURE_SYSTEM: &str = r#"Eres el diseñador de bases de datos de NEXUS (app personal de productividad).
A partir de la descripción del usuario, propón una base de datos personalizada.
Responde EXCLUSIVAMENTE con JSON válido, sin markdown, con esta forma:
{"name": string, "description": string, "icon": string (nombre de icono lucide en kebab-case), "color": "gray"|"blue"|"green"|"yellow"|"orange"|"red"|"purple"|"pink"|"cyan"|"teal",
 "fields": [{"name": string, "kind": "text"|"long_text"|"integer"|"decimal"|"date"|"datetime"|"checkbox"|"select"|"multiselect"|"url"|"email"|"phone"|"rating", "options": string[] (solo select/multiselect), "reason": string}]}
Reglas: nombres de campo en español, el primer campo debe ser text (título del registro); entre 4 y 12 campos;
no inventes tipos distintos de los listados; no incluyas campos relation ni formula."#;

const SUGGEST_SYSTEM: &str = r#"Eres el diseñador de bases de datos de NEXUS. Se te da la estructura actual de una base.
Propón entre 2 y 6 campos NUEVOS que la complementen (no repitas los existentes).
Responde EXCLUSIVAMENTE con JSON válido, sin markdown:
{"fields": [{"name": string, "kind": "text"|"long_text"|"integer"|"decimal"|"date"|"datetime"|"checkbox"|"select"|"multiselect"|"url"|"email"|"phone"|"rating", "options": string[] (solo select/multiselect), "reason": string}]}
Nombres en español; no incluyas relation ni formula."#;

const DUPLICATES_SYSTEM: &str = r#"Eres un detector de duplicados de NEXUS. Se te dan registros (id y título) de una base de datos.
Identifica pares que probablemente sean el mismo elemento (mismo título con variaciones, erratas, ediciones…).
Responde EXCLUSIVAMENTE con JSON válido, sin markdown:
{"pairs": [{"record_a": string (id), "record_b": string (id), "score": number (0-1), "reason": string (breve, en español)}]}
Usa SOLO ids de la lista; máximo 20 pares; si no hay duplicados claros devuelve {"pairs": []}."#;

pub async fn dispatch(ctx: &AppContext, command: &str, payload: Value) -> Result<Value> {
    let s = &ctx.services.databases;
    match command {
        // ------------------------------------------------------------- bases
        "list" => ok(s.list().await?),
        "get" => ok(s.get(&arg::<String>(&payload, "id")?).await?),
        "create" => {
            let d = s
                .create(arg::<CreateDatabase>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("database");
            ok(d)
        }
        "update" => {
            let id: String = arg(&payload, "id")?;
            let d = s
                .update(&id, arg::<UpdateDatabase>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("database");
            ok(d)
        }
        "duplicate" => {
            let detail = s
                .duplicate(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("database");
            ok(detail)
        }
        "delete" => {
            s.delete(&arg::<String>(&payload, "id")?, "user").await?;
            ctx.data_changed("database");
            ok(json!({"deleted": true}))
        }

        // ------------------------------------------------------------ campos
        "fields.list" => {
            let detail = s.get(&arg::<String>(&payload, "database_id")?).await?;
            ok(detail.fields)
        }
        "fields.create" => {
            let database_id: String = arg(&payload, "database_id")?;
            let f = s
                .create_field(
                    &database_id,
                    arg::<CreateDbField>(&payload, "data")?,
                    "user",
                )
                .await?;
            ctx.data_changed("database");
            ok(f)
        }
        "fields.update" => {
            let id: String = arg(&payload, "id")?;
            let f = s
                .update_field(&id, arg::<UpdateDbField>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("database");
            ok(f)
        }
        "fields.delete" => {
            s.delete_field(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("database");
            ok(json!({"deleted": true}))
        }
        "fields.reorder" => {
            let database_id: String = arg(&payload, "database_id")?;
            let ids: Vec<String> = arg(&payload, "field_ids")?;
            let fields = s.reorder_fields(&database_id, ids).await?;
            ctx.data_changed("database");
            ok(fields)
        }

        // --------------------------------------------------------- registros
        "records.query" => {
            let database_id: String = arg(&payload, "database_id")?;
            ok(s.query_records(
                &database_id,
                arg_or_default::<RecordQuery>(&payload, "query")?,
            )
            .await?)
        }
        "records.get" => {
            let database_id: String = arg(&payload, "database_id")?;
            let id: String = arg(&payload, "id")?;
            ok(s.get_record(&database_id, &id).await?)
        }
        "records.create" => {
            let database_id: String = arg(&payload, "database_id")?;
            let r = s
                .create_record(
                    &database_id,
                    arg_or_default::<CreateDbRecord>(&payload, "data")?,
                    "user",
                )
                .await?;
            ctx.data_changed("database");
            ok(r)
        }
        "records.update" => {
            let database_id: String = arg(&payload, "database_id")?;
            let id: String = arg(&payload, "id")?;
            let r = s
                .update_record(
                    &database_id,
                    &id,
                    arg::<UpdateDbRecord>(&payload, "data")?,
                    "user",
                )
                .await?;
            ctx.data_changed("database");
            ok(r)
        }
        "records.delete" => {
            let database_id: String = arg(&payload, "database_id")?;
            let ids: Vec<String> = arg(&payload, "ids")?;
            let deleted = s.delete_records(&database_id, ids, "user").await?;
            ctx.data_changed("database");
            ok(json!({"deleted": deleted}))
        }
        "record_preview" => {
            let database_id: String = arg(&payload, "database_id")?;
            let record_id: String = arg(&payload, "record_id")?;
            ok(s.record_preview(&database_id, &record_id).await?)
        }

        // ------------------------------------------------------------- vistas
        "views.create" => {
            let database_id: String = arg(&payload, "database_id")?;
            let v = s
                .create_view(&database_id, arg::<CreateDbView>(&payload, "data")?)
                .await?;
            ctx.data_changed("database");
            ok(v)
        }
        "views.update" => {
            let id: String = arg(&payload, "id")?;
            let v = s
                .update_view(&id, arg::<UpdateDbView>(&payload, "data")?)
                .await?;
            ctx.data_changed("database");
            ok(v)
        }
        "views.delete" => {
            s.delete_view(&arg::<String>(&payload, "id")?).await?;
            ctx.data_changed("database");
            ok(json!({"deleted": true}))
        }

        // ------------------------------------------------------ import/export
        "import_preview" => {
            let format: String = arg(&payload, "format")?;
            let content: String = arg(&payload, "content")?;
            ok(DatabaseService::parse_import(&format, &content)?)
        }
        "import" => {
            let result = s
                .import(arg::<ImportRequest>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("database");
            ok(result)
        }
        "export" => {
            let database_id: String = arg(&payload, "database_id")?;
            let format: String = arg(&payload, "format")?;
            let view_id: Option<String> = opt_arg(&payload, "view_id")?;
            ok(s.export(&database_id, &format, view_id.as_deref()).await?)
        }

        // --------------------------------------------------------- plantillas
        "templates" => ok(DatabaseService::templates()),
        "install_template" => {
            let template_id: String = arg(&payload, "template_id")?;
            let detail = s.install_template(&template_id, "user").await?;
            ctx.data_changed("database");
            ok(detail)
        }

        // ------------------------------------------------------- IA opcional
        "ai_structure" => {
            let description: String = arg(&payload, "description")?;
            if description.trim().is_empty() {
                return Err(NexusError::validation("describe qué quieres organizar"));
            }
            let v = crate::ai_json::ask_json(ctx, STRUCTURE_SYSTEM, &description, 2000).await?;
            ok(DatabaseService::parse_ai_structure(&v)?)
        }
        "ai_suggest_fields" => {
            let database_id: String = arg(&payload, "database_id")?;
            let detail = s.get(&database_id).await?;
            let existing: Vec<Value> = detail
                .fields
                .iter()
                .map(|f| {
                    json!({
                        "name": f.name,
                        "kind": f.kind.as_str(),
                        "options": f.config.options.iter().map(|o| &o.label).collect::<Vec<_>>(),
                    })
                })
                .collect();
            let user = format!(
                "Base: {}\nDescripción: {}\nCampos actuales:\n{}",
                detail.summary.database.name,
                detail.summary.database.description,
                serde_json::to_string(&existing).unwrap_or_default()
            );
            let v = crate::ai_json::ask_json(ctx, SUGGEST_SYSTEM, &user, 1500).await?;
            let proposal = DatabaseService::parse_ai_structure(
                &json!({"name": detail.summary.database.name, "fields": v.get("fields").cloned().unwrap_or(json!([]))}),
            )?;
            ok(proposal.fields)
        }
        "ai_duplicates" => {
            let database_id: String = arg(&payload, "database_id")?;
            // La heurística local siempre funciona; la IA solo añade pares.
            let ai_pairs = match build_duplicates_prompt(s, &database_id).await? {
                Some(user) => {
                    match crate::ai_json::ask_json(ctx, DUPLICATES_SYSTEM, &user, 1500).await {
                        Ok(v) => v.get("pairs").cloned(),
                        Err(NexusError::AiUnavailable(_)) => None,
                        Err(e) => return Err(e),
                    }
                }
                None => None,
            };
            ok(s.find_duplicates(&database_id, ai_pairs).await?)
        }

        other => Err(NexusError::NotFound(format!(
            "comando desconocido: databases.{other}"
        ))),
    }
}

async fn build_duplicates_prompt(s: &DatabaseService, database_id: &str) -> Result<Option<String>> {
    let detail = s.get(database_id).await?;
    let title_field = detail
        .fields
        .iter()
        .find(|f| f.kind == DbFieldKind::Text)
        .map(|f| f.id.clone());
    let page = s
        .query_records(
            database_id,
            RecordQuery {
                limit: Some(200),
                ..Default::default()
            },
        )
        .await?;
    if page.records.len() < 2 {
        return Ok(None);
    }
    let items: Vec<Value> = page
        .records
        .iter()
        .map(|r| {
            let title = title_field
                .as_ref()
                .and_then(|fid| r.values.get(fid))
                .and_then(Value::as_str)
                .unwrap_or("(sin título)");
            json!({"id": r.id, "title": title})
        })
        .collect();
    Ok(Some(format!(
        "Registros de la base «{}»:\n{}",
        detail.summary.database.name,
        serde_json::to_string(&items).unwrap_or_default()
    )))
}
