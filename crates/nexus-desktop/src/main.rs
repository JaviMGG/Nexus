#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use nexus_app::AppContext;
use serde::Serialize;
use tauri::Manager;

#[derive(Clone, Serialize)]
struct ServerInfo {
    port: u16,
    token: String,
}

#[tauri::command]
fn server_info(state: tauri::State<'_, ServerInfo>) -> ServerInfo {
    state.inner().clone()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexus=info,tower_http=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let port: u16 = std::env::var("NEXUS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(47800);

            // El servicio local se arranca antes de mostrar la ventana para que
            // el frontend siempre encuentre la API disponible.
            let server = tauri::async_runtime::block_on(async {
                let ctx = Arc::new(AppContext::new(data_dir).await.map_err(|e| e.to_string())?);
                let (port, token) = nexus_api::serve(ctx, port)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(ServerInfo { port, token })
            })?;
            app.manage(server);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![server_info])
        .run(tauri::generate_context!())
        .expect("error ejecutando NEXUS");
}
