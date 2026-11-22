//! Puente entre el Automation Center y las herramientas de agentes:
//! implementa `nexus_tools::AutomationOps` delegando en AutomationManager.

use nexus_core::{Automation, CreateAutomation, Result};

use crate::AutomationManager;

#[async_trait::async_trait]
impl nexus_tools::AutomationOps for AutomationManager {
    async fn list(&self, enabled_only: bool) -> Result<Vec<Automation>> {
        self.list(enabled_only).await
    }

    async fn get(&self, id: &str) -> Result<Automation> {
        self.get(id).await
    }

    async fn create(&self, data: CreateAutomation) -> Result<Automation> {
        self.create(data).await
    }

    async fn set_enabled(&self, id: &str, enabled: bool) -> Result<Automation> {
        self.set_enabled(id, enabled).await
    }
}
