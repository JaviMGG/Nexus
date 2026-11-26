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
        field("name", "Nombre", K::Text, vec![], false, "Cadena UTF-8"),
        field(
            "status",
            "Estado",
            K::Enum,
            vec!["active".into(), "paused".into(), "completed".into(), "archived".into()],
            false,
            "Uno de: active,paused,completed,archived",
        ),
        field(
            "previous_status",
            "Estado anterior",
            K::Enum,
            vec!["active".into(), "paused".into(), "completed".into(), "archived".into()],
            true,
            "Uno de: active,paused,completed,archived",
        ),
    ]
}
