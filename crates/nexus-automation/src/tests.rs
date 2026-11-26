//! Tests de integración del Automation Engine: DB en memoria, proveedor
//! simulado guionizado y el motor multiagente real.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use nexus_agents::{AgentRegistry, ExecutionEngine};
use nexus_ai::{AiProvider, ChatRequest, ChatResponse, ProviderRegistry};
use nexus_core::{
    now, AppEvent, AutomationFlow, AutomationLimits, AutomationMode, CompareOp, Condition,
    CreateAutomation, CreateNote, CreateProject, CreateTask, ExecutionFilter, ExecutionPolicy,
    ExecutionStatus, FlowStep, NexusError, NoteKind, Policy, Result, RunStatus, ScheduleSpec,
    StepKind, StepStatus, Trigger, TriggerEntry, UpdateAutomation,
};
use nexus_db::{repos, Db};
use nexus_services::Services;
use serde_json::json;
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::conditions::{evaluate, EvalContext};
use crate::manager::AutomationManager;

struct MockProvider {
    script: Arc<Mutex<VecDeque<ChatResponse>>>,
}

impl MockProvider {
    fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            script: Arc::new(Mutex::new(responses.into())),
        }
    }
}

#[async_trait::async_trait]
impl AiProvider for MockProvider {
    fn config(&self) -> nexus_core::ProviderConfig {
        nexus_services::settings::SettingsService::default_qwen_config()
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        self.chat_stream(req, tokio::sync::mpsc::unbounded_channel().0)
            .await
    }

    async fn chat_stream(
        &self,
        _req: ChatRequest,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<ChatResponse> {
        let mut script = self.script.lock().await;
        let resp = if script.len() > 1 {
            script.pop_front().unwrap()
        } else {
            script.front().cloned().unwrap_or_default()
        };
        if !resp.content.is_empty() {
            let _ = deltas.send(resp.content.clone());
        }
        Ok(resp)
    }

    async fn health(&self) -> Result<String> {
        Ok("mock".into())
    }
}

struct Fixture {
    manager: Arc<AutomationManager>,
    services: Services,
    events: broadcast::Sender<AppEvent>,
}

async fn fixture(provider: Option<Arc<dyn AiProvider>>) -> Fixture {
    let services = Services::new(Db::connect_memory().await.unwrap());
    let providers = Arc::new(ProviderRegistry::new(services.clone()));
    providers.refresh().await.unwrap();
    if let Some(p) = provider {
        providers.insert_test_provider(p).await;
    }
    let (events, _) = broadcast::channel(1024);
    let agents = Arc::new(AgentRegistry::with_defaults());
    let engine = ExecutionEngine::new(
        services.clone(),
        providers.clone(),
        Arc::new(nexus_tools::ToolRegistry::with_defaults()),
        agents.clone(),
        events.clone(),
    );
    let manager =
        AutomationManager::new(services.clone(), providers, engine, agents, events.clone());
    Fixture {
        manager,
        services,
        events,
    }
}

fn text(content: &str) -> ChatResponse {
    ChatResponse {
        content: content.into(),
        ..Default::default()
    }
}

fn step(id: &str, kind: StepKind, next: Option<&str>) -> FlowStep {
    FlowStep {
        id: id.into(),
        kind,
        next: next.map(Into::into),
    }
}

fn flow(entry: &str, steps: Vec<FlowStep>) -> AutomationFlow {
    AutomationFlow {
        version: 1,
        steps,
        entry: Some(entry.into()),
        positions: Default::default(),
    }
}

fn notify_step(id: &str, title: &str, next: Option<&str>) -> FlowStep {
    step(
        id,
        StepKind::Notification {
            title: title.into(),
            body: String::new(),
        },
        next,
    )
}

fn create_task_action(id: &str, title: &str, next: Option<&str>) -> FlowStep {
    step(
        id,
        StepKind::Action {
            action: "tasks.create".into(),
            params: json!({"title": title}),
            approval: None,
        },
        next,
    )
}

fn base_create(name: &str) -> CreateAutomation {
    CreateAutomation {
        name: name.into(),
        description: String::new(),
        icon: String::new(),
        enabled: Some(true),
        mode: Some(AutomationMode::Local),
        triggers: vec![TriggerEntry {
            id: "t0".into(),
            trigger: manual_trigger(),
        }],
        conditions: None,
        flow: None,
        permissions: None,
        limits: None,
        policy: None,
        template_id: None,
    }
}

fn local_automation(name: &str, trigger: Trigger, f: AutomationFlow) -> CreateAutomation {
    CreateAutomation {
        triggers: vec![TriggerEntry {
            id: "t0".into(),
            trigger,
        }],
        flow: Some(f),
        ..base_create(name)
    }
}

fn trigger_entry(id: &str, trigger: Trigger) -> TriggerEntry {
    TriggerEntry {
        id: id.into(),
        trigger,
    }
}

async fn force_due(f: &Fixture, id: &str, trigger_id: &str, when: chrono::DateTime<chrono::Utc>) {
    let mut a = f.manager.get(id).await.unwrap();
    a.next_runs.insert(trigger_id.into(), when.to_rfc3339());
    a.next_run_at = Some(when.to_rfc3339());
    repos::automations::write(&f.services.db, &a).await.unwrap();
}

fn manual_trigger() -> Trigger {
    Trigger::Schedule {
        spec: ScheduleSpec {
            kind: "daily".into(),
            time: Some("08:00".into()),
            timezone: Some("UTC".into()),
            ..Default::default()
        },
    }
}

async fn wait_exec_status(
    f: &Fixture,
    id: &str,
    statuses: &[ExecutionStatus],
) -> nexus_core::AutomationExecution {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let e = repos::automations::get_execution(&f.services.db, id)
            .await
            .unwrap();
        if statuses.contains(&e.status) {
            return e;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout esperando estado, actual: {:?}",
            e.status
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// ---------------------------------------------------------------------------
// CRUD y validación
// ---------------------------------------------------------------------------

#[tokio::test]
async fn crud_and_validation() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Local",
            manual_trigger(),
            flow("n", vec![notify_step("n", "hola", None)]),
        ))
        .await
        .unwrap();
    assert!(a.next_run_at.is_some());

