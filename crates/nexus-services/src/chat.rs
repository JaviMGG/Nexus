use nexus_core::{new_id, ChatMessage, Conversation, MessageRole, Result};
use nexus_db::{repos, Db};

use crate::validate;

#[derive(Clone)]
pub struct ChatService {
    pub db: Db,
}

impl ChatService {
    pub async fn list_conversations(&self) -> Result<Vec<Conversation>> {
        repos::chat::list_conversations(&self.db).await
    }

    pub async fn create_conversation(
        &self,
        agent_id: &str,
        title: Option<&str>,
    ) -> Result<Conversation> {
        let title = title.unwrap_or("Nueva conversación");
        validate::title(title, "el título")?;
        repos::chat::create_conversation(&self.db, agent_id, title, new_id()).await
    }

    pub async fn get_conversation(&self, id: &str) -> Result<Conversation> {
        repos::chat::get_conversation(&self.db, id).await
    }

    pub async fn rename_conversation(&self, id: &str, title: &str) -> Result<Conversation> {
        let title = validate::title(title, "el título")?;
        repos::chat::rename_conversation(&self.db, id, &title).await
    }

    pub async fn delete_conversation(&self, id: &str) -> Result<()> {
        repos::chat::delete_conversation(&self.db, id).await
    }

    pub async fn messages(&self, conversation_id: &str) -> Result<Vec<ChatMessage>> {
        repos::chat::list_messages(&self.db, conversation_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_message(
        &self,
        conversation_id: &str,
        role: MessageRole,
        content: &str,
        agent_id: Option<&str>,
        run_id: Option<&str>,
        tool_calls: Option<serde_json::Value>,
    ) -> Result<ChatMessage> {
        repos::chat::add_message(
            &self.db,
            conversation_id,
            role,
            content,
            agent_id,
            run_id,
            tool_calls,
            new_id(),
        )
        .await
    }

    /// Título automático a partir del primer mensaje del usuario.
    pub async fn auto_title(&self, conversation_id: &str) -> Result<()> {
        if repos::chat::count_messages(&self.db, conversation_id).await? <= 1 {
            let msgs = repos::chat::list_messages(&self.db, conversation_id).await?;
            if let Some(first) = msgs.iter().find(|m| m.role == MessageRole::User) {
                let title: String = first.content.chars().take(60).collect();
                repos::chat::rename_conversation(&self.db, conversation_id, &title).await?;
            }
        }
        Ok(())
    }
}
