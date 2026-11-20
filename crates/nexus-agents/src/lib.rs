//! nexus-agents: orquestador multiagente con bucle real de herramientas.
//! Streaming con eventos, aprobaciones humanas para acciones de riesgo,
//! delegación padre/hijo con límite de profundidad, cancelación cooperativa,
//! límites de iteraciones/tokens y auditoría completa.

mod engine;
mod registry;

pub use engine::ExecutionEngine;
pub use registry::{AgentRegistry, AgentSpec};

#[cfg(test)]
mod tests;