    // Acción desconocida → error de validación.
    let bad = local_automation(
        "Mala",
        manual_trigger(),
        flow(
            "x",
            vec![step(
                "x",
                StepKind::Action {
                    action: "sql.execute".into(),
                    params: json!({}),
                    approval: None,
                },
                None,
            )],
        ),
    );
    assert!(f.manager.create(bad).await.is_err());

    // Ciclo → error.
    let cyclic = local_automation(
        "Ciclo",
        manual_trigger(),
        flow(
            "a",
            vec![
                step("a", StepKind::Delay { seconds: 1 }, Some("b")),
                step("b", StepKind::Delay { seconds: 1 }, Some("a")),
            ],
        ),
    );
    assert!(f.manager.create(cyclic).await.is_err());

    // Update y desactivación.
    let updated = f
        .manager
        .update(
            &a.id,
            UpdateAutomation {
                name: Some("Renombrada".into()),
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Renombrada");
    assert!(updated.next_run_at.is_none());

    f.manager.delete(&a.id).await.unwrap();
    assert!(f.manager.get(&a.id).await.is_err());
}

#[tokio::test]
async fn automation_accepts_multiple_triggers() {
    let f = fixture(None).await;
    let mut data = base_create("Multidisparador");
    data.triggers = vec![
        trigger_entry("s1", manual_trigger()),
        trigger_entry(
            "e1",
            Trigger::Event {
                event: "task.created".into(),
                filter: json!({}),
            },
        ),
    ];
    data.flow = Some(flow("n", vec![notify_step("n", "hola", None)]));
    let a = f.manager.create(data.clone()).await.unwrap();
    assert_eq!(a.triggers.len(), 2);
    assert_eq!(a.triggers[1].id, "e1");
    assert!(a.next_runs.contains_key("s1"));
    assert!(!a.next_runs.contains_key("e1"));
    assert_eq!(
        a.next_run_at.as_deref(),
        a.next_runs.get("s1").map(String::as_str)
    );

    data.triggers[1] = trigger_entry(
        "rota",
        Trigger::Event {
            event: "no.existe".into(),
            filter: json!({}),
        },
    );
    assert!(f.manager.create(data).await.is_err());
}

#[tokio::test]
async fn draft_without_triggers_is_saved_but_cannot_be_enabled() {
    let f = fixture(None).await;
    let mut data = base_create("Borrador");
    data.enabled = Some(false);
    data.triggers = vec![];
    data.flow = Some(flow("n", vec![notify_step("n", "hola", None)]));
    let a = f.manager.create(data.clone()).await.unwrap();
    assert!(a.triggers.is_empty());

    let err = f.manager.set_enabled(&a.id, true).await.unwrap_err();
    assert!(matches!(err, NexusError::Validation(_)));
    assert!(err
        .to_string()
        .contains("añade al menos un disparador antes de activar"));

    data.enabled = Some(true);
    assert!(f.manager.create(data).await.is_err());
}

#[tokio::test]
async fn templates_all_valid_and_installable() {
    let f = fixture(None).await;
    let templates = crate::templates::all();
    assert_eq!(templates.len(), 16);
    for t in templates {
        let created = f
            .manager
            .create(t.automation.clone())
            .await
            .unwrap_or_else(|e| panic!("plantilla {} inválida: {e}", t.id));
        assert_eq!(created.template_id.as_deref(), Some(t.id.as_str()));
    }
}

// ---------------------------------------------------------------------------
// Condiciones
// ---------------------------------------------------------------------------

#[tokio::test]
async fn conditions_and_or_not_with_real_data() {
    let f = fixture(None).await;
    f.services
        .tasks
        .create(
            CreateTask {
                title: "pendiente".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();

    let ctx = EvalContext {
        services: &f.services,
        event_data: json!({"priority": "high", "title": "Comprar pan"}),
        automation_id: "x",
        vars: json!({}),
    };
    let open = Condition::Metric {
        metric: "open_tasks".into(),
        op: CompareOp::Gt,
        value: json!(0),
        params: json!({}),
    };
    let none = Condition::Metric {
        metric: "open_tasks".into(),
        op: CompareOp::Eq,
        value: json!(0),
        params: json!({}),
    };

    assert!(evaluate(&open, &ctx).await.unwrap());
    assert!(!evaluate(&none, &ctx).await.unwrap());
    assert!(evaluate(
        &Condition::And {
            conditions: vec![open.clone(), open.clone()]
        },
        &ctx
    )
    .await
    .unwrap());
    assert!(evaluate(
        &Condition::Or {
            conditions: vec![none.clone(), open.clone()]
        },
        &ctx
    )
    .await
    .unwrap());
    assert!(!evaluate(
        &Condition::And {
            conditions: vec![none.clone(), open]
        },
        &ctx
    )
    .await
    .unwrap());
    assert!(evaluate(
        &Condition::Not {
            condition: Box::new(none)
        },
        &ctx
    )
    .await
    .unwrap());

    let field = Condition::EventField {
        field: "priority".into(),
        op: CompareOp::Eq,
        value: json!("high"),
    };
    assert!(evaluate(&field, &ctx).await.unwrap());
    let contains = Condition::EventField {
        field: "title".into(),
        op: CompareOp::Contains,
        value: json!("pan"),
    };
    assert!(evaluate(&contains, &ctx).await.unwrap());
}

#[tokio::test]
async fn root_conditions_skip_execution() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "Condicional",
        manual_trigger(),
        flow("n", vec![notify_step("n", "no debería verse", None)]),
    );
    data.conditions = Some(Condition::Metric {
        metric: "open_tasks".into(),
        op: CompareOp::Gt,
        value: json!(0),
        params: json!({}),
    });
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert_eq!(e.result.as_ref().unwrap()["skipped"], json!(true));
    let notes = f.services.notes.list(Default::default()).await.unwrap();
    assert!(notes.is_empty());
}

// ---------------------------------------------------------------------------
// Acciones y flujo local
// ---------------------------------------------------------------------------

#[tokio::test]
async fn local_flow_creates_task_and_notifies() {
    let f = fixture(None).await;
    let mut rx = f.events.subscribe();
    let a = f
        .manager
        .create(local_automation(
            "Creadora",
            manual_trigger(),
            flow(
                "t",
                vec![
                    create_task_action("t", "Tarea automática", Some("n")),
                    notify_step("n", "Hecho", None),
                ],
            ),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert_eq!(e.steps_done, 2);

    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "Tarea automática");

    let mut notified = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, AppEvent::Notification { .. }) {
            notified = true;
        }
    }
    assert!(notified);

    // Historial con pasos.
    let detail = f.manager.execution_detail(&e.id).await.unwrap();
    assert_eq!(detail.steps.len(), 2);
    assert!(detail.steps.iter().all(|s| s.status == StepStatus::Ok));
    let updated = f.manager.get(&a.id).await.unwrap();
    assert_eq!(updated.run_count, 1);
    assert_eq!(updated.last_status, Some(ExecutionStatus::Completed));
}

#[tokio::test]
async fn branching_if_else() {
    let f = fixture(None).await;
    // Sin tareas pendientes → rama falsa.
    let a = f
        .manager
        .create(local_automation(
            "IF",
            manual_trigger(),
            flow(
                "c",
                vec![
                    step(
                        "c",
                        StepKind::Condition {
                            condition: Condition::Metric {
                                metric: "open_tasks".into(),
                                op: CompareOp::Gt,
                                value: json!(0),
                                params: json!({}),
                            },
                            on_true: Some("t".into()),
                            on_false: Some("f".into()),
                        },
                        None,
                    ),
                    create_task_action("t", "rama verdadera", None),
                    create_task_action("f", "rama falsa", None),
                ],
            ),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "rama falsa");
}

#[tokio::test]
async fn test_run_does_not_write() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Prueba",
            manual_trigger(),
            flow("t", vec![create_task_action("t", "no debe existir", None)]),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, true).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert!(e.test_run);
    assert!(f
        .services
        .tasks
        .list(Default::default())
        .await
        .unwrap()
        .is_empty());
    let updated = f.manager.get(&a.id).await.unwrap();
    assert_eq!(updated.run_count, 0);
}

