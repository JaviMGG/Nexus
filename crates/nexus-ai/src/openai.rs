//! Proveedor compatible con la API de OpenAI (incluye Qwen vía DashScope
//! compatible-mode). Todo el parseo vive en funciones puras testeables.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::StreamExt;
use nexus_core::{NexusError, ProviderConfig, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::types::{AiMessage, AiProvider, ChatRequest, ChatResponse, ToolCallRequest, Usage};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const RATE_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Parseo de respuestas (funciones puras)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct WireToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: WireFunction,
}

#[derive(Debug, Default, Deserialize)]
struct WireFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<WireToolCall>>,
}

#[derive(Debug, Deserialize)]
struct WireChoice {
    #[serde(default)]
    message: Option<WireMessage>,
}

#[derive(Debug, Default, Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: Option<i64>,
    #[serde(default)]
    completion_tokens: Option<i64>,
}

fn parse_arguments(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| json!({}))
}

fn wire_calls(calls: Vec<WireToolCall>) -> Vec<ToolCallRequest> {
    calls
        .into_iter()
        .map(|c| ToolCallRequest {
            id: c.id.unwrap_or_else(nexus_core::new_id),
            name: c.function.name.unwrap_or_default(),
            arguments: parse_arguments(c.function.arguments.as_deref().unwrap_or("{}")),
        })
        .filter(|c| !c.name.is_empty())
        .collect()
}

/// Parsea una respuesta no-streaming de `/chat/completions`.
pub(crate) fn parse_chat_response(body: &Value) -> Result<ChatResponse> {
    #[derive(Deserialize)]
    struct Wire {
        #[serde(default)]
        choices: Vec<WireChoice>,
        #[serde(default)]
        usage: Option<WireUsage>,
    }
    let wire: Wire = serde_json::from_value(body.clone()).map_err(|e| {
        NexusError::Ai(format!(
            "respuesta del proveedor con formato inesperado: {e}"
        ))
    })?;
    let usage = wire.usage.unwrap_or_default();
    let mut resp = ChatResponse {
        usage: Usage {
            prompt_tokens: usage.prompt_tokens.unwrap_or(0),
            completion_tokens: usage.completion_tokens.unwrap_or(0),
        },
        ..Default::default()
    };
    if let Some(choice) = wire.choices.into_iter().next() {
        if let Some(msg) = choice.message {
            resp.content = msg.content.unwrap_or_default();
            if let Some(calls) = msg.tool_calls {
                resp.tool_calls = wire_calls(calls);
            }
        }
    }
    Ok(resp)
}

/// Convierte un cuerpo de error HTTP en un `NexusError` claro.
/// Nunca incluye la API key: solo códigos y mensajes del proveedor.
pub(crate) fn map_http_error(status: u16, body: &str) -> NexusError {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| body.chars().take(300).collect());
    match status {
        401 | 403 => NexusError::Ai(format!(
            "credenciales inválidas o sin permisos (HTTP {status}). Revisa la API key del proveedor en Ajustes. {detail}"
        )),
        404 => NexusError::Ai(format!(
            "endpoint o modelo no encontrado (HTTP 404). Revisa base_url y modelo. {detail}"
        )),
        429 => NexusError::Ai(format!(
            "límite de peticiones del proveedor alcanzado (HTTP 429). Inténtalo de nuevo en unos segundos. {detail}"
        )),
        500..=599 => NexusError::Ai(format!("error del servidor del proveedor (HTTP {status}). {detail}")),
        _ => NexusError::Ai(format!("error del proveedor (HTTP {status}). {detail}")),
    }
}

pub(crate) fn map_transport_error(e: &reqwest::Error) -> NexusError {
    if e.is_timeout() {
        NexusError::Ai("tiempo de espera agotado al contactar con el proveedor de IA".into())
    } else if e.is_connect() {
        NexusError::Ai(
            "no se pudo conectar con el proveedor de IA (revisa la red y la base_url)".into(),
        )
    } else {
        NexusError::Ai(format!(
            "error de red al contactar con el proveedor de IA: {e}"
        ))
    }
}

