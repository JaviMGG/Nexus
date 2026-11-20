//! Herramientas integradas: tareas, proyectos, notas, inbox, dashboard y
//! búsqueda web (sin proveedor configurado devuelve error claro).

use std::sync::Arc;

use nexus_core::{
    CreateEvent, CreateNote, CreateProject, CreateTask, EventColor, EventFilter, InboxStatus,
    NexusError, NoteFilter, NoteKind, Priority, ProjectStatus, Result, Risk, TaskFilter,
    TaskStatus, UpdateEvent, UpdateNote, UpdateProject, UpdateTask,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::{Tool, ToolContext, WebSearchProvider};

fn req_str(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| {
            NexusError::validation(format!("falta el campo obligatorio '{key}' (string)"))
        })
}

fn dto<T: DeserializeOwned>(args: Value) -> Result<T> {
    serde_json::from_value(args)
        .map_err(|e| NexusError::validation(format!("argumentos inválidos: {e}")))
}

fn val<T: Serialize>(v: T) -> Result<Value> {
    serde_json::to_value(v).map_err(|e| NexusError::Internal(e.to_string()))
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn enum_prop(values: &[&str]) -> Value {
    json!({ "type": "string", "enum": values })
}

macro_rules! define_tool {
    ($ty:ident, $name:literal, $risk:expr, $desc:literal, $params:expr, |$args:ident, $ctx:ident| $body:block) => {
        struct $ty;

        #[async_trait::async_trait]
        impl Tool for $ty {
            fn spec(&self) -> nexus_ai::ToolSpec {
                nexus_ai::ToolSpec {
                    name: $name.to_string(),
                    description: $desc.to_string(),
                    parameters: $params,
                }
            }

            fn risk(&self) -> Risk {
                $risk
            }

            async fn execute(&self, $args: Value, $ctx: &ToolContext) -> Result<Value> {
                $body
            }
        }
    };
}

define_tool!(TasksList, "tasks.list", Risk::Read,
    "Lista tareas reales del usuario con filtros opcionales (estado, proyecto, prioridad, búsqueda, etiqueta, fecha límite).",
    schema(json!({
        "status": enum_prop(TaskStatus::all()),
        "project_id": { "type": "string" },
        "priority": enum_prop(Priority::all()),
        "search": { "type": "string" },
        "tag": { "type": "string" },
        "due_before": { "type": "string", "description": "Fecha YYYY-MM-DD" },
        "limit": { "type": "integer" }
    }), &[]),
    |args, ctx| {
        let f: TaskFilter = dto(args)?;
        val(ctx.services.tasks.list(f).await?)
    }
);

define_tool!(
    TasksGet,
    "tasks.get",
    Risk::Read,
    "Obtiene el detalle de una tarea por id, incluidas sus subtareas.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        val(ctx.services.tasks.get(&id).await?)
    }
);

define_tool!(
    TasksCreate,
    "tasks.create",
    Risk::Write,
    "Crea una tarea real. Las fechas deben ir en formato YYYY-MM-DD.",
    schema(
        json!({
            "title": { "type": "string" },
            "description": { "type": "string" },
            "status": enum_prop(TaskStatus::all()),
            "priority": enum_prop(Priority::all()),
            "due_date": { "type": "string", "description": "YYYY-MM-DD" },
            "project_id": { "type": "string" },
            "parent_id": { "type": "string" },
            "category": { "type": "string" },
            "tags": { "type": "array", "items": { "type": "string" } }
        }),
        &["title"]
    ),
    |args, ctx| {
        let data: CreateTask = dto(args)?;
        val(ctx.services.tasks.create(data, &ctx.actor()).await?)
    }
);