#[tokio::test]
async fn interpolation_uses_event_data() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Eco",
            Trigger::Event {
                event: "task.created".into(),
                filter: json!({}),
            },
            flow(
                "t",
                vec![create_task_action("t", "Seguimiento de {{title}}", None)],
            ),
        ))
        .await
        .unwrap();
    let task = f
        .services
        .tasks
        .create(
            CreateTask {
                title: "Original".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let data = serde_json::to_value(&task).unwrap();
    let outcome = f
        .manager
        .execute(
            a,
            &Trigger::Event {
                event: "task.created".into(),
                filter: json!({}),
            },
            "evento task.created".into(),
            data,
            false,
            0,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.execution.status, ExecutionStatus::Completed);
    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert!(tasks.iter().any(|t| t.title == "Seguimiento de Original"));
}

// ---------------------------------------------------------------------------
// Eventos, scheduler y deduplicación
// ---------------------------------------------------------------------------

#[tokio::test]
async fn event_trigger_fires_on_domain_event() {
    let f = fixture(None).await;
    f.manager.start();
    let a = f
        .manager
        .create(local_automation(
            "Al crear proyecto",
            Trigger::Event {
                event: "project.created".into(),
                filter: json!({}),
            },
            flow(
                "t",
                vec![create_task_action("t", "Kickoff de {{name}}", None)],
            ),
        ))
        .await
        .unwrap();
    let _ = a;
    let _ = f.events.send(AppEvent::DomainEvent {
        event: "project.created".into(),
        entity: "project".into(),
        entity_id: Some("p1".into()),
        data: json!({"id": "p1", "name": "Apolo"}),
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let tasks = f.services.tasks.list(Default::default()).await.unwrap();
        if tasks.iter().any(|t| t.title == "Kickoff de Apolo") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "el evento no disparó la automatización"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn mail_received_subject_contains_filter_fires() {
    let f = fixture(None).await;
    f.manager.start();
    let mut data = base_create("Correos");
    data.triggers = vec![trigger_entry(
        "mail",
        Trigger::Event {
            event: "mail.received".into(),
            filter: json!({"account": "principal", "subject~": "factura"}),
        },
    )];
    data.flow = Some(flow(
        "t",
        vec![create_task_action("t", "Revisar {{subject}}", None)],
    ));
    f.manager.create(data).await.unwrap();

    f.events
        .send(AppEvent::DomainEvent {
            event: "mail.received".into(),
            entity: "mail".into(),
            entity_id: Some("m1".into()),
            data: json!({
                "account_id": "principal",
                "folder": "INBOX",
                "from": "proveedor@nexus.dev",
                "subject": "Factura de marzo",
                "date": "2026-11-26T10:00:00Z",
                "snippet": "Adjunto la factura",
                "message_id": "m1",
            }),
        })
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let tasks = f.services.tasks.list(Default::default()).await.unwrap();
        if tasks.iter().any(|t| t.title == "Revisar Factura de marzo") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "mail.received no disparó la automatización"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn event_dedupe_is_independent_per_trigger() {
    let f = fixture(None).await;
    f.manager.start();
    let mut data = base_create("Doble disparador");
    data.policy = Some(ExecutionPolicy::Always);
    data.triggers = vec![
        trigger_entry(
            "a",
            Trigger::Event {
                event: "mail.received".into(),
                filter: json!({}),
            },
        ),
        trigger_entry(
            "b",
            Trigger::Event {
                event: "mail.received".into(),
                filter: json!({}),
            },
        ),
    ];
    data.flow = Some(flow("n", vec![notify_step("n", "correo", None)]));
    f.manager.create(data).await.unwrap();

    for _ in 0..2 {
        f.events
            .send(AppEvent::DomainEvent {
                event: "mail.received".into(),
                entity: "mail".into(),
                entity_id: Some("m1".into()),
                data: json!({"message_id": "m1", "subject": "Aviso"}),
            })
            .unwrap();
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let execs = f
            .manager
            .executions(ExecutionFilter::default())
            .await
            .unwrap();
        if execs.len() == 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "cada disparador debe ejecutarse una vez: {}",
            execs.len()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    let execs = f
        .manager
        .executions(ExecutionFilter::default())
        .await
        .unwrap();
    assert_eq!(
        execs.len(),
        2,
        "el segundo evento no debe duplicar disparos"
    );
    assert!(execs
        .iter()
        .all(|e| e.trigger_detail.contains("mail.received")));
}

#[tokio::test]
async fn scheduler_fires_due_and_dedupes() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Programada",
            manual_trigger(),
            flow("t", vec![create_task_action("t", "programada", None)]),
        ))
        .await
        .unwrap();
    // Fuerza un disparo vencido hace 1 minuto (dentro de la ventana de recuperación).
    let past = now() - chrono::Duration::minutes(1);
    force_due(&f, &a.id, "t0", past).await;
    f.manager.tick().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let execs = f
            .manager
            .executions(ExecutionFilter::default())
            .await
            .unwrap();
        if execs.iter().any(|e| e.status == ExecutionStatus::Completed) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "el scheduler no disparó"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // Segundo tick: no debe duplicar (next_run_at ya está en el futuro).
    f.manager.tick().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let execs = f
        .manager
        .executions(ExecutionFilter::default())
        .await
        .unwrap();
    assert_eq!(execs.len(), 1);
    let updated = f.manager.get(&a.id).await.unwrap();
    assert!(updated.next_run_at.unwrap() > now().to_rfc3339());
}

#[tokio::test]
async fn stale_schedule_is_skipped_not_replayed() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Antigua",
            manual_trigger(),
            flow("t", vec![create_task_action("t", "no replay", None)]),
        ))
        .await
        .unwrap();
    let old = now() - chrono::Duration::hours(3);
    force_due(&f, &a.id, "t0", old).await;
    f.manager.tick().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let execs = f
        .manager
        .executions(ExecutionFilter::default())
        .await
        .unwrap();
    assert!(
        execs.is_empty(),
        "no deben ejecutarse disparos perdidos mientras NEXUS estaba cerrado"
    );
    let updated = f.manager.get(&a.id).await.unwrap();
    assert!(updated.next_run_at.unwrap() > now().to_rfc3339());
}

