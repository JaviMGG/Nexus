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

export interface PaletteDef {
  id: string;
  name: string;
  swatch: string;
  group: "basicas" | "curadas";
  description?: string;
  /** Colores reales del tema oscuro para la mini-previsualización [fondo, tarjeta, primario, acento] */
  preview: [string, string, string, string];
}

export const PALETTES: PaletteDef[] = [
  { id: "violet", name: "Violet", swatch: "#8b5cf6", group: "basicas", preview: ["#0b0a12", "#12111a", "#8b5cf6", "#c4b5fd"] },
  { id: "graphite", name: "Graphite", swatch: "#a1a1aa", group: "basicas", preview: ["#0a0a0b", "#111113", "#d4d4d8", "#71717a"] },
  { id: "ocean", name: "Ocean", swatch: "#38bdf8", group: "basicas", preview: ["#081018", "#0d1722", "#38bdf8", "#7dd3fc"] },
  { id: "forest", name: "Forest", swatch: "#34d399", group: "basicas", preview: ["#07120d", "#0c1a13", "#34d399", "#6ee7b7"] },
  { id: "sunset", name: "Sunset", swatch: "#fb923c", group: "basicas", preview: ["#140d08", "#1c130c", "#fb923c", "#fdba74"] },
  { id: "rose", name: "Rose", swatch: "#fb7185", group: "basicas", preview: ["#140a10", "#1c0f16", "#fb7185", "#fda4af"] },
  {
    id: "nord", name: "Nord", swatch: "#88c0d0", group: "curadas",
    description: "Azul ártico sereno, inspirado en el tema Nord",
    preview: ["#242933", "#2e3440", "#88c0d0", "#81a1c1"],
  },
  {
    id: "dracula", name: "Dracula", swatch: "#bd93f9", group: "curadas",
    description: "Púrpura nocturno con acentos cian y rosa",
    preview: ["#191a21", "#21222c", "#bd93f9", "#ff79c6"],
  },
  {
    id: "tokyo", name: "Tokyo Night", swatch: "#7aa2f7", group: "curadas",
    description: "Noche de neón azul sobre índigo profundo",
    preview: ["#16161e", "#1a1b26", "#7aa2f7", "#bb9af7"],
  },
  {
    id: "catppuccin", name: "Catppuccin", swatch: "#cba6f7", group: "curadas",
    description: "Pasteles suaves sobre base mauve (Mocha)",
    preview: ["#11111b", "#181825", "#cba6f7", "#f5c2e7"],
  },
  {
    id: "gruvbox", name: "Gruvbox", swatch: "#d79921", group: "curadas",
    description: "Retro cálido: mostaza, tierra y verde oliva",
    preview: ["#1d2021", "#282828", "#d79921", "#b8bb26"],
  },
  {
    id: "everforest", name: "Everforest", swatch: "#a7c080", group: "curadas",
    description: "Bosque brumoso, verdes apagados y cálidos",
    preview: ["#272e33", "#2d353b", "#a7c080", "#dbbc7f"],
  },
  {
    id: "kanagawa", name: "Kanagawa", swatch: "#7e9cd8", group: "curadas",
    description: "Ola japonesa: índigo, oro viejo y sakura",
    preview: ["#16161d", "#1f1f28", "#7e9cd8", "#dcd7ba"],
  },
  {
    id: "cyberpunk", name: "Cyberpunk", swatch: "#ff2a6d", group: "curadas",
    description: "Neón magenta y cian eléctrico sobre negro",
    preview: ["#0a0a12", "#10101c", "#ff2a6d", "#05d9e8"],
  },
];

export type PaletteId = string;

export const EVENT_COLORS: { id: EventColor; label: string }[] = [
  { id: "primary", label: "Principal" },
  { id: "success", label: "Verde" },
  { id: "warning", label: "Ámbar" },
  { id: "destructive", label: "Rojo" },
  { id: "info", label: "Azul" },
];
