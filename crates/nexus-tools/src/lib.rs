//! nexus-tools: registro de herramientas reales que operan sobre los servicios.
//! Cada herramienta declara su JSON Schema y su nivel de riesgo; las
//! mutaciones se auditan con actor `agent:<id>`. No hay SQL, shell ni
//! filesystem: solo la API de servicios validada.

mod builtins;
mod schema;

use std::collections::HashMap;
use std::sync::Arc;

use nexus_core::{Result, Risk};
use nexus_services::Services;

pub use schema::{needs_approval, validate_args};

pub struct ToolContext {
    pub services: Services,
    pub run_id: String,
    pub agent_id: String,
}

impl ToolContext {
    pub(crate) fn actor(&self) -> String {
        format!("agent:{}", self.agent_id)
    }
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> nexus_ai::ToolSpec;
    fn risk(&self) -> Risk;
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
        for tool in builtins::defaults() {
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