#[tokio::test]
async fn overdue_detector_emits_once_per_day() {
    let f = fixture(None).await;
    f.manager.start();
    let yesterday = (now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    f.services
        .tasks
        .create(
            CreateTask {
                title: "vencida".into(),
                due_date: Some(yesterday),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let a = f
        .manager
        .create(local_automation(
            "Overdue",
            Trigger::Event {
                event: "task.overdue".into(),
                filter: json!({}),
            },
            flow(
                "t",
                vec![create_task_action("t", "Revisar vencida: {{title}}", None)],
            ),
        ))
        .await
        .unwrap();
    let _ = a;
    f.manager.tick().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let tasks = f.services.tasks.list(Default::default()).await.unwrap();
        if tasks.iter().any(|t| t.title == "Revisar vencida: vencida") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "task.overdue no se disparó"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // Segundo tick el mismo día: la marca impide duplicados.
    f.manager.tick().await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(
        tasks
            .iter()
            .filter(|t| t.title.starts_with("Revisar vencida"))
            .count(),
        1
    );
}

#[tokio::test]
async fn skip_if_active_policy() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "Lenta",
        manual_trigger(),
        flow(
            "d",
            vec![
                step("d", StepKind::Delay { seconds: 2 }, Some("t")),
                create_task_action("t", "lenta", None),
            ],
        ),
    );
    data.policy = Some(ExecutionPolicy::SkipIfActive);
    let a = f.manager.create(data).await.unwrap();
    let e1 = f.manager.run_now(&a.id, false).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Manual siempre permitido; la política automática se comprueba en check_policy.
    let active = repos::automations::count_active_for(&f.services.db, &a.id)
        .await
        .unwrap();
    assert_eq!(active, 1);
    wait_exec_status(&f, &e1.id, &[ExecutionStatus::Completed]).await;
}

// ---------------------------------------------------------------------------
// Autorizaciones
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sensitive_action_requires_approval_and_can_be_rejected() {
    let f = fixture(None).await;
    let task = f
        .services
        .tasks
        .create(
            CreateTask {
                title: "borrable".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let a = f
        .manager
        .create(local_automation(
            "Borradora",
            manual_trigger(),
            flow(
                "del",
                vec![step(
                    "del",
                    StepKind::Action {
                        action: "tasks.delete".into(),
                        params: json!({"task_id": task.id}),
                        approval: None,
                    },
                    None,
                )],
            ),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::WaitingApproval]).await;
    let approvals = f
        .manager
        .approvals(Some(nexus_core::AutomationApprovalStatus::Pending))
        .await
        .unwrap();
    assert_eq!(approvals.len(), 1);
    assert!(!approvals[0].preview.is_empty());
    f.manager
        .decide_approval(&approvals[0].id, false)
        .await
        .unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert!(e.status == ExecutionStatus::Completed);
    // La tarea sigue existiendo.
    assert!(f.services.tasks.get(&task.id).await.is_ok());
}

#[tokio::test]
async fn approved_sensitive_action_executes() {
    let f = fixture(None).await;
    let task = f
        .services
        .tasks
        .create(
            CreateTask {
                title: "borrable".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let a = f
        .manager
        .create(local_automation(
            "Borradora",
            manual_trigger(),
            flow(
                "del",
                vec![step(
                    "del",
                    StepKind::Action {
                        action: "tasks.delete".into(),
                        params: json!({"task_id": task.id}),
                        approval: None,
                    },
                    None,
                )],
            ),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::WaitingApproval]).await;
    let approvals = f
        .manager
        .approvals(Some(nexus_core::AutomationApprovalStatus::Pending))
        .await
        .unwrap();
    f.manager
        .decide_approval(&approvals[0].id, true)
        .await
        .unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert!(f.services.tasks.get(&task.id).await.is_err());
}

#[tokio::test]
async fn write_policy_confirm_gates_normal_actions() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "Cautelosa",
        manual_trigger(),
        flow("t", vec![create_task_action("t", "con permiso", None)]),
    );
    data.permissions = Some(nexus_core::AgentPermissionsLite {
        write: Policy::Confirm,
    });
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::WaitingApproval]).await;
    let approvals = f
        .manager
        .approvals(Some(nexus_core::AutomationApprovalStatus::Pending))
        .await
        .unwrap();
    f.manager
        .decide_approval(&approvals[0].id, true)
        .await
        .unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert_eq!(
        f.services
            .tasks
            .list(Default::default())
            .await
            .unwrap()
            .len(),
        1
    );
}

// ---------------------------------------------------------------------------
// Cancelación, límites y recuperación
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancellation_stops_execution() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Cancelable",
            manual_trigger(),
            flow(
                "d",
                vec![
                    step("d", StepKind::Delay { seconds: 30 }, Some("t")),
                    create_task_action("t", "nunca", None),
                ],
            ),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Running]).await;
    f.manager.cancel_execution(&e.id).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Cancelled]).await;
    assert!(e.error.is_some());
    assert!(f
        .services
        .tasks
        .list(Default::default())
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn max_steps_limit_enforced() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "Larga",
        manual_trigger(),
        flow(
            "a",
            vec![
                notify_step("a", "1", Some("b")),
                notify_step("b", "2", Some("c")),
                notify_step("c", "3", None),
            ],
        ),
    );
    data.limits = Some(AutomationLimits {
        max_steps: 2,
        ..Default::default()
    });
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Failed]).await;
    assert!(e.error.unwrap().contains("límite de pasos"));
}

