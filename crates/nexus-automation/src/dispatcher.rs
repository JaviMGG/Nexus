//! AgentDispatcher: puente entre el Automation Engine y el motor multiagente
//! existente. Controla presupuesto de tokens, número de llamadas, timeouts,
//! cancelación y comportamiento sin proveedor (nunca bloquea lo local).

use std::sync::Arc;
use std::time::Duration;

use nexus_agents::ExecutionEngine;
use nexus_ai::ProviderRegistry;
use nexus_core::{AgentRun, AutomationMode, NexusError, Result, RunStatus};
use nexus_db::repos;
use nexus_services::Services;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

pub struct AgentOutcome {
    pub run: AgentRun,
    pub deferred: bool,
    pub detail: Value,
}

pub struct Budget<'a> {
    pub tokens_left: &'a mut i64,
    pub calls_left: &'a mut i64,
    pub tokens_used_total: &'a mut i64,
    pub agent_calls_total: &'a mut i64,
}

pub struct AgentDispatcher {
    pub engine: Arc<ExecutionEngine>,
    pub providers: Arc<ProviderRegistry>,
    pub services: Services,
}

impl AgentDispatcher {
    /// Invoca un agente real con el objetivo dado. En modo híbrido sin
    /// proveedor devuelve `deferred` (la ejecución local continúa).
    pub async fn dispatch(
        &self,
        agent_id: &str,
        goal: &str,
        mode: AutomationMode,
        budget: &mut Budget<'_>,
        token: &CancellationToken,
        timeout: Duration,
    ) -> Result<AgentOutcome> {
        if *budget.calls_left <= 0 {
            return Err(NexusError::LimitExceeded(
                "límite de llamadas a agentes alcanzado en esta ejecución".into(),
            ));
        }
        if *budget.tokens_left <= 0 {
            return Err(NexusError::LimitExceeded(
                "presupuesto de tokens agotado en esta ejecución".into(),
            ));
        }
        let available = self.providers.default_provider().await.is_ok();
        if !available {
            if mode == AutomationMode::Hybrid {
                return Ok(AgentOutcome {
                    run: placeholder_run(agent_id, goal),
                    deferred: true,
                    detail: json!({
                        "deferred": true,
                        "reason": "proveedor de IA no disponible; acción inteligente pendiente",
                        "agent_id": agent_id,
                        "goal": goal,
                    }),
                });
            }
            return Err(NexusError::AiUnavailable(
                "no hay proveedor de IA disponible para esta automatización".into(),
            ));
        }

        *budget.calls_left -= 1;
        *budget.agent_calls_total += 1;
        let run = self
            .engine
            .start_run(goal.to_string(), agent_id.to_string(), None, None, 0)
            .await?;
        let final_run = self.wait_run(&run.id, token, timeout).await;
        match final_run {
            Ok(run) => {
                *budget.tokens_used_total += run.tokens_used;
                *budget.tokens_left -= run.tokens_used;
                let ok = run.status == RunStatus::Completed;
                let detail = json!({
                    "run_id": run.id,
                    "agent_id": run.agent_id,
                    "status": run.status,
                    "tokens_used": run.tokens_used,
                    "result": run.result,
                    "error": run.error,
                });
                if !ok {
                    return Err(NexusError::Ai(format!(
                        "el agente {} terminó en estado {}: {}",
                        agent_id,
                        run.status,
                        run.error.as_deref().unwrap_or("sin detalle")
                    )));
                }
                Ok(AgentOutcome {
                    run,
                    deferred: false,
                    detail,
                })
            }
            Err(e) => {
                let _ = self.engine.cancel_run(&run.id).await;
                Err(e)
            }
        }
    }

    async fn wait_run(
        &self,
        run_id: &str,
        token: &CancellationToken,
        timeout: Duration,
    ) -> Result<AgentRun> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if token.is_cancelled() {
                return Err(NexusError::Cancelled);
            }
            let run = repos::runs::get_run(&self.services.db, run_id).await?;
            if matches!(
                run.status,
                RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
            ) {
                return Ok(run);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(NexusError::Ai(format!(
                    "timeout esperando al agente ({})",
                    timeout.as_secs()
                )));
            }
            tokio::select! {
                _ = token.cancelled() => return Err(NexusError::Cancelled),
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            }
        }
    }
}

fn placeholder_run(agent_id: &str, goal: &str) -> AgentRun {
    let now = nexus_core::now_str();
    AgentRun {
        id: nexus_core::new_id(),
        conversation_id: None,
        agent_id: agent_id.to_string(),
        parent_run_id: None,
        goal: goal.to_string(),
        status: RunStatus::Pending,
        result: None,
        error: None,
        iterations: 0,
        tokens_used: 0,
        created_at: now.clone(),
        started_at: None,
        finished_at: None,
    }
}
