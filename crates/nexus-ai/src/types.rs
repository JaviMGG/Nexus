//! Tipos públicos de la abstracción de IA: mensajes, herramientas y respuestas.

use nexus_core::MessageRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema de los parámetros.
    pub parameters: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct AiMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Vec<ToolCallRequest>,
    pub tool_call_id: Option<String>,
}

impl AiMessage {
    pub fn system(s: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: s.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn user(s: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: s.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(s: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: s.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(content: String, calls: Vec<ToolCallRequest>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content,
            tool_calls: calls,
            tool_call_id: None,
        }
    }

    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id.into()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChatRequest {
    pub messages: Vec<AiMessage>,
    pub tools: Vec<ToolSpec>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

impl Usage {
    pub fn total(&self) -> i64 {
        self.prompt_tokens + self.completion_tokens
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallRequest>,
    pub usage: Usage,
}

#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    fn config(&self) -> nexus_core::ProviderConfig;
    async fn chat(&self, req: ChatRequest) -> nexus_core::Result<ChatResponse>;
    async fn chat_stream(
        &self,
        req: ChatRequest,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> nexus_core::Result<ChatResponse>;
    async fn health(&self) -> nexus_core::Result<String>;
}