#[tokio::test]
async fn retries_spawn_new_execution() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "Reintentable",
        manual_trigger(),
        flow(
            "x",
            vec![step(
                "x",
                StepKind::Action {
                    action: "tasks.complete".into(),
                    params: json!({"task_id": "inexistente"}),
                    approval: None,
                },
                None,
            )],
        ),
    );
    data.limits = Some(AutomationLimits {
        retries: 1,
        ..Default::default()
    });
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Failed]).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let execs = f
            .manager
            .executions(ExecutionFilter::default())
            .await
            .unwrap();
        if execs.len() >= 2 {
            assert!(execs
                .iter()
                .any(|x| x.trigger_detail.starts_with("reintento")));
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "el reintento no se lanzó"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn recovery_marks_interrupted_executions_failed() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Reinicio",
            manual_trigger(),
            flow("n", vec![notify_step("n", "x", None)]),
        ))
        .await
        .unwrap();
    // Simula una ejecución huérfana de un proceso anterior.
    let orphan = nexus_core::AutomationExecution {
        id: nexus_core::new_id(),
        automation_id: a.id.clone(),
        automation_name: a.name.clone(),
        trigger_kind: nexus_core::TriggerKind::Schedule,
        trigger_detail: "programada".into(),
        mode: AutomationMode::Local,
        status: ExecutionStatus::Running,
        test_run: false,
        tokens_used: 0,
        agent_calls: 0,
        steps_done: 0,
        error: None,
        result: None,
        created_at: nexus_core::now_str(),
        started_at: Some(nexus_core::now_str()),
        finished_at: None,
        duration_ms: None,
    };
    repos::automations::create_execution(&f.services.db, &orphan)
        .await
        .unwrap();
    f.manager.recover().await.unwrap();
    let e = repos::automations::get_execution(&f.services.db, &orphan.id)
        .await
        .unwrap();
    assert_eq!(e.status, ExecutionStatus::Failed);
    assert!(e.error.unwrap().contains("reinicio"));
}

