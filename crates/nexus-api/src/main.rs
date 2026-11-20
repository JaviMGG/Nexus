//! Servidor headless de NEXUS para desarrollo web o uso sin escritorio.
//! Uso: cargo run -p nexus-api   (frontend: npm run dev en /frontend)

use std::path::PathBuf;
use std::sync::Arc;

use nexus_app::AppContext;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexus=info,tower_http=warn".into()),
        )
        .init();

    let data_dir = std::env::var("NEXUS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let base = std::env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
                        .join(".local/share")
                });
            base.join("nexus")
        });

    let ctx = match AppContext::new(data_dir.clone()).await {
        Ok(ctx) => Arc::new(ctx),
        Err(e) => {
            eprintln!("Error iniciando NEXUS: {e}");
            std::process::exit(1);
        }
    };

    let port: u16 = std::env::var("NEXUS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47800);
    match nexus_api::serve(ctx, port).await {
        Ok((port, token)) => {
            println!("NEXUS listo.");
            println!("  Datos:    {}", data_dir.display());
            println!("  API:      http://127.0.0.1:{port}");
            println!("  Frontend: http://localhost:5173 (npm run dev en frontend/)");
            println!("  Token de sesión: {token}");
            // Mantener el proceso vivo hasta Ctrl+C.
            tokio::signal::ctrl_c().await.ok();
            println!("Cerrando NEXUS…");
        }
        Err(e) => {
            eprintln!("Error iniciando el servidor: {e}");
            std::process::exit(1);
        }
    }
}
