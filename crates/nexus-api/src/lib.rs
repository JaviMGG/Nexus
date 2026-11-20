//! nexus-api: servicio HTTP local (Axum) sobre el monolito.
//! Escucha SOLO en loopback. Endpoints:
//! - GET  /api/health   → estado del servicio
//! - GET  /api/session  → token de sesión (solo con Host loopback)
//! - POST /api/command  → {command, payload} → dispatch del monolito
//! - GET  /api/events   → SSE con eventos de aplicación (agentes, datos, IA)

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::Stream;
use nexus_app::AppContext;
use nexus_core::{AppEvent, NexusError};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct ApiState {
    ctx: Arc<AppContext>,
    token: String,
}

#[derive(Debug, Deserialize)]
struct CommandBody {
    command: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

fn error_response(e: NexusError) -> Response {
    let status = match &e {
        NexusError::NotFound(_) => StatusCode::NOT_FOUND,
        NexusError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
        NexusError::PermissionDenied(_) => StatusCode::FORBIDDEN,
        NexusError::Conflict(_) => StatusCode::CONFLICT,
        NexusError::AiUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        NexusError::Cancelled => StatusCode::OK,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(json!({"error": e}))).into_response()
}

/// Protección básica anti DNS-rebinding: solo se aceptan Host loopback.
fn host_is_loopback(headers: &HeaderMap) -> bool {
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let name = host.rsplit_once(':').map(|(n, _)| n).unwrap_or(host);
    matches!(name, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

fn authorized(state: &ApiState, headers: &HeaderMap, query_token: Option<&str>) -> bool {
    if let Some(auth) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(bearer) = auth.strip_prefix("Bearer ") {
            return bearer == state.token;
        }
    }
    query_token == Some(state.token.as_str())
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "service": "nexus", "version": env!("CARGO_PKG_VERSION")}))
}

async fn session(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    if !host_is_loopback(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": {"code": "forbidden", "message": "host no permitido"}})),
        )
            .into_response();
    }
    Json(json!({"token": state.token})).into_response()
}

async fn command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
    Json(body): Json<CommandBody>,
) -> Response {
    if !host_is_loopback(&headers) {
        return error_response(NexusError::PermissionDenied("host no permitido".into()));
    }
    if !authorized(&state, &headers, q.token.as_deref()) {
        return error_response(NexusError::PermissionDenied(
            "token de sesión inválido".into(),
        ));
    }
    match state.ctx.dispatch(&body.command, body.payload).await {
        Ok(result) => Json(json!({"result": result})).into_response(),
        Err(e) => {
            tracing::warn!(command = body.command, error = %e, "comando fallido");
            error_response(e)
        }
    }
}

async fn events(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(q): Query<TokenQuery>,
) -> Response {
    if !host_is_loopback(&headers) || !authorized(&state, &headers, q.token.as_deref()) {
        return error_response(NexusError::PermissionDenied(
            "token de sesión inválido".into(),
        ));
    }
    let rx = state.ctx.events.subscribe();
    let stream = event_stream(rx);
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response()
}

fn event_stream(
    rx: broadcast::Receiver<AppEvent>,
) -> impl Stream<Item = Result<Event, std::convert::Infallible>> {
    futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let ev = Event::default()
                        .json_data(&event)
                        .unwrap_or_else(|_| Event::default().comment("error"));
                    return Some((Ok(ev), rx));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!("cliente SSE retrasado, {n} eventos descartados");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    })
}

pub fn router(ctx: Arc<AppContext>, token: String) -> Router {
    let state = ApiState { ctx, token };
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse().unwrap(),
            "http://127.0.0.1:5173".parse().unwrap(),
            "tauri://localhost".parse().unwrap(),
            "http://tauri.localhost".parse().unwrap(),
            "https://tauri.localhost".parse().unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .route("/api/health", get(health))
        .route("/api/session", get(session))
        .route("/api/command", post(command))
        .route("/api/events", get(events))
        .layer(cors)
        .with_state(state)
}

/// Arranca el servidor en loopback. Si el puerto está ocupado prueba los 10 siguientes.
/// Devuelve (puerto real, token de sesión).
pub async fn serve(ctx: Arc<AppContext>, preferred_port: u16) -> nexus_core::Result<(u16, String)> {
    let token = uuid::Uuid::new_v4().to_string();
    let mut app = router(ctx.clone(), token.clone());
    // Modo web instalado: si existe un build del frontend, se sirve en la raíz.
    let web_dir = std::env::var("NEXUS_WEB_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| ctx.data_dir.join("web"));
    if web_dir.join("index.html").exists() {
        tracing::info!("sirviendo frontend web desde {}", web_dir.display());
        app = app.fallback_service(tower_http::services::ServeDir::new(&web_dir));
    }
    let mut last_err = None;
    for port in preferred_port..preferred_port.saturating_add(10) {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                tracing::info!("servicio local NEXUS escuchando en http://{addr}");
                tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!("servidor HTTP terminado: {e}");
                    }
                });
                return Ok((port, token));
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(NexusError::Internal(format!(
        "no se pudo abrir el puerto {preferred_port}: {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    )))
}
