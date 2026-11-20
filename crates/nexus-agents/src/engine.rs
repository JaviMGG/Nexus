//! Motor de ejecución multiagente: bucle de herramientas con streaming,
//! aprobaciones humanas, delegación padre/hijo, cancelación cooperativa,
//! límites de uso y auditoría completa en base de datos.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use nexus_ai::{AiMessage, AiProvider, ChatRequest, ChatResponse, ToolCallRequest, ToolSpec};
use nexus_core::{
    new_id, now_str, AgentRun, AppEvent, Approval, ApprovalStatus, MessageRole, NexusError, Result,
    Risk, RunDetail, RunStatus, ToolCall, ToolCallStatus, UsageLimits,
};
use nexus_db::repos;
use nexus_services::Services;
use tokio::sync::{broadcast, mpsc, Mutex, Notify};
use tokio_util::sync::CancellationToken;

use crate::registry::AgentRegistry;

const TOOL_TIMEOUT: Duration = Duration::from_secs(60);
const RETRY_BACKOFFS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(3)];
const HISTORY_MESSAGES: usize = 20;
const RESULT_PREVIEW_CHARS: usize = 2000;
const TOOL_ERROR_PREVIEW_CHARS: usize = 500;

struct RunHandle {
    token: CancellationToken,
}

pub struct ExecutionEngine {
    services: Services,
    providers: Arc<nexus_ai::ProviderRegistry>,
    tools: Arc<nexus_tools::ToolRegistry>,
    agents: Arc<AgentRegistry>,
    events: broadcast::Sender<AppEvent>,
    runs: Mutex<HashMap<String, RunHandle>>,
    approvals: Mutex<HashMap<String, Arc<Notify>>>,
}

