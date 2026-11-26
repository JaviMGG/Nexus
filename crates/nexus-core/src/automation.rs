//! Dominio del Automation Center: automatizaciones, triggers, condiciones,
//! pasos de flujo, ejecuciones, autorizaciones y plantillas.
//! Todo es serializable a JSON para persistirse en SQLite y viajar al frontend.

use serde::{Deserialize, Serialize};

use crate::types::{Id, NoteKind, Policy, Priority, ProjectStatus, TaskStatus};

// ---------------------------------------------------------------------------
// Enums
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
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

str_enum!(AutomationMode, Local => "local", Ai => "ai", Hybrid => "hybrid");
str_enum!(ExecutionStatus, Pending => "pending", Running => "running", WaitingApproval => "waiting_approval", Completed => "completed", Failed => "failed", Cancelled => "cancelled");
str_enum!(StepStatus, Ok => "ok", Error => "error", Skipped => "skipped", Deferred => "deferred", WaitingApproval => "waiting_approval", Cancelled => "cancelled");
str_enum!(TriggerKind, Schedule => "schedule", Event => "event");
str_enum!(ExecutionPolicy, Always => "always", SkipIfActive => "skip_if_active", Cooldown => "cooldown");
str_enum!(AutomationApprovalStatus, Pending => "pending", Approved => "approved", Rejected => "rejected", Expired => "expired");

#[allow(clippy::derivable_impls)]
impl Default for AutomationMode {
    fn default() -> Self {
        Self::Local
    }
}
#[allow(clippy::derivable_impls)]
impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self::SkipIfActive
    }
}

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

/// Disparador temporal. Las horas se interpretan en la zona horaria de la
/// automatización (IANA, p. ej. "Europe/Madrid"); por defecto la local del equipo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScheduleSpec {
    /// "daily" | "weekly" | "monthly" | "interval" | "once"
    pub kind: String,
    /// Hora local HH:MM (daily/weekly/monthly/once).
    pub time: Option<String>,
    /// Días de la semana "mon".."sun" (weekly).
    #[serde(default)]
    pub weekdays: Vec<String>,
    /// Día del mes 1-31; si el mes es más corto se usa el último día (monthly).
    pub day_of_month: Option<i64>,
    /// Minutos entre ejecuciones (interval), mínimo 5.
    pub interval_minutes: Option<i64>,
    /// Fecha YYYY-MM-DD para "once" o fecha límite de validez.
    pub date: Option<String>,
    /// Zona horaria IANA; None = hora local del sistema.
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Schedule {
        spec: ScheduleSpec,
    },
    Event {
        /// Clave del TriggerRegistry: task.created, task.completed, task.updated,
        /// task.overdue, project.created, project.status_changed, note.created,
        /// idea.created, goal.created, agent.finished.
        event: String,
        /// Filtro opcional sobre el payload del evento (claves planas).
        #[serde(default)]
        filter: serde_json::Value,
    },
}

impl Trigger {
    pub fn kind(&self) -> TriggerKind {
        match self {
            Trigger::Schedule { .. } => TriggerKind::Schedule,
            Trigger::Event { .. } => TriggerKind::Event,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerEntry {
    pub id: String,
    pub trigger: Trigger,
}

// ---------------------------------------------------------------------------
// Condiciones (árbol AND/OR/NOT + hojas deterministas, nunca SQL libre)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
}

impl CompareOp {
    pub fn apply(&self, left: &serde_json::Value, right: &serde_json::Value) -> bool {
        match self {
            CompareOp::Eq => left == right,
            CompareOp::Ne => left != right,
            CompareOp::Contains => match (left, right) {
                (serde_json::Value::String(a), serde_json::Value::String(b)) => {
                    a.to_lowercase().contains(&b.to_lowercase())
                }
                (serde_json::Value::Array(a), b) => a.contains(b),
                _ => false,
            },
            op => match (as_f64(left), as_f64(right)) {
                (Some(a), Some(b)) => match op {
                    CompareOp::Gt => a > b,
                    CompareOp::Gte => a >= b,
                    CompareOp::Lt => a < b,
                    CompareOp::Lte => a <= b,
                    _ => false,
                },
                _ => false,
            },
        }
    }
}

fn as_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Hoja del árbol de condiciones. `metric` se resuelve con datos reales de
/// NEXUS mediante consultas tipadas; el modelo de IA nunca genera SQL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    And {
        conditions: Vec<Condition>,
    },
    Or {
        conditions: Vec<Condition>,
    },
    Not {
        condition: Box<Condition>,
    },
    /// Métrica determinista: open_tasks, overdue_tasks, completed_today,
    /// completed_week, active_projects, project_progress, project_idle_days,
    /// note_matches, pending_tasks_in_project, event_field, last_execution_ok, ...
    Metric {
        metric: String,
        op: CompareOp,
        #[serde(default)]
        value: serde_json::Value,
        #[serde(default)]
        params: serde_json::Value,
    },
    /// Compara un campo del payload del evento con un valor literal.
    EventField {
        field: String,
        op: CompareOp,
        #[serde(default)]
        value: serde_json::Value,
    },
}

