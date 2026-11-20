use nexus_core::{NexusError, Result};

pub fn title(s: &str, field: &str) -> Result<String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(NexusError::validation(format!(
            "{field} no puede estar vacío"
        )));
    }
    if t.chars().count() > 300 {
        return Err(NexusError::validation(format!(
            "{field} demasiado largo (máx. 300 caracteres)"
        )));
    }
    Ok(t.to_string())
}

pub fn long_text(s: &str, field: &str, max: usize) -> Result<String> {
    if s.chars().count() > max {
        return Err(NexusError::validation(format!(
            "{field} demasiado largo (máx. {max} caracteres)"
        )));
    }
    Ok(s.to_string())
}

pub fn tags(tags: &[String]) -> Result<Vec<String>> {
    if tags.len() > 20 {
        return Err(NexusError::validation("máximo 20 etiquetas"));
    }
    let mut out = Vec::new();
    for t in tags {
        let t = t.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > 40 {
            return Err(NexusError::validation("etiqueta demasiado larga (máx. 40)"));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    Ok(out)
}

/// Acepta YYYY-MM-DD o RFC3339.
pub fn due_date(s: &str) -> Result<String> {
    let s = s.trim();
    if chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() {
        return Ok(s.to_string());
    }
    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
        return Ok(s.to_string());
    }
    Err(NexusError::validation(format!(
        "fecha límite inválida: {s} (usa YYYY-MM-DD)"
    )))
}

pub fn url(s: &str) -> Result<String> {
    let s = s.trim();
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        return Err(NexusError::validation(
            "la URL debe empezar por http:// o https://",
        ));
    }
    if s.chars().count() > 2000 {
        return Err(NexusError::validation("URL demasiado larga"));
    }
    Ok(s.to_string())
}
