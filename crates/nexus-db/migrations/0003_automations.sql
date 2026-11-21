-- NEXUS: Automation Center (motor local de automatizaciones)
CREATE TABLE IF NOT EXISTS automations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    icon TEXT NOT NULL DEFAULT 'zap',
    enabled INTEGER NOT NULL DEFAULT 0,
    mode TEXT NOT NULL DEFAULT 'local',
    trigger_json TEXT NOT NULL,
    conditions_json TEXT,
    flow_json TEXT NOT NULL,
    permissions_json TEXT NOT NULL DEFAULT '{}',
    limits_json TEXT NOT NULL DEFAULT '{}',
    policy TEXT NOT NULL DEFAULT 'skip_if_active',
    template_id TEXT,
    last_run_at TEXT,
    next_run_at TEXT,
    run_count INTEGER NOT NULL DEFAULT 0,
    last_status TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_automations_enabled ON automations(enabled);
CREATE INDEX IF NOT EXISTS idx_automations_next_run ON automations(next_run_at);

CREATE TABLE IF NOT EXISTS automation_executions (
    id TEXT PRIMARY KEY,
    automation_id TEXT NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    automation_name TEXT NOT NULL DEFAULT '',
    trigger_kind TEXT NOT NULL,
    trigger_detail TEXT NOT NULL DEFAULT '',
    mode TEXT NOT NULL DEFAULT 'local',
    status TEXT NOT NULL DEFAULT 'pending',
    test_run INTEGER NOT NULL DEFAULT 0,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    agent_calls INTEGER NOT NULL DEFAULT 0,
    steps_done INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    result_json TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    duration_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_auto_exec_automation ON automation_executions(automation_id);
CREATE INDEX IF NOT EXISTS idx_auto_exec_status ON automation_executions(status);
CREATE INDEX IF NOT EXISTS idx_auto_exec_created ON automation_executions(created_at);

CREATE TABLE IF NOT EXISTS automation_step_logs (
    id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES automation_executions(id) ON DELETE CASCADE,
    step_id TEXT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    detail_json TEXT,
    error TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_auto_steps_exec ON automation_step_logs(execution_id);

CREATE TABLE IF NOT EXISTS automation_approvals (
    id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES automation_executions(id) ON DELETE CASCADE,
    automation_id TEXT NOT NULL,
    step_id TEXT,
    action TEXT NOT NULL,
    params_json TEXT NOT NULL DEFAULT '{}',
    preview TEXT NOT NULL DEFAULT '',
    agent_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    decided_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_auto_approvals_status ON automation_approvals(status);
CREATE INDEX IF NOT EXISTS idx_auto_approvals_exec ON automation_approvals(execution_id);

-- Marcas de disparo para prevención de duplicados (p. ej. task.overdue por día).
CREATE TABLE IF NOT EXISTS automation_fired_marks (
    automation_id TEXT NOT NULL,
    mark_key TEXT NOT NULL,
    fired_at TEXT NOT NULL,
    PRIMARY KEY (automation_id, mark_key)
);
