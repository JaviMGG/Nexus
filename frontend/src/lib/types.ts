export type Id = string;

export type TaskStatus = "todo" | "in_progress" | "done" | "cancelled";
export type Priority = "low" | "medium" | "high" | "urgent";
export type ProjectStatus = "active" | "paused" | "completed" | "archived";
export type NoteKind = "note" | "idea" | "document" | "link";
export type InboxStatus = "pending" | "processed" | "archived";
export type RunStatus = "pending" | "running" | "waiting_approval" | "completed" | "failed" | "cancelled";
export type ToolCallStatus = "pending" | "pending_approval" | "ok" | "error" | "denied";
export type ApprovalStatus = "pending" | "approved" | "rejected";
export type Risk = "read" | "write" | "destructive";
export type Policy = "auto" | "confirm";
export type AiState = "connected" | "disconnected" | "error";
export type MessageRole = "system" | "user" | "assistant" | "tool";
export type EventColor = "primary" | "success" | "warning" | "destructive" | "info";

export interface Task {
  id: Id;
  title: string;
  description: string;
  status: TaskStatus;
  priority: Priority;
  due_date: string | null;
  project_id: Id | null;
  parent_id: Id | null;
  category: string | null;
  tags: string[];
  position: number;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}

export interface Project {
  id: Id;
  name: string;
  description: string;
  status: ProjectStatus;
  priority: Priority;
  goal: string;
  created_at: string;
  updated_at: string;
}

export interface Note {
  id: Id;
  title: string;
  content: string;
  kind: NoteKind;
  url: string | null;
  project_id: Id | null;
  tags: string[];
  pinned: boolean;
  created_at: string;
  updated_at: string;
}

export interface CalendarEvent {
  id: Id;
  title: string;
  description: string;
  date: string;
  start_time: string | null;
  end_time: string | null;
  all_day: boolean;
  color: EventColor;
  location: string;
  project_id: Id | null;
  task_id: Id | null;
  created_at: string;
  updated_at: string;
}

export interface InboxItem {
  id: Id;
  content: string;
  status: InboxStatus;
  processed_into: Id | null;
  created_at: string;
}

export interface Conversation {
  id: Id;
  title: string;
  agent_id: string;
  created_at: string;
  updated_at: string;
}

export interface ChatMessage {
  id: Id;
  conversation_id: Id;
  role: MessageRole;
  content: string;
  agent_id: string | null;
  run_id: Id | null;
  tool_calls: unknown | null;
  created_at: string;
}

