use nexus_core::{now_str, ChatMessage, Conversation, Id, MessageRole, NexusError, Result};
use sqlx::Row;

use crate::{Db, DbResultExt};

#[derive(sqlx::FromRow)]
struct ConversationRow {
    id: String,
    title: String,
    agent_id: String,
    created_at: String,
    updated_at: String,
}

impl From<ConversationRow> for Conversation {
    fn from(r: ConversationRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            agent_id: r.agent_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    agent_id: Option<String>,
    run_id: Option<String>,
    tool_calls: Option<String>,
    created_at: String,
}

impl From<MessageRow> for ChatMessage {
    fn from(r: MessageRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            role: MessageRole::parse(&r.role).unwrap_or(MessageRole::User),
            content: r.content,
            agent_id: r.agent_id,
            run_id: r.run_id,
            tool_calls: r.tool_calls.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: r.created_at,
        }
    }
}

pub async fn list_conversations(db: &Db) -> Result<Vec<Conversation>> {
    let rows: Vec<ConversationRow> =
        sqlx::query_as("SELECT * FROM conversations ORDER BY updated_at DESC")
            .fetch_all(&db.pool)
            .await
            .db()?;
    Ok(rows.into_iter().map(Conversation::from).collect())
}

pub async fn get_conversation(db: &Db, id: &str) -> Result<Conversation> {
    let row: Option<ConversationRow> = sqlx::query_as("SELECT * FROM conversations WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.pool)
        .await
        .db()?;
    row.map(Conversation::from)
        .ok_or_else(|| NexusError::not_found("conversación", id))
}

pub async fn create_conversation(
    db: &Db,
    agent_id: &str,
    title: &str,
    id: Id,
) -> Result<Conversation> {
    let now = now_str();
    sqlx::query("INSERT INTO conversations (id, title, agent_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(title)
        .bind(agent_id)
        .bind(&now)
        .bind(&now)
        .execute(&db.pool)
        .await
        .db()?;
    get_conversation(db, &id).await
}

pub async fn rename_conversation(db: &Db, id: &str, title: &str) -> Result<Conversation> {
    sqlx::query("UPDATE conversations SET title = ?, updated_at = ? WHERE id = ?")
        .bind(title)
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    get_conversation(db, id).await
}

pub async fn delete_conversation(db: &Db, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn touch_conversation(db: &Db, id: &str) -> Result<()> {
    sqlx::query("UPDATE conversations SET updated_at = ? WHERE id = ?")
        .bind(now_str())
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn list_messages(db: &Db, conversation_id: &str) -> Result<Vec<ChatMessage>> {
    let rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT * FROM chat_messages WHERE conversation_id = ? ORDER BY created_at ASC, rowid ASC",
    )
    .bind(conversation_id)
    .fetch_all(&db.pool)
    .await
    .db()?;
    Ok(rows.into_iter().map(ChatMessage::from).collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn add_message(
    db: &Db,
    conversation_id: &str,
    role: MessageRole,
    content: &str,
    agent_id: Option<&str>,
    run_id: Option<&str>,
    tool_calls: Option<serde_json::Value>,
    id: Id,
) -> Result<ChatMessage> {
    let now = now_str();
    sqlx::query(
        "INSERT INTO chat_messages (id, conversation_id, role, content, agent_id, run_id, tool_calls, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(conversation_id)
    .bind(role.as_str())
    .bind(content)
    .bind(agent_id)
    .bind(run_id)
    .bind(tool_calls.map(|v| v.to_string()))
    .bind(&now)
    .execute(&db.pool)
    .await
    .db()?;
    touch_conversation(db, conversation_id).await?;
    let row: MessageRow = sqlx::query_as("SELECT * FROM chat_messages WHERE id = ?")
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .db()?;
    Ok(row.into())
}

pub async fn update_message_content(db: &Db, id: &str, content: &str) -> Result<()> {
    sqlx::query("UPDATE chat_messages SET content = ? WHERE id = ?")
        .bind(content)
        .bind(id)
        .execute(&db.pool)
        .await
        .db()?;
    Ok(())
}

pub async fn count_messages(db: &Db, conversation_id: &str) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM chat_messages WHERE conversation_id = ?")
        .bind(conversation_id)
        .fetch_one(&db.pool)
        .await
        .db()?;
    row.try_get::<i64, _>("n").db()
}
