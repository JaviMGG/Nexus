use serde::Serialize;

pub type Result<T> = std::result::Result<T, NexusError>;

#[derive(Debug, thiserror::Error)]
pub enum NexusError {
    #[error("no encontrado: {0}")]
    NotFound(String),
    #[error("validación: {0}")]
    Validation(String),
    #[error("base de datos: {0}")]
    Database(String),
    #[error("conflicto: {0}")]
    Conflict(String),
    #[error("ia: {0}")]
    Ai(String),
    #[error("ia no disponible: {0}")]
    AiUnavailable(String),
    #[error("permiso denegado: {0}")]
    PermissionDenied(String),
    #[error("operación cancelada")]
    Cancelled,
    #[error("límite excedido: {0}")]
    LimitExceeded(String),
    #[error("interno: {0}")]
    Internal(String),
}

impl NexusError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Validation(_) => "validation",
            Self::Database(_) => "database",
            Self::Conflict(_) => "conflict",
            Self::Ai(_) => "ai",
            Self::AiUnavailable(_) => "ai_unavailable",
            Self::PermissionDenied(_) => "permission_denied",
            Self::Cancelled => "cancelled",
            Self::LimitExceeded(_) => "limit_exceeded",
            Self::Internal(_) => "internal",
        }
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn not_found(entity: &str, id: &str) -> Self {
        Self::NotFound(format!("{entity} {id}"))
    }
}

impl Serialize for NexusError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("NexusError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}