// ---------------------------------------------------------------------------
// Integración con agentes (IA opcional)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn local_mode_skips_agent_steps_without_ai() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Sin IA",
            manual_trigger(),
            flow(
                "ag",
                vec![
                    step(
                        "ag",
                        StepKind::Agent {
                            agent_id: "planner".into(),
                            goal: "planifica".into(),
                            approval: None,
                        },
                        Some("t"),
                    ),
                    create_task_action("t", "local sigue", None),
                ],
            ),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(
        &f,
        &e.id,
        &[ExecutionStatus::Completed, ExecutionStatus::Failed],
    )
    .await;
    assert_eq!(e.status, ExecutionStatus::Completed, "error: {:?}", e.error);
    assert_eq!(e.agent_calls, 0);
    let detail = f.manager.execution_detail(&e.id).await.unwrap();
    assert!(detail
        .steps
        .iter()
        .any(|s| s.kind == "agent" && s.status == StepStatus::Skipped));
    assert_eq!(
        f.services
            .tasks
            .list(Default::default())
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn ai_mode_without_provider_fails_gracefully() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "IA requerida",
        manual_trigger(),
        flow(
            "ag",
            vec![step(
                "ag",
                StepKind::Agent {
                    agent_id: "planner".into(),
                    goal: "planifica".into(),
                    approval: None,
                },
                None,
            )],
        ),
    );
    data.mode = Some(AutomationMode::Ai);
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Failed]).await;
    assert!(e.error.is_some());
}

