use nexus_core::{TriggerFieldKind as K, TriggerFieldSpec as F};

pub fn fields() -> Vec<F> {
    let field = |key: &str, label: &str, kind: K, options: Vec<String>, nullable: bool, hint: &str| F {
        key: key.into(),
        label: label.into(),
        kind,
        options,
        nullable,
        hint: hint.into(),
    };

    vec![
        field("message_id", "ID del mensaje", K::Id, vec![], false, "Identificador del mensaje"),
        field("account", "Cuenta", K::Text, vec![], false, "Alias de account_id, texto"),
        field("account_id", "ID de la cuenta", K::Id, vec![], false, "UUID v4 en texto"),
        field("folder", "Carpeta", K::Text, vec![], false, "Cadena UTF-8"),
        field("from", "Remitente", K::Text, vec![], false, "Cadena UTF-8"),
        field("subject", "Asunto", K::Text, vec![], false, "Cadena UTF-8"),
        field("date", "Fecha", K::DateTime, vec![], false, "RFC3339 o RFC2822"),
        field("snippet", "Fragmento", K::Text, vec![], false, "Cadena UTF-8"),
    ]
}
