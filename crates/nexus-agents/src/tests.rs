//! Tests de integración del motor con un proveedor simulado guionizado,
//! base de datos en memoria y el registro de herramientas real.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use nexus_ai::{AiProvider, ChatRequest, ChatResponse, ToolCallRequest};
use nexus_core::{
    new_id, now_str, AgentRun, AppEvent, ApprovalStatus, MessageRole, NexusError, Result,
    RunStatus, ToolCallStatus,
};
use nexus_db::{repos, Db};
use nexus_services::Services;
use serde_json::json;
use tokio::sync::{broadcast, Mutex};

use crate::{AgentRegistry, ExecutionEngine};

/// Proveedor simulado: respuestas guionizadas; al agotarse repite la última.
struct MockProvider {
    script: Arc<Mutex<VecDeque<ChatResponse>>>,
    delay: Duration,
}

impl MockProvider {
    fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            script: Arc::new(Mutex::new(responses.into())),
            delay: Duration::ZERO,
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
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
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
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
    engine: Arc<ExecutionEngine>,
    services: Services,
    events: broadcast::Sender<AppEvent>,
}

async fn fixture_with(provider: Arc<dyn AiProvider>) -> Fixture {
    let services = Services::new(Db::connect_memory().await.unwrap());
    let registry = Arc::new(nexus_ai::ProviderRegistry::new(services.clone()));
    registry.refresh().await.unwrap();
    registry.insert_test_provider(provider).await;
    let (events, _) = broadcast::channel(256);
    let engine = ExecutionEngine::new(
        services.clone(),
        registry,
        Arc::new(nexus_tools::ToolRegistry::with_defaults()),
        Arc::new(AgentRegistry::with_defaults()),
        events.clone(),
    );
    Fixture {
        engine,
        services,
        events,
    }
}

async fn fixture(responses: Vec<ChatResponse>) -> Fixture {
    fixture_with(Arc::new(MockProvider::new(responses))).await
}

