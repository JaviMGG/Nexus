use nexus_core::{
    new_id, now_str, Automation, AutomationApproval, AutomationApprovalStatus, AutomationExecution,
    AutomationFlow, AutomationMode, Condition, ExecutionFilter, ExecutionPolicy, ExecutionStatus,
    Id, NexusError, Result, StepLog, StepStatus, Trigger, TriggerEntry, TriggerKind,
    UpdateAutomation,
};
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::{Db, DbResultExt};

// ---------------------------------------------------------------------------
// Automations
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct AutomationRow {
    id: String,
    name: String,
    description: String,
    icon: String,
    enabled: i64,
    mode: String,
    trigger_json: String,
    next_runs_json: String,
    conditions_json: Option<String>,
    flow_json: String,
    permissions_json: String,
    limits_json: String,
    policy: String,
    template_id: Option<String>,
    last_run_at: Option<String>,
    next_run_at: Option<String>,
    run_count: i64,
    last_status: Option<String>,
    created_at: String,
    updated_at: String,
}

fn parse_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|e| NexusError::Internal(format!("json corrupto: {e}")))
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StoredTriggers {
    Entries(Vec<TriggerEntry>),
    Legacy(Trigger),
}

fn normalize_trigger_ids(triggers: &mut [TriggerEntry]) {
    for entry in triggers {
        if entry.id.is_empty() {
            entry.id = new_id();
        }
    }
}

fn normalized_triggers(triggers: &[TriggerEntry]) -> Vec<TriggerEntry> {
    let mut result = triggers.to_vec();
    normalize_trigger_ids(&mut result);
    result
}

impl TryFrom<AutomationRow> for Automation {
    type Error = NexusError;

