mod agent_finished;
mod goal_created;
mod idea_created;
mod mail_received;
mod note_created;
mod project_created;
mod project_status_changed;
mod task_completed;
mod task_created;
mod task_overdue;
mod task_updated;

use nexus_core::TriggerFieldSpec;

pub fn fields_for(event: &str) -> Vec<TriggerFieldSpec> {
    match event {
        "task.created" => task_created::fields(),
        "task.completed" => task_completed::fields(),
        "task.updated" => task_updated::fields(),
        "task.overdue" => task_overdue::fields(),
        "project.created" => project_created::fields(),
        "project.status_changed" => project_status_changed::fields(),
        "note.created" => note_created::fields(),
        "idea.created" => idea_created::fields(),
        "goal.created" => goal_created::fields(),
        "agent.finished" => agent_finished::fields(),
        "mail.received" => mail_received::fields(),
        _ => Vec::new(),
    }
}