define_tool!(TasksUpdate, "tasks.update", Risk::Write,
    "Actualiza una tarea existente. En due_date/project_id/parent_id/category, null borra el valor; omitir lo conserva.",
    schema(json!({
        "id": { "type": "string" },
        "title": { "type": "string" },
        "description": { "type": "string" },
        "status": enum_prop(TaskStatus::all()),
        "priority": enum_prop(Priority::all()),
        "due_date": { "type": ["string", "null"] },
        "project_id": { "type": ["string", "null"] },
        "parent_id": { "type": ["string", "null"] },
        "category": { "type": ["string", "null"] },
        "tags": { "type": "array", "items": { "type": "string" } },
        "position": { "type": "integer" }
    }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let patch: UpdateTask = dto(args)?;
        val(ctx.services.tasks.update(&id, patch, &ctx.actor()).await?)
    }
);

define_tool!(TasksDelete, "tasks.delete", Risk::Destructive,
    "Elimina (papelera, restaurable) una tarea por id. Acción destructiva: puede requerir confirmación del usuario.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        ctx.services.tasks.delete(&id, &ctx.actor()).await?;
        val(json!({ "deleted": true, "id": id }))
    }
);

define_tool!(
    ProjectsList,
    "projects.list",
    Risk::Read,
    "Lista los proyectos reales del usuario.",
    schema(json!({ "include_archived": { "type": "boolean" } }), &[]),
    |args, ctx| {
        let include = args
            .get("include_archived")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        val(ctx.services.projects.list(include).await?)
    }
);

define_tool!(
    ProjectsGet,
    "projects.get",
    Risk::Read,
    "Obtiene el detalle de un proyecto: tareas, notas, progreso y actividad reciente.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        val(ctx.services.projects.get(&id).await?)
    }
);

define_tool!(
    ProjectsCreate,
    "projects.create",
    Risk::Write,
    "Crea un proyecto real con nombre, descripción y objetivo.",
    schema(
        json!({
            "name": { "type": "string" },
            "description": { "type": "string" },
            "status": enum_prop(ProjectStatus::all()),
            "priority": enum_prop(Priority::all()),
            "goal": { "type": "string" }
        }),
        &["name"]
    ),
    |args, ctx| {
        let data: CreateProject = dto(args)?;
        val(ctx.services.projects.create(data, &ctx.actor()).await?)
    }
);

define_tool!(
    ProjectsUpdate,
    "projects.update",
    Risk::Write,
    "Actualiza un proyecto existente (nombre, descripción, estado, prioridad u objetivo).",
    schema(
        json!({
            "id": { "type": "string" },
            "name": { "type": "string" },
            "description": { "type": "string" },
            "status": enum_prop(ProjectStatus::all()),
            "priority": enum_prop(Priority::all()),
            "goal": { "type": "string" }
        }),
        &["id"]
    ),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let patch: UpdateProject = dto(args)?;
        val(ctx
            .services
            .projects
            .update(&id, patch, &ctx.actor())
            .await?)
    }
);

define_tool!(
    NotesList,
    "notes.list",
    Risk::Read,
    "Lista notas reales con filtros opcionales.",
    schema(
        json!({
            "search": { "type": "string" },
            "kind": enum_prop(NoteKind::all()),
            "project_id": { "type": "string" },
            "tag": { "type": "string" },
            "limit": { "type": "integer" }
        }),
        &[]
    ),
    |args, ctx| {
        let f: NoteFilter = dto(args)?;
        val(ctx.services.notes.list(f).await?)
    }
);

define_tool!(
    NotesSearch,
    "notes.search",
    Risk::Read,
    "Búsqueda de texto completo (FTS) en la base de conocimiento del usuario.",
    schema(
        json!({
            "query": { "type": "string" },
            "limit": { "type": "integer" }
        }),
        &["query"]
    ),
    |args, ctx| {
        let query = req_str(&args, "query")?;
        let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(10);
        val(ctx.services.notes.search(&query, limit).await?)
    }
);

define_tool!(
    NotesGet,
    "notes.get",
    Risk::Read,
    "Obtiene una nota completa por id.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        val(ctx.services.notes.get(&id).await?)
    }
);

