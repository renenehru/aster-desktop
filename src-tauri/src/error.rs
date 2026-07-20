use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("input validation failed")]
    Validation(&'static str),
    #[error("requested resource was not found")]
    NotFound(&'static str),
    #[error("operation conflicts with current state")]
    Conflict(&'static str),
    #[error("the conversation provider and model are locked")]
    ConversationModelLocked,
    #[error("external processing notice has not been acknowledged")]
    ExternalProcessingNoticeRequired,
    #[error("the API key is not configured")]
    CredentialNotConfigured,
    #[error("the credential vault is unavailable")]
    CredentialVault,
    #[error("the native credential prompt is unavailable")]
    CredentialPrompt,
    #[error("the captured credential is invalid")]
    CredentialInvalid,
    #[error("a native credential prompt is already active")]
    CredentialPromptBusy,
    #[error("provider authentication failed")]
    ProviderAuthentication,
    #[error("provider rate limit reached")]
    ProviderRateLimited,
    #[error("provider is temporarily unavailable")]
    ProviderUnavailable,
    #[error("provider rejected the request")]
    ProviderRejected,
    #[error("provider rejected the verified request contract")]
    ProviderContract,
    #[error("provider content policy rejected the response")]
    ProviderContentRejected,
    #[error("provider context limit was exceeded")]
    ProviderContextLimit,
    #[error("provider requested an unsupported capability")]
    UnsupportedProviderCapability,
    #[error("provider balance response was malformed")]
    MalformedBalance,
    #[error("network request failed")]
    Network,
    #[error("network request timed out")]
    Timeout,
    #[error("provider stream was malformed")]
    MalformedStream,
    #[error("generation was cancelled")]
    Cancelled,
    #[error("database operation failed")]
    Database(#[source] rusqlite::Error),
    #[error("database schema is unsupported or corrupt")]
    DatabaseIntegrity,
    #[error("file operation failed")]
    File(#[source] std::io::Error),
    #[error("external navigation failed")]
    ExternalNavigation,
    #[error("serialization failed")]
    Serialization,
    #[error("internal operation failed")]
    Internal,
}

impl AppError {
    pub fn public(&self) -> PublicError {
        match self {
            Self::Validation(message) => PublicError {
                code: "validation_error",
                message,
                retryable: false,
            },
            Self::NotFound(message) => PublicError {
                code: "not_found",
                message,
                retryable: false,
            },
            Self::Conflict(message) => PublicError {
                code: "conflict",
                message,
                retryable: false,
            },
            Self::ConversationModelLocked => PublicError {
                code: "conversation_model_locked",
                message: "This conversation already has messages. Start a new chat to use another model.",
                retryable: false,
            },
            Self::ExternalProcessingNoticeRequired => PublicError {
                code: "external_processing_notice_required",
                message: "Review and acknowledge the external-processing notice before sending.",
                retryable: false,
            },
            Self::CredentialNotConfigured => PublicError {
                code: "credential_not_configured",
                message: "Add the selected provider's API key in Settings before sending a message.",
                retryable: false,
            },
            Self::CredentialVault => PublicError {
                code: "credential_vault_unavailable",
                message: "Windows Credential Manager is unavailable. Unlock it and try again.",
                retryable: true,
            },
            Self::CredentialPrompt => PublicError {
                code: "credential_prompt_unavailable",
                message: "Aster could not open the Windows credential prompt. Try again.",
                retryable: true,
            },
            Self::CredentialInvalid => PublicError {
                code: "credential_invalid",
                message: "The API key must contain 8 to 255 printable ASCII characters without whitespace.",
                retryable: false,
            },
            Self::CredentialPromptBusy => PublicError {
                code: "credential_prompt_busy",
                message: "A credential prompt is already open.",
                retryable: false,
            },
            Self::ProviderAuthentication => PublicError {
                code: "provider_authentication_failed",
                message: "The selected provider rejected its API key. Replace it in Settings and try again.",
                retryable: false,
            },
            Self::ProviderRateLimited => PublicError {
                code: "provider_rate_limited",
                message: "The selected provider is rate limiting requests. Wait briefly and try again.",
                retryable: true,
            },
            Self::ProviderUnavailable => PublicError {
                code: "provider_unavailable",
                message: "The selected provider is temporarily unavailable. Try again shortly.",
                retryable: true,
            },
            Self::ProviderRejected => PublicError {
                code: "provider_rejected_request",
                message: "The selected provider rejected this request. Review the message and response profile.",
                retryable: false,
            },
            Self::ProviderContract => PublicError {
                code: "provider_contract_rejected",
                message: "The selected provider rejected Aster's verified request contract.",
                retryable: false,
            },
            Self::ProviderContentRejected => PublicError {
                code: "provider_content_rejected",
                message: "The selected provider stopped this response because of its content policy.",
                retryable: false,
            },
            Self::ProviderContextLimit => PublicError {
                code: "provider_context_limit",
                message: "This conversation exceeds the selected provider's context limit. Start a new conversation.",
                retryable: false,
            },
            Self::UnsupportedProviderCapability => PublicError {
                code: "unsupported_provider_capability",
                message: "The provider attempted a capability that Aster does not support.",
                retryable: false,
            },
            Self::MalformedBalance => PublicError {
                code: "malformed_balance",
                message: "DeepSeek returned invalid balance data.",
                retryable: true,
            },
            Self::Network => PublicError {
                code: "network_error",
                message: "A secure connection to the selected provider could not be established.",
                retryable: true,
            },
            Self::Timeout => PublicError {
                code: "request_timeout",
                message: "The provider request timed out. Try again.",
                retryable: true,
            },
            Self::MalformedStream => PublicError {
                code: "malformed_stream",
                message: "The selected provider returned an invalid or incomplete response stream.",
                retryable: true,
            },
            Self::Cancelled => PublicError {
                code: "cancelled",
                message: "Generation was stopped.",
                retryable: false,
            },
            Self::Database(_) => PublicError {
                code: "database_error",
                message: "Local conversation storage is unavailable.",
                retryable: true,
            },
            Self::DatabaseIntegrity => PublicError {
                code: "database_integrity_error",
                message: "The local conversation database is incompatible or damaged.",
                retryable: false,
            },
            Self::File(_) => PublicError {
                code: "file_error",
                message: "Aster could not read or write the selected conversation file.",
                retryable: true,
            },
            Self::ExternalNavigation => PublicError {
                code: "external_navigation_failed",
                message: "Aster could not open this HTTPS link in the default browser.",
                retryable: true,
            },
            Self::Serialization => PublicError {
                code: "serialization_error",
                message: "The conversation data is not valid Aster JSON.",
                retryable: false,
            },
            Self::Internal => PublicError {
                code: "internal_error",
                message: "Aster could not complete the operation.",
                retryable: true,
            },
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.public().serialize(serializer)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::File(value)
    }
}
