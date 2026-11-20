//! Validación de argumentos de herramientas contra JSON Schema básico
//! (required + tipos) y política de aprobaciones según riesgo/permisos.

use nexus_core::{AgentPermissions, NexusError, Policy, Result, Risk};
use serde_json::Value;

use nexus_ai::ToolSpec;

/// Valida que `args` sea un objeto con los campos `required` presentes y
/// los tipos básicos correctos. Solo se soporta el subconjunto de JSON Schema
/// que usan las herramientas: string/number/integer/boolean/array/object.
pub fn validate_args(spec: &ToolSpec, args: &Value) -> Result<()> {
    let obj = args.as_object().ok_or_else(|| {
        NexusError::validation(format!(
            "{}: los argumentos deben ser un objeto JSON",
            spec.name
        ))
    })?;
    let schema = &spec.parameters;
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required {
            if let Some(name) = field.as_str() {
                match obj.get(name) {
                    None | Some(Value::Null) => {
                        return Err(NexusError::validation(format!(
                            "{}: falta el campo obligatorio '{name}'",
                            spec.name
                        )));
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (name, value) in obj {
            if value.is_null() {
                continue;
            }
            let Some(prop) = props.get(name) else {
                continue;
            };
            let ok = if let Some(list) = prop.get("type").and_then(Value::as_array) {
                list.iter()
                    .filter_map(Value::as_str)
                    .any(|t| type_matches(t, value))
            } else if let Some(ty) = prop.get("type").and_then(Value::as_str) {
                type_matches(ty, value)
            } else {
                true
            };
            if !ok {
                return Err(NexusError::validation(format!(
                    "{}: el campo '{name}' tiene un tipo incorrecto (esperado: {})",
                    spec.name,
                    prop.get("type").cloned().unwrap_or(Value::Null)
                )));
            }
            if let Some(values) = prop.get("enum").and_then(Value::as_array) {
                if !values.contains(value) {
                    let allowed: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                    return Err(NexusError::validation(format!(
                        "{}: el campo '{name}' debe ser uno de [{}]",
                        spec.name,
                        allowed.join(", ")
                    )));
                }
            }
        }
    }
    Ok(())
}

fn type_matches(ty: &str, value: &Value) -> bool {
    match ty {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
}

/// Matriz riesgo × política: `Confirm` siempre requiere aprobación; `Auto`
/// exige que el permiso correspondiente esté habilitado (también Auto).
pub fn needs_approval(risk: Risk, perms: &AgentPermissions) -> bool {
    let policy = match risk {
        Risk::Read => perms.read,
        Risk::Write => perms.write,
        Risk::Destructive => perms.destructive,
    };
    matches!(policy, Policy::Confirm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec() -> ToolSpec {
        ToolSpec {
            name: "demo".into(),
            description: "demo".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "count": { "type": "integer" },
                    "ratio": { "type": "number" },
                    "flag": { "type": "boolean" },
                    "tags": { "type": "array" },
                    "data": { "type": "object" },
                    "status": { "type": "string", "enum": ["todo", "done"] }
                },
                "required": ["title"]
            }),
        }
    }

    #[test]
    fn missing_required_fails() {
        let err = validate_args(&spec(), &json!({"count": 1})).unwrap_err();
        assert!(matches!(err, NexusError::Validation(_)));
        assert!(err.to_string().contains("title"));
    }

    #[test]
    fn wrong_type_fails() {
        assert!(validate_args(&spec(), &json!({"title": 42})).is_err());
        assert!(validate_args(&spec(), &json!({"title": "x", "count": "uno"})).is_err());
        assert!(validate_args(&spec(), &json!({"title": "x", "count": 1.5})).is_err());
        assert!(validate_args(&spec(), &json!({"title": "x", "flag": "true"})).is_err());
        assert!(validate_args(&spec(), &json!({"title": "x", "tags": "a,b"})).is_err());
        assert!(validate_args(&spec(), &json!("no objeto")).is_err());
        assert!(validate_args(&spec(), &json!({"title": "x", "status": "otro"})).is_err());
    }

    #[test]
    fn valid_args_pass() {
        assert!(validate_args(&spec(), &json!({"title": "x"})).is_ok());
        assert!(validate_args(
            &spec(),
            &json!({"title": "x", "count": 3, "ratio": 0.5, "flag": true, "tags": [], "data": {}, "status": "todo"})
        )
        .is_ok());
        assert!(validate_args(&spec(), &json!({"title": "x", "extra": 1})).is_ok());
    }

    #[test]
    fn approval_matrix() {
        for risk in [Risk::Read, Risk::Write, Risk::Destructive] {
            for read in [Policy::Auto, Policy::Confirm] {
                for write in [Policy::Auto, Policy::Confirm] {
                    for destructive in [Policy::Auto, Policy::Confirm] {
                        let perms = AgentPermissions {
                            read,
                            write,
                            destructive,
                        };
                        let expected = match risk {
                            Risk::Read => read == Policy::Confirm,
                            Risk::Write => write == Policy::Confirm,
                            Risk::Destructive => destructive == Policy::Confirm,
                        };
                        assert_eq!(needs_approval(risk, &perms), expected, "{risk:?} {perms:?}");
                    }
                }
            }
        }
        let default = AgentPermissions::default();
        assert!(!needs_approval(Risk::Read, &default));
        assert!(!needs_approval(Risk::Write, &default));
        assert!(needs_approval(Risk::Destructive, &default));
    }
}