    fn try_from(r: AutomationRow) -> Result<Self> {
        let triggers = match serde_json::from_str::<StoredTriggers>(&r.trigger_json)
            .map_err(|e| NexusError::Internal(format!("trigger corrupto en {}: {e}", r.id)))?
        {
            StoredTriggers::Entries(v) => v,
            StoredTriggers::Legacy(trigger) => vec![TriggerEntry {
                id: "t0".into(),
                trigger,
            }],
        };
        let mut next_runs: std::collections::HashMap<String, String> =
            parse_json(&r.next_runs_json)?;
        if next_runs.is_empty() {
            if let (Some(next), Some(entry)) = (
                r.next_run_at.as_ref(),
                triggers.iter().find(|e| e.id == "t0"),
            ) {
                next_runs.insert(entry.id.clone(), next.clone());
            }
        }
        let conditions = match &r.conditions_json {
            Some(s) => Some(
                serde_json::from_str::<Condition>(s)
                    .map_err(|e| NexusError::Internal(format!("condición corrupta: {e}")))?,
            ),
            None => None,
        };
        let flow: AutomationFlow = serde_json::from_str(&r.flow_json)
            .map_err(|e| NexusError::Internal(format!("flujo corrupto en {}: {e}", r.id)))?;
        Ok(Self {
            id: r.id,
            name: r.name,
            description: r.description,
            icon: r.icon,
            enabled: r.enabled != 0,
            mode: AutomationMode::parse(&r.mode).unwrap_or_default(),
            triggers,
            next_runs,
            conditions,
            flow,
            permissions: parse_json(&r.permissions_json)?,
            limits: parse_json(&r.limits_json)?,
            policy: ExecutionPolicy::parse(&r.policy).unwrap_or_default(),
            template_id: r.template_id,
            last_run_at: r.last_run_at,
            next_run_at: r.next_run_at,
            run_count: r.run_count,
            last_status: r.last_status.as_deref().and_then(ExecutionStatus::parse),
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

fn json_string<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|e| NexusError::Internal(e.to_string()))
}

pub async fn create(db: &Db, a: &Automation) -> Result<Automation> {
    let triggers = normalized_triggers(&a.triggers);
    sqlx::query(
        "INSERT INTO automations (id, name, description, icon, enabled, mode, trigger_json, next_runs_json, conditions_json, flow_json, permissions_json, limits_json, policy, template_id, last_run_at, next_run_at, run_count, last_status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&a.id)
    .bind(&a.name)
    .bind(&a.description)
    .bind(&a.icon)
    .bind(a.enabled as i64)
    .bind(a.mode.as_str())
    .bind(json_string(&triggers)?)
    .bind(json_string(&a.next_runs)?)
    .bind(a.conditions.as_ref().map(json_string).transpose()?)
    .bind(json_string(&a.flow)?)
    .bind(json_string(&a.permissions)?)
    .bind(json_string(&a.limits)?)
    .bind(a.policy.as_str())
    .bind(&a.template_id)
    .bind(&a.last_run_at)
    .bind(&a.next_run_at)
    .bind(a.run_count)
    .bind(a.last_status.map(|s| s.as_str().to_string()))
    .bind(&a.created_at)
    .bind(&a.updated_at)
    .execute(&db.pool)
    .await
    .db()?;
    get(db, &a.id).await
}

pub async fn get(db: &Db, id: &str) -> Result<Automation> {
    let row: Option<AutomationRow> =
        sqlx::query_as("SELECT * FROM automations WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(Automation::try_from)
        .transpose()?
        .ok_or_else(|| NexusError::not_found("automatización", id))
}

pub async fn list(db: &Db, enabled_only: bool) -> Result<Vec<Automation>> {
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM automations WHERE deleted_at IS NULL");
    if enabled_only {
        qb.push(" AND enabled = 1");
    }
    qb.push(" ORDER BY created_at ASC");
    let rows: Vec<AutomationRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    rows.into_iter().map(Automation::try_from).collect()
}

pub async fn update(db: &Db, id: &str, patch: UpdateAutomation) -> Result<Automation> {
    let mut a = get(db, id).await?;
    if let Some(v) = patch.name {
        a.name = v;
    }
    if let Some(v) = patch.description {
        a.description = v;
    }
    if let Some(v) = patch.icon {
        a.icon = v;
    }
    if let Some(v) = patch.enabled {
        a.enabled = v;
    }
    if let Some(v) = patch.mode {
        a.mode = v;
    }
    if let Some(mut v) = patch.triggers {
        normalize_trigger_ids(&mut v);
        a.triggers = v;
    }
    if let Some(v) = patch.conditions {
        a.conditions = v;
    }
    if let Some(v) = patch.flow {
        a.flow = v;
    }
    if let Some(v) = patch.permissions {
        a.permissions = v;
    }
    if let Some(v) = patch.limits {
        a.limits = v;
    }
    if let Some(v) = patch.policy {
        a.policy = v;
    }
    a.updated_at = now_str();
    write(db, &a).await?;
    Ok(a)
}

pub async fn write(db: &Db, a: &Automation) -> Result<()> {
    let triggers = normalized_triggers(&a.triggers);
    sqlx::query(
        "UPDATE automations SET name=?, description=?, icon=?, enabled=?, mode=?, trigger_json=?, next_runs_json=?, conditions_json=?, flow_json=?, permissions_json=?, limits_json=?, policy=?, template_id=?, last_run_at=?, next_run_at=?, run_count=?, last_status=?, updated_at=? WHERE id=?",
    )
    .bind(&a.name)
    .bind(&a.description)
    .bind(&a.icon)
    .bind(a.enabled as i64)
    .bind(a.mode.as_str())
    .bind(json_string(&triggers)?)
    .bind(json_string(&a.next_runs)?)
    .bind(a.conditions.as_ref().map(json_string).transpose()?)
    .bind(json_string(&a.flow)?)
    .bind(json_string(&a.permissions)?)
    .bind(json_string(&a.limits)?)
    .bind(a.policy.as_str())
    .bind(&a.template_id)
    .bind(&a.last_run_at)
    .bind(&a.next_run_at)
    .bind(a.run_count)
    .bind(a.last_status.map(|s| s.as_str().to_string()))
    .bind(&a.updated_at)
    .bind(&a.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn set_schedule_state(
    db: &Db,
    id: &str,
    next_run_at: Option<&str>,
    last_run_at: Option<&str>,
) -> Result<()> {
    sqlx::query("UPDATE automations SET next_run_at=?, last_run_at=? WHERE id=?")
        .bind(next_run_at)
        .bind(last_run_at)
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn record_run_result(db: &Db, id: &str, status: ExecutionStatus) -> Result<()> {
    sqlx::query("UPDATE automations SET run_count = run_count + 1, last_status = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE automations SET deleted_at = ?, enabled = 0 WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now_str())
    .bind(id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Executions
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ExecutionRow {
    id: String,
    automation_id: String,
    automation_name: String,
    trigger_kind: String,
    trigger_detail: String,
    mode: String,
    status: String,
    test_run: i64,
    tokens_used: i64,
    agent_calls: i64,
    steps_done: i64,
    error: Option<String>,
    result_json: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    duration_ms: Option<i64>,
}

impl From<ExecutionRow> for AutomationExecution {
    fn from(r: ExecutionRow) -> Self {
        Self {
            id: r.id,
            automation_id: r.automation_id,
            automation_name: r.automation_name,
            trigger_kind: TriggerKind::parse(&r.trigger_kind).unwrap_or(TriggerKind::Event),
            trigger_detail: r.trigger_detail,
            mode: AutomationMode::parse(&r.mode).unwrap_or_default(),
            status: ExecutionStatus::parse(&r.status).unwrap_or(ExecutionStatus::Pending),
            test_run: r.test_run != 0,
            tokens_used: r.tokens_used,
            agent_calls: r.agent_calls,
            steps_done: r.steps_done,
            error: r.error,
            result: r.result_json.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: r.created_at,
            started_at: r.started_at,
            finished_at: r.finished_at,
            duration_ms: r.duration_ms,
        }
    }
}

pub async fn create_execution(db: &Db, e: &AutomationExecution) -> Result<AutomationExecution> {
    sqlx::query(
        "INSERT INTO automation_executions (id, automation_id, automation_name, trigger_kind, trigger_detail, mode, status, test_run, tokens_used, agent_calls, steps_done, error, result_json, created_at, started_at, finished_at, duration_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&e.id)
    .bind(&e.automation_id)
    .bind(&e.automation_name)
    .bind(e.trigger_kind.as_str())
    .bind(&e.trigger_detail)
    .bind(e.mode.as_str())
    .bind(e.status.as_str())
    .bind(e.test_run as i64)
    .bind(e.tokens_used)
    .bind(e.agent_calls)
    .bind(e.steps_done)
    .bind(&e.error)
    .bind(e.result.as_ref().map(json_string).transpose()?)
    .bind(&e.created_at)
    .bind(&e.started_at)
    .bind(&e.finished_at)
    .bind(e.duration_ms)
    .execute(&db.pool)
    .await
    .db()?;
    get_execution(db, &e.id).await
}

pub async fn get_execution(db: &Db, id: &str) -> Result<AutomationExecution> {
    let row: Option<ExecutionRow> =
        sqlx::query_as("SELECT * FROM automation_executions WHERE id = ?")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(AutomationExecution::from)
        .ok_or_else(|| NexusError::not_found("ejecución de automatización", id))
}

pub async fn update_execution(db: &Db, e: &AutomationExecution) -> Result<()> {
    sqlx::query(
        "UPDATE automation_executions SET status=?, tokens_used=?, agent_calls=?, steps_done=?, error=?, result_json=?, started_at=?, finished_at=?, duration_ms=? WHERE id=?",
    )
    .bind(e.status.as_str())
    .bind(e.tokens_used)
    .bind(e.agent_calls)
    .bind(e.steps_done)
    .bind(&e.error)
    .bind(e.result.as_ref().map(json_string).transpose()?)
    .bind(&e.started_at)
    .bind(&e.finished_at)
    .bind(e.duration_ms)
    .bind(&e.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn list_executions(db: &Db, f: &ExecutionFilter) -> Result<Vec<AutomationExecution>> {
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM automation_executions WHERE 1=1");
    if let Some(a) = &f.automation_id {
        qb.push(" AND automation_id = ").push_bind(a);
    }
    if let Some(s) = &f.status {
        qb.push(" AND status = ").push_bind(s.as_str());
    }
    if let Some(t) = f.test_run {
        qb.push(" AND test_run = ").push_bind(t as i64);
    }
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(f.limit.unwrap_or(100).clamp(1, 500));
    let rows: Vec<ExecutionRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(AutomationExecution::from).collect())
}

pub async fn list_active_executions(db: &Db) -> Result<Vec<AutomationExecution>> {
    let rows: Vec<ExecutionRow> = sqlx::query_as(
        "SELECT * FROM automation_executions WHERE status IN ('pending','running','waiting_approval') ORDER BY created_at ASC",
    )
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(AutomationExecution::from).collect())
}

pub async fn count_active_for(db: &Db, automation_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM automation_executions WHERE automation_id = ? AND status IN ('pending','running','waiting_approval')",
    )
    .bind(automation_id)
    .fetch_one(&db.pool)
    .await
    .db()?;
    row.try_get::<i64, _>("n").db()
}

pub async fn last_finished_execution(
    db: &Db,
    automation_id: &str,
) -> Result<Option<AutomationExecution>> {
    let row: Option<ExecutionRow> = sqlx::query_as(
        "SELECT * FROM automation_executions WHERE automation_id = ? AND status IN ('completed','failed','cancelled') ORDER BY created_at DESC LIMIT 1",
    )
    .bind(automation_id)
    .fetch_optional(&db.pool)
    .await
    .db()?;
    Ok(row.map(AutomationExecution::from))
}

pub async fn execution_stats(db: &Db) -> Result<(i64, i64, i64)> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS total,
                SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END) AS ok,
                SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END) AS failed
         FROM automation_executions WHERE test_run = 0",
    )
    .fetch_one(&db.pool)
    .await
    .db()?;
    Ok((
        row.try_get::<i64, _>("total").db()?,
        row.try_get::<Option<i64>, _>("ok").db()?.unwrap_or(0),
        row.try_get::<Option<i64>, _>("failed").db()?.unwrap_or(0),
    ))
}

pub async fn tokens_since(db: &Db, since: Option<&str>) -> Result<i64> {
    let row = match since {
        Some(s) => sqlx::query(
            "SELECT COALESCE(SUM(tokens_used), 0) AS n FROM automation_executions WHERE created_at >= ?",
        )
        .bind(s),
        None => sqlx::query("SELECT COALESCE(SUM(tokens_used), 0) AS n FROM automation_executions"),
    }
    .fetch_one(&db.pool)
    .await
    .db()?;
    row.try_get::<i64, _>("n").db()
}

pub async fn executions_since(db: &Db, since: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM automation_executions WHERE test_run = 0 AND created_at >= ?",
    )
    .bind(since)
    .fetch_one(&db.pool)
    .await
    .db()?;
    row.try_get::<i64, _>("n").db()
}

// ---------------------------------------------------------------------------
// Step logs
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct StepLogRow {
    id: String,
    execution_id: String,
    step_id: Option<String>,
    kind: String,
    name: String,
    status: String,
    detail_json: Option<String>,
    error: Option<String>,
    duration_ms: i64,
    created_at: String,
}

impl From<StepLogRow> for StepLog {
    fn from(r: StepLogRow) -> Self {
        Self {
            id: r.id,
            execution_id: r.execution_id,
            step_id: r.step_id,
            kind: r.kind,
            name: r.name,
            status: StepStatus::parse(&r.status).unwrap_or(StepStatus::Error),
            detail: r.detail_json.and_then(|s| serde_json::from_str(&s).ok()),
            error: r.error,
            duration_ms: r.duration_ms,
            created_at: r.created_at,
        }
    }
}

pub async fn add_step_log(db: &Db, log: &StepLog) -> Result<()> {
    sqlx::query(
        "INSERT INTO automation_step_logs (id, execution_id, step_id, kind, name, status, detail_json, error, duration_ms, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&log.id)
    .bind(&log.execution_id)
    .bind(&log.step_id)
    .bind(&log.kind)
    .bind(&log.name)
    .bind(log.status.as_str())
    .bind(log.detail.as_ref().map(json_string).transpose()?)
    .bind(&log.error)
    .bind(log.duration_ms)
    .bind(&log.created_at)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn list_step_logs(db: &Db, execution_id: &str) -> Result<Vec<StepLog>> {
    let rows: Vec<StepLogRow> = sqlx::query_as(
        "SELECT * FROM automation_step_logs WHERE execution_id = ? ORDER BY created_at ASC, rowid ASC",
    )
    .bind(execution_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(StepLog::from).collect())
}

// ---------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ApprovalRow {
    id: String,
    execution_id: String,
    automation_id: String,
    step_id: Option<String>,
    action: String,
    params_json: String,
    preview: String,
    agent_id: Option<String>,
    status: String,
    created_at: String,
    decided_at: Option<String>,
}

impl From<ApprovalRow> for AutomationApproval {
    fn from(r: ApprovalRow) -> Self {
        Self {
            id: r.id,
            execution_id: r.execution_id,
            automation_id: r.automation_id,
            step_id: r.step_id,
            action: r.action,
            params: serde_json::from_str(&r.params_json).unwrap_or(serde_json::Value::Null),
            preview: r.preview,
            agent_id: r.agent_id,
            status: AutomationApprovalStatus::parse(&r.status)
                .unwrap_or(AutomationApprovalStatus::Pending),
            created_at: r.created_at,
            decided_at: r.decided_at,
        }
    }
}

pub async fn create_approval(db: &Db, a: &AutomationApproval) -> Result<AutomationApproval> {
    sqlx::query(
        "INSERT INTO automation_approvals (id, execution_id, automation_id, step_id, action, params_json, preview, agent_id, status, created_at, decided_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&a.id)
    .bind(&a.execution_id)
    .bind(&a.automation_id)
    .bind(&a.step_id)
    .bind(&a.action)
    .bind(json_string(&a.params)?)
    .bind(&a.preview)
    .bind(&a.agent_id)
    .bind(a.status.as_str())
    .bind(&a.created_at)
    .bind(&a.decided_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_approval(db, &a.id).await
}

pub async fn get_approval(db: &Db, id: &str) -> Result<AutomationApproval> {
    let row: Option<ApprovalRow> =
        sqlx::query_as("SELECT * FROM automation_approvals WHERE id = ?")
            .bind(id)
            .fetch_optional(&db.pool)
            .await
            .db()?;
    row.map(AutomationApproval::from)
        .ok_or_else(|| NexusError::not_found("autorización", id))
}

pub async fn decide_approval(
    db: &Db,
    id: &str,
    status: AutomationApprovalStatus,
) -> Result<AutomationApproval> {
    sqlx::query("UPDATE automation_approvals SET status = ?, decided_at = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    get_approval(db, id).await
}

pub async fn list_approvals(
    db: &Db,
    status: Option<AutomationApprovalStatus>,
) -> Result<Vec<AutomationApproval>> {
    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT * FROM automation_approvals WHERE 1=1");
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.as_str());
    }
    qb.push(" ORDER BY created_at DESC LIMIT 200");
    let rows: Vec<ApprovalRow> = qb.build_query_as().fetch_all(&db.pool).await.db()?;
    Ok(rows.into_iter().map(AutomationApproval::from).collect())
}

pub async fn list_execution_approvals(
    db: &Db,
    execution_id: &str,
) -> Result<Vec<AutomationApproval>> {
    let rows: Vec<ApprovalRow> = sqlx::query_as(
        "SELECT * FROM automation_approvals WHERE execution_id = ? ORDER BY created_at ASC",
    )
    .bind(execution_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(AutomationApproval::from).collect())
}

pub async fn count_pending_approvals(db: &Db) -> Result<i64> {
    let row =
        sqlx::query("SELECT COUNT(*) AS n FROM automation_approvals WHERE status = 'pending'")
            .fetch_one(&db.pool)
            .await
            .db()?;
    row.try_get::<i64, _>("n").db()
}

pub async fn expire_stale_approvals(db: &Db, older_than: &str) -> Result<Vec<Id>> {
    let rows = sqlx::query(
        "SELECT id FROM automation_approvals WHERE status = 'pending' AND created_at < ?",
    )
    .bind(older_than)
    .fetch_all(&db.pool)
    .await
    .db()?;
    let ids: Vec<Id> = rows
        .into_iter()
        .map(|r| r.try_get::<String, _>("id"))
        .collect::<std::result::Result<_, _>>()
        .db()?;
    if !ids.is_empty() {
        sqlx::query(
            "UPDATE automation_approvals SET status = 'expired', decided_at = ? WHERE status = 'pending' AND created_at < ?",
        )
        .bind(now_str())
        .bind(older_than)
        .execute(&db.pool)
        .await
        .db()?;
    }
    Ok(ids)
}

// ---------------------------------------------------------------------------
// Marcas de disparo (deduplicación)
// ---------------------------------------------------------------------------

/// Inserta la marca; devuelve false si ya existía (disparo duplicado).
pub async fn try_mark_fired(db: &Db, automation_id: &str, mark_key: &str) -> Result<bool> {
    let res = sqlx::query(
        "INSERT OR IGNORE INTO automation_fired_marks (automation_id, mark_key, fired_at) VALUES (?, ?, ?)",
    )
    .bind(automation_id)
    .bind(mark_key)
    .bind(now_str())
    .execute(&db.pool)
    .await
    .db()?;
    Ok(res.rows_affected() > 0)
}

pub async fn prune_marks(db: &Db, older_than: &str) -> Result<()> {
    sqlx::query("DELETE FROM automation_fired_marks WHERE fired_at < ?")
        .bind(older_than)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}