impl ExecutionEngine {
    pub fn new(
        services: Services,
        providers: Arc<nexus_ai::ProviderRegistry>,
        tools: Arc<nexus_tools::ToolRegistry>,
        agents: Arc<AgentRegistry>,
        events: broadcast::Sender<AppEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            services,
            providers,
            tools,
            agents,
            events,
            runs: Mutex::new(HashMap::new()),
            approvals: Mutex::new(HashMap::new()),
        })
    }

    /// Crea el run en DB, emite RunStarted y lanza el bucle en segundo plano.
    /// Sin proveedor disponible devuelve el error SIN crear ningún run.
    pub async fn start_run(
        self: &Arc<Self>,
        goal: String,
        agent_id: String,
        conversation_id: Option<String>,
        parent_run_id: Option<String>,
        depth: i64,
    ) -> Result<AgentRun> {
        if goal.trim().is_empty() {
            return Err(NexusError::validation(
                "el objetivo del run no puede estar vacío",
            ));
        }
        if self.agents.get(&agent_id).is_none() {
            return Err(NexusError::not_found("agente", &agent_id));
        }
        let provider = self.providers.default_provider().await?;
        let now = now_str();
        let run = AgentRun {
            id: new_id(),
            conversation_id,
            agent_id,
            parent_run_id,
            goal,
            status: RunStatus::Running,
            result: None,
            error: None,
            iterations: 0,
            tokens_used: 0,
            created_at: now.clone(),
            started_at: Some(now),
            finished_at: None,
        };
        let run = repos::runs::create_run(&self.services.db, &run).await?;
        let token = CancellationToken::new();
        self.runs.lock().await.insert(
            run.id.clone(),
            RunHandle {
                token: token.clone(),
            },
        );
        let _ = self.events.send(AppEvent::RunStarted { run: run.clone() });
        let engine = self.clone();
        let spawn_run = run.clone();
        tokio::spawn(async move {
            let id = spawn_run.id.clone();
            let outcome = engine.execute_run(spawn_run, provider, depth, token).await;
            engine.runs.lock().await.remove(&id);
            if let Err(e) = outcome {
                tracing::error!("fallo interno inesperado en run {id}: {e}");
            }
        });
        Ok(run)
    }

    pub async fn cancel_run(&self, run_id: &str) -> Result<()> {
        let mut run = repos::runs::get_run(&self.services.db, run_id).await?;
        if matches!(
            run.status,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        ) {
            return Err(NexusError::Conflict("la ejecución ya terminó".into()));
        }
        if let Some(handle) = self.runs.lock().await.get(run_id) {
            handle.token.cancel();
        }
        for child in repos::runs::list_children(&self.services.db, run_id).await? {
            if let Some(handle) = self.runs.lock().await.get(&child.id) {
                handle.token.cancel();
            }
        }
        run.status = RunStatus::Cancelled;
        run.finished_at = Some(now_str());
        repos::runs::update_run(&self.services.db, &run).await?;
        self.notify_run_approvals(run_id).await;
        let _ = self.events.send(AppEvent::RunFinished { run });
        Ok(())
    }

    pub async fn resolve_approval(&self, approval_id: &str, approved: bool) -> Result<Approval> {
        let approval = repos::runs::get_approval(&self.services.db, approval_id).await?;
        if approval.status != ApprovalStatus::Pending {
            return Err(NexusError::Conflict("la aprobación ya fue decidida".into()));
        }
        let status = if approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Rejected
        };
        let approval = repos::runs::decide_approval(&self.services.db, approval_id, status).await?;
        let _ = self.events.send(AppEvent::ApprovalResolved {
            approval: approval.clone(),
        });
        if let Some(notify) = self.approvals.lock().await.get(approval_id) {
            notify.notify_waiters();
        }
        Ok(approval)
    }

    pub async fn run_detail(&self, run_id: &str) -> Result<RunDetail> {
        let run = repos::runs::get_run(&self.services.db, run_id).await?;
        let tool_calls = repos::runs::list_tool_calls(&self.services.db, run_id).await?;
        let approvals = repos::runs::list_run_approvals(&self.services.db, run_id).await?;
        let children = repos::runs::list_children(&self.services.db, run_id).await?;
        Ok(RunDetail {
            run,
            tool_calls,
            approvals,
            children,
        })
    }

    pub async fn active_runs(&self) -> Result<Vec<AgentRun>> {
        repos::runs::list_active_runs(&self.services.db).await
    }

    /// Al arrancar: los runs activos de procesos anteriores no tienen tarea
    /// viva que los ejecute; se marcan como fallidos con un error claro.
    pub async fn recover_stale_runs(&self) -> Result<()> {
        for mut run in repos::runs::list_active_runs(&self.services.db).await? {
            run.status = RunStatus::Failed;
            run.error = Some("interrumpida por reinicio".into());
            run.finished_at = Some(now_str());
            repos::runs::update_run(&self.services.db, &run).await?;
            let _ = self.events.send(AppEvent::RunFinished { run });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Bucle de ejecución
    // -----------------------------------------------------------------------

    async fn execute_run(
        &self,
        mut run: AgentRun,
        provider: Arc<dyn AiProvider>,
        depth: i64,
        token: CancellationToken,
    ) -> Result<()> {
        let limits = self
            .services
            .settings
            .get_app()
            .await
            .unwrap_or_default()
            .limits;
        let outcome = self
            .run_loop(&mut run, provider, depth, &token, &limits)
            .await;
        match outcome {
            Ok(()) => Ok(()),
            Err(NexusError::Cancelled) => self.finish_cancelled(&mut run).await,
            Err(e) => {
                self.finish(&mut run, RunStatus::Failed, None, Some(e.to_string()))
                    .await
            }
        }
    }

    fn run_loop<'a>(
        &'a self,
        run: &'a mut AgentRun,
        provider: Arc<dyn AiProvider>,
        depth: i64,
        token: &'a CancellationToken,
        limits: &'a UsageLimits,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let spec = self
                .agents
                .get(&run.agent_id)
                .ok_or_else(|| NexusError::not_found("agente", &run.agent_id))?;
            let max_iterations = spec.def.max_iterations.min(limits.max_iterations).max(1);
            let tool_specs = self.tool_specs(&spec.def.tools);
            let mut messages = self.build_context(run, &spec.system_prompt).await?;
            messages.push(AiMessage::user(run.goal.clone()));

            loop {
                if token.is_cancelled() {
                    return Err(NexusError::Cancelled);
                }
                if run.iterations >= max_iterations {
                    return self
                        .summarize_and_finish(run, &provider, &mut messages, token)
                        .await;
                }
                run.iterations += 1;
                let req = ChatRequest {
                    messages: messages.clone(),
                    tools: tool_specs.clone(),
                    ..Default::default()
                };
                let resp = self.chat_with_retries(&provider, req, run, token).await?;
                run.tokens_used += resp.usage.total();
                repos::runs::update_run(&self.services.db, run).await?;
                let _ = self.events.send(AppEvent::RunUpdated { run: run.clone() });

                if run.tokens_used > limits.max_tokens_per_run {
                    let result = format!(
                        "Ejecución detenida: se alcanzó el límite de tokens por ejecución ({} usados de {}). \
                         Último contenido parcial:\n{}",
                        run.tokens_used,
                        limits.max_tokens_per_run,
                        truncate(&resp.content, RESULT_PREVIEW_CHARS)
                    );
                    return self.complete_with_message(run, result).await;
                }

                if resp.tool_calls.is_empty() {
                    return self.complete_with_message(run, resp.content.clone()).await;
                }

                messages.push(AiMessage::assistant_tool_calls(
                    resp.content.clone(),
                    resp.tool_calls.clone(),
                ));
                for call in &resp.tool_calls {
                    if token.is_cancelled() {
                        return Err(NexusError::Cancelled);
                    }
                    let content = self
                        .process_tool_call(run, call, depth, limits, token)
                        .await?;
                    messages.push(AiMessage::tool_result(call.id.clone(), content));
                }
            }
        })
    }

    /// Límite de iteraciones alcanzado: una última llamada SIN herramientas
    /// pidiendo el resumen final; si falla, se completa con mensaje de límite.
    async fn summarize_and_finish(
        &self,
        run: &mut AgentRun,
        provider: &Arc<dyn AiProvider>,
        messages: &mut Vec<AiMessage>,
        token: &CancellationToken,
    ) -> Result<()> {
        messages.push(AiMessage::user(
            "Has alcanzado el límite de iteraciones de esta ejecución. No llames a más herramientas. \
             Resume en español qué lograste, qué quedó pendiente y los próximos pasos sugeridos.",
        ));
        let req = ChatRequest {
            messages: messages.clone(),
            tools: Vec::new(),
            ..Default::default()
        };
        let result = match self.chat_with_retries(provider, req, run, token).await {
            Ok(resp) => {
                run.tokens_used += resp.usage.total();
                if resp.content.trim().is_empty() {
                    "Ejecución finalizada al alcanzar el límite de iteraciones.".to_string()
                } else {
                    resp.content
                }
            }
            Err(NexusError::Cancelled) => return Err(NexusError::Cancelled),
            Err(e) => format!(
                "Ejecución finalizada al alcanzar el límite de iteraciones; no se pudo generar el resumen final: {e}"
            ),
        };
        self.complete_with_message(run, result).await
    }

    async fn chat_with_retries(
        &self,
        provider: &Arc<dyn AiProvider>,
        req: ChatRequest,
        run: &AgentRun,
        token: &CancellationToken,
    ) -> Result<ChatResponse> {
        let mut attempt = 0usize;
        loop {
            if token.is_cancelled() {
                return Err(NexusError::Cancelled);
            }
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            let conversation_id = run.conversation_id.clone();
            let run_id = run.id.clone();
            let events = self.events.clone();
            let forward = tokio::spawn(async move {
                while let Some(delta) = rx.recv().await {
                    if let Some(cid) = &conversation_id {
                        let _ = events.send(AppEvent::MessageDelta {
                            conversation_id: cid.clone(),
                            run_id: run_id.clone(),
                            delta,
                        });
                    }
                }
            });
            let chat = provider.chat_stream(req.clone(), tx);
            tokio::pin!(chat);
            let result = tokio::select! {
                r = &mut chat => r,
                _ = token.cancelled() => Err(NexusError::Cancelled),
            };
            let _ = forward.await;
            match result {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if token.is_cancelled() || matches!(e, NexusError::Cancelled) {
                        return Err(NexusError::Cancelled);
                    }
                    if attempt < RETRY_BACKOFFS.len() && nexus_ai::is_transient(&e) {
                        tracing::warn!(
                            "error transitorio del proveedor (intento {}): {e}",
                            attempt + 1
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(RETRY_BACKOFFS[attempt]) => {}
                            _ = token.cancelled() => return Err(NexusError::Cancelled),
                        }
                        attempt += 1;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Herramientas
    // -----------------------------------------------------------------------

    fn tool_specs(&self, allowlist: &[String]) -> Vec<ToolSpec> {
        let mut specs = self.tools.specs(allowlist);
        if allowlist.iter().any(|t| t == "agents.delegate") {
            specs.push(delegate_spec());
        }
        specs
    }

    async fn process_tool_call(
        &self,
        run: &mut AgentRun,
        call: &ToolCallRequest,
        depth: i64,
        limits: &UsageLimits,
        token: &CancellationToken,
    ) -> Result<String> {
        if call.name == "agents.delegate" {
            return self.handle_delegate(run, call, depth, limits, token).await;
        }

        let mut row = new_tool_call(&run.id, &call.name, call.arguments.clone());
        let allowed = self
            .agents
            .get(&run.agent_id)
            .map(|a| a.def.tools.iter().any(|t| t == &call.name))
            .unwrap_or(false);
        let tool = if allowed {
            self.tools.get(&call.name)
        } else {
            None
        };

        let (tool, validation) = match tool {
            Some(t) => {
                let v = nexus_tools::validate_args(&t.spec(), &call.arguments);
                (Some(t), v)
            }
            None => (None, Ok(())),
        };

        if !allowed || tool.is_none() {
            row.status = ToolCallStatus::Denied;
            row.error = Some("herramienta no permitida para este agente".into());
            let row = repos::runs::create_tool_call(&self.services.db, &row).await?;
            let _ = self.events.send(AppEvent::ToolFinished { call: row });
            return Ok(json_error("herramienta no permitida para este agente"));
        }
        let tool = match tool {
            Some(t) => t,
            None => return Ok(json_error("herramienta desconocida")),
        };
        if let Err(e) = validation {
            row.status = ToolCallStatus::Error;
            row.error = Some(e.to_string());
            let row = repos::runs::create_tool_call(&self.services.db, &row).await?;
            let _ = self.events.send(AppEvent::ToolFinished { call: row });
            return Ok(json_error(&e.to_string()));
        }

        let mut row = repos::runs::create_tool_call(&self.services.db, &row).await?;
        let _ = self
            .events
            .send(AppEvent::ToolStarted { call: row.clone() });

        let perms = self
            .services
            .settings
            .get_app()
            .await
            .unwrap_or_default()
            .permissions;
        if nexus_tools::needs_approval(tool.risk(), &perms) {
            match self
                .wait_for_approval(run, &mut row, tool.risk(), token)
                .await?
            {
                ApprovalOutcome::Approved => {}
                ApprovalOutcome::Rejected => {
                    return Ok(json_error("El usuario rechazó esta acción"));
                }
                ApprovalOutcome::Cancelled => return Err(NexusError::Cancelled),
            }
        }

        self.execute_tool(run, &mut row, tool, call, token).await
    }

    async fn wait_for_approval(
        &self,
        run: &mut AgentRun,
        row: &mut ToolCall,
        risk: Risk,
        token: &CancellationToken,
    ) -> Result<ApprovalOutcome> {
        row.status = ToolCallStatus::PendingApproval;
        repos::runs::update_tool_call(&self.services.db, row).await?;
        let approval = Approval {
            id: new_id(),
            run_id: run.id.clone(),
            tool_call_id: row.id.clone(),
            tool_name: row.tool_name.clone(),
            args: row.args.clone(),
            risk,
            status: ApprovalStatus::Pending,
            created_at: now_str(),
            decided_at: None,
        };
        let approval = repos::runs::create_approval(&self.services.db, &approval).await?;
        let notify = Arc::new(Notify::new());
        self.approvals
            .lock()
            .await
            .insert(approval.id.clone(), notify.clone());
        let _ = self.events.send(AppEvent::ApprovalRequested {
            approval: approval.clone(),
        });
        run.status = RunStatus::WaitingApproval;
        repos::runs::update_run(&self.services.db, run).await?;
        let _ = self.events.send(AppEvent::RunUpdated { run: run.clone() });

        let outcome = loop {
            if token.is_cancelled() {
                break ApprovalOutcome::Cancelled;
            }
            let notified = notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let current = repos::runs::get_approval(&self.services.db, &approval.id).await?;
            match current.status {
                ApprovalStatus::Approved => break ApprovalOutcome::Approved,
                ApprovalStatus::Rejected => break ApprovalOutcome::Rejected,
                ApprovalStatus::Pending => {}
            }
            tokio::select! {
                _ = notified => {}
                _ = token.cancelled() => break ApprovalOutcome::Cancelled,
            }
        };
        self.approvals.lock().await.remove(&approval.id);

        run.status = RunStatus::Running;
        repos::runs::update_run(&self.services.db, run).await?;
        let _ = self.events.send(AppEvent::RunUpdated { run: run.clone() });

        match outcome {
            ApprovalOutcome::Rejected => {
                row.status = ToolCallStatus::Denied;
                row.error = Some("El usuario rechazó esta acción".into());
                repos::runs::update_tool_call(&self.services.db, row).await?;
                let _ = self
                    .events
                    .send(AppEvent::ToolFinished { call: row.clone() });
            }
            ApprovalOutcome::Cancelled => {
                row.status = ToolCallStatus::Denied;
                row.error = Some("ejecución cancelada mientras esperaba aprobación".into());
                repos::runs::update_tool_call(&self.services.db, row).await?;
                let _ = self
                    .events
                    .send(AppEvent::ToolFinished { call: row.clone() });
            }
            ApprovalOutcome::Approved => {}
        }
        Ok(outcome)
    }

    async fn execute_tool(
        &self,
        run: &AgentRun,
        row: &mut ToolCall,
        tool: Arc<dyn nexus_tools::Tool>,
        call: &ToolCallRequest,
        token: &CancellationToken,
    ) -> Result<String> {
        let ctx = nexus_tools::ToolContext {
            services: self.services.clone(),
            run_id: run.id.clone(),
            agent_id: run.agent_id.clone(),
        };
        let start = std::time::Instant::now();
        let exec = tool.execute(call.arguments.clone(), &ctx);
        tokio::pin!(exec);
        let result = tokio::select! {
            r = tokio::time::timeout(TOOL_TIMEOUT, &mut exec) => r,
            _ = token.cancelled() => {
                row.status = ToolCallStatus::Error;
                row.error = Some("ejecución cancelada".into());
                row.duration_ms = Some(start.elapsed().as_millis() as i64);
                repos::runs::update_tool_call(&self.services.db, row).await?;
                let _ = self.events.send(AppEvent::ToolFinished { call: row.clone() });
                return Err(NexusError::Cancelled);
            }
        };
        row.duration_ms = Some(start.elapsed().as_millis() as i64);
        let content = match result {
            Ok(Ok(value)) => {
                row.status = ToolCallStatus::Ok;
                row.result = Some(value.clone());
                value.to_string()
            }
            Ok(Err(e)) => {
                row.status = ToolCallStatus::Error;
                row.error = Some(e.to_string());
                json_error(&e.to_string())
            }
            Err(_) => {
                let msg = format!(
                    "la herramienta superó el tiempo máximo de {} s",
                    TOOL_TIMEOUT.as_secs()
                );
                row.status = ToolCallStatus::Error;
                row.error = Some(msg.clone());
                json_error(&msg)
            }
        };
        repos::runs::update_tool_call(&self.services.db, row).await?;
        let _ = self
            .events
            .send(AppEvent::ToolFinished { call: row.clone() });
        Ok(content)
    }

    // -----------------------------------------------------------------------
    // Delegación
    // -----------------------------------------------------------------------

    async fn handle_delegate(
        &self,
        run: &mut AgentRun,
        call: &ToolCallRequest,
        depth: i64,
        limits: &UsageLimits,
        token: &CancellationToken,
    ) -> Result<String> {
        let mut row = new_tool_call(&run.id, "agents.delegate", call.arguments.clone());
        let allowed = self
            .agents
            .get(&run.agent_id)
            .map(|a| a.def.tools.iter().any(|t| t == "agents.delegate"))
            .unwrap_or(false);
        if !allowed {
            return self.deny_delegate(row).await;
        }
        let target = call
            .arguments
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let goal = call
            .arguments
            .get("goal")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let error = if target.is_empty() || goal.trim().is_empty() {
            Some("agents.delegate requiere 'agent_id' y 'goal' (strings no vacíos)".to_string())
        } else if target == "orchestrator" {
            Some("no se puede delegar en el orchestrator (recursión)".to_string())
        } else if self.agents.get(target).is_none() {
            Some(format!("el agente destino '{target}' no existe"))
        } else if depth >= limits.max_delegation_depth {
            Some(format!(
                "límite de profundidad de delegación alcanzado (máximo {})",
                limits.max_delegation_depth
            ))
        } else {
            None
        };
        if let Some(msg) = error {
            row.status = ToolCallStatus::Error;
            row.error = Some(msg.clone());
            let row = repos::runs::create_tool_call(&self.services.db, &row).await?;
            let _ = self.events.send(AppEvent::ToolFinished { call: row });
            return Ok(json_error(&msg));
        }

        let mut row = repos::runs::create_tool_call(&self.services.db, &row).await?;
        let _ = self
            .events
            .send(AppEvent::ToolStarted { call: row.clone() });
        let start = std::time::Instant::now();
        let child = self
            .execute_child(
                goal.to_string(),
                target.to_string(),
                run.id.clone(),
                depth + 1,
                token,
            )
            .await;
        row.duration_ms = Some(start.elapsed().as_millis() as i64);
        let content = match child {
            Ok(child) => {
                let value = serde_json::json!({
                    "run_id": child.id,
                    "agent_id": child.agent_id,
                    "status": child.status.as_str(),
                    "result": child.result,
                    "error": child.error,
                });
                if child.status == RunStatus::Completed {
                    row.status = ToolCallStatus::Ok;
                    row.result = Some(value.clone());
                } else {
                    row.status = ToolCallStatus::Error;
                    row.error = child.error.clone().or_else(|| {
                        Some(format!(
                            "el run hijo terminó en estado {}",
                            child.status.as_str()
                        ))
                    });
                }
                value.to_string()
            }
            Err(NexusError::Cancelled) => {
                row.status = ToolCallStatus::Error;
                row.error = Some("ejecución cancelada".into());
                repos::runs::update_tool_call(&self.services.db, &row).await?;
                let _ = self.events.send(AppEvent::ToolFinished { call: row });
                return Err(NexusError::Cancelled);
            }
            Err(e) => {
                row.status = ToolCallStatus::Error;
                row.error = Some(e.to_string());
                json_error(&e.to_string())
            }
        };
        repos::runs::update_tool_call(&self.services.db, &row).await?;
        let _ = self.events.send(AppEvent::ToolFinished { call: row });
        Ok(content)
    }

    async fn deny_delegate(&self, mut row: ToolCall) -> Result<String> {
        row.status = ToolCallStatus::Denied;
        row.error = Some("herramienta no permitida para este agente".into());
        let row = repos::runs::create_tool_call(&self.services.db, &row).await?;
        let _ = self.events.send(AppEvent::ToolFinished { call: row });
        Ok(json_error("herramienta no permitida para este agente"))
    }

    /// Crea y EJECUTA el run hijo de forma síncrona (await), con su propio
    /// token enlazado al del padre. Emite RunStarted/RunFinished normales.
    async fn execute_child(
        &self,
        goal: String,
        agent_id: String,
        parent_run_id: String,
        depth: i64,
        parent_token: &CancellationToken,
    ) -> Result<AgentRun> {
        let provider = self.providers.default_provider().await?;
        let now = now_str();
        let mut child = AgentRun {
            id: new_id(),
            conversation_id: None,
            agent_id,
            parent_run_id: Some(parent_run_id),
            goal,
            status: RunStatus::Running,
            result: None,
            error: None,
            iterations: 0,
            tokens_used: 0,
            created_at: now.clone(),
            started_at: Some(now),
            finished_at: None,
        };
        child = repos::runs::create_run(&self.services.db, &child).await?;
        let token = parent_token.child_token();
        self.runs.lock().await.insert(
            child.id.clone(),
            RunHandle {
                token: token.clone(),
            },
        );
        let _ = self
            .events
            .send(AppEvent::RunStarted { run: child.clone() });
        let limits = self
            .services
            .settings
            .get_app()
            .await
            .unwrap_or_default()
            .limits;
        let outcome = self
            .run_loop(&mut child, provider, depth, &token, &limits)
            .await;
        match outcome {
            Ok(()) => {}
            Err(NexusError::Cancelled) => {
                let _ = self.finish_cancelled(&mut child).await;
            }
            Err(e) => {
                let _ = self
                    .finish(&mut child, RunStatus::Failed, None, Some(e.to_string()))
                    .await;
            }
        }
        self.runs.lock().await.remove(&child.id);
        repos::runs::get_run(&self.services.db, &child.id).await
    }

    // -----------------------------------------------------------------------
    // Contexto y finalización
    // -----------------------------------------------------------------------

    async fn build_context(&self, run: &AgentRun, system_prompt: &str) -> Result<Vec<AiMessage>> {
        let app = self.services.settings.get_app().await.unwrap_or_default();
        let today = chrono::Utc::now().format("%Y-%m-%d");
        let mut system = format!("{system_prompt}\n\nFecha de hoy: {today}.");
        if !app.user_name.trim().is_empty() {
            system.push_str(&format!("\nNombre del usuario: {}.", app.user_name.trim()));
        }
        let mut messages = vec![AiMessage::system(system)];
        if let Some(cid) = &run.conversation_id {
            let history = repos::chat::list_messages(&self.services.db, cid).await?;
            let start = history.len().saturating_sub(HISTORY_MESSAGES);
            for m in &history[start..] {
                match m.role {
                    MessageRole::User => messages.push(AiMessage::user(m.content.clone())),
                    MessageRole::Assistant | MessageRole::Tool => {
                        if !m.content.trim().is_empty() {
                            messages.push(AiMessage::assistant(m.content.clone()));
                        }
                    }
                    MessageRole::System => {}
                }
            }
        }
        if let Some(parent_id) = &run.parent_run_id {
            if let Ok(parent) = repos::runs::get_run(&self.services.db, parent_id).await {
                let summary = parent
                    .result
                    .clone()
                    .or(parent.error)
                    .unwrap_or_else(|| "(sin resultado todavía)".into());
                messages.push(AiMessage::system(format!(
                    "Contexto del run padre (agente {}, objetivo: {}):\n{}",
                    parent.agent_id,
                    parent.goal,
                    truncate(&summary, RESULT_PREVIEW_CHARS)
                )));
            }
        }
        Ok(messages)
    }

    async fn complete_with_message(&self, run: &mut AgentRun, content: String) -> Result<()> {
        if let Some(cid) = &run.conversation_id {
            let message = repos::chat::add_message(
                &self.services.db,
                cid,
                MessageRole::Assistant,
                &content,
                Some(&run.agent_id),
                Some(&run.id),
                None,
                new_id(),
            )
            .await?;
            let _ = self.events.send(AppEvent::MessageCreated { message });
        }
        self.finish(run, RunStatus::Completed, Some(content), None)
            .await
    }

    async fn finish_cancelled(&self, run: &mut AgentRun) -> Result<()> {
        if let Ok(current) = repos::runs::get_run(&self.services.db, &run.id).await {
            if current.status == RunStatus::Cancelled {
                *run = current;
                self.record_run_activity(run).await;
                return Ok(());
            }
        }
        self.finish(run, RunStatus::Cancelled, None, None).await
    }

    async fn finish(
        &self,
        run: &mut AgentRun,
        status: RunStatus,
        result: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        run.status = status;
        run.result = result;
        run.error = error;
        run.finished_at = Some(now_str());
        repos::runs::update_run(&self.services.db, run).await?;
        let _ = self.events.send(AppEvent::RunFinished { run: run.clone() });
        self.record_run_activity(run).await;
        Ok(())
    }

    async fn record_run_activity(&self, run: &AgentRun) {
        let summary = match run.status {
            RunStatus::Completed => format!(
                "Ejecución completada ({}): {}",
                run.agent_id,
                truncate(&run.goal, 120)
            ),
            RunStatus::Cancelled => format!(
                "Ejecución cancelada ({}): {}",
                run.agent_id,
                truncate(&run.goal, 120)
            ),
            _ => format!(
                "Ejecución fallida ({}): {}",
                run.agent_id,
                truncate(run.error.as_deref().unwrap_or(&run.goal), 160)
            ),
        };
        if let Err(e) = repos::activity::record(
            &self.services.db,
            "agent_run",
            "run",
            Some(&run.id),
            &summary,
            &format!("agent:{}", run.agent_id),
            None,
        )
        .await
        {
            tracing::warn!("no se pudo registrar la actividad del run {}: {e}", run.id);
        }
    }

    async fn notify_run_approvals(&self, run_id: &str) {
        if let Ok(pending) = repos::runs::list_run_approvals(&self.services.db, run_id).await {
            let map = self.approvals.lock().await;
            for a in pending
                .iter()
                .filter(|a| a.status == ApprovalStatus::Pending)
            {
                if let Some(notify) = map.get(&a.id) {
                    notify.notify_waiters();
                }
            }
        }
    }
}

enum ApprovalOutcome {
    Approved,
    Rejected,
    Cancelled,
}

fn new_tool_call(run_id: &str, name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: new_id(),
        run_id: run_id.to_string(),
        tool_name: name.to_string(),
        args,
        result: None,
        status: ToolCallStatus::Pending,
        error: None,
        duration_ms: None,
        created_at: now_str(),
    }
}

fn delegate_spec() -> ToolSpec {
    ToolSpec {
        name: "agents.delegate".into(),
        description: "Delega un subobjetivo concreto en otro agente del equipo (planner, project_manager, knowledge, researcher, executor) y espera su resultado. No se puede delegar en orchestrator.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "string", "enum": ["planner", "project_manager", "knowledge", "researcher", "executor"] },
                "goal": { "type": "string", "description": "Subobjetivo claro y autocontenido para el agente destino" }
            },
            "required": ["agent_id", "goal"]
        }),
    }
}

fn json_error(msg: &str) -> String {
    serde_json::json!({ "error": truncate(msg, TOOL_ERROR_PREVIEW_CHARS) }).to_string()
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars).collect();
    format!("{cut}…")
}
