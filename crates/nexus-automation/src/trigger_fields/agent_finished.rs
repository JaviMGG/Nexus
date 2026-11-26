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
        field("id", "ID", K::Id, vec![], false, "UUID v4 en texto"),
        field("agent_id", "ID del agente", K::Id, vec![], false, "UUID v4 en texto"),
        field(
            "status",
            "Estado",
            K::Enum,
            vec![
                "pending".into(),
                "running".into(),
                "waiting_approval".into(),
                "completed".into(),
                "failed".into(),
                "cancelled".into(),
            ],
            false,
            "Uno de: pending,running,waiting_approval,completed,failed,cancelled",
        ),
        field("goal", "Objetivo", K::Text, vec![], false, "Cadena UTF-8"),
        field("result", "Resultado", K::Text, vec![], true, "Cadena UTF-8"),
    ]
}
