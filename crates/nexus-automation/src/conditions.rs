//! ConditionEngine: evaluación determinista de condiciones con datos reales
//! de NEXUS. Nunca se ejecuta SQL generado por un modelo: solo consultas
//! tipadas de los repositorios existentes.

use nexus_core::{Condition, NoteFilter, Result, TaskFilter, TaskStatus};
use nexus_db::repos;
use nexus_services::Services;
use serde_json::{json, Value};

pub const METRICS: &[(&str, &str)] = &[
    (
        "open_tasks",
        "Número de tareas pendientes (todo + en progreso)",
    ),
    ("overdue_tasks", "Número de tareas vencidas sin completar"),
    ("completed_today", "Tareas completadas hoy"),
    ("completed_week", "Tareas completadas en los últimos 7 días"),
    ("active_projects", "Número de proyectos activos"),
    ("total_notes", "Número de notas en la base de conocimiento"),
    ("inbox_pending", "Elementos pendientes en el inbox"),
    (
        "project_progress",
        "Progreso (0-100) de un proyecto (params.project_id)",
    ),
    (
        "project_idle_days",
        "Días sin actividad de un proyecto (params.project_id)",
    ),
    (
        "pending_tasks_in_project",
        "Tareas pendientes de un proyecto (params.project_id)",
    ),
    (
        "note_matches",
        "Notas que contienen un texto (params.search)",
    ),
    (
        "task_is_overdue",
        "La tarea del evento o params.task_id está vencida",
    ),
    (
        "last_execution_ok",
        "La última ejecución de esta automatización terminó bien",
    ),
];

pub fn is_known_metric(m: &str) -> bool {
    METRICS.iter().any(|(id, _)| *id == m)
}

/// Contexto de evaluación: payload del evento, ids y datos de la ejecución.
pub struct EvalContext<'a> {
    pub services: &'a Services,
    pub event_data: Value,
    pub automation_id: &'a str,
    /// Variables del flujo (resultados de pasos anteriores).
    pub vars: Value,
}

pub fn evaluate<'a>(
    cond: &'a Condition,
    ctx: &'a EvalContext<'a>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>> {
    Box::pin(async move {
        match cond {
            Condition::And { conditions } => {
                for c in conditions {
                    if !evaluate(c, ctx).await? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Condition::Or { conditions } => {
                for c in conditions {
                    if evaluate(c, ctx).await? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Condition::Not { condition } => Ok(!evaluate(condition, ctx).await?),
            Condition::EventField { field, op, value } => {
                let left = lookup_field(&ctx.event_data, field)
                    .or_else(|| lookup_field(&ctx.vars, field))
                    .cloned()
                    .unwrap_or(Value::Null);
                Ok(op.apply(&left, value))
            }
            Condition::Metric {
                metric,
                op,
                value,
                params,
            } => {
                let left = metric_value(metric, params, ctx).await?;
                Ok(op.apply(&left, value))
            }
        }
    })
}

fn interpolate_params(params: &Value, event: &Value, vars: &Value) -> Value {
    match params {
        Value::String(s) => Value::String(crate::actions::interpolate(s, event, vars)),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), interpolate_params(v, event, vars)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn lookup_field<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

async fn metric_value(metric: &str, params: &Value, ctx: &EvalContext<'_>) -> Result<Value> {
    let s = ctx.services;
    let today = nexus_core::now().format("%Y-%m-%d").to_string();
    let params = interpolate_params(params, &ctx.event_data, &ctx.vars);
    let params = &params;
    let param_str = |k: &str| -> Option<String> {
        params
            .get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                ctx.event_data
                    .get(k)
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| ctx.vars.get(k).and_then(Value::as_str).map(str::to_string))
    };
    let v = match metric {
        "open_tasks" => json!(repos::tasks::count_open(&s.db).await?),
        "overdue_tasks" => {
            let tasks = s
                .tasks
                .list(TaskFilter {
                    due_before: Some(today.clone()),
                    ..Default::default()
                })
                .await?;
            let n = tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress))
                .count();
            json!(n)
        }
        "completed_today" => {
            let since = format!("{today}T00:00:00");
            json!(repos::tasks::count_completed_since(&s.db, &since).await?)
        }
        "completed_week" => {
            let since = (nexus_core::now() - chrono::Duration::days(7)).to_rfc3339();
            json!(repos::tasks::count_completed_since(&s.db, &since).await?)
        }
        "active_projects" => json!(s.projects.active_with_progress().await?.len()),
        "total_notes" => json!(s.notes.list(NoteFilter::default()).await?.len()),
        "inbox_pending" => json!(s
            .inbox
            .list(Some(nexus_core::InboxStatus::Pending))
            .await?
            .len()),
        "project_progress" => {
            let id = param_str("project_id")
                .ok_or_else(|| nexus_core::NexusError::validation("falta project_id"))?;
            let detail = s.projects.get(&id).await?;
            json!(detail.progress.progress)
        }
        "project_idle_days" => {
            let id = param_str("project_id")
                .ok_or_else(|| nexus_core::NexusError::validation("falta project_id"))?;
            let detail = s.projects.get(&id).await?;
            let mut latest = detail.project.updated_at.clone();
            for t in &detail.tasks {
                if t.updated_at > latest {
                    latest = t.updated_at.clone();
                }
            }
            for a in &detail.activity {
                if a.created_at > latest {
                    latest = a.created_at.clone();
                }
            }
            let days = chrono::DateTime::parse_from_rfc3339(&latest)
                .map(|dt| {
                    (nexus_core::now() - dt.with_timezone(&chrono::Utc))
                        .num_days()
                        .max(0)
                })
                .unwrap_or(0);
            json!(days)
        }
        "pending_tasks_in_project" => {
            let id = param_str("project_id")
                .ok_or_else(|| nexus_core::NexusError::validation("falta project_id"))?;
            let tasks = s
                .tasks
                .list(TaskFilter {
                    project_id: Some(id),
                    ..Default::default()
                })
                .await?;
            let n = tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress))
                .count();
            json!(n)
        }
        "note_matches" => {
            let search = param_str("search").unwrap_or_default();
            if search.trim().is_empty() {
                json!(0)
            } else {
                json!(s.notes.search(&search, 100).await?.len())
            }
        }
        "task_is_overdue" => {
            let id = param_str("task_id").or_else(|| {
                ctx.event_data
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            match id {
                Some(id) => match repos::tasks::get(&s.db, &id).await {
                    Ok(t) => {
                        let overdue = t
                            .due_date
                            .as_deref()
                            .map(|d| &d[..10.min(d.len())] < today.as_str())
                            .unwrap_or(false)
                            && matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress);
                        json!(overdue)
                    }
                    Err(_) => json!(false),
                },
                None => json!(false),
            }
        }
        "last_execution_ok" => {
            let last =
                repos::automations::last_finished_execution(&s.db, ctx.automation_id).await?;
            json!(last
                .map(|e| e.status == nexus_core::ExecutionStatus::Completed)
                .unwrap_or(false))
        }
        other => {
            return Err(nexus_core::NexusError::validation(format!(
                "métrica desconocida: {other}"
            )))
        }
    };
    Ok(v)
}