async fn empty_fixture() -> Fixture {
    let services = Services::new(Db::connect_memory().await.unwrap());
    let registry = Arc::new(nexus_ai::ProviderRegistry::new(services.clone()));
    registry.refresh().await.unwrap();
    let (events, _) = broadcast::channel(256);
    let engine = ExecutionEngine::new(
        services.clone(),
        registry,
        Arc::new(nexus_tools::ToolRegistry::with_defaults()),
        Arc::new(AgentRegistry::with_defaults()),
        events.clone(),
    );
    Fixture {
        engine,
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

fn tool_call(name: &str, args: serde_json::Value) -> ChatResponse {
    ChatResponse {
        content: String::new(),
        tool_calls: vec![ToolCallRequest {
            id: new_id(),
            name: name.into(),
            arguments: args,
        }],
        usage: Default::default(),
    }
}

async fn wait_status(services: &Services, run_id: &str, statuses: &[RunStatus]) -> AgentRun {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let run = repos::runs::get_run(&services.db, run_id).await.unwrap();
        if statuses.contains(&run.status) {
            return run;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout esperando estado, actual: {:?}",
            run.status
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_approval(services: &Services, run_id: &str) -> nexus_core::Approval {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let approvals = repos::runs::list_run_approvals(&services.db, run_id)
            .await
            .unwrap();
        if let Some(a) = approvals
            .into_iter()
            .find(|a| a.status == ApprovalStatus::Pending)
        {
            return a;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout esperando aprobación"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn happy_path_creates_task_and_completes() {
    let f = fixture(vec![
        tool_call("tasks.create", json!({"title": "Tarea del agente"})),
        text("He creado la tarea."),
    ])
    .await;
    let conv = f
        .services
        .chat
        .create_conversation("executor", None)
        .await
        .unwrap();
    f.services
        .chat
        .add_message(
            &conv.id,
            MessageRole::User,
            "crea una tarea",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let mut rx = f.events.subscribe();
    let run = f
        .engine
        .start_run(
            "crea una tarea".into(),
            "executor".into(),
            Some(conv.id.clone()),
            None,
            0,
        )
        .await
        .unwrap();
    let done = wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    assert_eq!(done.result.as_deref(), Some("He creado la tarea."));
    assert_eq!(done.iterations, 2);

    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "Tarea del agente");

    let calls = repos::runs::list_tool_calls(&f.services.db, &run.id)
        .await
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "tasks.create");
    assert_eq!(calls[0].status, ToolCallStatus::Ok);
    assert!(calls[0].duration_ms.is_some());

    let msgs = f.services.chat.messages(&conv.id).await.unwrap();
    let last = msgs.last().unwrap();
    assert_eq!(last.role, MessageRole::Assistant);
    assert_eq!(last.run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(last.agent_id.as_deref(), Some("executor"));

    let activity = repos::activity::list(&f.services.db, 50, None, None)
        .await
        .unwrap();
    assert!(activity
        .iter()
        .any(|a| a.kind == "agent_run" && a.actor == "agent:executor"));
    assert!(activity
        .iter()
        .any(|a| a.kind == "created" && a.actor == "agent:executor"));

    let mut saw_delta = false;
    let mut saw_finished = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            AppEvent::MessageDelta {
                conversation_id,
                delta,
                ..
            } => {
                assert_eq!(conversation_id, conv.id);
                assert!(!delta.is_empty());
                saw_delta = true;
            }
            AppEvent::RunFinished { run: r } if r.id == run.id => saw_finished = true,
            _ => {}
        }
    }
    assert!(saw_delta);
    assert!(saw_finished);
}

#[tokio::test]
async fn iteration_limit_terminates_without_infinite_loop() {
    let f = fixture(vec![tool_call("tasks.list", json!({}))]).await;
    f.services
        .settings
        .update_app(json!({"limits": {"max_iterations": 3}}))
        .await
        .unwrap();
    let run = f
        .engine
        .start_run("bucle".into(), "executor".into(), None, None, 0)
        .await
        .unwrap();
    let done = wait_status(
        &f.services,
        &run.id,
        &[RunStatus::Completed, RunStatus::Failed],
    )
    .await;
    assert_eq!(done.status, RunStatus::Completed);
    assert!(done.iterations <= 3, "iterations: {}", done.iterations);
    assert!(done.result.is_some());
    let calls = repos::runs::list_tool_calls(&f.services.db, &run.id)
        .await
        .unwrap();
    assert!(calls.len() <= 3);
}

#[tokio::test]
async fn delegation_creates_child_run_with_parent() {
    let f = fixture(vec![
        tool_call(
            "agents.delegate",
            json!({"agent_id": "executor", "goal": "crea la tarea delegada"}),
        ),
        tool_call("tasks.create", json!({"title": "Tarea delegada"})),
        text("Tarea delegada creada."),
        text("Delegación completada."),
    ])
    .await;
    let run = f
        .engine
        .start_run("organiza".into(), "orchestrator".into(), None, None, 0)
        .await
        .unwrap();
    let done = wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    assert_eq!(done.result.as_deref(), Some("Delegación completada."));

    let children = repos::runs::list_children(&f.services.db, &run.id)
        .await
        .unwrap();
    assert_eq!(children.len(), 1);
    let child = &children[0];
    assert_eq!(child.agent_id, "executor");
    assert_eq!(child.parent_run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(child.status, RunStatus::Completed);
    assert!(child.conversation_id.is_none());

    let tasks = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "Tarea delegada");

    let detail = f.engine.run_detail(&run.id).await.unwrap();
    assert_eq!(detail.children.len(), 1);
    assert_eq!(detail.tool_calls.len(), 1);
    assert_eq!(detail.tool_calls[0].status, ToolCallStatus::Ok);
}

#[tokio::test]
async fn delegation_depth_exceeded_returns_clear_error() {
    let f = fixture(vec![tool_call(
        "agents.delegate",
        json!({"agent_id": "executor", "goal": "algo"}),
    )])
    .await;
    f.services
        .settings
        .update_app(json!({"limits": {"max_delegation_depth": 0}}))
        .await
        .unwrap();
    let run = f
        .engine
        .start_run("delegar".into(), "orchestrator".into(), None, None, 0)
        .await
        .unwrap();
    let done = wait_status(
        &f.services,
        &run.id,
        &[RunStatus::Completed, RunStatus::Failed],
    )
    .await;
    let calls = repos::runs::list_tool_calls(&f.services.db, &run.id)
        .await
        .unwrap();
    assert!(calls
        .iter()
        .any(|c| c.error.as_deref().unwrap_or("").contains("profundidad")));
    assert!(repos::runs::list_children(&f.services.db, &run.id)
        .await
        .unwrap()
        .is_empty());
    assert!(done.result.is_some() || done.error.is_some());
}

#[tokio::test]
async fn approval_approved_executes_destructive_tool() {
    let provider = Arc::new(MockProvider::new(vec![text("init")]));
    let f = fixture_with(provider.clone()).await;
    let task = f
        .services
        .tasks
        .create(
            nexus_core::CreateTask {
                title: "A borrar".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    {
        let mut script = provider.script.lock().await;
        script.clear();
        script.push_back(tool_call("tasks.delete", json!({"id": task.id})));
        script.push_back(text("Tarea eliminada."));
    }
    let run = f
        .engine
        .start_run("borra la tarea".into(), "executor".into(), None, None, 0)
        .await
        .unwrap();
    let waiting = wait_status(&f.services, &run.id, &[RunStatus::WaitingApproval]).await;
    assert_eq!(waiting.status, RunStatus::WaitingApproval);
    let approval = wait_approval(&f.services, &run.id).await;
    assert_eq!(approval.tool_name, "tasks.delete");

    let decided = f.engine.resolve_approval(&approval.id, true).await.unwrap();
    assert_eq!(decided.status, ApprovalStatus::Approved);
    let done = wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    assert_eq!(done.result.as_deref(), Some("Tarea eliminada."));
    let remaining = f.services.tasks.list(Default::default()).await.unwrap();
    assert!(remaining.is_empty());
    let calls = repos::runs::list_tool_calls(&f.services.db, &run.id)
        .await
        .unwrap();
    assert_eq!(calls[0].status, ToolCallStatus::Ok);
}

#[tokio::test]
async fn approval_rejected_denies_tool() {
    let provider = Arc::new(MockProvider::new(vec![text("init")]));
    let f = fixture_with(provider.clone()).await;
    let task = f
        .services
        .tasks
        .create(
            nexus_core::CreateTask {
                title: "No borrar".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    {
        let mut script = provider.script.lock().await;
        script.clear();
        script.push_back(tool_call("tasks.delete", json!({"id": task.id})));
        script.push_back(text("Entendido, no la borro."));
    }
    let run = f
        .engine
        .start_run("borra".into(), "executor".into(), None, None, 0)
        .await
        .unwrap();
    wait_status(&f.services, &run.id, &[RunStatus::WaitingApproval]).await;
    let approval = wait_approval(&f.services, &run.id).await;
    f.engine
        .resolve_approval(&approval.id, false)
        .await
        .unwrap();
    let done = wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    assert_eq!(done.result.as_deref(), Some("Entendido, no la borro."));
    let calls = repos::runs::list_tool_calls(&f.services.db, &run.id)
        .await
        .unwrap();
    assert_eq!(calls[0].status, ToolCallStatus::Denied);
    let remaining = f.services.tasks.list(Default::default()).await.unwrap();
    assert_eq!(remaining.len(), 1);
    let err = f.engine.resolve_approval(&approval.id, true).await;
    assert!(matches!(err, Err(NexusError::Conflict(_))));
}

#[tokio::test]
async fn cancellation_stops_delayed_run() {
    let mock = Arc::new(MockProvider::new(vec![text("tarde")]).with_delay(Duration::from_secs(30)));
    let f = fixture_with(mock).await;
    let run = f
        .engine
        .start_run("lento".into(), "executor".into(), None, None, 0)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    f.engine.cancel_run(&run.id).await.unwrap();
    let done = wait_status(&f.services, &run.id, &[RunStatus::Cancelled]).await;
    assert_eq!(done.status, RunStatus::Cancelled);
    assert!(done.finished_at.is_some());
    let active = f.engine.active_runs().await.unwrap();
    assert!(active.is_empty());
    let err = f.engine.cancel_run(&run.id).await;
    assert!(matches!(err, Err(NexusError::Conflict(_))));
}

#[tokio::test]
async fn tool_outside_allowlist_is_denied_without_execution() {
    let f = fixture(vec![
        tool_call("tasks.delete", json!({"id": "cualquiera"})),
        text("No puedo borrar."),
    ])
    .await;
    let run = f
        .engine
        .start_run("borra algo".into(), "planner".into(), None, None, 0)
        .await
        .unwrap();
    let done = wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    assert_eq!(done.result.as_deref(), Some("No puedo borrar."));
    let calls = repos::runs::list_tool_calls(&f.services.db, &run.id)
        .await
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].status, ToolCallStatus::Denied);
    assert!(calls[0].error.as_deref().unwrap().contains("no permitida"));
}

#[tokio::test]
async fn no_provider_returns_ai_unavailable_without_run() {
    let f = empty_fixture().await;
    let err = f
        .engine
        .start_run("algo".into(), "executor".into(), None, None, 0)
        .await
        .expect_err("debe fallar");
    assert!(matches!(err, NexusError::AiUnavailable(_)));
    assert!(repos::runs::list_runs(&f.services.db, 10)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn recover_stale_runs_marks_old_runs_failed() {
    let f = empty_fixture().await;
    for status in [
        RunStatus::Running,
        RunStatus::Pending,
        RunStatus::WaitingApproval,
        RunStatus::Completed,
    ] {
        let now = now_str();
        let run = AgentRun {
            id: new_id(),
            conversation_id: None,
            agent_id: "executor".into(),
            parent_run_id: None,
            goal: "viejo".into(),
            status,
            result: None,
            error: None,
            iterations: 0,
            tokens_used: 0,
            created_at: now.clone(),
            started_at: Some(now),
            finished_at: None,
        };
        repos::runs::create_run(&f.services.db, &run).await.unwrap();
    }
    f.engine.recover_stale_runs().await.unwrap();
    let runs = repos::runs::list_runs(&f.services.db, 10).await.unwrap();
    let failed: Vec<_> = runs
        .iter()
        .filter(|r| r.status == RunStatus::Failed)
        .collect();
    assert_eq!(failed.len(), 3);
    for r in failed {
        assert_eq!(r.error.as_deref(), Some("interrumpida por reinicio"));
        assert!(r.finished_at.is_some());
    }
    assert_eq!(
        runs.iter()
            .filter(|r| r.status == RunStatus::Completed)
            .count(),
        1
    );
    assert!(f.engine.active_runs().await.unwrap().is_empty());
}

#[tokio::test]
async fn token_limit_finishes_gracefully() {
    let f = fixture(vec![ChatResponse {
        content: "parcial".into(),
        tool_calls: vec![ToolCallRequest {
            id: new_id(),
            name: "tasks.list".into(),
            arguments: json!({}),
        }],
        usage: nexus_ai::Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
        },
    }])
    .await;
    f.services
        .settings
        .update_app(json!({"limits": {"max_tokens_per_run": 100}}))
        .await
        .unwrap();
    let run = f
        .engine
        .start_run("costoso".into(), "executor".into(), None, None, 0)
        .await
        .unwrap();
    let done = wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    assert!(done.result.as_deref().unwrap().contains("límite de tokens"));
}

#[tokio::test]
async fn child_runs_do_not_emit_message_deltas() {
    let f = fixture(vec![
        tool_call(
            "agents.delegate",
            json!({"agent_id": "executor", "goal": "crea nota"}),
        ),
        text("hijo listo"),
        text("padre listo"),
    ])
    .await;
    let conv = f
        .services
        .chat
        .create_conversation("orchestrator", None)
        .await
        .unwrap();
    let mut rx = f.events.subscribe();
    let run = f
        .engine
        .start_run(
            "delegar".into(),
            "orchestrator".into(),
            Some(conv.id.clone()),
            None,
            0,
        )
        .await
        .unwrap();
    wait_status(&f.services, &run.id, &[RunStatus::Completed]).await;
    let children = repos::runs::list_children(&f.services.db, &run.id)
        .await
        .unwrap();
    let child_id = children[0].id.clone();
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::MessageDelta { run_id, .. } = ev {
            assert_ne!(run_id, child_id, "los hijos no deben emitir deltas");
        }
    }
}

#[tokio::test]
async fn unknown_agent_is_not_found() {
    let f = fixture(vec![text("x")]).await;
    let err = f
        .engine
        .start_run("algo".into(), "no-existe".into(), None, None, 0)
        .await
        .err()
        .unwrap();
    assert!(matches!(err, NexusError::NotFound(_)));
}
