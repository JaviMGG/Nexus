//! ActionRegistry: catálogo cerrado de acciones deterministas. Cada acción
//! usa los servicios reales de NEXUS; no existe ejecución de código ni SQL
//! arbitrario. Las acciones sensibles exigen autorización explícita.

use nexus_core::{
    ActionSpec, CreateNote, CreateNoteParams, CreateProject, CreateProjectParams, CreateTask,
    CreateTaskParams, NexusError, NoteRefParams, ProjectRefParams, Result, RunAutomationParams,
    TaskFilter, TaskRefParams, TaskStatus, UpdateNote, UpdateProject, UpdateTask,
};
use nexus_services::Services;
use serde::Deserialize;
use serde_json::{json, Value};

/// (id, nombre, descripción, riesgo, entidad)
pub const CATALOG: &[(&str, &str, &str, &str, &str)] = &[
    (
        "tasks.create",
        "Crear tarea",
        "Crea una tarea con título, prioridad, fecha y etiquetas",
        "write",
        "task",
    ),
    (
        "tasks.update",
        "Modificar tarea",
        "Actualiza título, descripción o fecha de una tarea",
        "write",
        "task",
    ),
    (
        "tasks.complete",
        "Completar tarea",
        "Marca una tarea como completada",
        "write",
        "task",
    ),
    (
        "tasks.set_priority",
        "Cambiar prioridad",
        "Cambia la prioridad de una tarea",
        "write",
        "task",
    ),
    (
        "tasks.add_tags",
        "Añadir etiquetas",
        "Añade etiquetas a una tarea sin perder las existentes",
        "write",
        "task",
    ),
    (
        "tasks.create_subtask",
        "Crear subtarea",
        "Crea una subtarea dentro de una tarea existente",
        "write",
        "task",
    ),
    (
        "tasks.reorder",
        "Reorganizar tarea",
        "Mueve una tarea de estado/posición",
        "write",
        "task",
    ),
    (
        "projects.create",
        "Crear proyecto",
        "Crea un proyecto con nombre, descripción y objetivo",
        "write",
        "project",
    ),
    (
        "projects.update_description",
        "Modificar descripción",
        "Actualiza la descripción de un proyecto",
        "write",
        "project",
    ),
    (
        "projects.update_status",
        "Actualizar estado",
        "Cambia el estado de un proyecto",
        "write",
        "project",
    ),
    (
        "projects.add_task",
        "Añadir tarea al proyecto",
        "Crea una tarea vinculada a un proyecto",
        "write",
        "project",
    ),
    (
        "notes.create",
        "Crear nota",
        "Crea una nota, idea o documento",
        "write",
        "note",
    ),
    (
        "notes.update",
        "Modificar nota",
        "Actualiza título o contenido de una nota",
        "write",
        "note",
    ),
    (
        "notes.append",
        "Añadir contenido a nota",
        "Añade texto al final de una nota (p. ej. un resumen)",
        "write",
        "note",
    ),
    (
        "notes.classify",
        "Clasificar nota",
        "Cambia el tipo de una nota (nota/idea/documento/enlace)",
        "write",
        "note",
    ),
    (
        "notes.add_tags",
        "Etiquetar nota",
        "Añade etiquetas a una nota",
        "write",
        "note",
    ),
    (
        "notes.link_project",
        "Relacionar nota con proyecto",
        "Vincula una nota a un proyecto",
        "write",
        "note",
    ),
    (
        "knowledge.save_summary",
        "Guardar resumen",
        "Guarda un resumen como documento en la base de conocimiento",
        "write",
        "note",
    ),
    (
        "system.report",
        "Generar informe",
        "Genera un informe local de tareas y proyectos y lo guarda como documento",
        "read",
        "note",
    ),
    (
        "system.run_automation",
        "Ejecutar otra automatización",
        "Lanza otra automatización (solo locales, sin cadenas)",
        "sensitive",
        "automation",
    ),
    (
        "notes.delete",
        "Eliminar nota",
        "Elimina una nota (papelera, reversible)",
        "sensitive",
        "note",
    ),
    (
        "tasks.delete",
        "Eliminar tarea",
        "Elimina una tarea (papelera, reversible)",
        "sensitive",
        "task",
    ),
    (
        "mail.send",
        "Enviar correo",
        "Envía un correo SMTP desde una cuenta configurada",
        "sensitive",
        "mail",
    ),
    (
        "mail.reply",
        "Responder correo",
        "Responde un correo SMTP desde una cuenta configurada",
        "sensitive",
        "mail",
    ),
];

