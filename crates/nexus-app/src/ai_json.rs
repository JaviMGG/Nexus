//! Helper compartido para funciones de IA opcionales de los módulos nuevos.
//! Sin proveedor → error AiUnavailable claro; nunca bloquea lo local.

use nexus_ai::{AiMessage, ChatRequest};
use nexus_core::{NexusError, Result};
use serde_json::Value;

use crate::context::AppContext;

/// Pide al proveedor una respuesta JSON estricta. `system` define el contrato.
pub async fn ask_json(
    ctx: &AppContext,
    system: &str,
    user: &str,
    max_tokens: i64,
) -> Result<Value> {
    let provider = ctx.providers.default_provider().await?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let req = ChatRequest {
        messages: vec![
            AiMessage::system(format!("{system}\nFecha actual: {today}")),
            AiMessage::user(user.to_string()),
        ],
        tools: vec![],
        temperature: Some(0.3),
        max_tokens: Some(max_tokens.clamp(100, 8000)),
    };
    let response =
        match tokio::time::timeout(std::time::Duration::from_secs(60), provider.chat(req)).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(NexusError::Ai(
                    "la IA no respondió a tiempo (timeout)".into(),
                ))
            }
        };
    parse_json(&response.content)
}

pub fn parse_json(content: &str) -> Result<Value> {
    let trimmed = content.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    serde_json::from_str(json_str)
        .map_err(|_| NexusError::Ai("la IA no devolvió un JSON válido; inténtalo de nuevo".into()))
}
