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
        field("title", "Título", K::Text, vec![], false, "Cadena UTF-8"),
        field("content", "Contenido", K::Text, vec![], false, "Cadena UTF-8"),
        field(
            "kind",
            "Tipo",
            K::Enum,
            vec!["note".into(), "idea".into(), "document".into(), "link".into()],
            false,
            "Uno de: note,idea,document,link",
        ),
        field("url", "URL", K::Text, vec![], true, "Cadena UTF-8"),
        field("project_id", "ID del proyecto", K::Id, vec![], true, "UUID v4 en texto"),
        field("tags", "Etiquetas", K::Tags, vec![], false, "Array de cadenas UTF-8"),
        field("pinned", "Fijado", K::Boolean, vec![], false, "Booleano"),
        field("created_at", "Creado el", K::DateTime, vec![], false, "RFC3339"),
        field("updated_at", "Actualizado el", K::DateTime, vec![], false, "RFC3339"),
    ]
}