/// Determina si un error de IA es transitorio (reintento con backoff).
pub fn is_transient(err: &NexusError) -> bool {
    match err {
        NexusError::Ai(msg) => {
            let m = msg.to_lowercase();
            !(m.contains("credenciales") || m.contains("http 404") || m.contains("no encontrado"))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Acumulación de deltas SSE (función pura testeable)
// ---------------------------------------------------------------------------

/// Acumulador de streaming SSE: junta deltas de texto y de tool_calls
/// (que llegan fragmentados por índice) hasta producir la respuesta final.
#[derive(Debug, Default)]
pub(crate) struct SseAccumulator {
    content: String,
    calls: Vec<PartialCall>,
    usage: Usage,
    finished: bool,
}

#[derive(Debug, Default, Clone)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

impl SseAccumulator {
    /// Procesa una línea SSE completa (`data: {...}` o `data: [DONE]`).
    /// Devuelve el delta de texto visible para reenviar al canal, si lo hay.
    pub(crate) fn push_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        let payload = line.strip_prefix("data:")?.trim();
        if payload.is_empty() {
            return None;
        }
        if payload == "[DONE]" {
            self.finished = true;
            return None;
        }
        let v: Value = serde_json::from_str(payload).ok()?;
        self.push_event(&v)
    }

    fn push_event(&mut self, v: &Value) -> Option<String> {
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            if let Some(p) = u.get("prompt_tokens").and_then(Value::as_i64) {
                self.usage.prompt_tokens = p;
            }
            if let Some(c) = u.get("completion_tokens").and_then(Value::as_i64) {
                self.usage.completion_tokens = c;
            }
        }
        let choice = v
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|c| c.first())?;
        let delta = choice.get("delta")?;
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for c in calls {
                let idx = c.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                while self.calls.len() <= idx {
                    self.calls.push(PartialCall::default());
                }
                let slot = &mut self.calls[idx];
                if let Some(id) = c.get("id").and_then(Value::as_str) {
                    if !id.is_empty() {
                        slot.id = id.to_string();
                    }
                }
                if let Some(f) = c.get("function") {
                    if let Some(n) = f.get("name").and_then(Value::as_str) {
                        slot.name.push_str(n);
                    }
                    if let Some(a) = f.get("arguments").and_then(Value::as_str) {
                        slot.arguments.push_str(a);
                    }
                }
            }
        }
        let text = delta.get("content").and_then(Value::as_str)?;
        if text.is_empty() {
            return None;
        }
        self.content.push_str(text);
        Some(text.to_string())
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.finished
    }

    pub(crate) fn into_response(self) -> ChatResponse {
        let tool_calls = self
            .calls
            .into_iter()
            .filter(|c| !c.name.is_empty())
            .map(|c| ToolCallRequest {
                id: if c.id.is_empty() {
                    nexus_core::new_id()
                } else {
                    c.id
                },
                name: c.name,
                arguments: parse_arguments(&c.arguments),
            })
            .collect();
        ChatResponse {
            content: self.content,
            tool_calls,
            usage: self.usage,
        }
    }
}

// ---------------------------------------------------------------------------
// Rate limiter por proveedor (ventana deslizante)
// ---------------------------------------------------------------------------

pub(crate) struct RateLimiter {
    per_minute: i64,
    hits: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    pub(crate) fn new(per_minute: i64) -> Self {
        Self {
            per_minute: per_minute.max(1),
            hits: Mutex::new(VecDeque::new()),
        }
    }