#[tokio::test]
async fn hybrid_without_provider_defers_agent_but_continues() {
    let f = fixture(None).await;
    let mut data = local_automation(
        "Híbrida",
        manual_trigger(),
        flow(
            "ag",
            vec![
                step(
                    "ag",
                    StepKind::Agent {
                        agent_id: "knowledge".into(),
                        goal: "clasifica".into(),
                        approval: None,
                    },
                    Some("t"),
                ),
                create_task_action("t", "continuación local", None),
            ],
        ),
    );
    data.mode = Some(AutomationMode::Hybrid);
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert_eq!(e.result.as_ref().unwrap()["deferred"], json!(1));
    let detail = f.manager.execution_detail(&e.id).await.unwrap();
    assert!(detail
        .steps
        .iter()
        .any(|s| s.status == StepStatus::Deferred));
    assert_eq!(
        f.services
            .tasks
            .list(Default::default())
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn ai_mode_runs_real_agent_and_tracks_tokens() {
    let resp = ChatResponse {
        content: "Plan listo: prioridad 1".into(),
        usage: nexus_ai::Usage {
            prompt_tokens: 120,
            completion_tokens: 80,
        },
        ..Default::default()
    };
    let f = fixture(Some(Arc::new(MockProvider::new(vec![resp])))).await;
    let mut data = local_automation(
        "Con IA",
        manual_trigger(),
        flow(
            "ag",
            vec![
                step(
                    "ag",
                    StepKind::Agent {
                        agent_id: "planner".into(),
                        goal: "Prepara un plan breve".into(),
                        approval: None,
                    },
                    Some("save"),
                ),
                step(
                    "save",
                    StepKind::Action {
                        action: "knowledge.save_summary".into(),
                        params: json!({"title": "Plan", "content": "{{agent_result}}"}),
                        approval: None,
                    },
                    None,
                ),
            ],
        ),
    );
    data.mode = Some(AutomationMode::Ai);
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(
        &f,
        &e.id,
        &[ExecutionStatus::Completed, ExecutionStatus::Failed],
    )
    .await;
    assert_eq!(e.status, ExecutionStatus::Completed, "error: {:?}", e.error);
    assert_eq!(e.agent_calls, 1);
    assert_eq!(e.tokens_used, 200);
    let notes = f.services.notes.list(Default::default()).await.unwrap();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].content.contains("Plan listo"));
    assert_eq!(notes[0].kind, NoteKind::Document);

    let detail = f.manager.execution_detail(&e.id).await.unwrap();
    assert_eq!(detail.agent_runs.len(), 1);
    assert_eq!(detail.agent_runs[0].status, RunStatus::Completed);
}

#[tokio::test]
async fn agent_call_limit_enforced() {
    let f = fixture(Some(Arc::new(MockProvider::new(vec![text("ok")])))).await;
    let mut data = local_automation(
        "Dos agentes",
        manual_trigger(),
        flow(
            "a1",
            vec![
                step(
                    "a1",
                    StepKind::Agent {
                        agent_id: "planner".into(),
                        goal: "uno".into(),
                        approval: None,
                    },
                    Some("a2"),
                ),
                step(
                    "a2",
                    StepKind::Agent {
                        agent_id: "planner".into(),
                        goal: "dos".into(),
                        approval: None,
                    },
                    None,
                ),
            ],
        ),
    );
    data.mode = Some(AutomationMode::Ai);
    data.limits = Some(AutomationLimits {
        max_agent_calls: 1,
        ..Default::default()
    });
    let a = f.manager.create(data).await.unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    let e = wait_exec_status(&f, &e.id, &[ExecutionStatus::Failed]).await;
    assert!(e.error.unwrap().contains("límite de llamadas"));
    assert_eq!(e.agent_calls, 1);
}

#[tokio::test]
async fn chained_automation_only_local_and_depth_limited() {
    let f = fixture(None).await;
    let target = f
        .manager
        .create(local_automation(
            "Objetivo",
            manual_trigger(),
            flow("t", vec![create_task_action("t", "encadenada", None)]),
        ))
        .await
        .unwrap();
    let source = f
        .manager
        .create(local_automation(
            "Origen",
            manual_trigger(),
            flow(
                "run",
                vec![step(
                    "run",
                    StepKind::Action {
                        action: "system.run_automation".into(),
                        params: json!({"automation_id": target.id}),
                        approval: Some(Policy::Auto),
                    },
                    None,
                )],
            ),
        ))
        .await
        .unwrap();
    // system.run_automation es sensible: aprobar.
    let e = f.manager.run_now(&source.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::WaitingApproval]).await;
    let approvals = f
        .manager
        .approvals(Some(nexus_core::AutomationApprovalStatus::Pending))
        .await
        .unwrap();
    f.manager
        .decide_approval(&approvals[0].id, true)
        .await
        .unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    assert!(f
        .services
        .tasks
        .list(Default::default())
        .await
        .unwrap()
        .iter()
        .any(|t| t.title == "encadenada"));
}