define_tool!(
    NotesCreate,
    "notes.create",
    Risk::Write,
    "Crea una nota real en la base de conocimiento (nota, idea, documento o enlace).",
    schema(
        json!({
            "title": { "type": "string" },
            "content": { "type": "string" },
            "kind": enum_prop(NoteKind::all()),
            "url": { "type": "string" },
            "project_id": { "type": "string" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "pinned": { "type": "boolean" }
        }),
        &["title"]
    ),
    |args, ctx| {
        let data: CreateNote = dto(args)?;
        val(ctx.services.notes.create(data, &ctx.actor()).await?)
    }
);

define_tool!(
    NotesUpdate,
    "notes.update",
    Risk::Write,
    "Actualiza una nota existente. En url/project_id, null borra el valor; omitir lo conserva.",
    schema(
        json!({
            "id": { "type": "string" },
            "title": { "type": "string" },
            "content": { "type": "string" },
            "kind": enum_prop(NoteKind::all()),
            "url": { "type": ["string", "null"] },
            "project_id": { "type": ["string", "null"] },
            "tags": { "type": "array", "items": { "type": "string" } },
            "pinned": { "type": "boolean" }
        }),
        &["id"]
    ),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let patch: UpdateNote = dto(args)?;
        val(ctx.services.notes.update(&id, patch, &ctx.actor()).await?)
    }
);

define_tool!(NotesDelete, "notes.delete", Risk::Destructive,
    "Elimina (papelera, restaurable) una nota por id. Acción destructiva: puede requerir confirmación del usuario.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        ctx.services.notes.delete(&id, &ctx.actor()).await?;
        val(json!({ "deleted": true, "id": id }))
    }
);

define_tool!(EventsList, "events.list", Risk::Read,
    "Lista los eventos del calendario del usuario con filtros opcionales de rango de fechas (from/to en YYYY-MM-DD) y proyecto.",
    schema(json!({
        "from": { "type": "string", "description": "Fecha inicial YYYY-MM-DD (inclusive)" },
        "to": { "type": "string", "description": "Fecha final YYYY-MM-DD (inclusive)" },
        "project_id": { "type": "string" },
        "include_deleted": { "type": "boolean" }
    }), &[]),
    |args, ctx| {
        let f: EventFilter = dto(args)?;
        val(ctx.services.events.list(f).await?)
    }
);

define_tool!(EventsCreate, "events.create", Risk::Write,
    "Crea un evento real en el calendario. La fecha es YYYY-MM-DD y las horas HH:MM; si all_day es true no llevan horas.",
    schema(json!({
        "title": { "type": "string" },
        "date": { "type": "string", "description": "YYYY-MM-DD" },
        "description": { "type": "string" },
        "start_time": { "type": "string", "description": "HH:MM" },
        "end_time": { "type": "string", "description": "HH:MM" },
        "all_day": { "type": "boolean" },
        "color": enum_prop(EventColor::all()),
        "location": { "type": "string" },
        "project_id": { "type": "string" },
        "task_id": { "type": "string" }
    }), &["title", "date"]),
    |args, ctx| {
        let data: CreateEvent = dto(args)?;
        val(ctx.services.events.create(data, &ctx.actor()).await?)
    }
);

