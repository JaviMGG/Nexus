//! Captura inteligente: usa el proveedor de IA si está disponible y cae
//! siempre a la heurística local si no lo está o si falla. Nunca bloquea.

use nexus_ai::{AiMessage, ChatRequest};
use nexus_core::CaptureResult;
use nexus_services::capture::heuristic_capture;

use crate::context::AppContext;

const CAPTURE_PROMPT: &str = r#"Eres el módulo de captura rápida de NEXUS, un centro de control personal.
Analiza el texto del usuario y decide si es una TAREA (algo que hacer) o una NOTA (información, idea, enlace).
Extrae los datos y responde EXCLUSIVAMENTE con un JSON válido, sin markdown ni texto adicional, con esta forma:
{
  "kind": "task" | "note",
  "task": {"title": string, "description": string, "priority": "low"|"medium"|"high"|"urgent", "due_date": "YYYY-MM-DD"|null, "tags": string[]} | null,
  "note": {"title": string, "content": string, "kind": "note"|"idea"|"document"|"link", "tags": string[]} | null,
  "explanation": string (breve, en español)
}
Reglas: título conciso en español; fechas relativas resueltas contra la fecha actual; si no hay fecha, null;
no inventes información que el usuario no haya dado."#;

pub async fn analyze(ctx: &AppContext, content: &str) -> CaptureResult {
    let fallback = heuristic_capture(content);
    let provider = match ctx.providers.default_provider().await {
        Ok(p) => p,
        Err(_) => return fallback,
    };
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let req = ChatRequest {
        messages: vec![
            AiMessage::system(format!("{CAPTURE_PROMPT}\nFecha actual: {today}")),
            AiMessage::user(content.to_string()),
        ],
        tools: vec![],
        temperature: Some(0.2),
        max_tokens: Some(800),
    };
    let response =
        match tokio::time::timeout(std::time::Duration::from_secs(25), provider.chat(req)).await {
            Ok(Ok(r)) => r,
            _ => return fallback,
        };
    match parse_capture(&response.content) {
        Some(mut r) => {
            r.ai_used = true;
            r
        }
        None => fallback,
    }
}

fn parse_capture(content: &str) -> Option<CaptureResult> {
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
    let start = json_str.find('{')?;
    let end = json_str.rfind('}')?;
    let mut result: CaptureResult = serde_json::from_str(&json_str[start..=end]).ok()?;
    if result.kind != "task" && result.kind != "note" {
        return None;
    }
    if result.kind == "task" && result.task.is_none() {
        return None;
    }
    if result.kind == "note" && result.note.is_none() {
        return None;
    }
    if let Some(t) = &mut result.task {
        if t.title.trim().is_empty() {
            return None;
        }
    }
    if let Some(n) = &mut result.note {
        if n.title.trim().is_empty() {
            return None;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json() {
        let r = parse_capture(
            r#"{"kind":"task","task":{"title":"Probar"},"note":null,"explanation":"ok","ai_used":false}"#,
        );
        assert!(r.is_some());
        assert_eq!(r.unwrap().kind, "task");
    }

    #[test]
    fn parses_fenced_json() {
        let r = parse_capture("```json\n{\"kind\":\"note\",\"task\":null,\"note\":{\"title\":\"Idea\",\"content\":\"x\"},\"explanation\":\"ok\",\"ai_used\":false}\n```");
        assert!(r.is_some());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_capture("no soy json").is_none());
        assert!(parse_capture(
            r#"{"kind":"task","task":null,"note":null,"explanation":"","ai_used":false}"#
        )
        .is_none());
    }
}
