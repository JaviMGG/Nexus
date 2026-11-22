//! nexus-tools: registro de herramientas reales que operan sobre los servicios.
//! Cada herramienta declara su JSON Schema y su nivel de riesgo; las
//! mutaciones se auditan con actor `agent:<id>`. No hay SQL, shell ni
//! filesystem: solo la API de servicios validada.

mod builtins;
mod tools_automations;
mod tools_canvas;
mod tools_databases;
mod tools_meals;
mod schema;

use std::collections::HashMap;
use std::sync::Arc;

use nexus_core::{Automation, CreateAutomation, NexusError, Result, Risk};
use nexus_services::Services;

pub use schema::{needs_approval, validate_args};

/// Acceso del agente al Automation Center sin acoplar nexus-tools a
/// nexus-automation (que depende de este crate). Lo implementa
/// AutomationManager y se inyecta en el ToolContext desde el motor.
#[async_trait::async_trait]
pub trait AutomationOps: Send + Sync {
    async fn list(&self, enabled_only: bool) -> Result<Vec<Automation>>;
    async fn get(&self, id: &str) -> Result<Automation>;
    async fn create(&self, data: CreateAutomation) -> Result<Automation>;
    async fn set_enabled(&self, id: &str, enabled: bool) -> Result<Automation>;
}

pub struct ToolContext {
    pub services: Services,
    pub run_id: String,
    pub agent_id: String,
    pub automation: Option<Arc<dyn AutomationOps>>,
}

impl ToolContext {
    pub(crate) fn automation_ops(&self) -> Result<Arc<dyn AutomationOps>> {
        self.automation.clone().ok_or_else(|| {
            NexusError::Internal("el Automation Center no está disponible en este contexto".into())
        })
    }

    pub(crate) fn actor(&self) -> String {
        format!("agent:{}", self.agent_id)
    }
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> nexus_ai::ToolSpec;
    fn risk(&self) -> Risk;
    /// Si es true, la herramienta pide autorización SIEMPRE (aunque la
    /// política global de escritura sea automática). Para acciones sensibles
    /// como crear o activar automatizaciones.
    fn always_confirm(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<serde_json::Value>;
}

/// Proveedor de búsqueda web conectable. Sin proveedor configurado, la
/// herramienta `web.search` devuelve un error claro: nunca se inventan
/// resultados.
#[async_trait::async_trait]
pub trait WebSearchProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<WebSearchResult>>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn with_defaults() -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let mut all = builtins::defaults();
        all.extend(tools_automations::tools());
        all.extend(tools_databases::tools());
        all.extend(tools_canvas::tools());
        all.extend(tools_meals::tools());
        for tool in all {
            tools.insert(tool.spec().name, tool);
        }
        Self { tools }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn specs(&self, names: &[String]) -> Vec<nexus_ai::ToolSpec> {
        names
            .iter()
            .filter_map(|n| self.tools.get(n).map(|t| t.spec()))
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }
}