    fn try_acquire(&self) -> std::result::Result<(), Duration> {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        while hits
            .front()
            .map(|t| now.duration_since(*t) >= Duration::from_secs(60))
            .unwrap_or(false)
        {
            hits.pop_front();
        }
        if (hits.len() as i64) < self.per_minute {
            hits.push_back(now);
            return Ok(());
        }
        let wait = hits
            .front()
            .map(|t| Duration::from_secs(60).saturating_sub(now.duration_since(*t)))
            .unwrap_or(Duration::from_secs(1));
        Err(wait.max(Duration::from_millis(50)))
    }

    /// Espera hueco en la ventana; si tardaría demasiado, `LimitExceeded`.
    pub(crate) async fn acquire(&self) -> Result<()> {
        let deadline = Instant::now() + RATE_WAIT_TIMEOUT;
        loop {
            match self.try_acquire() {
                Ok(()) => return Ok(()),
                Err(wait) => {
                    if Instant::now() + wait > deadline {
                        return Err(NexusError::LimitExceeded(
                            "límite de peticiones por minuto del proveedor alcanzado; inténtalo más tarde".into(),
                        ));
                    }
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Proveedor OpenAI-compatible
// ---------------------------------------------------------------------------

pub struct OpenAiCompatibleProvider {
    config: ProviderConfig,
    api_key: String,
    client: reqwest::Client,
    limiter: RateLimiter,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: ProviderConfig, api_key: String) -> Result<Self> {
        if api_key.trim().is_empty() {
            return Err(NexusError::AiUnavailable(format!(
                "el proveedor {} no tiene API key configurada",
                config.name
            )));
        }
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| NexusError::Internal(format!("no se pudo crear el cliente HTTP: {e}")))?;
        let limiter = RateLimiter::new(config.requests_per_minute);
        Ok(Self {
            config,
            api_key: api_key.trim().to_string(),
            client,
            limiter,
        })
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }

    fn build_body(&self, req: &ChatRequest, stream: bool) -> Value {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| {
                let mut obj = json!({ "role": m.role.as_str(), "content": m.content });
                if !m.tool_calls.is_empty() {
                    obj["tool_calls"] = Value::Array(
                        m.tool_calls
                            .iter()
                            .map(|c| {
                                json!({
                                    "id": c.id,
                                    "type": "function",
                                    "function": { "name": c.name, "arguments": c.arguments.to_string() }
                                })
                            })
                            .collect(),
                    );
                }
                if let Some(id) = &m.tool_call_id {
                    obj["tool_call_id"] = json!(id);
                }
                obj
            })
            .collect();
        let mut body = json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": req.temperature.unwrap_or(self.config.temperature),
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens),
        });
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
                        })
                    })
                    .collect(),
            );
            body["tool_choice"] = json!("auto");
        }
        if stream {
            body["stream"] = json!(true);
        }
        body
    }

    async fn send(&self, body: &Value) -> Result<reqwest::Response> {
        self.limiter.acquire().await?;
        let resp = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| map_transport_error(&e))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status.as_u16(), &text));
        }
        Ok(resp)
    }

    async fn chat_once(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let resp = self.send(&self.build_body(req, false)).await?;
        let body: Value = resp.json().await.map_err(|e| map_transport_error(&e))?;
        parse_chat_response(&body)
    }

    async fn chat_stream_impl(
        &self,
        req: &ChatRequest,
        deltas: UnboundedSender<String>,
    ) -> Result<ChatResponse> {
        let resp = self.send(&self.build_body(req, true)).await?;
        let mut acc = SseAccumulator::default();
        let mut buffer = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| map_transport_error(&e))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer.drain(..=pos).collect();
                if let Some(delta) = acc.push_line(&line) {
                    let _ = deltas.send(delta);
                }
            }
            if acc.is_finished() {
                break;
            }
        }
        if !buffer.trim().is_empty() {
            if let Some(delta) = acc.push_line(&buffer) {
                let _ = deltas.send(delta);
            }
        }
        Ok(acc.into_response())
    }
}

