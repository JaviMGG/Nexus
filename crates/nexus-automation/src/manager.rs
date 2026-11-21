//! AutomationManager: registro CRUD, TriggerRegistry en caliente,
//! AutomationScheduler persistente y cola de ejecuciones con control de
//! concurrencia, deduplicación y recuperación tras reinicio.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use nexus_agents::{AgentRegistry, ExecutionEngine};
use nexus_ai::ProviderRegistry;
use nexus_core::{
    new_id, now, now_str, AppEvent, Automation, AutomationApprovalStatus, AutomationExecution,
    AutomationMode, AutomationOverview, CreateAutomation, ExecutionFilter, ExecutionPolicy,
    ExecutionStatus, NexusError, Result, Trigger, UpdateAutomation,
};
use nexus_db::repos;
use nexus_services::Services;
use serde_json::{json, Value};
use tokio::sync::{broadcast, watch, Mutex, Notify, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::flow;
use crate::triggers;

const TICK: Duration = Duration::from_secs(20);
const MAX_CONCURRENT: usize = 3;

struct ExecHandle {
    token: CancellationToken,
}

pub struct AutomationManager {
    pub services: Services,
    pub providers: Arc<ProviderRegistry>,
    pub engine: Arc<ExecutionEngine>,
    pub agents: Arc<AgentRegistry>,
    pub events: broadcast::Sender<AppEvent>,
    pub(crate) approval_notifies: Mutex<HashMap<String, watch::Sender<bool>>>,
    exec_handles: Mutex<HashMap<String, ExecHandle>>,
    semaphore: Arc<Semaphore>,
    wake: Arc<Notify>,
}

impl AutomationManager {
    pub fn new(
        services: Services,
        providers: Arc<ProviderRegistry>,
        engine: Arc<ExecutionEngine>,
        agents: Arc<AgentRegistry>,
        events: broadcast::Sender<AppEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            services,
            providers,
            engine,
            agents,
            events,
            approval_notifies: Mutex::new(HashMap::new()),
            exec_handles: Mutex::new(HashMap::new()),
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
            wake: Arc::new(Notify::new()),
        })
    }

    /// Arranca el bucle del scheduler y el consumidor de eventos de dominio.
    /// Debe llamarse una sola vez tras la recuperación de estado.
    pub fn start(self: &Arc<Self>) {
        let manager = self.clone();
        tokio::spawn(async move { manager.scheduler_loop().await });
        let manager = self.clone();
        let mut rx = self.events.subscribe();
        tokio::spawn(async move { manager.event_loop(&mut rx).await });
    }

    // -----------------------------------------------------------------------
    // CRUD + validación
    // -----------------------------------------------------------------------

    pub async fn list(&self, enabled_only: bool) -> Result<Vec<Automation>> {
        repos::automations::list(&self.services.db, enabled_only).await
    }

    pub async fn get(&self, id: &str) -> Result<Automation> {
        repos::automations::get(&self.services.db, id).await
    }

    fn known_agents(&self) -> Vec<String> {
        self.agents.list().into_iter().map(|a| a.id).collect()
    }

    fn validate_all(
        &self,
        trigger: &Trigger,
        conditions: Option<&nexus_core::Condition>,
        flow: &nexus_core::AutomationFlow,
    ) -> Result<()> {
        flow::validate_trigger(trigger)?;
        if let Some(c) = conditions {
            flow::validate_condition(c, 0)?;
        }
        flow::validate(flow, &self.known_agents())
    }

    pub async fn create(&self, data: CreateAutomation) -> Result<Automation> {
        let name = data.name.trim().to_string();
        if name.is_empty() || name.len() > 120 {
            return Err(NexusError::validation(
                "el nombre debe tener entre 1 y 120 caracteres",
            ));
        }
        let flow = data.flow.unwrap_or_default();
        self.validate_all(&data.trigger, data.conditions.as_ref(), &flow)?;
        let mut automation = Automation {
            id: new_id(),
            name,
            description: data.description,
            icon: if data.icon.is_empty() {
                "zap".into()
            } else {
                data.icon
            },
            enabled: data.enabled.unwrap_or(false),
            mode: data.mode.unwrap_or_default(),
            trigger: data.trigger,
            conditions: data.conditions,
            flow,
            permissions: data.permissions.unwrap_or_default(),
            limits: data.limits.unwrap_or_default(),
            policy: data.policy.unwrap_or_default(),
            template_id: data.template_id,
            last_run_at: None,
            next_run_at: None,
            run_count: 0,
            last_status: None,
            created_at: now_str(),
            updated_at: now_str(),
        };
        self.refresh_next_run(&mut automation).await?;
        let created = repos::automations::create(&self.services.db, &automation).await?;
        repos::activity::record(
            &self.services.db,
            "created",
            "automation",
            Some(&created.id),
            &format!("Automatización creada: {}", created.name),
            "user",
            None,
        )
        .await?;
        self.data_changed();
        self.wake.notify_one();
        Ok(created)
    }

    pub async fn update(&self, id: &str, patch: UpdateAutomation) -> Result<Automation> {
        let current = self.get(id).await?;
        let trigger = patch.trigger.clone().unwrap_or(current.trigger.clone());
        let conditions = match &patch.conditions {
            Some(c) => c.clone(),
            None => current.conditions.clone(),
        };
        let flow = patch.flow.clone().unwrap_or(current.flow.clone());
        self.validate_all(&trigger, conditions.as_ref(), &flow)?;
        if let Some(name) = &patch.name {
            if name.trim().is_empty() || name.len() > 120 {
                return Err(NexusError::validation(
                    "el nombre debe tener entre 1 y 120 caracteres",
                ));
            }
        }
        let mut updated = repos::automations::update(&self.services.db, id, patch).await?;
        self.refresh_next_run(&mut updated).await?;
        repos::automations::write(&self.services.db, &updated).await?;
        self.data_changed();
        self.wake.notify_one();
        Ok(updated)
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<Automation> {
        self.update(
            id,
            UpdateAutomation {
                enabled: Some(enabled),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let a = self.get(id).await?;
        repos::automations::soft_delete(&self.services.db, id).await?;
        repos::activity::record(
            &self.services.db,
            "deleted",
            "automation",
            Some(id),
            &format!("Automatización eliminada: {}", a.name),
            "user",
            None,
        )
        .await?;
        self.data_changed();
        Ok(())
    }

    async fn refresh_next_run(&self, a: &mut Automation) -> Result<()> {
        if !a.enabled {
            a.next_run_at = None;
            return Ok(());
        }
        if let Trigger::Schedule { spec } = &a.trigger {
            let settings = self.services.settings.get_app().await.unwrap_or_default();
            let last = a.last_run_at.as_deref().and_then(parse_ts);
            let next = triggers::next_occurrence(
                spec,
                now(),
                last,
                settings.automations.timezone.as_deref(),
            )?;
            a.next_run_at = next.map(|d| d.to_rfc3339());
        } else {
            a.next_run_at = None;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Ejecución manual / prueba
    // -----------------------------------------------------------------------

    pub async fn run_now(
        self: &Arc<Self>,
        id: &str,
        test_run: bool,
    ) -> Result<AutomationExecution> {
        let automation = self.get(id).await?;
        if automation.flow.steps.is_empty() {
            return Err(NexusError::validation(
                "la automatización no tiene pasos que ejecutar",
            ));
        }
        self.check_policy(&automation, true).await?;
        let detail = if test_run {
            "prueba manual".to_string()
        } else {
            "manual".to_string()
        };
        let execution = self
            .spawn_execution(automation, detail, json!({"manual": true}), test_run, 0)
            .await?;
        Ok(execution)
    }

    fn spawn_execution(
        self: &Arc<Self>,
        automation: Automation,
        trigger_detail: String,
        event_data: Value,
        test_run: bool,
        chain_depth: i64,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AutomationExecution>> + Send + '_>>
    {
        Box::pin(async move {
            // Registro síncrono del handle para que cancel sea posible al instante.
            let execution = AutomationExecution {
                id: new_id(),
                automation_id: automation.id.clone(),
                automation_name: automation.name.clone(),
                trigger_kind: automation.trigger.kind(),
                trigger_detail,
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
            let execution =
                repos::automations::create_execution(&self.services.db, &execution).await?;
            let token = CancellationToken::new();
            self.exec_handles.lock().await.insert(
                execution.id.clone(),
                ExecHandle {
                    token: token.clone(),
                },
            );
            self.emit_exec(&execution);
            let manager = self.clone();
            let exec_id = execution.id.clone();
            let detail = execution.trigger_detail.clone();
            tokio::spawn(async move {
                let _permit = manager.semaphore.acquire().await;
                let outcome = manager
                    .run_registered(
                        &exec_id,
                        automation,
                        detail,
                        event_data,
                        test_run,
                        chain_depth,
                        token,
                    )
                    .await;
                manager.exec_handles.lock().await.remove(&exec_id);
                if let Err(e) = outcome {
                    tracing::error!("fallo interno en ejecución {exec_id}: {e}");
                }
            });
            Ok(execution)
        })
    }

    /// Corre una ejecución ya registrada (creada por spawn_execution).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_registered(
        self: &Arc<Self>,
        execution_id: &str,
        automation: Automation,
        trigger_detail: String,
        event_data: Value,
        test_run: bool,
        chain_depth: i64,
        token: CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let execution_id = execution_id.to_string();
        Box::pin(async move {
            let execution_id = execution_id.as_str();
            let mut execution =
                repos::automations::get_execution(&self.services.db, execution_id).await?;
            execution.status = ExecutionStatus::Running;
            execution.started_at = Some(now_str());
            repos::automations::update_execution(&self.services.db, &execution).await?;
            self.emit_exec(&execution);
            let started = std::time::Instant::now();
            let outcome = self
                .run_flow(
                    &automation,
                    &mut execution,
                    event_data,
                    chain_depth,
                    &token,
                    started,
                )
                .await;
            match outcome {
                Ok(result) => {
                    if execution.status != ExecutionStatus::WaitingApproval {
                        execution.status = ExecutionStatus::Completed;
                    }
                    execution.result = Some(result);
                }
                Err(NexusError::Cancelled) => {
                    execution.status = ExecutionStatus::Cancelled;
                    execution.error = Some("cancelada por el usuario".into());
                }
                Err(e) => {
                    execution.status = ExecutionStatus::Failed;
                    execution.error = Some(e.to_string());
                    if automation.limits.retries > 0 && !test_run && !token.is_cancelled() {
                        let manager = self.clone();
                        let mut retried = automation.clone();
                        retried.limits.retries -= 1;
                        let detail = format!("reintento: {trigger_detail}");
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            let _ = manager
                                .spawn_execution(retried, detail, json!({}), false, chain_depth)
                                .await;
                        });
                    }
                }
            }
            self.finish_execution(&automation, &mut execution, started)
                .await
        })
    }

    pub async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        let execution = repos::automations::get_execution(&self.services.db, execution_id).await?;
        if matches!(
            execution.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            return Err(NexusError::Conflict("la ejecución ya terminó".into()));
        }
        if let Some(handle) = self.exec_handles.lock().await.get(execution_id) {
            handle.token.cancel();
        }
        // Cancela también los runs de agente asociados aún vivos.
        let logs = repos::automations::list_step_logs(&self.services.db, execution_id).await?;
        for log in logs {
            if let Some(run_id) = log
                .detail
                .as_ref()
                .and_then(|d| d.get("run_id"))
                .and_then(Value::as_str)
            {
                let _ = self.engine.cancel_run(run_id).await;
            }
        }
        self.notify_execution_approvals(execution_id, AutomationApprovalStatus::Rejected)
            .await;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Autorizaciones
    // -----------------------------------------------------------------------

    pub async fn approvals(
        &self,
        status: Option<AutomationApprovalStatus>,
    ) -> Result<Vec<nexus_core::AutomationApproval>> {
        repos::automations::list_approvals(&self.services.db, status).await
    }

    pub async fn decide_approval(
        &self,
        approval_id: &str,
        approved: bool,
    ) -> Result<nexus_core::AutomationApproval> {
        let approval = repos::automations::get_approval(&self.services.db, approval_id).await?;
        if approval.status != AutomationApprovalStatus::Pending {
            return Err(NexusError::Conflict(
                "la autorización ya fue decidida".into(),
            ));
        }
        let status = if approved {
            AutomationApprovalStatus::Approved
        } else {
            AutomationApprovalStatus::Rejected
        };
        let decided =
            repos::automations::decide_approval(&self.services.db, approval_id, status).await?;
        if let Some(notify) = self.approval_notifies.lock().await.get(approval_id) {
            let _ = notify.send(true);
        }
        self.data_changed();
        Ok(decided)
    }

    async fn notify_execution_approvals(
        &self,
        execution_id: &str,
        status: AutomationApprovalStatus,
    ) {
        let approvals =
            repos::automations::list_execution_approvals(&self.services.db, execution_id)
                .await
                .unwrap_or_default();
        for a in approvals {
            if a.status == AutomationApprovalStatus::Pending {
                let _ = repos::automations::decide_approval(&self.services.db, &a.id, status).await;
                if let Some(notify) = self.approval_notifies.lock().await.get(&a.id) {
                    let _ = notify.send(true);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Historial y overview
    // -----------------------------------------------------------------------

    pub async fn executions(&self, filter: ExecutionFilter) -> Result<Vec<AutomationExecution>> {
        repos::automations::list_executions(&self.services.db, &filter).await
    }

    pub async fn execution_detail(&self, id: &str) -> Result<nexus_core::ExecutionDetail> {
        crate::runner::execution_detail(&self.services, id).await
    }

    pub async fn overview(&self) -> Result<AutomationOverview> {
        let all = self.list(false).await?;
        let (total, ok, failed) = repos::automations::execution_stats(&self.services.db).await?;
        let day_start = now().format("%Y-%m-%dT00:00:00").to_string();
        let settings = self.services.settings.get_app().await.unwrap_or_default();
        let mut upcoming: Vec<Automation> = all
            .iter()
            .filter(|a| a.enabled && a.next_run_at.is_some())
            .cloned()
            .collect();
        upcoming.sort_by(|a, b| a.next_run_at.cmp(&b.next_run_at));
        upcoming.truncate(8);
        Ok(AutomationOverview {
            active_count: all.iter().filter(|a| a.enabled).count() as i64,
            total_count: all.len() as i64,
            executions_total: total,
            executions_ok: ok,
            executions_failed: failed,
            executions_today: repos::automations::executions_since(&self.services.db, &day_start)
                .await?,
            tokens_today: repos::automations::tokens_since(&self.services.db, Some(&day_start))
                .await?,
            tokens_total: repos::automations::tokens_since(&self.services.db, None).await?,
            pending_approvals: repos::automations::count_pending_approvals(&self.services.db)
                .await?,
            top_agents: crate::runner::top_agents(&self.services, 5).await?,
            recent_executions: self
                .executions(ExecutionFilter {
                    limit: Some(10),
                    ..Default::default()
                })
                .await?,
            upcoming,
            ai_status: self.providers.status().await,
            daily_token_limit: settings.automations.daily_token_limit,
        })
    }

    // -----------------------------------------------------------------------
    // Scheduler
    // -----------------------------------------------------------------------

    async fn scheduler_loop(self: Arc<Self>) {
        loop {
            if let Err(e) = self.tick().await {
                tracing::warn!("tick del scheduler falló: {e}");
            }
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = self.wake.notified() => {}
            }
        }
    }

    /// Un ciclo del scheduler: disparos temporales vencidos + tareas vencidas.
    /// Público para poder testearlo de forma determinista.
    pub async fn tick(self: &Arc<Self>) -> Result<()> {
        let settings = self.services.settings.get_app().await.unwrap_or_default();
        if !settings.automations.enabled {
            return Ok(());
        }
        let automations = self.list(true).await?;
        let now_utc = now();
        for mut a in automations {
            match &a.trigger {
                Trigger::Schedule { spec } => {
                    let Some(next) = a.next_run_at.as_deref().and_then(parse_ts) else {
                        self.refresh_next_run(&mut a).await?;
                        repos::automations::write(&self.services.db, &a).await?;
                        continue;
                    };
                    if next > now_utc {
                        continue;
                    }
                    // Política de recuperación: si el disparo quedó muy atrás
                    // (NEXUS cerrado), se descarta y se reprograma; nunca se
                    // ejecutan en tromba todas las ocurrencias perdidas.
                    let stale = now_utc - next > chrono::Duration::minutes(10);
                    let mark = triggers::schedule_mark(next);
                    let fresh =
                        repos::automations::try_mark_fired(&self.services.db, &a.id, &mark).await?;
                    let new_next = triggers::next_occurrence(
                        spec,
                        now_utc,
                        Some(next),
                        settings.automations.timezone.as_deref(),
                    )?;
                    a.next_run_at = new_next.map(|d| d.to_rfc3339());
                    if stale || !fresh {
                        repos::automations::write(&self.services.db, &a).await?;
                        continue;
                    }
                    if self.check_policy(&a, false).await.is_err() {
                        repos::automations::write(&self.services.db, &a).await?;
                        continue;
                    }
                    a.last_run_at = Some(next.to_rfc3339());
                    repos::automations::write(&self.services.db, &a).await?;
                    let manager = self.clone();
                    let auto = a.clone();
                    tokio::spawn(async move {
                        let _ = manager
                            .spawn_execution(
                                auto,
                                format!("programada {mark}"),
                                json!({"scheduled_at": mark}),
                                false,
                                0,
                            )
                            .await;
                    });
                }
                Trigger::Event { event, .. } if event == "task.overdue" => {
                    // Detector determinista de vencimientos (sin IA).
                    let today = now_utc.format("%Y-%m-%d").to_string();
                    let tasks = self
                        .services
                        .tasks
                        .list(nexus_core::TaskFilter {
                            due_before: Some(today.clone()),
                            ..Default::default()
                        })
                        .await?;
                    for t in tasks {
                        if !matches!(
                            t.status,
                            nexus_core::TaskStatus::Todo | nexus_core::TaskStatus::InProgress
                        ) {
                            continue;
                        }
                        let mark = format!("overdue:{}:{today}", t.id);
                        if !repos::automations::try_mark_fired(&self.services.db, &a.id, &mark)
                            .await?
                        {
                            continue;
                        }
                        if self.check_policy(&a, false).await.is_err() {
                            continue;
                        }
                        let data = serde_json::to_value(&t).unwrap_or(json!({}));
                        let _ = self.events.send(AppEvent::DomainEvent {
                            event: "task.overdue".into(),
                            entity: "task".into(),
                            entity_id: Some(t.id.clone()),
                            data,
                        });
                    }
                }
                _ => {}
            }
        }
        // Poda de marcas antiguales y autorizaciones caducadas (>48 h).
        let cutoff = (now_utc - chrono::Duration::days(7)).to_rfc3339();
        let _ = repos::automations::prune_marks(&self.services.db, &cutoff).await;
        let cutoff = (now_utc - chrono::Duration::hours(48)).to_rfc3339();
        let _ = repos::automations::expire_stale_approvals(&self.services.db, &cutoff).await;
        Ok(())
    }

    async fn check_policy(&self, a: &Automation, manual: bool) -> Result<()> {
        if manual {
            return Ok(());
        }
        match a.policy {
            ExecutionPolicy::Always => Ok(()),
            ExecutionPolicy::SkipIfActive => {
                let active = repos::automations::count_active_for(&self.services.db, &a.id).await?;
                if active > 0 {
                    Err(NexusError::Conflict("ya hay una ejecución activa".into()))
                } else {
                    Ok(())
                }
            }
            ExecutionPolicy::Cooldown => {
                let cooldown = a.limits.cooldown_minutes.max(1);
                if let Some(last) = a.last_run_at.as_deref().and_then(parse_ts) {
                    if now() - last < chrono::Duration::minutes(cooldown) {
                        return Err(NexusError::Conflict("en periodo de enfriamiento".into()));
                    }
                }
                let active = repos::automations::count_active_for(&self.services.db, &a.id).await?;
                if active > 0 {
                    Err(NexusError::Conflict("ya hay una ejecución activa".into()))
                } else {
                    Ok(())
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Eventos de dominio
    // -----------------------------------------------------------------------

    async fn event_loop(self: Arc<Self>, rx: &mut broadcast::Receiver<AppEvent>) {
        loop {
            match rx.recv().await {
                Ok(AppEvent::DomainEvent {
                    event,
                    entity_id,
                    data,
                    ..
                }) => {
                    if let Err(e) = self
                        .handle_domain_event(&event, entity_id.as_deref(), &data)
                        .await
                    {
                        tracing::warn!("error procesando evento {event}: {e}");
                    }
                }
                Ok(AppEvent::RunFinished { run }) => {
                    let data = json!({
                        "id": run.id,
                        "agent_id": run.agent_id,
                        "status": run.status,
                        "goal": run.goal,
                        "result": run.result,
                    });
                    let _ = self
                        .handle_domain_event("agent.finished", Some(&run.id), &data)
                        .await;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("automation event loop perdió {n} eventos");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    async fn handle_domain_event(
        self: &Arc<Self>,
        event: &str,
        entity_id: Option<&str>,
        data: &Value,
    ) -> Result<()> {
        let settings = self.services.settings.get_app().await.unwrap_or_default();
        if !settings.automations.enabled {
            return Ok(());
        }
        let automations = self.list(true).await?;
        for a in automations {
            if !triggers::event_matches(&a.trigger, event, data) {
                continue;
            }
            // Deduplicación por evento+entidad en ventana corta.
            let dedup_key = format!("evt:{event}:{}", entity_id.unwrap_or("-"));
            if a.limits.cooldown_minutes == 0 {
                let recent = repos::automations::count_active_for(&self.services.db, &a.id).await?;
                if recent > 0 && a.policy != ExecutionPolicy::Always {
                    continue;
                }
            }
            if self.check_policy(&a, false).await.is_err() {
                continue;
            }
            let mark = format!("{dedup_key}:{}", now().format("%Y-%m-%dT%H:%M"));
            if !repos::automations::try_mark_fired(&self.services.db, &a.id, &mark).await? {
                continue;
            }
            let manager = self.clone();
            let auto = a.clone();
            let detail = format!("evento {event}");
            let payload = data.clone();
            tokio::spawn(async move {
                let _ = manager
                    .spawn_execution(auto, detail, payload, false, 0)
                    .await;
            });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Recuperación tras reinicio
    // -----------------------------------------------------------------------

    /// Política de recuperación: las ejecuciones interrumpidas se marcan como
    /// fallidas (no se relanzan a ciegas), las autorizaciones pendientes se
    /// conservan 48 h y los horarios se recalculan desde ahora.
    pub async fn recover(&self) -> Result<()> {
        for mut e in repos::automations::list_active_executions(&self.services.db).await? {
            e.status = ExecutionStatus::Failed;
            e.error = Some("interrumpida por reinicio de NEXUS".into());
            e.finished_at = Some(now_str());
            repos::automations::update_execution(&self.services.db, &e).await?;
        }
        for mut a in self.list(false).await? {
            self.refresh_next_run(&mut a).await?;
            repos::automations::write(&self.services.db, &a).await?;
        }
        Ok(())
    }

    fn data_changed(&self) {
        let _ = self.events.send(AppEvent::DataChanged {
            entity: "automation".into(),
        });
        let _ = self.events.send(AppEvent::DataChanged {
            entity: "activity".into(),
        });
    }

    /// Modo de una automatización para el frontend (etiquetas de disponibilidad).
    pub async fn mode_availability(&self, mode: AutomationMode) -> Value {
        let available = self.providers.default_provider().await.is_ok();
        json!({
            "mode": mode,
            "requires_ai": mode != AutomationMode::Local,
            "requires_internet": mode == AutomationMode::Ai,
            "ai_available": available,
            "will_run_offline": mode != AutomationMode::Ai,
        })
    }
}

pub(crate) fn parse_ts(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&chrono::Utc))
}