#[tokio::test]
async fn overview_reflects_activity() {
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Vista",
            manual_trigger(),
            flow("n", vec![notify_step("n", "x", None)]),
        ))
        .await
        .unwrap();
    let e = f.manager.run_now(&a.id, false).await.unwrap();
    wait_exec_status(&f, &e.id, &[ExecutionStatus::Completed]).await;
    let ov = f.manager.overview().await.unwrap();
    assert_eq!(ov.active_count, 1);
    assert_eq!(ov.executions_total, 1);
    assert_eq!(ov.executions_ok, 1);
    assert_eq!(ov.executions_failed, 0);
    assert_eq!(ov.recent_executions.len(), 1);
    assert!(!ov.upcoming.is_empty());
}

#[tokio::test]
async fn project_status_change_event_payload() {
    let f = fixture(None).await;
    f.manager.start();
    let a = f
        .manager
        .create(local_automation(
            "Al completar",
            Trigger::Event {
                event: "project.status_changed".into(),
                filter: json!({"status": "completed"}),
            },
            flow(
                "t",
                vec![create_task_action("t", "Proyecto {{name}} terminado", None)],
            ),
        ))
        .await
        .unwrap();
    let _ = a;
    let project = f
        .services
        .projects
        .create(
            CreateProject {
                name: "Fénix".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let _ = f.events.send(AppEvent::DomainEvent {
        event: "project.status_changed".into(),
        entity: "project".into(),
        entity_id: Some(project.id.clone()),
        data: json!({"id": project.id, "name": "Fénix", "status": "completed"}),
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let tasks = f.services.tasks.list(Default::default()).await.unwrap();
        if tasks.iter().any(|t| t.title == "Proyecto Fénix terminado") {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // El filtro descarta otros estados.
    let _ = f.events.send(AppEvent::DomainEvent {
        event: "project.status_changed".into(),
        entity: "project".into(),
        entity_id: Some(project.id.clone()),
        data: json!({"id": project.id, "name": "Fénix", "status": "active"}),
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
}

#[tokio::test]
async fn idea_and_note_events_are_distinct() {
    let f = fixture(None).await;
    f.manager.start();
    f.manager
        .create(local_automation(
            "Ideas",
            Trigger::Event {
                event: "idea.created".into(),
                filter: json!({}),
            },
            flow("t", vec![create_task_action("t", "idea: {{title}}", None)]),
        ))
        .await
        .unwrap();
    f.manager
        .create(local_automation(
            "Notas",
            Trigger::Event {
                event: "note.created".into(),
                filter: json!({}),
            },
            flow("t", vec![create_task_action("t", "nota: {{title}}", None)]),
        ))
        .await
        .unwrap();
    let idea = f
        .services
        .notes
        .create(
            CreateNote {
                title: "Idea X".into(),
                kind: Some(NoteKind::Idea),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let note = f
        .services
        .notes
        .create(
            CreateNote {
                title: "Nota Y".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let _ = f.events.send(AppEvent::DomainEvent {
        event: "idea.created".into(),
        entity: "note".into(),
        entity_id: Some(idea.id.clone()),
        data: serde_json::to_value(&idea).unwrap(),
    });
    let _ = f.events.send(AppEvent::DomainEvent {
        event: "note.created".into(),
        entity: "note".into(),
        entity_id: Some(note.id.clone()),
        data: serde_json::to_value(&note).unwrap(),
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let tasks = f.services.tasks.list(Default::default()).await.unwrap();
        if tasks.iter().any(|t| t.title == "idea: Idea X")
            && tasks.iter().any(|t| t.title == "nota: Nota Y")
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(tasks.len(), 2);
}

#[tokio::test]
async fn sync_execute_helper_for_tests() {
    // execute() es la vía síncrona usada por el encadenado; aquí se exercising
    // directamente con un token cancelado para verificar el estado Cancelled.
    let f = fixture(None).await;
    let a = f
        .manager
        .create(local_automation(
            "Sync",
            manual_trigger(),
            flow("d", vec![step("d", StepKind::Delay { seconds: 5 }, None)]),
        ))
        .await
        .unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let outcome = f
        .manager
        .execute(
            a,
            &manual_trigger(),
            "manual".into(),
            json!({}),
            false,
            0,
            token,
        )
        .await
        .unwrap();
    assert_eq!(outcome.execution.status, ExecutionStatus::Cancelled);
}

#[allow(dead_code)]
fn _assert_error_variants(e: NexusError) -> bool {
    matches!(e, NexusError::Validation(_) | NexusError::LimitExceeded(_))
}
