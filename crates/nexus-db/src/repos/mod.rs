pub mod activity;
pub mod chat;
pub mod events;
pub mod inbox;
pub mod notes;
pub mod projects;
pub mod runs;
pub mod settings;
pub mod tasks;

pub(crate) fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".into())
}

pub(crate) fn tags_from_json(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}
