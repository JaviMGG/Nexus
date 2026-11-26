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
        field("description", "Descripción", K::Text, vec![], false, "Cadena UTF-8"),
        field(
            "status",
            "Estado",
            K::Enum,
            vec!["todo".into(), "in_progress".into(), "done".into(), "cancelled".into()],
            false,
            "Uno de: todo,in_progress,done,cancelled",
        ),
        field(
            "priority",
            "Prioridad",
            K::Enum,
            vec!["low".into(), "medium".into(), "high".into(), "urgent".into()],
            false,
            "Uno de: low,medium,high,urgent",
        ),
        field(
            "due_date",
            "Fecha de vencimiento",
            K::Date,
            vec![],
            true,
            "YYYY-MM-DD o RFC3339",
        ),
        field("project_id", "ID del proyecto", K::Id, vec![], true, "UUID v4 en texto"),
        field(
            "parent_id",
            "ID de la tarea padre",
            K::Id,
            vec![],
            true,
            "UUID v4 en texto",
        ),
        field("category", "Categoría", K::Text, vec![], true, "Cadena UTF-8"),
        field("tags", "Etiquetas", K::Tags, vec![], false, "Array de cadenas UTF-8"),
        field("position", "Posición", K::Number, vec![], false, "Entero i64"),
        field("created_at", "Creado el", K::DateTime, vec![], false, "RFC3339"),
        field("updated_at", "Actualizado el", K::DateTime, vec![], false, "RFC3339"),
        field("completed_at", "Completado el", K::DateTime, vec![], true, "RFC3339"),
    ]
}
