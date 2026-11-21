use nexus_core::{
    AppEvent, ApprovalStatus, ChatSendResult, CreateEvent, CreateNote, CreateProject, EventFilter,
    InboxStatus, MessageRole, NexusError, NoteFilter, ProviderConfig, Result, TaskStatus,
    UpdateEvent, UpdateNote, UpdateProject, UpdateTask,
};
use nexus_db::repos;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::context::AppContext;

fn arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<T> {
    let v = payload
        .get(key)
        .ok_or_else(|| NexusError::validation(format!("falta el parámetro '{key}'")))?;
    serde_json::from_value(v.clone())
        .map_err(|e| NexusError::validation(format!("parámetro '{key}' inválido: {e}")))
}

fn opt_arg<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<Option<T>> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| NexusError::validation(format!("parámetro '{key}' inválido: {e}"))),
    }
}

fn arg_or_default<T: DeserializeOwned + Default>(payload: &Value, key: &str) -> Result<T> {
    Ok(opt_arg(payload, key)?.unwrap_or_default())
}

fn ok(v: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(v).map_err(|e| NexusError::Internal(e.to_string()))
}

pub async fn dispatch(ctx: &AppContext, command: &str, payload: Value) -> Result<Value> {
    let s = &ctx.services;
    match command {
        // ------------------------------------------------------------- dashboard
        "dashboard.summary" => {
            let app = s.settings.get_app().await?;
            ok(s.dashboard.summary(&app.user_name).await?)
        }

        // ------------------------------------------------------------- tasks
        "tasks.list" => ok(s.tasks.list(arg_or_default(&payload, "filter")?).await?),
        "tasks.get" => ok(s.tasks.get(&arg::<String>(&payload, "id")?).await?),
        "tasks.create" => {
            let t = s.tasks.create(arg(&payload, "data")?, "user").await?;
            ctx.data_changed("task");
            ctx.domain_event("task.created", "task", Some(&t.id), json!(t));
            ok(t)
        }
        "tasks.update" => {
            let id: String = arg(&payload, "id")?;
            let before = repos::tasks::get(&s.db, &id).await.ok();
            let t = s
                .tasks
                .update(&id, arg::<UpdateTask>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("task");
            ctx.domain_event("task.updated", "task", Some(&t.id), json!(t));
            if t.status == TaskStatus::Done
                && before
                    .as_ref()
                    .map(|b| b.status != TaskStatus::Done)
                    .unwrap_or(true)
            {
                ctx.domain_event("task.completed", "task", Some(&t.id), json!(t));
            }
            ok(t)
        }
        "tasks.delete" => {
            s.tasks
                .delete(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("task");
            ok(json!({"deleted": true}))
        }
        "tasks.restore" => {
            let t = s
                .tasks
                .restore(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("task");
            ok(t)
        }
        "tasks.reorder" => {
            let id: String = arg(&payload, "id")?;
            let status: TaskStatus = arg(&payload, "status")?;
            let position: i64 = arg(&payload, "position")?;
            let before = repos::tasks::get(&s.db, &id).await.ok();
            let t = s.tasks.reorder(&id, status, position, "user").await?;
            ctx.data_changed("task");
            if t.status == TaskStatus::Done
                && before
                    .as_ref()
                    .map(|b| b.status != TaskStatus::Done)
                    .unwrap_or(true)
            {
                ctx.domain_event("task.completed", "task", Some(&t.id), json!(t));
            }
            ok(t)
        }

        // ------------------------------------------------------------- projects
        "projects.list" => {
            let include_archived = opt_arg(&payload, "include_archived")?.unwrap_or(false);
            ok(s.projects.list(include_archived).await?)
        }
        "projects.list_with_progress" => {
            let include_archived = opt_arg(&payload, "include_archived")?.unwrap_or(false);
            ok(s.projects.list_with_progress(include_archived).await?)
        }
        "projects.get" => ok(s.projects.get(&arg::<String>(&payload, "id")?).await?),
        "projects.create" => {
            let p = s
                .projects
                .create(arg::<CreateProject>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("project");
            ctx.domain_event("project.created", "project", Some(&p.id), json!(p));
            if !p.goal.trim().is_empty() {
                ctx.domain_event("goal.created", "project", Some(&p.id), json!(p));
            }
            ok(p)
        }
        "projects.update" => {
            let id: String = arg(&payload, "id")?;
            let before = repos::projects::get(&s.db, &id).await.ok();
            let p = s
                .projects
                .update(&id, arg::<UpdateProject>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("project");
            if before
                .as_ref()
                .map(|b| b.status != p.status)
                .unwrap_or(false)
            {
                ctx.domain_event(
                    "project.status_changed",
                    "project",
                    Some(&p.id),
                    json!({"id": p.id, "name": p.name, "status": p.status, "previous_status": before.as_ref().map(|b| b.status)}),
                );
            }
            if !p.goal.trim().is_empty()
                && before
                    .as_ref()
                    .map(|b| b.goal.trim().is_empty())
                    .unwrap_or(false)
            {
                ctx.domain_event("goal.created", "project", Some(&p.id), json!(p));
            }
            ok(p)
        }
        "projects.delete" => {
            s.projects
                .delete(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("project");
            ok(json!({"deleted": true}))
        }
        "projects.restore" => {
            let p = s
                .projects
                .restore(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("project");
            ok(p)
        }

        // ------------------------------------------------------------- notes
        "notes.list" => ok(s
            .notes
            .list(arg_or_default::<NoteFilter>(&payload, "filter")?)
            .await?),
        "notes.search" => {
            let query: String = arg(&payload, "query")?;
            let limit = opt_arg(&payload, "limit")?.unwrap_or(30);
            ok(s.notes.search(&query, limit).await?)
        }
        "notes.get" => ok(s.notes.get(&arg::<String>(&payload, "id")?).await?),
        "notes.create" => {
            let n = s
                .notes
                .create(arg::<CreateNote>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("note");
            if n.kind == nexus_core::NoteKind::Idea {
                ctx.domain_event("idea.created", "note", Some(&n.id), json!(n));
            } else {
                ctx.domain_event("note.created", "note", Some(&n.id), json!(n));
            }
            ok(n)
        }
        "notes.update" => {
            let id: String = arg(&payload, "id")?;
            let n = s
                .notes
                .update(&id, arg::<UpdateNote>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("note");
            ok(n)
        }
        "notes.delete" => {
            s.notes
                .delete(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("note");
            ok(json!({"deleted": true}))
        }
        "notes.restore" => {
            let n = s
                .notes
                .restore(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("note");
            ok(n)
        }

        // ------------------------------------------------------------- events
        "events.list" => ok(s
            .events
            .list(arg_or_default::<EventFilter>(&payload, "filter")?)
            .await?),
        "events.get" => ok(s.events.get(&arg::<String>(&payload, "id")?).await?),
        "events.create" => {
            let e = s
                .events
                .create(arg::<CreateEvent>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("event");
            ok(e)
        }
        "events.update" => {
            let id: String = arg(&payload, "id")?;
            let e = s
                .events
                .update(&id, arg::<UpdateEvent>(&payload, "data")?, "user")
                .await?;
            ctx.data_changed("event");
            ok(e)
        }
        "events.delete" => {
            s.events
                .delete(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("event");
            ok(json!({"deleted": true}))
        }
        "events.restore" => {
            let e = s
                .events
                .restore(&arg::<String>(&payload, "id")?, "user")
                .await?;
            ctx.data_changed("event");
            ok(e)
        }

        // ------------------------------------------------------------- inbox
        "inbox.list" => ok(s
            .inbox
            .list(opt_arg::<InboxStatus>(&payload, "status")?)
            .await?),
        "inbox.create" => {
            let item = s
                .inbox
                .create(&arg::<String>(&payload, "content")?, "user")
                .await?;
            ctx.data_changed("inbox");
            ok(item)
        }
        "inbox.process" => {
            let id: String = arg(&payload, "id")?;
            let kind: String = arg(&payload, "kind")?;
            let data = opt_arg(&payload, "data")?;
            let created = s.inbox.process(&id, &kind, data, s, "user").await?;
            ctx.data_changed("inbox");
            ctx.data_changed(if kind == "task" { "task" } else { "note" });
            ok(json!({"created_id": created}))
        }
        "inbox.archive" => {
            let item = s.inbox.archive(&arg::<String>(&payload, "id")?).await?;
            ctx.data_changed("inbox");
            ok(item)
        }
        "inbox.delete" => {
            s.inbox.delete(&arg::<String>(&payload, "id")?).await?;
            ctx.data_changed("inbox");
            ok(json!({"deleted": true}))
        }

        // ------------------------------------------------------------- captura
        "capture.analyze" => {
            let content: String = arg(&payload, "content")?;
            if content.trim().is_empty() {
                return Err(NexusError::validation("el contenido no puede estar vacío"));
            }
            let result = crate::capture_ai::analyze(ctx, &content).await;
            ok(result)
        }

        // ------------------------------------------------------------- chat
        "chat.conversations" => ok(s.chat.list_conversations().await?),
        "chat.create_conversation" => {
            let agent_id =
                opt_arg(&payload, "agent_id")?.unwrap_or_else(|| "orchestrator".to_string());
            let title: Option<String> = opt_arg(&payload, "title")?;
            ok(s.chat
                .create_conversation(&agent_id, title.as_deref())
                .await?)
        }
        "chat.rename_conversation" => {
            let id: String = arg(&payload, "id")?;
            ok(s.chat
                .rename_conversation(&id, &arg::<String>(&payload, "title")?)
                .await?)
        }
        "chat.delete_conversation" => {
            s.chat
                .delete_conversation(&arg::<String>(&payload, "id")?)
                .await?;
            ok(json!({"deleted": true}))
        }
        "chat.messages" => ok(s
            .chat
            .messages(&arg::<String>(&payload, "conversation_id")?)
            .await?),
        "chat.send" => {
            let content: String = arg(&payload, "content")?;
            if content.trim().is_empty() {
                return Err(NexusError::validation("el mensaje no puede estar vacío"));
            }
            let conversation_id: Option<String> = opt_arg(&payload, "conversation_id")?;
            let agent_id: Option<String> = opt_arg(&payload, "agent_id")?;

            let conv = match conversation_id {
                Some(id) => s.chat.get_conversation(&id).await?,
                None => {
                    s.chat
                        .create_conversation(agent_id.as_deref().unwrap_or("orchestrator"), None)
                        .await?
                }
            };
            let agent = agent_id.unwrap_or(conv.agent_id.clone());
            let user_message = s
                .chat
                .add_message(&conv.id, MessageRole::User, &content, None, None, None)
                .await?;
            ctx.emit(AppEvent::MessageCreated {
                message: user_message.clone(),
            });
            s.chat.auto_title(&conv.id).await?;

            let available = ctx.providers.default_provider().await.is_ok();
            if !available {
                let offline = s.chat.add_message(
                    &conv.id,
                    MessageRole::Assistant,
                    "Ahora mismo no hay ningún proveedor de IA conectado, así que no puedo procesar esta petición. \
                     Tus datos están a salvo y puedes seguir trabajando manualmente: tareas, proyectos, notas y búsqueda \
                     funcionan con normalidad. Para activarme, configura un proveedor en Ajustes → Inteligencia Artificial.",
                    Some("orchestrator"),
                    None,
                    None,
                ).await?;
                ctx.emit(AppEvent::MessageCreated { message: offline });
                return ok(ChatSendResult {
                    conversation_id: conv.id,
                    run_id: None,
                    user_message,
                    ai_available: false,
                });
            }

            let run = ctx
                .engine
                .start_run(content, agent, Some(conv.id.clone()), None, 0)
                .await?;
            ok(ChatSendResult {
                conversation_id: conv.id,
                run_id: Some(run.id),
                user_message,
                ai_available: true,
            })
        }

        // ------------------------------------------------------------- agents
        "agents.list" => ok(ctx.agents.list()),
        "agents.runs" => {
            let limit = opt_arg(&payload, "limit")?.unwrap_or(50);
            ok(repos::runs::list_runs(&s.db, limit).await?)
        }
        "agents.run_detail" => ok(ctx
            .engine
            .run_detail(&arg::<String>(&payload, "id")?)
            .await?),
        "agents.start" => {
            let goal: String = arg(&payload, "goal")?;
            let agent_id =
                opt_arg(&payload, "agent_id")?.unwrap_or_else(|| "orchestrator".to_string());
            let conversation_id: Option<String> = opt_arg(&payload, "conversation_id")?;
            ok(ctx
                .engine
                .start_run(goal, agent_id, conversation_id, None, 0)
                .await?)
        }
        "agents.cancel" => {
            ctx.engine
                .cancel_run(&arg::<String>(&payload, "run_id")?)
                .await?;
            ok(json!({"cancelled": true}))
        }
        "agents.approve" => {
            let a = ctx
                .engine
                .resolve_approval(&arg::<String>(&payload, "approval_id")?, true)
                .await?;
            ok(a)
        }
        "agents.reject" => {
            let a = ctx
                .engine
                .resolve_approval(&arg::<String>(&payload, "approval_id")?, false)
                .await?;
            ok(a)
        }
        "agents.approvals" => {
            let status: Option<ApprovalStatus> = opt_arg(&payload, "status")?;
            ok(repos::runs::list_approvals(&s.db, status).await?)
        }

        // ------------------------------------------------------------- automations
        "automations.list" => {
            let enabled_only = opt_arg(&payload, "enabled_only")?.unwrap_or(false);
            ok(ctx.automation.list(enabled_only).await?)
        }
        "automations.get" => ok(ctx.automation.get(&arg::<String>(&payload, "id")?).await?),
        "automations.create" => ok(ctx.automation.create(arg(&payload, "data")?).await?),
        "automations.update" => {
            let id: String = arg(&payload, "id")?;
            ok(ctx.automation.update(&id, arg(&payload, "data")?).await?)
        }
        "automations.delete" => {
            ctx.automation
                .delete(&arg::<String>(&payload, "id")?)
                .await?;
            ok(json!({"deleted": true}))
        }
        "automations.set_enabled" => {
            let id: String = arg(&payload, "id")?;
            let enabled: bool = arg(&payload, "enabled")?;
            ok(ctx.automation.set_enabled(&id, enabled).await?)
        }
        "automations.run" => {
            let id: String = arg(&payload, "id")?;
            let test_run = opt_arg(&payload, "test")?.unwrap_or(false);
            ok(ctx.automation.run_now(&id, test_run).await?)
        }
        "automations.executions" => ok(ctx
            .automation
            .executions(arg_or_default(&payload, "filter")?)
            .await?),
        "automations.execution_detail" => ok(ctx
            .automation
            .execution_detail(&arg::<String>(&payload, "id")?)
            .await?),
        "automations.cancel" => {
            ctx.automation
                .cancel_execution(&arg::<String>(&payload, "id")?)
                .await?;
            ok(json!({"cancelled": true}))
        }
        "automations.approvals" => {
            let status: Option<nexus_core::AutomationApprovalStatus> = opt_arg(&payload, "status")?;
            ok(ctx.automation.approvals(status).await?)
        }
        "automations.approve" => ok(ctx
            .automation
            .decide_approval(&arg::<String>(&payload, "approval_id")?, true)
            .await?),
        "automations.reject" => ok(ctx
            .automation
            .decide_approval(&arg::<String>(&payload, "approval_id")?, false)
            .await?),
        "automations.overview" => ok(ctx.automation.overview().await?),
        "automations.templates" => ok(nexus_automation::templates::all()),
        "automations.install_template" => {
            let template_id: String = arg(&payload, "template_id")?;
            let template = nexus_automation::templates::all()
                .into_iter()
                .find(|t| t.id == template_id)
                .ok_or_else(|| NexusError::not_found("plantilla", &template_id))?;
            ok(ctx.automation.create(template.automation).await?)
        }
        "automations.triggers" => ok(nexus_automation::triggers::catalog()),
        "automations.actions" => ok(nexus_automation::actions::catalog()),
        "automations.metrics" => ok(nexus_automation::conditions::METRICS
            .iter()
            .map(|(id, desc)| json!({"id": id, "description": desc}))
            .collect::<Vec<_>>()),
        "automations.validate" => {
            let flow: nexus_core::AutomationFlow = arg(&payload, "flow")?;
            let agents: Vec<String> = ctx.agents.list().into_iter().map(|a| a.id).collect();
            nexus_automation::flow::validate(&flow, &agents)?;
            ok(json!({"valid": true}))
        }
        "automations.preview_condition" => {
            let condition: nexus_core::Condition = arg(&payload, "condition")?;
            let automation_id = opt_arg::<String>(&payload, "automation_id")?.unwrap_or_default();
            let event_data = opt_arg(&payload, "event_data")?.unwrap_or(json!({}));
            let passed = nexus_automation::runner::evaluate_preview(
                s,
                &automation_id,
                &condition,
                event_data,
            )
            .await?;
            ok(json!({"passed": passed}))
        }
        "automations.mode_info" => {
            let mode: nexus_core::AutomationMode = arg(&payload, "mode")?;
            ok(ctx.automation.mode_availability(mode).await)
        }

        // ------------------------------------------------------------- activity
        "activity.list" => {
            let limit = opt_arg(&payload, "limit")?.unwrap_or(100);
            let entity_type: Option<String> = opt_arg(&payload, "entity_type")?;
            let entity_id: Option<String> = opt_arg(&payload, "entity_id")?;
            ok(s.activity
                .list(limit, entity_type.as_deref(), entity_id.as_deref())
                .await?)
        }
        "activity.undo" => {
            let event = s.activity.undo(&arg::<String>(&payload, "id")?).await?;
            ctx.data_changed(&event.entity_type);
            ctx.data_changed("activity");
            ok(event)
        }

        // ------------------------------------------------------------- settings
        "settings.get" => {
            let app = s.settings.get_app().await?;
            let providers = s.settings.get_providers().await?;
            let ai_status = ctx.providers.status().await;
            ok(json!({"app": app, "providers": providers, "ai_status": ai_status}))
        }
        "settings.update" => {
            let patch: Value = arg(&payload, "patch")?;
            let app = s.settings.update_app(patch).await?;
            let _ = ctx.providers.refresh().await;
            ctx.emit(AppEvent::AiStatusChanged {
                status: ctx.providers.status().await,
            });
            ctx.data_changed("settings");
            ok(app)
        }
        "providers.save" => {
            let mut config: ProviderConfig = arg(&payload, "config")?;
            if config.id.trim().is_empty() {
                config.id = nexus_core::new_id();
            }
            let api_key: Option<String> = opt_arg(&payload, "api_key")?;
            let saved = s.settings.save_provider(config, api_key).await?;
            let _ = ctx.providers.refresh().await;
            ctx.emit(AppEvent::AiStatusChanged {
                status: ctx.providers.status().await,
            });
            ctx.data_changed("settings");
            ok(saved)
        }
        "providers.delete" => {
            s.settings
                .delete_provider(&arg::<String>(&payload, "id")?)
                .await?;
            let _ = ctx.providers.refresh().await;
            ctx.emit(AppEvent::AiStatusChanged {
                status: ctx.providers.status().await,
            });
            ctx.data_changed("settings");
            ok(json!({"deleted": true}))
        }
        "providers.set_default" => {
            let providers = s
                .settings
                .set_default_provider(&arg::<String>(&payload, "id")?)
                .await?;
            let _ = ctx.providers.refresh().await;
            ctx.emit(AppEvent::AiStatusChanged {
                status: ctx.providers.status().await,
            });
            ok(providers)
        }
        "providers.test" => {
            let id: Option<String> = opt_arg(&payload, "id")?;
            let message = ctx.providers.test(id.as_deref()).await?;
            ctx.emit(AppEvent::AiStatusChanged {
                status: ctx.providers.status().await,
            });
            ok(json!({"message": message}))
        }
        "ai.status" => ok(ctx.providers.status().await),

        // ------------------------------------------------------------- backup
        "backup.export" => ok(s.backup.export_json().await?),
        "backup.create" => {
            let path = s.backup.create_backup(&ctx.backups_dir()).await?;
            ok(json!({"path": path.display().to_string()}))
        }
        "backup.list" => {
            let paths = nexus_services::backup::BackupService::list_backups(&ctx.backups_dir())?;
            ok(json!({"paths": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()}))
        }
        "backup.restore" => {
            let path: String = arg(&payload, "path")?;
            let path = std::path::PathBuf::from(&path);
            let canonical = path
                .canonicalize()
                .map_err(|_| NexusError::not_found("backup", &path.display().to_string()))?;
            let backups = ctx
                .backups_dir()
                .canonicalize()
                .map_err(|e| NexusError::Internal(e.to_string()))?;
            if !canonical.starts_with(&backups) {
                return Err(NexusError::PermissionDenied(
                    "solo se pueden restaurar copias del directorio de backups".into(),
                ));
            }
            s.backup.restore(&canonical).await?;
            for entity in [
                "task", "project", "note", "inbox", "event", "settings", "activity",
            ] {
                ctx.data_changed(entity);
            }
            ok(json!({"restored": true}))
        }

        other => Err(NexusError::NotFound(format!(
            "comando desconocido: {other}"
        ))),
    }
}
