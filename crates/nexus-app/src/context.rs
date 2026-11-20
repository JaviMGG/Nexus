use std::path::PathBuf;
use std::sync::Arc;

use nexus_agents::{AgentRegistry, ExecutionEngine};
use nexus_ai::ProviderRegistry;
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
    pub events: broadcast::Sender<AppEvent>,
    pub data_dir: PathBuf,
}

impl AppContext {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let _ = dotenvy::dotenv();
        let db = Db::connect(&data_dir.join("nexus.sqlite")).await?;
        let services = Services::new(db);
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
        Ok(Self {
            services,
            providers,
            tools,
            agents,
            engine,
            events,
            data_dir,
        })
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