export interface AgentRun {
  id: Id;
  conversation_id: Id | null;
  agent_id: string;
  parent_run_id: Id | null;
  goal: string;
  status: RunStatus;
  result: string | null;
  error: string | null;
  iterations: number;
  tokens_used: number;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface ToolCall {
  id: Id;
  run_id: Id;
  tool_name: string;
  args: Record<string, unknown>;
  result: unknown | null;
  status: ToolCallStatus;
  error: string | null;
  duration_ms: number | null;
  created_at: string;
}

export interface Approval {
  id: Id;
  run_id: Id;
  tool_call_id: Id;
  tool_name: string;
  args: Record<string, unknown>;
  risk: Risk;
  status: ApprovalStatus;
  created_at: string;
  decided_at: string | null;
}

export interface ActivityEvent {
  id: Id;
  kind: string;
  entity_type: string;
  entity_id: Id | null;
  summary: string;
  actor: string;
  undo_json: { op: string; entity: string; id: Id } | null;
  created_at: string;
}

export interface CreateTask {
  title: string;
  description?: string;
  status?: TaskStatus;
  priority?: Priority;
  due_date?: string | null;
  project_id?: Id | null;
  parent_id?: Id | null;
  category?: string | null;
  tags?: string[];
}

export interface UpdateTask {
  title?: string;
  description?: string;
  status?: TaskStatus;
  priority?: Priority;
  due_date?: string | null;
  project_id?: Id | null;
  parent_id?: Id | null;
  category?: string | null;
  tags?: string[];
  position?: number;
}

export interface TaskFilter {
  status?: TaskStatus;
  project_id?: Id;
  priority?: Priority;
  search?: string;
  tag?: string;
  parent_id?: Id;
  root_only?: boolean;
  due_before?: string;
  include_deleted?: boolean;
  limit?: number;
}

export interface CreateProject {
  name: string;
  description?: string;
  status?: ProjectStatus;
  priority?: Priority;
  goal?: string;
}

export interface UpdateProject {
  name?: string;
  description?: string;
  status?: ProjectStatus;
  priority?: Priority;
  goal?: string;
}

export interface CreateNote {
  title: string;
  content?: string;
  kind?: NoteKind;
  url?: string | null;
  project_id?: Id | null;
  tags?: string[];
  pinned?: boolean;
}

export interface UpdateNote {
  title?: string;
  content?: string;
  kind?: NoteKind;
  url?: string | null;
  project_id?: Id | null;
  tags?: string[];
  pinned?: boolean;
}

export interface NoteFilter {
  search?: string;
  kind?: NoteKind;
  project_id?: Id;
  tag?: string;
  limit?: number;
}

export interface CreateEvent {
  title: string;
  date: string;
  description?: string;
  start_time?: string | null;
  end_time?: string | null;
  all_day?: boolean;
  color?: EventColor;
  location?: string;
  project_id?: Id | null;
  task_id?: Id | null;
}

export interface UpdateEvent {
  title?: string;
  description?: string;
  date?: string;
  start_time?: string | null;
  end_time?: string | null;
  all_day?: boolean;
  color?: EventColor;
  location?: string | null;
  project_id?: Id | null;
  task_id?: Id | null;
}

export interface EventFilter {
  from?: string;
  to?: string;
  project_id?: Id;
  include_deleted?: boolean;
}

export interface AiStatus {
  state: AiState;
  provider: string | null;
  model: string | null;
  message: string | null;
}

export interface ProviderConfig {
  id: Id;
  name: string;
  kind: "openai_compatible";
  base_url: string;
  model: string;
  api_key_set: boolean;
  temperature: number;
  max_tokens: number;
  enabled: boolean;
  is_default: boolean;
  requests_per_minute: number;
}

export interface AgentPermissions {
  read: Policy;
  write: Policy;
  destructive: Policy;
}

export interface UsageLimits {
  max_iterations: number;
  max_tokens_per_run: number;
  max_delegation_depth: number;
  requests_per_minute: number;
}

export interface AppSettings {
  user_name: string;
  theme: string;
  palette: string;
  permissions: AgentPermissions;
  limits: UsageLimits;
  default_provider: Id | null;
}

export interface AgentDef {
  id: string;
  name: string;
  description: string;
  tools: string[];
  max_iterations: number;
}

export interface ProjectProgress {
  project: Project;
  total: number;
  done: number;
  progress: number;
  overdue: number;
}

export interface ProjectDetail {
  project: Project;
  tasks: Task[];
  notes: Note[];
  progress: ProjectProgress;
  activity: ActivityEvent[];
}

export interface TaskDetail {
  task: Task;
  subtasks: Task[];
}

export interface DashboardStats {
  open_tasks: number;
  completed_today: number;
  completed_week: number;
  overdue: number;
  total_notes: number;
  inbox_pending: number;
  active_projects: number;
}

export interface DashboardSummary {
  user_name: string;
  today: string;
  tasks_today: Task[];
  overdue: Task[];
  priority_tasks: Task[];
  upcoming: Task[];
  active_projects: ProjectProgress[];
  stats: DashboardStats;
  recent_activity: ActivityEvent[];
  recent_runs: AgentRun[];
}

export interface CaptureResult {
  kind: "task" | "note";
  task: CreateTask | null;
  note: CreateNote | null;
  explanation: string;
  ai_used: boolean;
}

export interface ChatSendResult {
  conversation_id: Id;
  run_id: Id | null;
  user_message: ChatMessage;
  ai_available: boolean;
}

export interface RunDetail {
  run: AgentRun;
  tool_calls: ToolCall[];
  approvals: Approval[];
  children: AgentRun[];
}

export interface SettingsPayload {
  app: AppSettings;
  providers: ProviderConfig[];
  ai_status: AiStatus;
}

export type AppEvent =
  | { type: "run_started"; run: AgentRun }
  | { type: "run_updated"; run: AgentRun }
  | { type: "run_finished"; run: AgentRun }
  | { type: "message_delta"; conversation_id: Id; run_id: Id; delta: string }
  | { type: "message_created"; message: ChatMessage }
  | { type: "tool_started"; call: ToolCall }
  | { type: "tool_finished"; call: ToolCall }
  | { type: "approval_requested"; approval: Approval }
  | { type: "approval_resolved"; approval: Approval }
  | { type: "ai_status_changed"; status: AiStatus }
  | { type: "data_changed"; entity: string };

export const PALETTES = [
  { id: "violet", name: "Violet", swatch: "#8b5cf6" },
  { id: "graphite", name: "Graphite", swatch: "#a1a1aa" },
  { id: "ocean", name: "Ocean", swatch: "#38bdf8" },
  { id: "forest", name: "Forest", swatch: "#34d399" },
  { id: "sunset", name: "Sunset", swatch: "#fb923c" },
  { id: "rose", name: "Rose", swatch: "#fb7185" },
] as const;

export type PaletteId = (typeof PALETTES)[number]["id"];

export const EVENT_COLORS: { id: EventColor; label: string }[] = [
  { id: "primary", label: "Principal" },
  { id: "success", label: "Verde" },
  { id: "warning", label: "Ámbar" },
  { id: "destructive", label: "Rojo" },
  { id: "info", label: "Azul" },
];