define_tool!(EventsUpdate, "events.update", Risk::Write,
    "Actualiza un evento del calendario. En start_time/end_time/location/project_id/task_id, null borra el valor; omitir lo conserva.",
    schema(json!({
        "id": { "type": "string" },
        "title": { "type": "string" },
        "description": { "type": "string" },
        "date": { "type": "string", "description": "YYYY-MM-DD" },
        "start_time": { "type": ["string", "null"], "description": "HH:MM" },
        "end_time": { "type": ["string", "null"], "description": "HH:MM" },
        "all_day": { "type": "boolean" },
        "color": enum_prop(EventColor::all()),
        "location": { "type": ["string", "null"] },
        "project_id": { "type": ["string", "null"] },
        "task_id": { "type": ["string", "null"] }
    }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let patch: UpdateEvent = dto(args)?;
        val(ctx.services.events.update(&id, patch, &ctx.actor()).await?)
    }
);

define_tool!(EventsDelete, "events.delete", Risk::Destructive,
    "Elimina (papelera, restaurable) un evento del calendario por id. Acción destructiva: puede requerir confirmación del usuario.",
    schema(json!({ "id": { "type": "string" } }), &["id"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        ctx.services.events.delete(&id, &ctx.actor()).await?;
        val(json!({ "deleted": true, "id": id }))
    }
);

define_tool!(
    InboxList,
    "inbox.list",
    Risk::Read,
    "Lista los elementos del inbox (capturas rápidas sin procesar).",
    schema(json!({ "status": enum_prop(InboxStatus::all()) }), &[]),
    |args, ctx| {
        let status = match args.get("status").and_then(Value::as_str) {
            Some(s) => Some(InboxStatus::parse(s).ok_or_else(|| {
                NexusError::validation(format!("estado de inbox desconocido: {s}"))
            })?),
            None => None,
        };
        val(ctx.services.inbox.list(status).await?)
    }
);

define_tool!(InboxProcess, "inbox.process", Risk::Write,
    "Convierte un elemento del inbox en una tarea o nota real. 'data' puede traer campos estructurados (title, description, etc.).",
    schema(json!({
        "id": { "type": "string" },
        "kind": { "type": "string", "enum": ["task", "note"] },
        "data": { "type": "object" }
    }), &["id", "kind"]),
    |args, ctx| {
        let id = req_str(&args, "id")?;
        let kind = req_str(&args, "kind")?;
        let data = args.get("data").filter(|v| v.is_object()).cloned();
        let created_id = ctx.services.inbox.process(&id, &kind, data, &ctx.services, &ctx.actor()).await?;
        val(json!({ "created_id": created_id, "kind": kind }))
    }
);

define_tool!(DashboardSummary, "dashboard.summary", Risk::Read,
    "Resumen real del día: tareas de hoy, vencidas, prioritarias, proyectos activos y estadísticas.",
    schema(json!({}), &[]),
    |args, ctx| {
        let _ = args;
        let app = ctx.services.settings.get_app().await?;
        val(ctx.services.dashboard.summary(&app.user_name).await?)
    }
);

define_tool!(WebSearch, "web.search", Risk::Read,
    "Búsqueda web real a través del proveedor configurado en Ajustes. Sin proveedor configurado devuelve un error claro; nunca inventa resultados.",
    schema(json!({ "query": { "type": "string" } }), &["query"]),
    |args, ctx| {
        let query = req_str(&args, "query")?;
        let provider = load_web_search_provider(ctx).await?;
        let results = provider.search(&query).await?;
        val(json!({ "query": query, "results": results }))
    }
);

/// Lee la configuración `web_search` de Ajustes. Sin configuración (o con un
/// proveedor aún no soportado) devuelve un error explícito: jamás se fabrican
/// resultados de búsqueda.
async fn load_web_search_provider(ctx: &ToolContext) -> Result<Arc<dyn WebSearchProvider>> {
    let raw = nexus_db::repos::settings::get(&ctx.services.db, "web_search").await?;
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return Err(NexusError::AiUnavailable(
            "No hay proveedor de búsqueda web configurado. Configura uno en Ajustes para investigar. No se inventarán resultados.".into(),
        ));
    };
    let config: Value = serde_json::from_str(&raw)
        .map_err(|e| NexusError::validation(format!("configuración web_search inválida: {e}")))?;
    let kind = config.get("provider").and_then(Value::as_str).unwrap_or("");
    match web_search_provider_from_config(kind, &config) {
        Some(p) => Ok(p),
        None => Err(NexusError::AiUnavailable(format!(
            "El proveedor de búsqueda web '{kind}' no está soportado todavía. Configura uno válido en Ajustes. No se inventarán resultados."
        ))),
    }
}

/// Punto de extensión: aquí se conectarán proveedores reales de búsqueda.
fn web_search_provider_from_config(
    _kind: &str,
    _config: &Value,
) -> Option<Arc<dyn WebSearchProvider>> {
    None
}

pub(crate) fn defaults() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(TasksList),
        Arc::new(TasksGet),
        Arc::new(TasksCreate),
        Arc::new(TasksUpdate),
        Arc::new(TasksDelete),
        Arc::new(ProjectsList),
        Arc::new(ProjectsGet),
        Arc::new(ProjectsCreate),
        Arc::new(ProjectsUpdate),
        Arc::new(NotesList),
        Arc::new(NotesSearch),
        Arc::new(NotesGet),
        Arc::new(NotesCreate),
        Arc::new(NotesUpdate),
        Arc::new(NotesDelete),
        Arc::new(EventsList),
        Arc::new(EventsCreate),
        Arc::new(EventsUpdate),
        Arc::new(EventsDelete),
        Arc::new(InboxList),
        Arc::new(InboxProcess),
        Arc::new(DashboardSummary),
        Arc::new(WebSearch),
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
        }
    }

    #[tokio::test]
    async fn tasks_create_and_list_real() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("tasks.create").unwrap();
        let args = json!({"title": "Comprar pan", "priority": "high", "tags": ["casa"]});
        validate_args(&create.spec(), &args).unwrap();
        let created = create.execute(args, &ctx).await.unwrap();
        assert_eq!(created["title"], "Comprar pan");
        assert_eq!(created["priority"], "high");

        let list = registry.get("tasks.list").unwrap();
        let listed = list.execute(json!({"search": "pan"}), &ctx).await.unwrap();
        let arr = listed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], created["id"]);

        let activity = nexus_db::repos::activity::list(&ctx.services.db, 10, None, None)
            .await
            .unwrap();
        assert!(activity
            .iter()
            .any(|a| a.actor == "agent:executor" && a.kind == "created"));
    }

    #[tokio::test]
    async fn tasks_create_requires_title() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("tasks.create").unwrap();
        let err = validate_args(&create.spec(), &json!({})).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
        let err = create.execute(json!({"title": 5}), &ctx).await.unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
    }

    #[tokio::test]
    async fn web_search_without_provider_is_clear_error() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let tool = registry.get("web.search").unwrap();
        let err = tool
            .execute(json!({"query": "rust async"}), &ctx)
            .await
            .unwrap_err();
        match err {
            NexusError::AiUnavailable(msg) => {
                assert!(msg.contains("No hay proveedor de búsqueda web configurado"));
                assert!(msg.contains("No se inventarán resultados"));
            }
            other => panic!("error inesperado: {other:?}"),
        }
    }

    #[tokio::test]
    async fn inbox_process_creates_task() {
        let ctx = ctx().await;
        let item = ctx
            .services
            .inbox
            .create("Llamar al dentista", "user")
            .await
            .unwrap();
        let registry = ToolRegistry::with_defaults();
        let tool = registry.get("inbox.process").unwrap();
        let out = tool
            .execute(json!({"id": item.id, "kind": "task"}), &ctx)
            .await
            .unwrap();
        let task = ctx
            .services
            .tasks
            .get(out["created_id"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(task.task.title, "Llamar al dentista");
    }

    #[tokio::test]
    async fn events_create_and_list_real() {
        let ctx = ctx().await;
        let registry = ToolRegistry::with_defaults();
        let create = registry.get("events.create").unwrap();
        let args = json!({
            "title": "Reunión de equipo",
            "date": "2026-11-20",
            "start_time": "10:00",
            "end_time": "11:00",
            "color": "info"
        });
        validate_args(&create.spec(), &args).unwrap();
        let created = create.execute(args, &ctx).await.unwrap();
        assert_eq!(created["title"], "Reunión de equipo");
        assert_eq!(created["date"], "2026-11-20");
        assert_eq!(created["color"], "info");
        assert_eq!(created["all_day"], false);

        let list = registry.get("events.list").unwrap();
        let listed = list
            .execute(json!({"from": "2026-11-01", "to": "2026-11-30"}), &ctx)
            .await
            .unwrap();
        let arr = listed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], created["id"]);

        let activity = nexus_db::repos::activity::list(&ctx.services.db, 10, None, None)
            .await
            .unwrap();
        assert!(activity.iter().any(|a| a.actor == "agent:executor"
            && a.kind == "created"
            && a.entity_type == "event"));
    }

    #[test]
    fn registry_has_all_tools_with_schemas() {
        let registry = ToolRegistry::with_defaults();
        let names = registry.names();
        assert_eq!(names.len(), 23);
        for name in &names {
            let tool = registry.get(name).unwrap();
            let spec = tool.spec();
            assert_eq!(&spec.name, name);
            assert!(!spec.description.is_empty());
            assert_eq!(spec.parameters["type"], "object");
        }
        assert!(registry.get("no.existe").is_none());
        let specs = registry.specs(&["tasks.list".into(), "no.existe".into()]);
        assert_eq!(specs.len(), 1);
    }
}
