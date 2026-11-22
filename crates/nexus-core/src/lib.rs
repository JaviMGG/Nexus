//! nexus-core: dominio puro de NEXUS.
//! Tipos, entidades, DTOs y errores compartidos por todos los módulos del monolito.
//! No depende de ningún otro crate del workspace ni de infraestructura.

pub mod automation;
pub mod canvas;
pub mod databases;
pub mod error;
pub mod meals;
pub mod types;

pub use automation::*;
pub use canvas::*;
pub use databases::*;
pub use error::{NexusError, Result};
pub use meals::*;
pub use types::*;
