use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{AppError, AppResult, PublicError};

pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderId {
    #[serde(rename = "zai")]
    Zai,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "alibaba-us")]
    AlibabaUs,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "nvidia")]
    Nvidia,
}

impl ProviderId {
    pub const ALL: [Self; 5] = [
        Self::Zai,
        Self::DeepSeek,
        Self::AlibabaUs,
        Self::Google,
        Self::Nvidia,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zai => "zai",
            Self::DeepSeek => "deepseek",
            Self::AlibabaUs => "alibaba-us",
            Self::Google => "google",
            Self::Nvidia => "nvidia",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "zai" => Some(Self::Zai),
            "deepseek" => Some(Self::DeepSeek),
            "alibaba-us" => Some(Self::AlibabaUs),
            "google" => Some(Self::Google),
            "nvidia" => Some(Self::Nvidia),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderAccountAction {
    #[serde(rename = "usage")]
    Usage,
    #[serde(rename = "billing")]
    Billing,
    #[serde(rename = "addCredits")]
    AddCredits,
    #[serde(rename = "spend")]
    Spend,
    #[serde(rename = "deployment")]
    Deployment,
}

impl ProviderAccountAction {
    #[cfg(test)]
    pub const ALL: [Self; 5] = [
        Self::Usage,
        Self::Billing,
        Self::AddCredits,
        Self::Spend,
        Self::Deployment,
    ];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseProfile {
    Fast,
    #[default]
    Standard,
    Deep,
}

impl ResponseProfile {
    pub const ALL: [Self; 3] = [Self::Fast, Self::Standard, Self::Deep];

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

    pub const fn max_output_tokens(self) -> u32 {
        match self {
            Self::Fast => 4_096,
            Self::Standard => 8_192,
            Self::Deep => 16_384,
        }
    }

    // Retained only for the verified Z.AI mapping and v1 compatibility tests.
    pub const fn thinking_type(self) -> &'static str {
        match self {
            Self::Fast => "disabled",
            Self::Standard | Self::Deep => "enabled",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageFinishReason {
    Stop,
    OutputLimit,
    Unknown,
}

impl MessageFinishReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::OutputLimit => "output_limit",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "stop" => Some(Self::Stop),
            "output_limit" => Some(Self::OutputLimit),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

impl TokenUsage {
    pub fn new(
        input_tokens: Option<u64>,
        cached_input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> AppResult<Self> {
        let usage = Self {
            input_tokens,
            cached_input_tokens,
            output_tokens,
            total_tokens,
        };
        usage.validate()?;
        Ok(usage)
    }

    pub fn validate(&self) -> AppResult<()> {
        if [
            self.input_tokens,
            self.cached_input_tokens,
            self.output_tokens,
            self.total_tokens,
        ]
        .into_iter()
        .flatten()
        .any(|value| value > MAX_SAFE_INTEGER)
        {
            return Err(AppError::Validation("Token usage is out of range."));
        }
        if let (Some(input), Some(cached), Some(output), Some(total)) = (
            self.input_tokens,
            self.cached_input_tokens,
            self.output_tokens,
            self.total_tokens,
        ) {
            let expected = input
                .checked_add(cached)
                .and_then(|value| value.checked_add(output))
                .filter(|value| *value <= MAX_SAFE_INTEGER)
                .ok_or(AppError::Validation("Token usage is out of range."))?;
            if expected != total {
                return Err(AppError::Validation("Token usage totals are inconsistent."));
            }
        }
        Ok(())
    }

    pub const fn is_complete(&self) -> bool {
        self.input_tokens.is_some()
            && self.cached_input_tokens.is_some()
            && self.output_tokens.is_some()
            && self.total_tokens.is_some()
    }

    pub const fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.cached_input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
    }

    #[cfg(test)]
    pub const fn total_only(total_tokens: u64) -> Self {
        Self {
            input_tokens: None,
            cached_input_tokens: None,
            output_tokens: None,
            total_tokens: Some(total_tokens),
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
    pub finish_reason: Option<MessageFinishReason>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub provider_id: ProviderId,
    pub model_id: String,
    pub response_profile: ResponseProfile,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub provider_id: ProviderId,
    pub model_id: String,
    pub response_profile: ResponseProfile,
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
            provider_id: self.provider_id,
            model_id: self.model_id.clone(),
            response_profile: self.response_profile,
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
    pub database_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub provider_id: ProviderId,
    pub configured: bool,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPromptResult {
    pub provider_id: ProviderId,
    pub configured: bool,
    pub source: &'static str,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: ProviderId,
    pub configured: bool,
    pub reachability: &'static str,
    pub notice_version: u32,
    pub notice_acknowledged: bool,
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
    pub provider_id: ProviderId,
    pub model_id: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    pub provider_id: ProviderId,
    pub model_id: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProfile {
    pub id: ResponseProfile,
    pub label: &'static str,
    pub description: &'static str,
    pub enabled: bool,
    pub disabled_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub delivery: &'static str,
    pub profiles: Vec<CatalogProfile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAccountAction {
    pub action: ProviderAccountAction,
    pub label: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvider {
    pub id: ProviderId,
    pub display_name: &'static str,
    pub region_label: Option<&'static str>,
    pub notice_version: u32,
    pub processing_notice: &'static str,
    pub account_actions: Vec<CatalogAccountAction>,
    pub models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    pub version: u32,
    pub default_selection: ModelSelection,
    pub providers: Vec<CatalogProvider>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisoryBudget {
    pub token_budget: u64,
    pub known_used_tokens: Option<u64>,
    pub remaining_tokens: u64,
    pub remaining_percentage: f64,
    pub state: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub provider_id: ProviderId,
    pub model_id: Option<String>,
    pub window_start: String,
    pub window_end: String,
    pub observed_at: String,
    pub usage: TokenUsage,
    pub complete_observations: u64,
    pub partial_observations: u64,
    pub coverage: &'static str,
    pub budget: Option<AdvisoryBudget>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BalanceInfo {
    pub currency: String,
    pub total_balance: String,
    pub granted_balance: String,
    pub topped_up_balance: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepSeekBalanceStatus {
    pub status: &'static str,
    pub observed_at: Option<String>,
    pub is_available: Option<bool>,
    pub balance_infos: Vec<BalanceInfo>,
    pub error: Option<PublicError>,
}

impl DeepSeekBalanceStatus {
    pub fn not_checked() -> Self {
        Self {
            status: "notChecked",
            observed_at: None,
            is_available: None,
            balance_infos: Vec::new(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderMessage {
    pub role: MessageRole,
    pub content: String,
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
    #[serde(rename = "provider")]
    pub provider_id: ProviderId,
    #[serde(rename = "model")]
    pub model_id: String,
    pub response_profile: ResponseProfile,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ExportMessage>,
}

impl From<Conversation> for ExportConversation {
    fn from(conversation: Conversation) -> Self {
        Self {
            title: conversation.title,
            provider_id: conversation.provider_id,
            model_id: conversation.model_id,
            response_profile: conversation.response_profile,
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
    pub finish_reason: Option<MessageFinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

impl From<Message> for ExportMessage {
    fn from(message: Message) -> Self {
        Self {
            role: message.role,
            content: message.content,
            created_at: message.created_at,
            status: message.status,
            finish_reason: message.finish_reason,
            usage: message.usage,
        }
    }
}

#[derive(Debug)]
pub enum ImportBundle {
    V1(ImportBundleV1),
    V2(ImportBundleV2),
}

impl<'de> Deserialize<'de> for ImportBundle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value.get("version").and_then(serde_json::Value::as_u64) {
            Some(1) => serde_json::from_value(value)
                .map(Self::V1)
                .map_err(serde::de::Error::custom),
            Some(2) => serde_json::from_value(value)
                .map(Self::V2)
                .map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("unsupported import version")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportBundleV1 {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub conversations: Vec<ImportConversationV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportConversationV1 {
    pub title: String,
    pub model: String,
    pub reasoning_mode: ResponseProfile,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ImportMessageV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMessageV1 {
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
    pub status: MessageStatus,
    #[serde(default)]
    pub token_usage: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportBundleV2 {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub conversations: Vec<ImportConversationV2>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportConversationV2 {
    pub title: String,
    #[serde(rename = "provider")]
    pub provider_id: ProviderId,
    #[serde(rename = "model")]
    pub model_id: String,
    pub response_profile: ResponseProfile,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ImportMessageV2>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMessageV2 {
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
    pub status: MessageStatus,
    #[serde(default)]
    pub finish_reason: Option<MessageFinishReason>,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mvp_v2_domain_enums_are_exact_and_case_sensitive() {
        assert_eq!(ProviderId::parse("zai"), Some(ProviderId::Zai));
        assert_eq!(ProviderId::parse("deepseek"), Some(ProviderId::DeepSeek));
        assert_eq!(ProviderId::parse("alibaba-us"), Some(ProviderId::AlibabaUs));
        assert_eq!(ProviderId::parse("google"), Some(ProviderId::Google));
        assert_eq!(ProviderId::parse("nvidia"), Some(ProviderId::Nvidia));
        assert_eq!(ProviderId::parse("ZAI"), None);
        assert_eq!(
            serde_json::from_str::<ProviderAccountAction>("\"addCredits\"").unwrap(),
            ProviderAccountAction::AddCredits
        );
        assert!(serde_json::from_str::<ProviderAccountAction>("\"addcredits\"").is_err());
    }

    #[test]
    fn complete_usage_requires_a_checked_disjoint_sum() {
        assert!(TokenUsage::new(Some(8), Some(2), Some(32), Some(42)).is_ok());
        assert!(TokenUsage::new(Some(8), Some(2), Some(32), Some(41)).is_err());
        assert!(TokenUsage::new(None, None, None, Some(42)).is_ok());
    }

    #[test]
    fn import_versions_have_exact_distinct_shapes() {
        let v1 = r#"{
          "format":"aster-conversation","version":1,"exportedAt":"2026-01-01T00:00:00Z",
          "conversations":[{"title":"Old","model":"glm-5.1","reasoningMode":"standard",
          "createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","messages":[]}]
        }"#;
        assert!(matches!(
            serde_json::from_str::<ImportBundle>(v1),
            Ok(ImportBundle::V1(_))
        ));
        let v2 = r#"{
          "format":"aster-conversation","version":2,"exportedAt":"2026-01-01T00:00:00Z",
          "conversations":[{"title":"New","provider":"google","model":"gemini-2.5-pro",
          "responseProfile":"fast","createdAt":"2026-01-01T00:00:00Z",
          "updatedAt":"2026-01-01T00:00:00Z","messages":[]}]
        }"#;
        assert!(matches!(
            serde_json::from_str::<ImportBundle>(v2),
            Ok(ImportBundle::V2(_))
        ));
    }

    #[test]
    fn export_schema_omits_local_identifiers_and_emits_v2_pair() {
        let conversation = Conversation {
            id: "local-conversation-id".to_owned(),
            title: "Minimal export".to_owned(),
            provider_id: ProviderId::Zai,
            model_id: "glm-5.1".to_owned(),
            response_profile: ResponseProfile::Standard,
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
                finish_reason: None,
                usage: None,
            }],
        };
        let value = serde_json::to_value(ExportConversation::from(conversation))
            .expect("export should serialize");
        let serialized = value.to_string();
        assert!(!serialized.contains("local-conversation-id"));
        assert!(!serialized.contains("local-message-id"));
        assert_eq!(value["provider"], "zai");
        assert_eq!(value["model"], "glm-5.1");
        assert_eq!(value["responseProfile"], "standard");
    }
}