pub fn catalog() -> Vec<ActionSpec> {
    CATALOG
        .iter()
        .map(|(id, name, desc, risk, entity)| ActionSpec {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            risk: risk.to_string(),
            entity: entity.to_string(),
        })
        .collect()
}

pub fn is_known(id: &str) -> bool {
    CATALOG.iter().any(|(a, ..)| *a == id)
}

pub fn risk_of(id: &str) -> &'static str {
    CATALOG
        .iter()
        .find(|(a, ..)| *a == id)
        .map(|(_, _, _, risk, _)| *risk)
        .unwrap_or("sensitive")
}

pub fn entity_of(id: &str) -> &'static str {
    CATALOG
        .iter()
        .find(|(a, ..)| *a == id)
        .map(|(_, _, _, _, e)| *e)
        .unwrap_or("automation")
}

pub fn display_name(id: &str) -> String {
    CATALOG
        .iter()
        .find(|(a, ..)| *a == id)
        .map(|(_, n, ..)| n.to_string())
        .unwrap_or_else(|| id.to_string())
}

/// Callback que lanza otra automatización (lo aporta el manager).
pub type AutomationRunner<'a> = dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>>
    + Sync
    + 'a;

#[derive(Deserialize)]
struct MailSendParams {
    account: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    body: String,
    #[serde(default)]
    in_reply_to: Option<String>,
}

/// Contexto de ejecución de una acción.
pub struct ActionContext<'a> {
    pub services: &'a Services,
    pub actor: String,
    pub event_data: Value,
    pub vars: Value,
    /// Ejecución de prueba: no escribe, solo describe lo que haría.
    pub dry_run: bool,
    /// Profundidad de cadena entre automatizaciones (máx. 1).
    pub chain_depth: i64,
    /// Lanza otra automatización (lo aporta el manager).
    pub run_automation: Option<&'a AutomationRunner<'a>>,
}

impl ActionContext<'_> {
    /// Resuelve un id: clave de vars/evento ("task", "note", "project", "id")
    /// o id literal.
    fn resolve_id(&self, reference: &str) -> Result<String> {
        if reference.trim().is_empty() {
            return Err(NexusError::validation("falta el id de destino"));
        }
        for source in [&self.vars, &self.event_data] {
            if let Some(v) = source.get(reference).and_then(Value::as_str) {
                return Ok(v.to_string());
            }
        }
        if reference == "id" {
            if let Some(v) = self.event_data.get("id").and_then(Value::as_str) {
                return Ok(v.to_string());
            }
        }
        Ok(reference.to_string())
    }

    fn interpolated(&self, s: &str) -> String {
        interpolate(s, &self.event_data, &self.vars)
    }
}

/// Sustituye {{clave}} y {{clave.sub}} con valores del evento y de las vars.
pub fn interpolate(template: &str, event: &Value, vars: &Value) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let key = after[..end].trim();
                let val = lookup(event, key)
                    .or_else(|| lookup(vars, key))
                    .map(value_to_string)
                    .unwrap_or_default();
                out.push_str(&val);
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Vista previa legible de una acción (para autorizaciones y modo prueba).
pub fn preview(action: &str, params: &Value, ctx: &ActionContext<'_>) -> String {
    let p = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("");
    match action {
        "tasks.create" | "tasks.create_subtask" | "projects.add_task" => {
            format!("Crear tarea «{}»", ctx.interpolated(p("title")))
        }
        "projects.create" => format!("Crear proyecto «{}»", ctx.interpolated(p("name"))),
        "notes.create" | "knowledge.save_summary" | "system.report" => {
            format!("Crear nota «{}»", ctx.interpolated(p("title")))
        }
        "tasks.complete" => format!("Completar la tarea {}", p("task_id")),
        "tasks.delete" => format!("Eliminar la tarea {} (papelera)", p("task_id")),
        "notes.delete" => format!("Eliminar la nota {} (papelera)", p("note_id")),
        "system.run_automation" => format!("Ejecutar la automatización {}", p("automation_id")),
        "mail.send" | "mail.reply" => format!(
            "{} correo a {} con asunto «{}»",
            if action == "mail.reply" {
                "Responder"
            } else {
                "Enviar"
            },
            ctx.interpolated(p("to")),
            ctx.interpolated(p("subject"))
        ),
        _ => format!("{} con {}", display_name(action), params),
    }
}

