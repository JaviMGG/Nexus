//! nexus-core: dominio puro de NEXUS.
//! Tipos, entidades, DTOs y errores compartidos por todos los módulos del monolito.
//! No depende de ningún otro crate del workspace ni de infraestructura.

pub mod error;
pub mod types;

pub use error::{NexusError, Result};
pub use types::*;
