use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningMode {
    Fast,
    #[default]
    Standard,
    Deep,
}

impl ReasoningMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fast" => Some(Self::Fast),
            "standard" => Some(Self::Standard),
            "deep" => Some(Self::Deep),
            _ => None,
        }
    }

    pub const fn thinking_type(self) -> &'static str {
        match self {
            Self::Fast => "disabled",
            Self::Standard | Self::Deep => "enabled",
        }
    }

    pub const fn max_tokens(self) -> u32 {
        match self {
            Self::Fast => 4_096,
            Self::Standard => 8_192,
            Self::Deep => 16_384,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageStatus {
    Complete,
    Cancelled,
    Error,
}

impl MessageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "complete" => Some(Self::Complete),
            "cancelled" => Some(Self::Cancelled),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
    pub status: MessageStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub token_usage: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub model: String,
    pub reasoning_mode: ReasoningMode,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model: String,
    pub reasoning_mode: ReasoningMode,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u64,
    pub messages: Vec<Message>,
}

impl Conversation {
    pub fn summary(&self) -> ConversationSummary {
        ConversationSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            model: self.model.clone(),
            reasoning_mode: self.reasoning_mode,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            message_count: self.message_count,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub mode: &'static str,
    pub version: &'static str,
    pub online: bool,
    pub provider_reachability: &'static str,
    pub database_ready: bool,
    pub external_processing_acknowledged: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub configured: bool,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPromptResult {
    pub configured: bool,
    pub source: &'static str,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    pub request_id: String,
    pub conversation_id: String,
    pub sequence: u64,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub format: &'static str,
    pub version: u32,
    pub exported_at: String,
    pub conversations: Vec<ExportConversation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportConversation {
    pub title: String,
    pub model: String,
    pub reasoning_mode: ReasoningMode,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ExportMessage>,
}

impl From<Conversation> for ExportConversation {
    fn from(conversation: Conversation) -> Self {
        Self {
            title: conversation.title,
            model: conversation.model,
            reasoning_mode: conversation.reasoning_mode,
            created_at: conversation.created_at,
            updated_at: conversation.updated_at,
            messages: conversation.messages.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportMessage {
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
    pub status: MessageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<u64>,
}

impl From<Message> for ExportMessage {
    fn from(message: Message) -> Self {
        Self {
            role: message.role,
            content: message.content,
            created_at: message.created_at,
            status: message.status,
            token_usage: message.token_usage,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportBundle {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub conversations: Vec<ImportConversation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportConversation {
    pub title: String,
    pub model: String,
    pub reasoning_mode: ReasoningMode,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ImportMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMessage {
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
    pub status: MessageStatus,
    #[serde(default)]
    pub token_usage: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderMessage {
    pub role: MessageRole,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_modes_have_explicit_provider_budgets() {
        assert_eq!(ReasoningMode::Fast.thinking_type(), "disabled");
        assert_eq!(ReasoningMode::Fast.max_tokens(), 4_096);
        assert_eq!(ReasoningMode::Standard.thinking_type(), "enabled");
        assert_eq!(ReasoningMode::Standard.max_tokens(), 8_192);
        assert_eq!(ReasoningMode::Deep.thinking_type(), "enabled");
        assert_eq!(ReasoningMode::Deep.max_tokens(), 16_384);
    }

    #[test]
    fn credential_prompt_result_is_strictly_status_only() {
        let value = serde_json::to_value(CredentialPromptResult {
            configured: true,
            source: "credential-vault",
            cancelled: false,
        })
        .expect("prompt status should serialize");
        assert_eq!(
            value,
            serde_json::json!({
                "configured": true,
                "source": "credential-vault",
                "cancelled": false
            })
        );
    }

    #[test]
    fn import_rejects_system_roles() {
        let input = r#"{
          "role":"system","content":"x",
          "createdAt":"2026-01-01T00:00:00Z","status":"complete"
        }"#;
        assert!(serde_json::from_str::<ImportMessage>(input).is_err());
    }

    #[test]
    fn minimal_import_schema_rejects_internal_identifiers() {
        let input = r#"{
          "format":"aster-conversation",
          "version":1,
          "exportedAt":"2026-01-01T00:00:00Z",
          "conversations":[{
            "id":"source-id",
            "title":"Imported",
            "model":"glm-5.1",
            "reasoningMode":"standard",
            "createdAt":"2026-01-01T00:00:00Z",
            "updatedAt":"2026-01-01T00:00:00Z",
            "messages":[]
          }]
        }"#;
        assert!(serde_json::from_str::<ImportBundle>(input).is_err());
    }

    #[test]
    fn export_schema_omits_all_local_identifiers_and_derived_counts() {
        let conversation = Conversation {
            id: "local-conversation-id".to_owned(),
            title: "Minimal export".to_owned(),
            model: "glm-5.1".to_owned(),
            reasoning_mode: ReasoningMode::Standard,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            message_count: 1,
            messages: vec![Message {
                id: "local-message-id".to_owned(),
                conversation_id: "local-conversation-id".to_owned(),
                role: MessageRole::User,
                content: "Hello".to_owned(),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                status: MessageStatus::Complete,
                token_usage: None,
            }],
        };
        let value = serde_json::to_value(ExportConversation::from(conversation))
            .expect("export should serialize");
        let serialized = value.to_string();
        assert!(!serialized.contains("local-conversation-id"));
        assert!(!serialized.contains("local-message-id"));
        assert!(value.get("id").is_none());
        assert!(value.get("messageCount").is_none());
        assert!(value["messages"][0].get("conversationId").is_none());
    }
}