/// Ejecuta la acción y devuelve (detalle del resultado, entidades afectadas).
pub async fn execute(
    action: &str,
    params: &Value,
    ctx: &ActionContext<'_>,
) -> Result<(Value, Vec<&'static str>)> {
    if !is_known(action) {
        return Err(NexusError::validation(format!(
            "acción desconocida: {action}"
        )));
    }
    if ctx.dry_run {
        return Ok((
            json!({"dry_run": true, "preview": preview(action, params, ctx)}),
            vec![],
        ));
    }
    let s = ctx.services;
    let actor = ctx.actor.as_str();
    let ip = |v: &str| ctx.interpolated(v);

    match action {
        "tasks.create" | "tasks.create_subtask" | "projects.add_task" => {
            let p: CreateTaskParams = parse(params)?;
            let mut data = CreateTask {
                title: ip(&p.title),
                description: ip(&p.description),
                priority: p.priority,
                due_date: p.due_date.map(|d| ip(&d)),
                project_id: p.project_id.map(|id| ctx.resolve_id(&id)).transpose()?,
                parent_id: p.parent_id.map(|id| ctx.resolve_id(&id)).transpose()?,
                tags: p.tags,
                ..Default::default()
            };
            if action == "tasks.create_subtask" && data.parent_id.is_none() {
                data.parent_id = Some(
                    ctx.resolve_id(
                        params
                            .get("task_id")
                            .and_then(Value::as_str)
                            .unwrap_or("task"),
                    )?,
                );
            }
            if action == "projects.add_task" && data.project_id.is_none() {
                data.project_id = Some(
                    ctx.resolve_id(
                        params
                            .get("project_id")
                            .and_then(Value::as_str)
                            .unwrap_or("project"),
                    )?,
                );
            }
            let task = s.tasks.create(data, actor).await?;
            let mut detail = json!({"task_id": task.id, "title": task.title});
            if let Some(key) = &p.save_as {
                detail[key] = json!(task.id);
            }
            Ok((detail, vec!["task"]))
        }
        "tasks.update" | "tasks.complete" | "tasks.set_priority" | "tasks.add_tags"
        | "tasks.reorder" => {
            let p: TaskRefParams = parse(params)?;
            let id = ctx.resolve_id(&p.task_id)?;
            let mut patch = UpdateTask {
                title: p.title.map(|t| ip(&t)),
                description: p.description.map(|d| ip(&d)),
                status: p.status,
                priority: p.priority,
                due_date: p.due_date.map(|d| Some(ip(&d))),
                ..Default::default()
            };
            if action == "tasks.complete" {
                patch.status = Some(TaskStatus::Done);
            }
            if action == "tasks.reorder" {
                patch.position = params.get("position").and_then(Value::as_i64);
            }
            if !p.add_tags.is_empty() || action == "tasks.add_tags" {
                let current = s.tasks.get(&id).await?;
                let mut tags = current.task.tags;
                for t in &p.add_tags {
                    if !tags.contains(t) {
                        tags.push(t.clone());
                    }
                }
                patch.tags = Some(tags);
            }
            let task = s.tasks.update(&id, patch, actor).await?;
            Ok((
                json!({"task_id": task.id, "status": task.status}),
                vec!["task"],
            ))
        }
        "tasks.delete" => {
            let p: TaskRefParams = parse(params)?;
            let id = ctx.resolve_id(&p.task_id)?;
            s.tasks.delete(&id, actor).await?;
            Ok((json!({"task_id": id, "deleted": true}), vec!["task"]))
        }
        "projects.create" => {
            let p: CreateProjectParams = parse(params)?;
            let project = s
                .projects
                .create(
                    CreateProject {
                        name: ip(&p.name),
                        description: ip(&p.description),
                        goal: ip(&p.goal),
                        priority: p.priority,
                        ..Default::default()
                    },
                    actor,
                )
                .await?;
            let mut detail = json!({"project_id": project.id, "name": project.name});
            if let Some(key) = &p.save_as {
                detail[key] = json!(project.id);
            }
            Ok((detail, vec!["project"]))
        }
        "projects.update_description" | "projects.update_status" => {
            let p: ProjectRefParams = parse(params)?;
            let id = ctx.resolve_id(&p.project_id)?;
            let patch = UpdateProject {
                description: p.description.map(|d| ip(&d)),
                status: p.status,
                goal: p.goal.map(|g| ip(&g)),
                ..Default::default()
            };
            let project = s.projects.update(&id, patch, actor).await?;
            Ok((
                json!({"project_id": project.id, "status": project.status}),
                vec!["project"],
            ))
        }
        "notes.create" | "knowledge.save_summary" | "system.report" => {
            let p: CreateNoteParams = parse(params)?;
            let mut content = ip(&p.content);
            let mut title = ip(&p.title);
            let mut kind = p.kind;
            if action == "system.report" {
                let report = build_report(s).await?;
                if title.trim().is_empty() {
                    title = format!("Informe {}", nexus_core::now().format("%Y-%m-%d %H:%M"));
                }
                content = report;
                kind = Some(nexus_core::NoteKind::Document);
            }
            if action == "knowledge.save_summary" {
                kind = Some(nexus_core::NoteKind::Document);
            }
            let note = s
                .notes
                .create(
                    CreateNote {
                        title,
                        content,
                        kind,
                        project_id: p.project_id.map(|id| ctx.resolve_id(&id)).transpose()?,
                        tags: p.tags,
                        ..Default::default()
                    },
                    actor,
                )
                .await?;
            let mut detail = json!({"note_id": note.id, "title": note.title});
            if let Some(key) = &p.save_as {
                detail[key] = json!(note.id);
            }
            Ok((detail, vec!["note"]))
        }
        "notes.update" | "notes.append" | "notes.classify" | "notes.add_tags"
        | "notes.link_project" => {
            let p: NoteRefParams = parse(params)?;
            let id = ctx.resolve_id(&p.note_id)?;
            let mut patch = UpdateNote {
                title: p.title.map(|t| ip(&t)),
                content: p.content.map(|c| ip(&c)),
                kind: p.kind,
                project_id: p.project_id.map(Some),
                ..Default::default()
            };
            if action == "notes.append" {
                let current = s.notes.get(&id).await?;
                let extra = ip(p.append_content.as_deref().unwrap_or(""));
                patch.content = Some(format!("{}\n\n{}", current.content, extra));
            }
            if action == "notes.link_project" {
                let pid = params
                    .get("project_id")
                    .and_then(Value::as_str)
                    .unwrap_or("project");
                patch.project_id = Some(Some(ctx.resolve_id(pid)?));
            }
            if !p.add_tags.is_empty() || action == "notes.add_tags" {
                let current = s.notes.get(&id).await?;
                let mut tags = current.tags;
                for t in &p.add_tags {
                    if !tags.contains(t) {
                        tags.push(t.clone());
                    }
                }
                patch.tags = Some(tags);
            }
            let note = s.notes.update(&id, patch, actor).await?;
            Ok((json!({"note_id": note.id}), vec!["note"]))
        }
        "notes.delete" => {
            let p: NoteRefParams = parse(params)?;
            let id = ctx.resolve_id(&p.note_id)?;
            s.notes.delete(&id, actor).await?;
            Ok((json!({"note_id": id, "deleted": true}), vec!["note"]))
        }
        "system.run_automation" => {
            if ctx.chain_depth >= 1 {
                return Err(NexusError::validation(
                    "no se permiten cadenas de automatizaciones de más de un nivel",
                ));
            }
            let p: RunAutomationParams = parse(params)?;
            let target = nexus_db::repos::automations::get(&s.db, &p.automation_id).await?;
            if target.mode != nexus_core::AutomationMode::Local {
                return Err(NexusError::validation(
                    "solo se pueden encadenar automatizaciones en modo local",
                ));
            }
            let runner = ctx.run_automation.ok_or_else(|| {
                NexusError::Internal("el motor no permite encadenar en este contexto".into())
            })?;
            let result = runner(p.automation_id.clone()).await?;
            Ok((result, vec!["automation"]))
        }
        "mail.send" | "mail.reply" => {
            let p: MailSendParams = parse(params)?;
            let account_ref = ip(&p.account);
            let account = ctx.resolve_id(&account_ref)?;
            let to_ref =
                p.to.as_deref()
                    .or_else(|| {
                        if action == "mail.reply" {
                            ctx.event_data.get("from").and_then(Value::as_str)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| NexusError::validation("falta el destinatario del correo"))?;
            let to = ip(to_ref);
            let subject_ref = p.subject.as_deref().or_else(|| {
                if action == "mail.reply" {
                    ctx.event_data.get("subject").and_then(Value::as_str)
                } else {
                    None
                }
            });
            let subject = ip(subject_ref.unwrap_or(""));
            let subject = if action == "mail.reply"
                && !subject.trim().to_ascii_lowercase().starts_with("re:")
            {
                format!("Re: {subject}")
            } else {
                subject
            };
            let body = ip(&p.body);
            let reply = p.in_reply_to.as_deref().or_else(|| {
                if action == "mail.reply" {
                    ctx.event_data.get("message_id").and_then(Value::as_str)
                } else {
                    None
                }
            });
            let in_reply_to = reply.map(ip);
            s.mail
                .send(&account, &to, &subject, &body, in_reply_to.as_deref())
                .await?;
            Ok((
                json!({"sent": true, "account": account, "to": to}),
                vec!["mail"],
            ))
        }
        other => Err(NexusError::validation(format!(
            "acción desconocida: {other}"
        ))),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: &Value) -> Result<T> {
    serde_json::from_value(params.clone())
        .map_err(|e| NexusError::validation(format!("parámetros de acción inválidos: {e}")))
}

async fn build_report(s: &Services) -> Result<String> {
    let today = nexus_core::now().format("%Y-%m-%d").to_string();
    let open = s.tasks.list(TaskFilter::default()).await?;
    let open: Vec<_> = open
        .into_iter()
        .filter(|t| matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress))
        .collect();
    let overdue: Vec<_> = open
        .iter()
        .filter(|t| {
            t.due_date
                .as_deref()
                .map(|d| &d[..10.min(d.len())] < today.as_str())
                .unwrap_or(false)
        })
        .collect();
    let projects = s.projects.active_with_progress().await?;
    let completed_week = nexus_db::repos::tasks::count_completed_since(
        &s.db,
        &(nexus_core::now() - chrono::Duration::days(7)).to_rfc3339(),
    )
    .await?;
    let mut md = format!(
        "# Informe NEXUS — {}\n\n## Resumen\n- Tareas pendientes: {}\n- Tareas vencidas: {}\n- Completadas (7 días): {}\n- Proyectos activos: {}\n\n## Proyectos\n",
        nexus_core::now().format("%Y-%m-%d %H:%M"),
        open.len(),
        overdue.len(),
        completed_week,
        projects.len()
    );
    for p in &projects {
        md.push_str(&format!(
            "- **{}** — {:.0}% ({} de {} tareas)\n",
            p.project.name, p.progress, p.done, p.total
        ));
    }
    md.push_str("\n## Tareas vencidas\n");
    if overdue.is_empty() {
        md.push_str("- Ninguna \u{1F389}\n");
    }
    for t in overdue.iter().take(20) {
        md.push_str(&format!(
            "- {} (vencía {})\n",
            t.title,
            t.due_date.as_deref().unwrap_or("?")
        ));
    }
    md.push_str("\n## Próximas tareas\n");
    for t in open.iter().take(15) {
        md.push_str(&format!("- [{}] {}\n", t.priority, t.title));
    }
    Ok(md)
}
