use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Id = String;

pub fn new_id() -> Id {
    uuid::Uuid::new_v4().to_string()
}

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn now_str() -> String {
    now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Enums de dominio
// ---------------------------------------------------------------------------

macro_rules! str_enum {
    ($name:ident, $($variant:ident => $s:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$variant),)+ _ => None }
            }
            pub fn all() -> &'static [&'static str] {
                &[$($s),+]
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

str_enum!(TaskStatus, Todo => "todo", InProgress => "in_progress", Done => "done", Cancelled => "cancelled");
str_enum!(Priority, Low => "low", Medium => "medium", High => "high", Urgent => "urgent");
str_enum!(ProjectStatus, Active => "active", Paused => "paused", Completed => "completed", Archived => "archived");
str_enum!(NoteKind, Note => "note", Idea => "idea", Document => "document", Link => "link");
str_enum!(InboxStatus, Pending => "pending", Processed => "processed", Archived => "archived");
str_enum!(RunStatus, Pending => "pending", Running => "running", WaitingApproval => "waiting_approval", Completed => "completed", Failed => "failed", Cancelled => "cancelled");
str_enum!(ToolCallStatus, Pending => "pending", PendingApproval => "pending_approval", Ok => "ok", Error => "error", Denied => "denied");
str_enum!(ApprovalStatus, Pending => "pending", Approved => "approved", Rejected => "rejected");
str_enum!(Risk, Read => "read", Write => "write", Destructive => "destructive");
str_enum!(Policy, Auto => "auto", Confirm => "confirm");
str_enum!(AiState, Connected => "connected", Disconnected => "disconnected", Error => "error");
str_enum!(MessageRole, System => "system", User => "user", Assistant => "assistant", Tool => "tool");
str_enum!(ProviderKind, OpenaiCompatible => "openai_compatible");
str_enum!(EventColor, Primary => "primary", Success => "success", Warning => "warning", Destructive => "destructive", Info => "info");

#[allow(clippy::derivable_impls)]
impl Default for TaskStatus {
    fn default() -> Self {
        Self::Todo
    }
}
#[allow(clippy::derivable_impls)]
impl Default for Priority {
    fn default() -> Self {
        Self::Medium
    }
}
#[allow(clippy::derivable_impls)]
impl Default for ProjectStatus {
    fn default() -> Self {
        Self::Active
    }
}
#[allow(clippy::derivable_impls)]
impl Default for NoteKind {
    fn default() -> Self {
        Self::Note
    }
}
#[allow(clippy::derivable_impls)]
impl Default for EventColor {
    fn default() -> Self {
        Self::Primary
    }
}

// ---------------------------------------------------------------------------
// Entidades
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Id,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: Priority,
    /// Fecha límite en formato YYYY-MM-DD o RFC3339.
    pub due_date: Option<String>,
    pub project_id: Option<Id>,
    pub parent_id: Option<Id>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub status: ProjectStatus,
    pub priority: Priority,
    pub goal: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub title: String,
    pub content: String,
    pub kind: NoteKind,
    pub url: Option<String>,
    pub project_id: Option<Id>,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: Id,
    pub title: String,
    pub description: String,
    /// Fecha del evento en formato YYYY-MM-DD.
    pub date: String,
    /// Hora de inicio HH:MM; `None` si es de todo el día.
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub all_day: bool,
    pub color: EventColor,
    pub location: String,
    pub project_id: Option<Id>,
    pub task_id: Option<Id>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: Id,
    pub content: String,
    pub status: InboxStatus,
    pub processed_into: Option<Id>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Id,
    pub title: String,
    pub agent_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Id,
    pub conversation_id: Id,
    pub role: MessageRole,
    pub content: String,
    pub agent_id: Option<String>,
    pub run_id: Option<Id>,
    pub tool_calls: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: Id,
    pub conversation_id: Option<Id>,
    pub agent_id: String,
    pub parent_run_id: Option<Id>,
    pub goal: String,
    pub status: RunStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub iterations: i64,
    pub tokens_used: i64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: Id,
    pub run_id: Id,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub status: ToolCallStatus,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub id: Id,
    pub run_id: Id,
    pub tool_call_id: Id,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub risk: Risk,
    pub status: ApprovalStatus,
    pub created_at: String,
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub id: Id,
    pub kind: String,
    pub entity_type: String,
    pub entity_id: Option<Id>,
    pub summary: String,
    pub actor: String,
    pub undo_json: Option<serde_json::Value>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// DTOs de entrada
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateTask {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    pub due_date: Option<String>,
    pub project_id: Option<Id>,
    pub parent_id: Option<Id>,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    /// `null` borra la fecha; ausente la conserva; cadena la fija.
    pub due_date: Option<Option<String>>,
    pub project_id: Option<Option<Id>>,
    pub parent_id: Option<Option<Id>>,
    pub category: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub position: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub project_id: Option<Id>,
    pub priority: Option<Priority>,
    pub search: Option<String>,
    pub tag: Option<String>,
    pub parent_id: Option<Id>,
    pub root_only: Option<bool>,
    pub due_before: Option<String>,
    pub include_deleted: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub status: Option<ProjectStatus>,
    pub priority: Option<Priority>,
    #[serde(default)]
    pub goal: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<ProjectStatus>,
    pub priority: Option<Priority>,
    pub goal: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateNote {
    pub title: String,
    #[serde(default)]
    pub content: String,
    pub kind: Option<NoteKind>,
    pub url: Option<String>,
    pub project_id: Option<Id>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateNote {
    pub title: Option<String>,
    pub content: Option<String>,
    pub kind: Option<NoteKind>,
    pub url: Option<Option<String>>,
    pub project_id: Option<Option<Id>>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoteFilter {
    pub search: Option<String>,
    pub kind: Option<NoteKind>,
    pub project_id: Option<Id>,
    pub tag: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateEvent {
    pub title: String,
    pub date: String,
    #[serde(default)]
    pub description: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    #[serde(default)]
    pub all_day: bool,
    pub color: Option<EventColor>,
    #[serde(default)]
    pub location: String,
    pub project_id: Option<Id>,
    pub task_id: Option<Id>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateEvent {
    pub title: Option<String>,
    pub description: Option<String>,
    pub date: Option<String>,
    /// `null` borra la hora; ausente la conserva; cadena la fija.
    pub start_time: Option<Option<String>>,
    pub end_time: Option<Option<String>>,
    pub all_day: Option<bool>,
    pub color: Option<EventColor>,
    pub location: Option<Option<String>>,
    pub project_id: Option<Option<Id>>,
    pub task_id: Option<Option<Id>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    pub from: Option<String>,
    pub to: Option<String>,
    pub project_id: Option<Id>,
    pub include_deleted: Option<bool>,
}

// ---------------------------------------------------------------------------
// IA y configuración
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatus {
    pub state: AiState,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub message: Option<String>,
}

impl Default for AiStatus {
    fn default() -> Self {
        Self {
            state: AiState::Disconnected,
            provider: None,
            model: None,
            message: Some("Sin proveedor de IA configurado. NEXUS funciona en modo manual.".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: Id,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key_set: bool,
    pub temperature: f64,
    pub max_tokens: i64,
    pub enabled: bool,
    pub is_default: bool,
    pub requests_per_minute: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermissions {
    pub read: Policy,
    pub write: Policy,
    pub destructive: Policy,
}

impl Default for AgentPermissions {
    fn default() -> Self {
        Self {
            read: Policy::Auto,
            write: Policy::Auto,
            destructive: Policy::Confirm,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLimits {
    pub max_iterations: i64,
    pub max_tokens_per_run: i64,
    pub max_delegation_depth: i64,
    pub requests_per_minute: i64,
}

impl Default for UsageLimits {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            max_tokens_per_run: 32_000,
            max_delegation_depth: 2,
            requests_per_minute: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub user_name: String,
    pub theme: String,
    pub palette: String,
    pub permissions: AgentPermissions,
    pub limits: UsageLimits,
    pub default_provider: Option<Id>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            user_name: String::new(),
            theme: "dark".into(),
            palette: "violet".into(),
            permissions: AgentPermissions::default(),
            limits: UsageLimits::default(),
            default_provider: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub max_iterations: i64,
}

// ---------------------------------------------------------------------------
// Vistas compuestas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProgress {
    pub project: Project,
    pub total: i64,
    pub done: i64,
    pub progress: f64,
    pub overdue: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetail {
    pub project: Project,
    pub tasks: Vec<Task>,
    pub notes: Vec<Note>,
    pub progress: ProjectProgress,
    pub activity: Vec<ActivityEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task: Task,
    pub subtasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub open_tasks: i64,
    pub completed_today: i64,
    pub completed_week: i64,
    pub overdue: i64,
    pub total_notes: i64,
    pub inbox_pending: i64,
    pub active_projects: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub user_name: String,
    pub today: String,
    pub tasks_today: Vec<Task>,
    pub overdue: Vec<Task>,
    pub priority_tasks: Vec<Task>,
    pub upcoming: Vec<Task>,
    pub active_projects: Vec<ProjectProgress>,
    pub stats: DashboardStats,
    pub recent_activity: Vec<ActivityEvent>,
    pub recent_runs: Vec<AgentRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    pub kind: String,
    #[serde(default)]
    pub task: Option<CreateTask>,
    #[serde(default)]
    pub note: Option<CreateNote>,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub ai_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendResult {
    pub conversation_id: Id,
    pub run_id: Option<Id>,
    pub user_message: ChatMessage,
    pub ai_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetail {
    pub run: AgentRun,
    pub tool_calls: Vec<ToolCall>,
    pub approvals: Vec<Approval>,
    pub children: Vec<AgentRun>,
}

// ---------------------------------------------------------------------------
// Eventos de aplicación (SSE hacia el frontend)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    RunStarted {
        run: AgentRun,
    },
    RunUpdated {
        run: AgentRun,
    },
    RunFinished {
        run: AgentRun,
    },
    MessageDelta {
        conversation_id: Id,
        run_id: Id,
        delta: String,
    },
    MessageCreated {
        message: ChatMessage,
    },
    ToolStarted {
        call: ToolCall,
    },
    ToolFinished {
        call: ToolCall,
    },
    ApprovalRequested {
        approval: Approval,
    },
    ApprovalResolved {
        approval: Approval,
    },
    AiStatusChanged {
        status: AiStatus,
    },
    DataChanged {
        entity: String,
    },
}