// ---------------------------------------------------------------------------
// Pasos del flujo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStep {
    pub id: Id,
    #[serde(flatten)]
    pub kind: StepKind,
    /// Siguiente paso por defecto (None = fin del flujo).
    pub next: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepKind {
    Condition {
        condition: Condition,
        on_true: Option<Id>,
        on_false: Option<Id>,
    },
    Action {
        action: String,
        #[serde(default)]
        params: serde_json::Value,
        /// auto = respeta la política de la automatización; confirm = siempre pide.
        #[serde(default)]
        approval: Option<Policy>,
    },
    Agent {
        agent_id: String,
        goal: String,
        #[serde(default)]
        approval: Option<Policy>,
    },
    Delay {
        seconds: i64,
    },
    Notification {
        title: String,
        #[serde(default)]
        body: String,
    },
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationFlow {
    pub version: i64,
    pub steps: Vec<FlowStep>,
    pub entry: Option<Id>,
    /// Posiciones de los nodos en el lienzo visual (id de paso → [x, y]).
    #[serde(default)]
    pub positions: std::collections::HashMap<Id, [f64; 2]>,
}

impl Default for AutomationFlow {
    fn default() -> Self {
        Self {
            version: 1,
            steps: Vec::new(),
            entry: None,
            positions: std::collections::HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Automatización
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationLimits {
    pub max_tokens: i64,
    pub max_agent_calls: i64,
    pub max_steps: i64,
    pub timeout_secs: i64,
    pub retries: i64,
    /// Mínimo de minutos entre dos ejecuciones de la misma automatización.
    pub cooldown_minutes: i64,
}

impl Default for AutomationLimits {
    fn default() -> Self {
        Self {
            max_tokens: 20_000,
            max_agent_calls: 3,
            max_steps: 100,
            timeout_secs: 300,
            retries: 0,
            cooldown_minutes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub enabled: bool,
    pub mode: AutomationMode,
    pub triggers: Vec<TriggerEntry>,
    #[serde(default)]
    pub next_runs: std::collections::HashMap<String, String>,
    pub conditions: Option<Condition>,
    pub flow: AutomationFlow,
    pub permissions: AgentPermissionsLite,
    pub limits: AutomationLimits,
    pub policy: ExecutionPolicy,
    pub template_id: Option<String>,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub run_count: i64,
    pub last_status: Option<ExecutionStatus>,
    pub created_at: String,
    pub updated_at: String,
}

/// Política de permisos propia de cada automatización (independiente de la global).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPermissionsLite {
    pub write: Policy,
}

impl Default for AgentPermissionsLite {
    fn default() -> Self {
        Self {
            write: Policy::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAutomation {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    pub enabled: Option<bool>,
    pub mode: Option<AutomationMode>,
    #[serde(default)]
    pub triggers: Vec<TriggerEntry>,
    pub conditions: Option<Condition>,
    pub flow: Option<AutomationFlow>,
    pub permissions: Option<AgentPermissionsLite>,
    pub limits: Option<AutomationLimits>,
    pub policy: Option<ExecutionPolicy>,
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAutomation {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub enabled: Option<bool>,
    pub mode: Option<AutomationMode>,
    pub triggers: Option<Vec<TriggerEntry>>,
    pub conditions: Option<Option<Condition>>,
    pub flow: Option<AutomationFlow>,
    pub permissions: Option<AgentPermissionsLite>,
    pub limits: Option<AutomationLimits>,
    pub policy: Option<ExecutionPolicy>,
}

// ---------------------------------------------------------------------------
// Ejecuciones e historial
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepLog {
    pub id: Id,
    pub execution_id: Id,
    pub step_id: Option<Id>,
    pub kind: String,
    pub name: String,
    pub status: StepStatus,
    pub detail: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationExecution {
    pub id: Id,
    pub automation_id: Id,
    pub automation_name: String,
    pub trigger_kind: TriggerKind,
    pub trigger_detail: String,
    pub mode: AutomationMode,
    pub status: ExecutionStatus,
    pub test_run: bool,
    pub tokens_used: i64,
    pub agent_calls: i64,
    pub steps_done: i64,
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetail {
    pub execution: AutomationExecution,
    pub steps: Vec<StepLog>,
    pub approvals: Vec<AutomationApproval>,
    pub agent_runs: Vec<crate::types::AgentRun>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionFilter {
    pub automation_id: Option<Id>,
    pub status: Option<ExecutionStatus>,
    pub test_run: Option<bool>,
    pub limit: Option<i64>,
}

/// Autorización de una operación sensible propuesta por una automatización.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationApproval {
    pub id: Id,
    pub execution_id: Id,
    pub automation_id: Id,
    pub step_id: Option<Id>,
    pub action: String,
    pub params: serde_json::Value,
    pub preview: String,
    pub agent_id: Option<String>,
    pub status: AutomationApprovalStatus,
    pub created_at: String,
    pub decided_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Plantillas y resumen
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub trigger_label: String,
    pub agents: Vec<String>,
    pub actions: Vec<String>,
    pub requires_ai: bool,
    pub default_mode: AutomationMode,
    pub estimated_tokens: i64,
    /// Dificultad/potencia: 1 = muy simple … 5 = avanzada.
    pub difficulty: i64,
    pub automation: CreateAutomation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsage {
    pub agent_id: String,
    pub calls: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationOverview {
    pub active_count: i64,
    pub total_count: i64,
    pub executions_total: i64,
    pub executions_ok: i64,
    pub executions_failed: i64,
    pub executions_today: i64,
    pub tokens_today: i64,
    pub tokens_total: i64,
    pub pending_approvals: i64,
    pub top_agents: Vec<AgentUsage>,
    pub recent_executions: Vec<AutomationExecution>,
    pub upcoming: Vec<Automation>,
    pub ai_status: crate::types::AiStatus,
    pub daily_token_limit: i64,
}

// ---------------------------------------------------------------------------
// Acciones tipadas (params de StepKind::Action)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateTaskParams {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub priority: Option<Priority>,
    pub due_date: Option<String>,
    pub project_id: Option<Id>,
    pub parent_id: Option<Id>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Guarda el id de la tarea creada en esta clave del contexto.
    pub save_as: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRefParams {
    /// Id literal o clave del contexto (p. ej. "task" del evento).
    pub task_id: String,
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    #[serde(default)]
    pub add_tags: Vec<String>,
    pub due_date: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateProjectParams {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub goal: String,
    pub priority: Option<Priority>,
    pub save_as: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectRefParams {
    pub project_id: String,
    pub status: Option<ProjectStatus>,
    pub description: Option<String>,
    pub goal: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateNoteParams {
    pub title: String,
    #[serde(default)]
    pub content: String,
    pub kind: Option<NoteKind>,
    pub project_id: Option<Id>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub save_as: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoteRefParams {
    pub note_id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub append_content: Option<String>,
    pub kind: Option<NoteKind>,
    pub project_id: Option<Id>,
    #[serde(default)]
    pub add_tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunAutomationParams {
    pub automation_id: Id,
}

/// Catálogo público de acciones: nombre, descripción y clase de riesgo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
    pub id: String,
    pub name: String,
    pub description: String,
    /// read | write | sensitive
    pub risk: String,
    pub entity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerFieldKind {
    Text,
    Date,
    DateTime,
    Number,
    Boolean,
    Enum,
    Tags,
    Id,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerFieldSpec {
    pub key: String,
    pub label: String,
    pub kind: TriggerFieldKind,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerSpec {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: TriggerKind,
    #[serde(default)]
    pub fields: Vec<TriggerFieldSpec>,
}
