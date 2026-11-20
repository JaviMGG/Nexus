//! nexus-ai: abstracción de proveedores de IA intercambiables.
//! OpenAI-compatible (incluido Qwen vía DashScope compatible-mode), streaming
//! SSE, rate limiting por proveedor y estado de conexión cacheado.
//! La IA es opcional: sin proveedor configurado todo degrada con errores claros.

mod openai;
mod registry;
mod types;

pub use openai::is_transient;
pub use registry::ProviderRegistry;
pub use types::{
    AiMessage, AiProvider, ChatRequest, ChatResponse, ToolCallRequest, ToolSpec, Usage,
};
