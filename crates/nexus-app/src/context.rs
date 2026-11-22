use std::path::PathBuf;
use std::sync::Arc;

use nexus_agents::{AgentRegistry, ExecutionEngine};
use nexus_ai::ProviderRegistry;
use nexus_automation::AutomationManager;
use nexus_core::{AppEvent, Result};
use nexus_db::Db;
use nexus_services::Services;
use nexus_tools::ToolRegistry;
use tokio::sync::broadcast;

pub struct AppContext {
    pub services: Services,
    pub providers: Arc<ProviderRegistry>,
    pub tools: Arc<ToolRegistry>,
    pub agents: Arc<AgentRegistry>,
    pub engine: Arc<ExecutionEngine>,
    pub automation: Arc<AutomationManager>,
    pub events: broadcast::Sender<AppEvent>,
    pub data_dir: PathBuf,
}

impl AppContext {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let _ = dotenvy::dotenv();
        let db = Db::connect(&data_dir.join("nexus.sqlite")).await?;
        let services = Services::with_data_dir(db, &data_dir);
        let providers = Arc::new(ProviderRegistry::new(services.clone()));
        if let Err(e) = providers.refresh().await {
            tracing::warn!("no se pudieron cargar los proveedores de IA: {e}");
        }
        let tools = Arc::new(ToolRegistry::with_defaults());
        let agents = Arc::new(AgentRegistry::with_defaults());
        let (events, _) = broadcast::channel(1024);
        let engine = ExecutionEngine::new(
            services.clone(),
            providers.clone(),
            tools.clone(),
            agents.clone(),
            events.clone(),
        );
        engine.recover_stale_runs().await?;
        let automation = AutomationManager::new(
            services.clone(),
            providers.clone(),
            engine.clone(),
            agents.clone(),
            events.clone(),
        );
        automation.recover().await?;
        automation.start();
        Ok(Self {
            services,
            providers,
            tools,
            agents,
            engine,
            automation,
            events,
            data_dir,
        })
    }

    /// Emite un evento de dominio para el Automation Engine y, si aplica,
    /// el data_changed clásico para el frontend.
    pub fn domain_event(
        &self,
        event: &str,
        entity: &str,
        entity_id: Option<&str>,
        data: serde_json::Value,
    ) {
        self.emit(AppEvent::DomainEvent {
            event: event.to_string(),
            entity: entity.to_string(),
            entity_id: entity_id.map(str::to_string),
            data,
        });
    }

    pub fn emit(&self, event: AppEvent) {
        let _ = self.events.send(event);
    }

    pub fn data_changed(&self, entity: &str) {
        self.emit(AppEvent::DataChanged {
            entity: entity.to_string(),
        });
    }

    pub async fn dispatch(
        &self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        crate::dispatch::dispatch(self, command, payload).await
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
}