#[async_trait::async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    fn config(&self) -> ProviderConfig {
        self.config.clone()
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        self.chat_once(&req).await
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
        deltas: UnboundedSender<String>,
    ) -> Result<ChatResponse> {
        self.chat_stream_impl(&req, deltas).await
    }

    async fn health(&self) -> Result<String> {
        let resp = self
            .chat_once(&ChatRequest {
                messages: vec![AiMessage::user("ping")],
                max_tokens: Some(8),
                ..Default::default()
            })
            .await?;
        let _ = resp;
        Ok(format!("{} ({})", self.config.name, self.config.model))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_response_with_tool_calls() {
        let body = json!({
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "tasks.create", "arguments": "{\"title\":\"Hola\"}" }
                    }]
                }
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        });
        let resp = parse_chat_response(&body).unwrap();
        assert!(resp.content.is_empty());
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].id, "call_1");
        assert_eq!(resp.tool_calls[0].name, "tasks.create");
        assert_eq!(resp.tool_calls[0].arguments["title"], "Hola");
        assert_eq!(resp.usage.prompt_tokens, 10);
        assert_eq!(resp.usage.completion_tokens, 5);
    }

    #[test]
    fn parses_plain_text_response() {
        let body = json!({"choices": [{"message": {"content": "hola"}}]});
        let resp = parse_chat_response(&body).unwrap();
        assert_eq!(resp.content, "hola");
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn maps_401_to_credentials_error() {
        let err = map_http_error(401, r#"{"error":{"message":"Invalid API key"}}"#);
        match err {
            NexusError::Ai(msg) => {
                assert!(msg.contains("credenciales inválidas"));
                assert!(msg.contains("Invalid API key"));
            }
            other => panic!("error inesperado: {other:?}"),
        }
        assert!(!is_transient(&map_http_error(401, "")));
        assert!(is_transient(&map_http_error(429, "")));
        assert!(is_transient(&map_http_error(503, "")));
    }

    #[test]
    fn accumulates_sse_deltas() {
        let mut acc = SseAccumulator::default();
        let d1 = acc.push_line(r#"data: {"choices":[{"delta":{"content":"Ho"}}]}"#);
        let d2 = acc.push_line(r#"data: {"choices":[{"delta":{"content":"la"}}]}"#);
        let d3 = acc.push_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"tasks.","arguments":"{\"ti"}}]}}]}"#,
        );
        let d4 = acc.push_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"create","arguments":"tle\":\"X\"}"}}]}}]}"#,
        );
        acc.push_line(r#"data: {"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":3}}"#);
        assert!(!acc.is_finished());
        acc.push_line("data: [DONE]");
        assert!(acc.is_finished());
        assert_eq!(d1.as_deref(), Some("Ho"));
        assert_eq!(d2.as_deref(), Some("la"));
        assert!(d3.is_none());
        assert!(d4.is_none());
        let resp = acc.into_response();
        assert_eq!(resp.content, "Hola");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].id, "c1");
        assert_eq!(resp.tool_calls[0].name, "tasks.create");
        assert_eq!(resp.tool_calls[0].arguments["title"], "X");
        assert_eq!(resp.usage.prompt_tokens, 7);
        assert_eq!(resp.usage.completion_tokens, 3);
    }

    #[test]
    fn rate_limiter_blocks_then_allows() {
        let limiter = RateLimiter::new(2);
        assert!(limiter.try_acquire().is_ok());
        assert!(limiter.try_acquire().is_ok());
        let wait = limiter.try_acquire().unwrap_err();
        assert!(wait > Duration::from_secs(50));
    }

    #[tokio::test]
    async fn rate_limiter_times_out_with_limit_exceeded() {
        let limiter = RateLimiter::new(1);
        limiter.acquire().await.unwrap();
        let err = limiter.acquire().await.unwrap_err();
        assert!(matches!(err, NexusError::LimitExceeded(_)));
    }
}
