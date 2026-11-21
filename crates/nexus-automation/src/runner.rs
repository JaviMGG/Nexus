//! ExecutionEngine de automatizaciones: recorre el flujo paso a paso con
//! presupuesto, timeouts, cancelación cooperativa, autorizaciones humanas,
//! reintentos controlados e historial completo en SQLite.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nexus_core::{
    new_id, now, now_str, AgentRun, AppEvent, Automation, AutomationApproval,
    AutomationApprovalStatus, AutomationExecution, AutomationMode, Condition, ExecutionStatus,
    NexusError, Policy, Result, StepKind, StepLog, StepStatus,
};
use nexus_db::repos;
use serde_json::{json, Value};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::actions::{self, ActionContext};
use crate::conditions::{self, EvalContext};
use crate::dispatcher::{AgentDispatcher, Budget};
use crate::manager::AutomationManager;

const APPROVAL_TIMEOUT: Duration = Duration::from_secs(24 * 3600);

pub struct RunOutcome {
    pub execution: AutomationExecution,
}

impl AutomationManager {
    /// Ejecución síncrona (encadenado entre automatizaciones y tests):
    /// crea el registro y espera a que el flujo termine.
    pub async fn execute(
        self: &Arc<Self>,
        automation: Automation,
        trigger_detail: String,
        event_data: Value,
        test_run: bool,
        chain_depth: i64,
        token: CancellationToken,
    ) -> Result<RunOutcome> {
        let execution = AutomationExecution {
            id: new_id(),
            automation_id: automation.id.clone(),
            automation_name: automation.name.clone(),
            trigger_kind: automation.trigger.kind(),
            trigger_detail: trigger_detail.clone(),
            mode: automation.mode,
            status: ExecutionStatus::Pending,
            test_run,
            tokens_used: 0,
            agent_calls: 0,
            steps_done: 0,
            error: None,
            result: None,
            created_at: now_str(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
        };
        let execution = repos::automations::create_execution(&self.services.db, &execution).await?;
        self.run_registered(
            &execution.id,
            automation,
            trigger_detail,
            event_data,
            test_run,
            chain_depth,
            token,
        )
        .await?;
        let execution = repos::automations::get_execution(&self.services.db, &execution.id).await?;
        Ok(RunOutcome { execution })
    }

    pub(crate) async fn finish_execution(
        &self,
        automation: &Automation,
        execution: &mut AutomationExecution,
        started: Instant,
    ) -> Result<()> {
        execution.finished_at = Some(now_str());
        execution.duration_ms = Some(started.elapsed().as_millis() as i64);
        repos::automations::update_execution(&self.services.db, execution).await?;
        if !execution.test_run {
            repos::automations::record_run_result(
                &self.services.db,
                &automation.id,
                execution.status,
            )
            .await?;
        }
        self.emit_exec(execution);
        let _ = self.events.send(AppEvent::DataChanged {
            entity: "automation".into(),
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_flow(
        self: &Arc<Self>,
        automation: &Automation,
        execution: &mut AutomationExecution,
        event_data: Value,
        chain_depth: i64,
        token: &CancellationToken,
        started: Instant,
    ) -> Result<Value> {
        let timeout = Duration::from_secs(automation.limits.timeout_secs.clamp(5, 3600) as u64);
        let mut vars = json!({});
        let mut tokens_left = automation.limits.max_tokens;
        let mut calls_left = automation.limits.max_agent_calls;
        let mut summaries: Vec<Value> = Vec::new();

        // Condiciones raíz de la automatización.
        if let Some(cond) = &automation.conditions {
            let ctx = EvalContext {
                services: &self.services,
                event_data: event_data.clone(),
                automation_id: &automation.id,
                vars: vars.clone(),
            };
            let t0 = Instant::now();
            let pass = conditions::evaluate(cond, &ctx).await?;
            self.log_step(
                &execution.id,
                None,
                "condition",
                "Condiciones de activación",
                if pass {
                    StepStatus::Ok
                } else {
                    StepStatus::Skipped
                },
                Some(json!({"passed": pass})),
                None,
                t0.elapsed().as_millis() as i64,
            )
            .await;
            if !pass {
                return Ok(json!({"skipped": true, "reason": "condiciones no cumplidas"}));
            }
        }

        let steps: HashMap<String, nexus_core::FlowStep> = automation
            .flow
            .steps
            .iter()
            .map(|s| (s.id.clone(), s.clone()))
            .collect();
        let mut current = automation.flow.entry.clone();
        let mut executed = 0i64;
        let mut deferred_count = 0i64;

        while let Some(step_id) = current.clone() {
            if token.is_cancelled() {
                return Err(NexusError::Cancelled);
            }
            if started.elapsed() > timeout {
                return Err(NexusError::LimitExceeded("timeout de la ejecución".into()));
            }
            executed += 1;
            if executed > automation.limits.max_steps {
                return Err(NexusError::LimitExceeded(
                    "límite de pasos alcanzado (posible flujo demasiado largo)".into(),
                ));
            }
            let step = steps
                .get(&step_id)
                .ok_or_else(|| NexusError::Internal(format!("paso inexistente: {step_id}")))?;
            let t0 = Instant::now();
            let mut next = step.next.clone();

            match &step.kind {
                StepKind::End => {
                    self.log_step(
                        &execution.id,
                        Some(&step.id),
                        "end",
                        "Fin",
                        StepStatus::Ok,
                        None,
                        None,
                        0,
                    )
                    .await;
                    break;
                }
                StepKind::Condition {
                    condition,
                    on_true,
                    on_false,
                } => {
                    let ctx = EvalContext {
                        services: &self.services,
                        event_data: event_data.clone(),
                        automation_id: &automation.id,
                        vars: vars.clone(),
                    };
                    let pass = conditions::evaluate(condition, &ctx).await?;
                    self.log_step(
                        &execution.id,
                        Some(&step.id),
                        "condition",
                        "Condición IF/ELSE",
                        StepStatus::Ok,
                        Some(json!({"passed": pass})),
                        None,
                        t0.elapsed().as_millis() as i64,
                    )
                    .await;
                    next = if pass {
                        on_true.clone()
                    } else {
                        on_false.clone()
                    }
                    .or_else(|| step.next.clone());
                }
                StepKind::Delay { seconds } => {
                    let dur = Duration::from_secs((*seconds).clamp(0, 3600) as u64);
                    tokio::select! {
                        _ = token.cancelled() => return Err(NexusError::Cancelled),
                        _ = tokio::time::sleep(dur) => {}
                    }
                    self.log_step(
                        &execution.id,
                        Some(&step.id),
                        "delay",
                        &format!("Espera {seconds}s"),
                        StepStatus::Ok,
                        None,
                        None,
                        dur.as_millis() as i64,
                    )
                    .await;
                }
                StepKind::Notification { title, body } => {
                    let title = actions::interpolate(title, &event_data, &vars);
                    let body = actions::interpolate(body, &event_data, &vars);
                    let _ = self.events.send(AppEvent::Notification {
                        title: title.clone(),
                        body: body.clone(),
                        automation_id: Some(automation.id.clone()),
                    });
                    self.log_step(
                        &execution.id,
                        Some(&step.id),
                        "notification",
                        &title,
                        StepStatus::Ok,
                        Some(json!({"body": body})),
                        None,
                        0,
                    )
                    .await;
                }
                StepKind::Action {
                    action,
                    params,
                    approval,
                } => {
                    let params = interpolate_value(params, &event_data, &vars);
                    let risk = actions::risk_of(action);
                    let needs_approval = risk == "sensitive"
                        || (*approval == Some(Policy::Confirm))
                        || (risk == "write" && automation.permissions.write == Policy::Confirm);
                    let actor = format!("automation:{}", automation.id);
                    let actx = ActionContext {
                        services: &self.services,
                        actor: actor.clone(),
                        event_data: event_data.clone(),
                        vars: vars.clone(),
                        dry_run: execution.test_run,
                        chain_depth,
                        run_automation: None,
                    };
                    if needs_approval && !execution.test_run {
                        let approved = self
                            .request_approval(
                                automation, execution, &step.id, action, &params, None, token,
                            )
                            .await?;
                        if !approved {
                            self.log_step(
                                &execution.id,
                                Some(&step.id),
                                "action",
                                &actions::display_name(action),
                                StepStatus::Skipped,
                                Some(json!({"reason": "autorización denegada"})),
                                None,
                                t0.elapsed().as_millis() as i64,
                            )
                            .await;
                            execution.steps_done += 1;
                            repos::automations::update_execution(&self.services.db, execution)
                                .await?;
                            current = next;
                            continue;
                        }
                    }
                    let chained = self.clone();
                    let depth = chain_depth;
                    let runner = |id: String| -> std::pin::Pin<
                        Box<dyn std::future::Future<Output = Result<Value>> + Send + '_>,
                    > {
                        let chained = chained.clone();
                        Box::pin(async move {
                            let target = repos::automations::get(&chained.services.db, &id).await?;
                            let outcome = chained
                                .execute(
                                    target,
                                    "encadenada".into(),
                                    json!({}),
                                    false,
                                    depth + 1,
                                    CancellationToken::new(),
                                )
                                .await?;
                            Ok(
                                json!({"execution_id": outcome.execution.id, "status": outcome.execution.status}),
                            )
                        })
                    };
                    let actx = ActionContext {
                        run_automation: Some(&runner),
                        ..actx
                    };
                    match actions::execute(action, &params, &actx).await {
                        Ok((detail, entities)) => {
                            merge_vars(&mut vars, &detail);
                            for e in entities {
                                let _ =
                                    self.events.send(AppEvent::DataChanged { entity: e.into() });
                            }
                            summaries
                                .push(json!({"step": step.id, "action": action, "detail": detail}));
                            self.log_step(
                                &execution.id,
                                Some(&step.id),
                                "action",
                                &actions::display_name(action),
                                StepStatus::Ok,
                                Some(detail),
                                None,
                                t0.elapsed().as_millis() as i64,
                            )
                            .await;
                        }
                        Err(e) => {
                            self.log_step(
                                &execution.id,
                                Some(&step.id),
                                "action",
                                &actions::display_name(action),
                                StepStatus::Error,
                                Some(params),
                                Some(e.to_string()),
                                t0.elapsed().as_millis() as i64,
                            )
                            .await;
                            return Err(e);
                        }
                    }
                }
                StepKind::Agent {
                    agent_id,
                    goal,
                    approval,
                } => {
                    if automation.mode == AutomationMode::Local {
                        self.log_step(
                            &execution.id,
                            Some(&step.id),
                            "agent",
                            &format!("Agente {agent_id}"),
                            StepStatus::Skipped,
                            Some(json!({"reason": "modo local: los pasos de IA no se ejecutan"})),
                            None,
                            0,
                        )
                        .await;
                        execution.steps_done += 1;
                        repos::automations::update_execution(&self.services.db, execution).await?;
                        current = next;
                        continue;
                    }
                    let goal = actions::interpolate(goal, &event_data, &vars);
                    if *approval == Some(Policy::Confirm) && !execution.test_run {
                        let approved = self
                            .request_approval(
                                automation,
                                execution,
                                &step.id,
                                "agents.invoke",
                                &json!({"agent_id": agent_id, "goal": goal}),
                                Some(agent_id),
                                token,
                            )
                            .await?;
                        if !approved {
                            self.log_step(
                                &execution.id,
                                Some(&step.id),
                                "agent",
                                &format!("Agente {agent_id}"),
                                StepStatus::Skipped,
                                Some(json!({"reason": "autorización denegada"})),
                                None,
                                0,
                            )
                            .await;
                            execution.steps_done += 1;
                            repos::automations::update_execution(&self.services.db, execution)
                                .await?;
                            current = next;
                            continue;
                        }
                    }
                    if execution.test_run {
                        let available = self.providers.default_provider().await.is_ok();
                        self.log_step(
                            &execution.id,
                            Some(&step.id),
                            "agent",
                            &format!("Agente {agent_id} (prueba)"),
                            StepStatus::Ok,
                            Some(json!({"dry_run": true, "goal": goal, "ai_available": available})),
                            None,
                            0,
                        )
                        .await;
                        execution.steps_done += 1;
                        repos::automations::update_execution(&self.services.db, execution).await?;
                        current = next;
                        continue;
                    }
                    // Presupuesto diario global.
                    let settings = self.services.settings.get_app().await.unwrap_or_default();
                    let day_start = now().format("%Y-%m-%dT00:00:00").to_string();
                    let used_today =
                        repos::automations::tokens_since(&self.services.db, Some(&day_start))
                            .await?;
                    if used_today >= settings.automations.daily_token_limit {
                        if automation.mode == AutomationMode::Hybrid {
                            deferred_count += 1;
                            self.log_step(&execution.id, Some(&step.id), "agent", &format!("Agente {agent_id}"), StepStatus::Deferred, Some(json!({"reason": "límite diario de tokens alcanzado", "goal": goal})), None, 0).await;
                            execution.steps_done += 1;
                            repos::automations::update_execution(&self.services.db, execution)
                                .await?;
                            current = next;
                            continue;
                        }
                        return Err(NexusError::LimitExceeded(
                            "límite diario de tokens de automatizaciones alcanzado".into(),
                        ));
                    }
                    let dispatcher = AgentDispatcher {
                        engine: self.engine.clone(),
                        providers: self.providers.clone(),
                        services: self.services.clone(),
                    };
                    let mut budget = Budget {
                        tokens_left: &mut tokens_left,
                        calls_left: &mut calls_left,
                        tokens_used_total: &mut execution.tokens_used,
                        agent_calls_total: &mut execution.agent_calls,
                    };
                    let remaining = timeout.saturating_sub(started.elapsed());
                    let result = dispatcher
                        .dispatch(
                            agent_id,
                            &goal,
                            automation.mode,
                            &mut budget,
                            token,
                            remaining.max(Duration::from_secs(10)),
                        )
                        .await;
                    match result {
                        Ok(outcome) => {
                            if outcome.deferred {
                                deferred_count += 1;
                                self.log_step(
                                    &execution.id,
                                    Some(&step.id),
                                    "agent",
                                    &format!("Agente {agent_id}"),
                                    StepStatus::Deferred,
                                    Some(outcome.detail),
                                    None,
                                    t0.elapsed().as_millis() as i64,
                                )
                                .await;
                            } else {
                                if let Some(result) = &outcome.run.result {
                                    vars["agent_result"] = json!(result);
                                }
                                vars["last_run_id"] = json!(outcome.run.id);
                                summaries.push(json!({"step": step.id, "agent": agent_id, "run_id": outcome.run.id}));
                                self.log_step(
                                    &execution.id,
                                    Some(&step.id),
                                    "agent",
                                    &format!("Agente {agent_id}"),
                                    StepStatus::Ok,
                                    Some(outcome.detail),
                                    None,
                                    t0.elapsed().as_millis() as i64,
                                )
                                .await;
                            }
                        }
                        Err(e) => {
                            self.log_step(
                                &execution.id,
                                Some(&step.id),
                                "agent",
                                &format!("Agente {agent_id}"),
                                StepStatus::Error,
                                Some(json!({"goal": goal})),
                                Some(e.to_string()),
                                t0.elapsed().as_millis() as i64,
                            )
                            .await;
                            return Err(e);
                        }
                    }
                }
            }
            execution.steps_done += 1;
            repos::automations::update_execution(&self.services.db, execution).await?;
            current = next;
        }

        Ok(json!({
            "steps": summaries,
            "deferred": deferred_count,
            "tokens_used": execution.tokens_used,
            "agent_calls": execution.agent_calls,
            "test_run": execution.test_run,
        }))
    }

    /// Crea la autorización, marca la ejecución WaitingApproval y espera la
    /// decisión humana (o cancelación/timeout).
    #[allow(clippy::too_many_arguments)]
    async fn request_approval(
        &self,
        automation: &Automation,
        execution: &mut AutomationExecution,
        step_id: &str,
        action: &str,
        params: &Value,
        agent_id: Option<&str>,
        token: &CancellationToken,
    ) -> Result<bool> {
        let ctx = ActionContext {
            services: &self.services,
            actor: format!("automation:{}", automation.id),
            event_data: json!({}),
            vars: json!({}),
            dry_run: true,
            chain_depth: 0,
            run_automation: None,
        };
        let approval = AutomationApproval {
            id: new_id(),
            execution_id: execution.id.clone(),
            automation_id: automation.id.clone(),
            step_id: Some(step_id.to_string()),
            action: action.to_string(),
            params: params.clone(),
            preview: if action == "agents.invoke" {
                format!(
                    "El agente {} ejecutará: {}",
                    agent_id.unwrap_or("?"),
                    params.get("goal").and_then(Value::as_str).unwrap_or("")
                )
            } else {
                actions::preview(action, params, &ctx)
            },
            agent_id: agent_id.map(str::to_string),
            status: AutomationApprovalStatus::Pending,
            created_at: now_str(),
            decided_at: None,
        };
        let approval = repos::automations::create_approval(&self.services.db, &approval).await?;
        let (tx, mut rx) = watch::channel(false);
        self.approval_notifies
            .lock()
            .await
            .insert(approval.id.clone(), tx);
        execution.status = ExecutionStatus::WaitingApproval;
        repos::automations::update_execution(&self.services.db, execution).await?;
        self.emit_exec(execution);
        let _ = self.events.send(AppEvent::AutomationApprovalRequested {
            approval: approval.clone(),
        });
        self.log_step(
            &execution.id,
            Some(step_id),
            "approval",
            &approval.preview,
            StepStatus::WaitingApproval,
            Some(json!({"approval_id": approval.id})),
            None,
            0,
        )
        .await;

        // Si la decisión llegó antes de empezar a esperar, no bloquearse.
        let current = repos::automations::get_approval(&self.services.db, &approval.id).await?;
        if current.status != AutomationApprovalStatus::Pending {
            self.approval_notifies.lock().await.remove(&approval.id);
            execution.status = ExecutionStatus::Running;
            repos::automations::update_execution(&self.services.db, execution).await?;
            self.emit_exec(execution);
            return Ok(current.status == AutomationApprovalStatus::Approved);
        }

        let decision = tokio::select! {
            _ = token.cancelled() => None,
            _ = tokio::time::sleep(APPROVAL_TIMEOUT) => None,
            res = rx.changed() => {
                if res.is_ok() {
                    repos::automations::get_approval(&self.services.db, &approval.id).await.ok()
                } else {
                    None
                }
            }
        };
        self.approval_notifies.lock().await.remove(&approval.id);
        execution.status = ExecutionStatus::Running;
        repos::automations::update_execution(&self.services.db, execution).await?;
        self.emit_exec(execution);
        match decision {
            Some(a) if a.status == AutomationApprovalStatus::Approved => Ok(true),
            Some(_) => Ok(false),
            None => {
                if token.is_cancelled() {
                    return Err(NexusError::Cancelled);
                }
                let _ = repos::automations::decide_approval(
                    &self.services.db,
                    &approval.id,
                    AutomationApprovalStatus::Expired,
                )
                .await;
                Ok(false)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn log_step(
        &self,
        execution_id: &str,
        step_id: Option<&str>,
        kind: &str,
        name: &str,
        status: StepStatus,
        detail: Option<Value>,
        error: Option<String>,
        duration_ms: i64,
    ) {
        let log = StepLog {
            id: new_id(),
            execution_id: execution_id.to_string(),
            step_id: step_id.map(str::to_string),
            kind: kind.to_string(),
            name: name.to_string(),
            status,
            detail,
            error,
            duration_ms,
            created_at: now_str(),
        };
        if let Err(e) = repos::automations::add_step_log(&self.services.db, &log).await {
            tracing::warn!("no se pudo registrar el paso: {e}");
        }
    }

    pub(crate) fn emit_exec(&self, execution: &AutomationExecution) {
        let _ = self.events.send(AppEvent::AutomationExecutionUpdated {
            execution: execution.clone(),
        });
    }
}

fn interpolate_value(v: &Value, event: &Value, vars: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(actions::interpolate(s, event, vars)),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, val)| (k.clone(), interpolate_value(val, event, vars)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|val| interpolate_value(val, event, vars))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn merge_vars(vars: &mut Value, detail: &Value) {
    if let (Some(dst), Some(src)) = (vars.as_object_mut(), detail.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
}

/// Detalle de ejecución con pasos, autorizaciones y runs de agente implicados.
pub async fn execution_detail(
    services: &nexus_services::Services,
    execution_id: &str,
) -> Result<nexus_core::ExecutionDetail> {
    let execution = repos::automations::get_execution(&services.db, execution_id).await?;
    let steps = repos::automations::list_step_logs(&services.db, execution_id).await?;
    let approvals =
        repos::automations::list_execution_approvals(&services.db, execution_id).await?;
    let mut agent_runs: Vec<AgentRun> = Vec::new();
    for step in &steps {
        if let Some(run_id) = step
            .detail
            .as_ref()
            .and_then(|d| d.get("run_id"))
            .and_then(Value::as_str)
        {
            if let Ok(run) = repos::runs::get_run(&services.db, run_id).await {
                agent_runs.push(run);
            }
        }
    }
    Ok(nexus_core::ExecutionDetail {
        execution,
        steps,
        approvals,
        agent_runs,
    })
}

/// Comprueba condiciones sin ejecutar nada (vista previa para el builder).
pub async fn evaluate_preview(
    services: &nexus_services::Services,
    automation_id: &str,
    condition: &Condition,
    event_data: Value,
) -> Result<bool> {
    let ctx = EvalContext {
        services,
        event_data,
        automation_id,
        vars: json!({}),
    };
    conditions::evaluate(condition, &ctx).await
}

/// Estadísticas de uso de agentes a partir de los logs de pasos.
pub async fn top_agents(
    services: &nexus_services::Services,
    limit: i64,
) -> Result<Vec<nexus_core::AgentUsage>> {
    let executions = repos::automations::list_executions(
        &services.db,
        &nexus_core::ExecutionFilter {
            limit: Some(200),
            ..Default::default()
        },
    )
    .await?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for e in executions {
        let logs = repos::automations::list_step_logs(&services.db, &e.id).await?;
        for log in logs {
            if log.kind == "agent" && log.status == StepStatus::Ok {
                if let Some(agent) = log
                    .detail
                    .as_ref()
                    .and_then(|d| d.get("agent_id"))
                    .and_then(Value::as_str)
                {
                    *counts.entry(agent.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    let mut usage: Vec<nexus_core::AgentUsage> = counts
        .into_iter()
        .map(|(agent_id, calls)| nexus_core::AgentUsage { agent_id, calls })
        .collect();
    usage.sort_by_key(|u| std::cmp::Reverse(u.calls));
    usage.truncate(limit as usize);
    Ok(usage)
}
