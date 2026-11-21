//! nexus-automation: Automation Center de NEXUS.
//! Motor local de automatizaciones (triggers → condiciones → acciones → agentes)
//! con scheduler persistente, historial, autorizaciones y control de consumo.
//! La IA es opcional: el modo local funciona siempre, incluso offline.

pub mod actions;
pub mod conditions;
pub mod dispatcher;
pub mod flow;
pub mod manager;
pub mod runner;
pub mod templates;
pub mod triggers;

pub use manager::AutomationManager;

#[cfg(test)]
mod tests;
