use nexus_core::{
    now_str, AgentRun, Approval, ApprovalStatus, NexusError, Result, Risk, RunStatus, ToolCall,
    ToolCallStatus,
};
use sqlx::Row;

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct RunRow {
    id: String,
    conversation_id: Option<String>,
    agent_id: String,
    parent_run_id: Option<String>,
    goal: String,
    status: String,
    result: Option<String>,
    error: Option<String>,
    iterations: i64,
    tokens_used: i64,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl From<RunRow> for AgentRun {
    fn from(r: RunRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            agent_id: r.agent_id,
            parent_run_id: r.parent_run_id,
            goal: r.goal,
            status: RunStatus::parse(&r.status).unwrap_or(RunStatus::Pending),
            result: r.result,
            error: r.error,
            iterations: r.iterations,
            tokens_used: r.tokens_used,
            created_at: r.created_at,
            started_at: r.started_at,
            finished_at: r.finished_at,
        }
    }
}

pub async fn create_run(db: &Db, run: &AgentRun) -> Result<AgentRun> {
    sqlx::query(
        "INSERT INTO agent_runs (id, conversation_id, agent_id, parent_run_id, goal, status, result, error, iterations, tokens_used, created_at, started_at, finished_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run.id)
    .bind(&run.conversation_id)
    .bind(&run.agent_id)
    .bind(&run.parent_run_id)
    .bind(&run.goal)
    .bind(run.status.as_str())
    .bind(&run.result)
    .bind(&run.error)
    .bind(run.iterations)
    .bind(run.tokens_used)
    .bind(&run.created_at)
    .bind(&run.started_at)
    .bind(&run.finished_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_run(db, &run.id).await
}

pub async fn get_run(db: &Db, id: &str) -> Result<AgentRun> {
    let row: Option<RunRow> = sqlx::query_as("SELECT * FROM agent_runs WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(AgentRun::from)
        .ok_or_else(|| NexusError::not_found("ejecución", id))
}

pub async fn list_runs(db: &Db, limit: i64) -> Result<Vec<AgentRun>> {
    let rows: Vec<RunRow> =
        sqlx::query_as("SELECT * FROM agent_runs ORDER BY created_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&db.pool)
            .await
            .db()?;
    Ok(rows.into_iter().map(AgentRun::from).collect())
}

pub async fn list_children(db: &Db, parent_id: &str) -> Result<Vec<AgentRun>> {
    let rows: Vec<RunRow> =
        sqlx::query_as("SELECT * FROM agent_runs WHERE parent_run_id = ? ORDER BY created_at ASC")
            .bind(parent_id)
            .fetch_all(&db.pool)
            .await
            .db()?;
    Ok(rows.into_iter().map(AgentRun::from).collect())
}

pub async fn update_run(db: &Db, run: &AgentRun) -> Result<()> {
    sqlx::query(
        "UPDATE agent_runs SET status=?, result=?, error=?, iterations=?, tokens_used=?, started_at=?, finished_at=? WHERE id=?",
    )
    .bind(run.status.as_str())
    .bind(&run.result)
    .bind(&run.error)
    .bind(run.iterations)
    .bind(run.tokens_used)
    .bind(&run.started_at)
    .bind(&run.finished_at)
    .bind(&run.id)
    .execute(&db.pool)
    .await
    .db()?;
    Ok(())
}

pub async fn list_active_runs(db: &Db) -> Result<Vec<AgentRun>> {
    let rows: Vec<RunRow> =
        sqlx::query_as("SELECT * FROM agent_runs WHERE status IN ('pending','running','waiting_approval') ORDER BY created_at DESC")
            .fetch_all(&db.pool)
            .await
            .db()?;
    Ok(rows.into_iter().map(AgentRun::from).collect())
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ToolCallRow {
    id: String,
    run_id: String,
    tool_name: String,
    args: String,
    result: Option<String>,
    status: String,
    error: Option<String>,
    duration_ms: Option<i64>,
    created_at: String,
}

impl From<ToolCallRow> for ToolCall {
    fn from(r: ToolCallRow) -> Self {
        Self {
            id: r.id,
            run_id: r.run_id,
            tool_name: r.tool_name,
            args: serde_json::from_str(&r.args).unwrap_or(serde_json::Value::Null),
            result: r.result.and_then(|s| serde_json::from_str(&s).ok()),
            status: ToolCallStatus::parse(&r.status).unwrap_or(ToolCallStatus::Pending),
            error: r.error,
            duration_ms: r.duration_ms,
            created_at: r.created_at,
        }
    }
}

pub async fn create_tool_call(db: &Db, call: &ToolCall) -> Result<ToolCall> {
    sqlx::query(
        "INSERT INTO tool_calls (id, run_id, tool_name, args, result, status, error, duration_ms, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&call.id)
    .bind(&call.run_id)
    .bind(&call.tool_name)
    .bind(call.args.to_string())
    .bind(call.result.as_ref().map(|v| v.to_string()))
    .bind(call.status.as_str())
    .bind(&call.error)
    .bind(call.duration_ms)
    .bind(&call.created_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_tool_call(db, &call.id).await
}

pub async fn get_tool_call(db: &Db, id: &str) -> Result<ToolCall> {
    let row: Option<ToolCallRow> = sqlx::query_as("SELECT * FROM tool_calls WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(ToolCall::from)
        .ok_or_else(|| NexusError::not_found("tool call", id))
}

pub async fn update_tool_call(db: &Db, call: &ToolCall) -> Result<()> {
    sqlx::query("UPDATE tool_calls SET result=?, status=?, error=?, duration_ms=? WHERE id=?")
        .bind(call.result.as_ref().map(|v| v.to_string()))
        .bind(call.status.as_str())
        .bind(&call.error)
        .bind(call.duration_ms)
        .bind(&call.id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn list_tool_calls(db: &Db, run_id: &str) -> Result<Vec<ToolCall>> {
    let rows: Vec<ToolCallRow> = sqlx::query_as(
        "SELECT * FROM tool_calls WHERE run_id = ? ORDER BY created_at ASC, rowid ASC",
    )
    .bind(run_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(ToolCall::from).collect())
}

// ---------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ApprovalRow {
    id: String,
    run_id: String,
    tool_call_id: String,
    tool_name: String,
    args: String,
    risk: String,
    status: String,
    created_at: String,
    decided_at: Option<String>,
}

impl From<ApprovalRow> for Approval {
    fn from(r: ApprovalRow) -> Self {
        Self {
            id: r.id,
            run_id: r.run_id,
            tool_call_id: r.tool_call_id,
            tool_name: r.tool_name,
            args: serde_json::from_str(&r.args).unwrap_or(serde_json::Value::Null),
            risk: Risk::parse(&r.risk).unwrap_or(Risk::Destructive),
            status: ApprovalStatus::parse(&r.status).unwrap_or(ApprovalStatus::Pending),
            created_at: r.created_at,
            decided_at: r.decided_at,
        }
    }
}

pub async fn create_approval(db: &Db, a: &Approval) -> Result<Approval> {
    sqlx::query(
        "INSERT INTO approvals (id, run_id, tool_call_id, tool_name, args, risk, status, created_at, decided_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&a.id)
    .bind(&a.run_id)
    .bind(&a.tool_call_id)
    .bind(&a.tool_name)
    .bind(a.args.to_string())
    .bind(a.risk.as_str())
    .bind(a.status.as_str())
    .bind(&a.created_at)
    .bind(&a.decided_at)
    .execute(&db.pool)
    .await
    .db()?;
    get_approval(db, &a.id).await
}

pub async fn get_approval(db: &Db, id: &str) -> Result<Approval> {
    let row: Option<ApprovalRow> = sqlx::query_as("SELECT * FROM approvals WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(Approval::from)
        .ok_or_else(|| NexusError::not_found("aprobación", id))
}

pub async fn decide_approval(db: &Db, id: &str, status: ApprovalStatus) -> Result<Approval> {
    sqlx::query(
        "UPDATE approvals SET status = ?, decided_at = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(status.as_str())
    .bind(now_str())
    .bind(id)
    .execute(&db.pool)
    .await
    .db()?;
    get_approval(db, id).await
}

pub async fn list_approvals(db: &Db, status: Option<ApprovalStatus>) -> Result<Vec<Approval>> {
    let rows: Vec<ApprovalRow> = match status {
        Some(s) => {
            sqlx::query_as("SELECT * FROM approvals WHERE status = ? ORDER BY created_at DESC")
                .bind(s.as_str())
        }
        None => sqlx::query_as("SELECT * FROM approvals ORDER BY created_at DESC LIMIT 200"),
    }
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(Approval::from).collect())
}

pub async fn list_run_approvals(db: &Db, run_id: &str) -> Result<Vec<Approval>> {
    let rows: Vec<ApprovalRow> =
        sqlx::query_as("SELECT * FROM approvals WHERE run_id = ? ORDER BY created_at ASC")
            .bind(run_id)
            .fetch_all(&db.pool)
            .await
            .db()?;
    Ok(rows.into_iter().map(Approval::from).collect())
}

pub async fn count_runs(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM agent_runs")
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}
