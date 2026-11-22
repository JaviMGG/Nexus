//! nexus-app: composition root del monolito modular.
//! Ensambla servicios, proveedores de IA, herramientas y agentes, y expone
//! un único punto de entrada (`dispatch`) usado por la API HTTP y por Tauri.

pub mod ai_json;
pub mod capture_ai;
pub mod context;
pub mod dispatch;
pub mod dispatch_canvas;
pub mod dispatch_databases;
pub mod dispatch_meals;

pub use context::AppContext;
