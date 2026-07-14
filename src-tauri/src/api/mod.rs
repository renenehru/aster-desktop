use std::collections::HashSet;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{Client, ClientBuilder, StatusCode, redirect};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::{
    BalanceInfo, CatalogAccountAction, CatalogModel, CatalogProfile, CatalogProvider,
    MAX_SAFE_INTEGER, MessageFinishReason, ModelCatalog, ModelSelection, ProviderAccountAction,
    ProviderId, ProviderMessage, ResponseProfile, TokenUsage,
};

const ZAI_URL: &str = "https://api.z.ai/api/paas/v4/chat/completions";
const DEEPSEEK_URL: &str = "https://api.deepseek.com/chat/completions";
const ALIBABA_US_URL: &str =
    "https://dashscope-us.aliyuncs.com/compatible-mode/v1/chat/completions";
const GOOGLE_PREFIX: &str = "https://generativelanguage.googleapis.com/v1beta/models/";
const GOOGLE_SUFFIX: &str = ":streamGenerateContent?alt=sse";
const NVIDIA_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";
const DEEPSEEK_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const OVERALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const BALANCE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_HEADERS: usize = 64;
const MAX_RESPONSE_HEADER_BYTES: usize = 32 * 1024;
const MAX_STREAM_BYTES: usize = 8 * 1024 * 1024;
const MAX_BALANCE_BYTES: usize = 64 * 1024;
const MAX_SSE_LINE_BYTES: usize = 64 * 1024;
const MAX_SSE_EVENT_BYTES: usize = 128 * 1024;
const MAX_DELTA_BYTES: usize = 64 * 1024;
const MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;

const ZAI_MODELS: [(&str, &str); 4] = [
    ("glm-4.7", "GLM-4.7"),
    ("glm-5", "GLM-5"),
    ("glm-5.1", "GLM-5.1"),
    ("glm-5.2", "GLM-5.2"),
];
const DEEPSEEK_MODELS: [(&str, &str); 2] = [
    ("deepseek-v4-flash", "DeepSeek V4 Flash"),
    ("deepseek-v4-pro", "DeepSeek V4 Pro"),
];
const ALIBABA_MODELS: [(&str, &str); 6] = [
    ("qwen3.5-plus", "Qwen3.5 Plus"),
    ("qwen3.5-flash", "Qwen3.5 Flash"),
    ("qwen3.6-plus", "Qwen3.6 Plus"),
    ("qwen3.6-flash", "Qwen3.6 Flash"),
    ("qwen3.7-plus", "Qwen3.7 Plus"),
    ("qwen3.7-max", "Qwen3.7 Max"),
];
const GOOGLE_MODELS: [(&str, &str); 3] = [
    ("gemini-2.5-flash", "Gemini 2.5 Flash"),
    ("gemini-2.5-flash-lite", "Gemini 2.5 Flash-Lite"),
    ("gemini-2.5-pro", "Gemini 2.5 Pro"),
];
const NVIDIA_MODELS: [(&str, &str); 2] = [
    ("nvidia/nemotron-3-super-120b-a12b", "Nemotron 3 Super"),
    ("nvidia/nemotron-3-ultra-550b-a55b", "Nemotron 3 Ultra"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Authentication {
    Bearer,
    GoogleApiKey,
}

#[derive(Debug, Clone)]
struct ProviderDefinition {
    id: ProviderId,
    display_name: &'static str,
    region_label: Option<&'static str>,
    notice_version: u32,
    processing_notice: &'static str,
    models: &'static [(&'static str, &'static str)],
    account_actions: &'static [(ProviderAccountAction, &'static str, &'static str)],
}

const ZAI_ACTIONS: [(ProviderAccountAction, &str, &str); 2] = [
    (
        ProviderAccountAction::Billing,
        "Manage billing",
        "Open Z.AI billing in your default browser.",
    ),
    (
        ProviderAccountAction::AddCredits,
        "Add credits",
        "Open Z.AI billing in your default browser.",
    ),
];
const DEEPSEEK_ACTIONS: [(ProviderAccountAction, &str, &str); 2] = [
    (
        ProviderAccountAction::Usage,
        "View usage",
        "Open DeepSeek usage in your default browser.",
    ),
    (
        ProviderAccountAction::AddCredits,
        "Add credits",
        "Open DeepSeek top-up in your default browser.",
    ),
];
const ALIBABA_ACTIONS: [(ProviderAccountAction, &str, &str); 3] = [
    (
        ProviderAccountAction::Usage,
        "View usage",
        "Open Alibaba Cloud usage in your default browser.",
    ),
    (
        ProviderAccountAction::Billing,
        "Manage billing",
        "Open Alibaba Cloud billing in your default browser.",
    ),
    (
        ProviderAccountAction::AddCredits,
        "Add credits",
        "Open Alibaba Cloud billing account in your default browser.",
    ),
];
const GOOGLE_ACTIONS: [(ProviderAccountAction, &str, &str); 3] = [
    (
        ProviderAccountAction::Usage,
        "View usage",
        "Open Google AI Studio usage in your default browser.",
    ),
    (
        ProviderAccountAction::Billing,
        "Manage billing",
        "Open Google AI Studio billing in your default browser.",
    ),
    (
        ProviderAccountAction::Spend,
        "View spend",
        "Open Google AI Studio spend in your default browser.",
    ),
];
const NVIDIA_ACTIONS: [(ProviderAccountAction, &str, &str); 1] = [(
    ProviderAccountAction::Deployment,
    "Deployment options",
    "Open NVIDIA Build in your default browser.",
)];

fn provider_definition(provider: ProviderId) -> ProviderDefinition {
    match provider {
        ProviderId::Zai => ProviderDefinition {
            id: provider,
            display_name: "Z.AI",
            region_label: None,
            notice_version: 1,
            processing_notice: "Aster sends relevant conversation messages directly to Z.AI. Review Z.AI data handling before sending sensitive information.",
            models: &ZAI_MODELS,
            account_actions: &ZAI_ACTIONS,
        },
        ProviderId::DeepSeek => ProviderDefinition {
            id: provider,
            display_name: "DeepSeek",
            region_label: None,
            notice_version: 1,
            processing_notice: "Aster sends relevant conversation messages directly to DeepSeek. Review DeepSeek data handling before sending sensitive information.",
            models: &DEEPSEEK_MODELS,
            account_actions: &DEEPSEEK_ACTIONS,
        },
        ProviderId::AlibabaUs => ProviderDefinition {
            id: provider,
            display_name: "Alibaba Cloud (US)",
            region_label: Some("United States"),
            notice_version: 1,
            processing_notice: "Aster sends relevant conversation messages directly to Alibaba Cloud's fixed United States region. Review Alibaba Cloud data handling before sending sensitive information.",
            models: &ALIBABA_MODELS,
            account_actions: &ALIBABA_ACTIONS,
        },
        ProviderId::Google => ProviderDefinition {
            id: provider,
            display_name: "Google Gemini",
            region_label: None,
            notice_version: 1,
            processing_notice: "Aster sends relevant conversation messages directly to Google Gemini. Review Google data handling before sending sensitive information.",
            models: &GOOGLE_MODELS,
            account_actions: &GOOGLE_ACTIONS,
        },
        ProviderId::Nvidia => ProviderDefinition {
            id: provider,
            display_name: "NVIDIA",
            region_label: None,
            notice_version: 1,
            processing_notice: "Aster sends relevant conversation messages to NVIDIA's hosted prototype service for evaluation. This is not a production deployment or evidence of NVIDIA AI Enterprise coverage.",
            models: &NVIDIA_MODELS,
            account_actions: &NVIDIA_ACTIONS,
        },
    }
}

pub fn model_catalog() -> ModelCatalog {
    ModelCatalog {
        version: 2,
        default_selection: ModelSelection {
            provider_id: ProviderId::Zai,
            model_id: "glm-5.1",
        },
        providers: ProviderId::ALL
            .into_iter()
            .map(|provider| {
                let definition = provider_definition(provider);
                CatalogProvider {
                    id: definition.id,
                    display_name: definition.display_name,
                    region_label: definition.region_label,
                    notice_version: definition.notice_version,
                    processing_notice: definition.processing_notice,
                    account_actions: definition
                        .account_actions
                        .iter()
                        .map(|(action, label, description)| CatalogAccountAction {
                            action: *action,
                            label,
                            description,
                        })
                        .collect(),
                    models: definition
                        .models
                        .iter()
                        .map(|(id, display_name)| CatalogModel {
                            id,
                            display_name,
                            delivery: if provider == ProviderId::Nvidia {
                                "hosted-prototype"
                            } else {
                                "official-api"
                            },
                            profiles: catalog_profiles(provider, id),
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

fn catalog_profiles(provider: ProviderId, model: &str) -> Vec<CatalogProfile> {
    ResponseProfile::ALL
        .into_iter()
        .map(|profile| CatalogProfile {
            id: profile,
            label: match profile {
                ResponseProfile::Fast => "Fast",
                ResponseProfile::Standard => "Standard",
                ResponseProfile::Deep => "Deep",
            },
            description: match (provider, model, profile) {
                (ProviderId::Google, "gemini-2.5-pro", ResponseProfile::Fast) => {
                    "Minimum documented thinking with a 4,096-token output cap."
                }
                (_, _, ResponseProfile::Fast) => {
                    "Provider thinking disabled where supported, with a 4,096-token output cap."
                }
                (_, _, ResponseProfile::Standard) => {
                    "Provider-specific standard mapping with an 8,192-token output cap."
                }
                (_, _, ResponseProfile::Deep) => {
                    "Provider-specific deep mapping with a 16,384-token output cap."
                }
            },
            enabled: true,
            disabled_reason: None,
        })
        .collect()
}

pub fn validate_selection(provider: ProviderId, model: &str) -> AppResult<()> {
    let definition = provider_definition(provider);
    if definition
        .models
        .iter()
        .any(|(candidate, _)| *candidate == model)
    {
        Ok(())
    } else {
        Err(AppError::Validation(
            "The provider and model pair is not in the Aster catalog.",
        ))
    }
}

pub fn provider_notice_version(provider: ProviderId) -> u32 {
    provider_definition(provider).notice_version
}

pub fn account_url(provider: ProviderId, action: ProviderAccountAction) -> AppResult<&'static str> {
    match (provider, action) {
        (ProviderId::Zai, ProviderAccountAction::Billing | ProviderAccountAction::AddCredits) => {
            Ok("https://z.ai/manage-apikey/billing")
        }
        (ProviderId::DeepSeek, ProviderAccountAction::Usage) => {
            Ok("https://platform.deepseek.com/usage")
        }
        (ProviderId::DeepSeek, ProviderAccountAction::AddCredits) => {
            Ok("https://platform.deepseek.com/top_up")
        }
        (ProviderId::AlibabaUs, ProviderAccountAction::Usage) => {
            Ok("https://modelstudio.console.alibabacloud.com/?tab=costing-balance")
        }
        (ProviderId::AlibabaUs, ProviderAccountAction::Billing) => Ok(
            "https://usercenter2-intl.console.alibabacloud.com/finance/expense-report/expense-detail-by-instance",
        ),
        (ProviderId::AlibabaUs, ProviderAccountAction::AddCredits) => {
            Ok("https://billing-cost-intl.aliyun.com/fortune/billing-account")
        }
        (ProviderId::Google, ProviderAccountAction::Usage) => {
            Ok("https://aistudio.google.com/usage")
        }
        (ProviderId::Google, ProviderAccountAction::Billing) => {
            Ok("https://aistudio.google.com/billing")
        }
        (ProviderId::Google, ProviderAccountAction::Spend) => {
            Ok("https://aistudio.google.com/spend")
        }
        (ProviderId::Nvidia, ProviderAccountAction::Deployment) => Ok("https://build.nvidia.com/"),
        _ => Err(AppError::Validation(
            "This account action is not supported for the selected provider.",
        )),
    }
}

#[derive(Debug, Clone)]
struct BuiltRequest {
    url: String,
    authentication: Authentication,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedProviderRequest {
    provider: ProviderId,
    request: BuiltRequest,
}

fn build_request(
    provider: ProviderId,
    model: &str,
    profile: ResponseProfile,
    messages: &[ProviderMessage],
) -> AppResult<BuiltRequest> {
    validate_selection(provider, model)?;
    let openai_messages = || {
        messages
            .iter()
            .map(|message| {
                json!({
                    "role": message.role.as_str(),
                    "content": message.content,
                })
            })
            .collect::<Vec<_>>()
    };
    let max_tokens = profile.max_output_tokens();
    let (url, authentication, body) = match provider {
        ProviderId::Zai => (
            ZAI_URL.to_owned(),
            Authentication::Bearer,
            json!({
                "model": model,
                "messages": openai_messages(),
                "stream": true,
                "thinking": {"type": profile.thinking_type()},
                "max_tokens": max_tokens,
            }),
        ),
        ProviderId::DeepSeek => {
            let mut body = json!({
                "model": model,
                "messages": openai_messages(),
                "stream": true,
                "stream_options": {"include_usage": true},
                "thinking": {"type": profile.thinking_type()},
                "max_tokens": max_tokens,
            });
            if profile != ResponseProfile::Fast {
                body.as_object_mut().ok_or(AppError::Internal)?.insert(
                    "reasoning_effort".to_owned(),
                    Value::String(
                        match profile {
                            ResponseProfile::Standard => "high",
                            ResponseProfile::Deep => "max",
                            ResponseProfile::Fast => unreachable!(),
                        }
                        .to_owned(),
                    ),
                );
            }
            (DEEPSEEK_URL.to_owned(), Authentication::Bearer, body)
        }
        ProviderId::AlibabaUs => (
            ALIBABA_US_URL.to_owned(),
            Authentication::Bearer,
            json!({
                "model": model,
                "messages": openai_messages(),
                "stream": true,
                "stream_options": {"include_usage": true},
                "enable_thinking": profile != ResponseProfile::Fast,
                "max_completion_tokens": max_tokens,
            }),
        ),
        ProviderId::Google => {
            let contents = messages
                .iter()
                .map(|message| {
                    json!({
                        "role": if message.role == crate::models::MessageRole::User { "user" } else { "model" },
                        "parts": [{"text": message.content}],
                    })
                })
                .collect::<Vec<_>>();
            let thinking_budget = match (model, profile) {
                ("gemini-2.5-pro", ResponseProfile::Fast) => 128,
                ("gemini-2.5-pro", ResponseProfile::Standard) => 4_096,
                ("gemini-2.5-pro", ResponseProfile::Deep) => 16_384,
                (_, ResponseProfile::Fast) => 0,
                (_, ResponseProfile::Standard) => 1_024,
                (_, ResponseProfile::Deep) => 8_192,
            };
            (
                format!("{GOOGLE_PREFIX}{model}{GOOGLE_SUFFIX}"),
                Authentication::GoogleApiKey,
                json!({
                    "contents": contents,
                    "generationConfig": {
                        "thinkingConfig": {"thinkingBudget": thinking_budget},
                        "maxOutputTokens": max_tokens,
                    },
                }),
            )
        }
        ProviderId::Nvidia => {
            let mut body = json!({
                "model": model,
                "messages": openai_messages(),
                "stream": true,
                "temperature": 1.0,
                "top_p": 0.95,
                "max_tokens": max_tokens,
                "chat_template_kwargs": {
                    "enable_thinking": profile != ResponseProfile::Fast,
                },
            });
            if profile != ResponseProfile::Fast {
                body.as_object_mut().ok_or(AppError::Internal)?.insert(
                    "reasoning_budget".to_owned(),
                    Value::from(match profile {
                        ResponseProfile::Standard => 4_096,
                        ResponseProfile::Deep => 16_384,
                        ResponseProfile::Fast => unreachable!(),
                    }),
                );
            }
            (NVIDIA_URL.to_owned(), Authentication::Bearer, body)
        }
    };
    let body = serde_json::to_vec(&body).map_err(|_| AppError::Serialization)?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(AppError::Validation(
            "The provider request context is too large.",
        ));
    }
    Ok(BuiltRequest {
        url,
        authentication,
        body,
    })
}

#[derive(Clone)]
pub struct ProviderClient {
    client: Client,
    #[cfg(test)]
    endpoint_override: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOutcome {
    pub usage: Option<TokenUsage>,
    pub finish_reason: MessageFinishReason,
}

#[cfg(test)]
pub struct ProviderChatRequest<'a> {
    pub provider: ProviderId,
    pub model: &'a str,
    pub profile: ResponseProfile,
    pub messages: &'a [ProviderMessage],
    pub api_key: &'a str,
    pub cancellation: &'a CancellationToken,
}

impl ProviderClient {
    pub fn new() -> AppResult<Self> {
        let client = provider_client_builder()
            .https_only(true)
            .user_agent(concat!("Aster/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| AppError::Internal)?;
        Ok(Self {
            client,
            #[cfg(test)]
            endpoint_override: None,
        })
    }

    #[cfg(test)]
    fn for_controlled_server(endpoint: String) -> AppResult<Self> {
        let client = provider_client_builder()
            .build()
            .map_err(|_| AppError::Internal)?;
        Ok(Self {
            client,
            endpoint_override: Some(endpoint),
        })
    }

    pub(crate) fn prepare_chat(
        &self,
        provider: ProviderId,
        model: &str,
        profile: ResponseProfile,
        messages: &[ProviderMessage],
    ) -> AppResult<PreparedProviderRequest> {
        let request = build_request(provider, model, profile, messages)?;
        #[cfg(test)]
        let request = {
            let mut request = request;
            if let Some(endpoint) = &self.endpoint_override {
                request.url.clone_from(endpoint);
            }
            request
        };
        Ok(PreparedProviderRequest { provider, request })
    }

    #[cfg(test)]
    pub async fn stream_chat<F>(
        &self,
        request: ProviderChatRequest<'_>,
        on_delta: F,
    ) -> AppResult<ProviderOutcome>
    where
        F: FnMut(String) -> AppResult<()>,
    {
        let ProviderChatRequest {
            provider,
            model,
            profile,
            messages,
            api_key,
            cancellation,
        } = request;
        let request = self.prepare_chat(provider, model, profile, messages)?;
        self.stream_prepared_chat(request, api_key, cancellation, on_delta)
            .await
    }

    pub(crate) async fn stream_prepared_chat<F>(
        &self,
        prepared: PreparedProviderRequest,
        api_key: &str,
        cancellation: &CancellationToken,
        mut on_delta: F,
    ) -> AppResult<ProviderOutcome>
    where
        F: FnMut(String) -> AppResult<()>,
    {
        let PreparedProviderRequest { provider, request } = prepared;
        timeout(
            OVERALL_TIMEOUT,
            self.stream_attempt(provider, request, api_key, cancellation, &mut on_delta),
        )
        .await
        .unwrap_or(Err(AppError::Timeout))
    }

    async fn stream_attempt<F>(
        &self,
        provider: ProviderId,
        request: BuiltRequest,
        api_key: &str,
        cancellation: &CancellationToken,
        on_delta: &mut F,
    ) -> AppResult<ProviderOutcome>
    where
        F: FnMut(String) -> AppResult<()>,
    {
        let mut builder = self
            .client
            .post(&request.url)
            .header(ACCEPT, "text/event-stream")
            .header(CONTENT_TYPE, "application/json");
        builder = match request.authentication {
            Authentication::Bearer => builder.bearer_auth(api_key),
            Authentication::GoogleApiKey => builder.header("x-goog-api-key", api_key),
        };
        let send = builder.body(request.body).send();
        // Poll the request future before observing cancellation. The caller
        // commits its partial usage marker only after the final pre-attempt
        // cancellation check, so a later cancellation must still cross an
        // actual request-attempt boundary rather than leaving a false marker.
        let response = tokio::select! {
            biased;
            response = send => response.map_err(map_transport_error)?,
            _ = cancellation.cancelled() => return Err(AppError::Cancelled),
        };
        validate_response_headers(&response, MAX_STREAM_BYTES)?;
        map_status(response.status())?;
        validate_content_type(&response, "text/event-stream")?;

        let mut parser = StreamParser::new(provider);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AppError::Cancelled),
            chunk = timeout(IDLE_TIMEOUT, stream.next()) => chunk.map_err(|_| AppError::Timeout)?,
        } {
            let chunk = chunk.map_err(map_transport_error)?;
            for output in parser.push(&chunk)? {
                let SseOutput::Delta(delta) = output;
                on_delta(delta)?;
            }
        }
        let (outputs, outcome) = parser.finish()?;
        for output in outputs {
            let SseOutput::Delta(delta) = output;
            on_delta(delta)?;
        }
        Ok(outcome)
    }

    pub async fn refresh_deepseek_balance(
        &self,
        api_key: &str,
        cancellation: &CancellationToken,
    ) -> AppResult<ParsedBalance> {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AppError::Cancelled),
            result = timeout(BALANCE_TIMEOUT, self.balance_attempt(api_key, cancellation)) => {
                result.unwrap_or(Err(AppError::Timeout))
            }
        }
    }

    async fn balance_attempt(
        &self,
        api_key: &str,
        cancellation: &CancellationToken,
    ) -> AppResult<ParsedBalance> {
        let send = self
            .client
            .get(DEEPSEEK_BALANCE_URL)
            .header(ACCEPT, "application/json")
            .bearer_auth(api_key)
            .send();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AppError::Cancelled),
            response = send => response.map_err(map_transport_error)?,
        };
        validate_response_headers(&response, MAX_BALANCE_BYTES).map_err(map_balance_validation)?;
        map_status(response.status())?;
        validate_content_type(&response, "application/json").map_err(map_balance_validation)?;
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AppError::Cancelled),
            chunk = stream.next() => chunk,
        } {
            let chunk = chunk.map_err(map_transport_error)?;
            if body
                .len()
                .checked_add(chunk.len())
                .is_none_or(|length| length > MAX_BALANCE_BYTES)
            {
                return Err(AppError::MalformedBalance);
            }
            body.extend_from_slice(&chunk);
        }
        parse_deepseek_balance(&body)
    }
}

fn provider_client_builder() -> ClientBuilder {
    Client::builder()
        .no_proxy()
        .redirect(redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(IDLE_TIMEOUT)
        .timeout(OVERALL_TIMEOUT)
}

fn map_transport_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::Timeout
    } else {
        AppError::Network
    }
}

fn map_balance_validation(error: AppError) -> AppError {
    match error {
        AppError::MalformedStream => AppError::MalformedBalance,
        error => error,
    }
}

fn map_status(status: StatusCode) -> AppResult<()> {
    match status.as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(AppError::ProviderAuthentication),
        400 | 422 => Err(AppError::ProviderContract),
        429 => Err(AppError::ProviderRateLimited),
        408 | 425 | 500..=599 => Err(AppError::ProviderUnavailable),
        _ => Err(AppError::ProviderRejected),
    }
}

fn validate_response_headers(response: &reqwest::Response, maximum_body: usize) -> AppResult<()> {
    let headers = response.headers();
    if headers.len() > MAX_RESPONSE_HEADERS {
        return Err(AppError::MalformedStream);
    }
    let total_bytes = headers.iter().try_fold(0usize, |total, (name, value)| {
        total
            .checked_add(name.as_str().len())
            .and_then(|value_total| value_total.checked_add(value.as_bytes().len()))
            .ok_or(AppError::MalformedStream)
    })?;
    if total_bytes > MAX_RESPONSE_HEADER_BYTES {
        return Err(AppError::MalformedStream);
    }
    if let Some(value) = headers.get(CONTENT_LENGTH) {
        let length = value
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or(AppError::MalformedStream)?;
        if length > maximum_body as u64 {
            return Err(AppError::MalformedStream);
        }
    }
    Ok(())
}

fn validate_content_type(response: &reqwest::Response, expected: &str) -> AppResult<()> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::MalformedStream)?;
    let media_type = content_type
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default();
    if !media_type.eq_ignore_ascii_case(expected) {
        return Err(AppError::MalformedStream);
    }
    Ok(())
}

#[derive(Debug)]
enum SseOutput {
    Delta(String),
}

struct StreamParser {
    provider: ProviderId,
    framing: SseFraming,
    saw_terminal: bool,
    saw_done: bool,
    saw_usage: bool,
    finish_reason: Option<MessageFinishReason>,
    content_bytes: usize,
    usage: Option<TokenUsage>,
}

impl StreamParser {
    fn new(provider: ProviderId) -> Self {
        Self {
            provider,
            framing: SseFraming::default(),
            saw_terminal: false,
            saw_done: false,
            saw_usage: false,
            finish_reason: None,
            content_bytes: 0,
            usage: None,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> AppResult<Vec<SseOutput>> {
        let events = self.framing.push(chunk)?;
        self.process_events(events)
    }

    fn process_events(&mut self, events: Vec<String>) -> AppResult<Vec<SseOutput>> {
        let mut outputs = Vec::new();
        for event in events {
            if self.saw_done {
                return Err(AppError::MalformedStream);
            }
            match self.provider {
                ProviderId::Google => self.process_google_event(&event, &mut outputs)?,
                _ => self.process_openai_event(&event, &mut outputs)?,
            }
        }
        Ok(outputs)
    }

    fn finish(mut self) -> AppResult<(Vec<SseOutput>, ProviderOutcome)> {
        let events = self.framing.finish()?;
        let outputs = self.process_events(events)?;
        let valid_terminal = if self.provider == ProviderId::Google {
            self.saw_terminal && !self.saw_done
        } else {
            self.saw_terminal && self.saw_done
        };
        if !valid_terminal || self.content_bytes == 0 {
            return Err(AppError::MalformedStream);
        }
        let finish_reason = self.finish_reason.ok_or(AppError::MalformedStream)?;
        Ok((
            outputs,
            ProviderOutcome {
                usage: self.usage,
                finish_reason,
            },
        ))
    }

    fn process_openai_event(&mut self, event: &str, outputs: &mut Vec<SseOutput>) -> AppResult<()> {
        if event.trim() == "[DONE]" {
            if !self.saw_terminal || self.saw_done {
                return Err(AppError::MalformedStream);
            }
            self.saw_done = true;
            return Ok(());
        }
        let value: Value = serde_json::from_str(event).map_err(|_| AppError::MalformedStream)?;
        let object = value.as_object().ok_or(AppError::MalformedStream)?;
        let choices = object
            .get("choices")
            .and_then(Value::as_array)
            .ok_or(AppError::MalformedStream)?;
        if choices.len() > 1 || (choices.is_empty() && !object.contains_key("usage")) {
            return Err(AppError::MalformedStream);
        }
        if self.saw_terminal && !choices.is_empty() {
            return Err(AppError::MalformedStream);
        }
        let terminal_before_event = self.saw_terminal;
        if let Some(choice) = choices.first() {
            let choice = choice.as_object().ok_or(AppError::MalformedStream)?;
            let valid_index = match choice.get("index") {
                None => true,
                Some(Value::Number(index)) => index.as_u64() == Some(0),
                Some(_) => false,
            };
            if !valid_index || choice.get("logprobs").is_some_and(|value| !value.is_null()) {
                return Err(AppError::MalformedStream);
            }
            if let Some(delta) = choice.get("delta").filter(|value| !value.is_null()) {
                let delta = delta.as_object().ok_or(AppError::MalformedStream)?;
                for key in delta.keys() {
                    match key.as_str() {
                        "role" | "content" | "reasoning_content" => {}
                        "tool_calls" | "function_call" => {
                            return Err(AppError::UnsupportedProviderCapability);
                        }
                        _ => return Err(AppError::UnsupportedProviderCapability),
                    }
                }
                match delta.get("role") {
                    None => {}
                    Some(Value::String(role)) if role == "assistant" => {}
                    Some(_) => return Err(AppError::MalformedStream),
                }
                if let Some(content) = delta.get("content").filter(|value| !value.is_null()) {
                    let content = content.as_str().ok_or(AppError::MalformedStream)?;
                    self.push_visible_delta(content, outputs)?;
                }
                if delta
                    .get("reasoning_content")
                    .filter(|value| !value.is_null())
                    .is_some_and(|value| !value.is_string())
                {
                    return Err(AppError::MalformedStream);
                }
                // `reasoning_content` is intentionally discarded.
            }
            if let Some(reason) = choice.get("finish_reason").filter(|value| !value.is_null()) {
                let reason = reason.as_str().ok_or(AppError::MalformedStream)?;
                self.map_openai_finish(reason)?;
            }
        }
        let terminalized_this_event = !terminal_before_event && self.saw_terminal;
        if let Some(raw_usage) = object.get("usage").filter(|value| !value.is_null()) {
            let authoritative_position = match self.provider {
                ProviderId::DeepSeek | ProviderId::AlibabaUs => {
                    choices.is_empty() && self.saw_terminal
                }
                ProviderId::Zai | ProviderId::Nvidia => {
                    !choices.is_empty() && terminalized_this_event
                }
                ProviderId::Google => unreachable!(),
            };
            if !authoritative_position || self.saw_usage {
                return Err(AppError::MalformedStream);
            }
            self.saw_usage = true;
            self.usage = normalize_usage(self.provider, raw_usage);
        }
        Ok(())
    }

    fn map_openai_finish(&mut self, reason: &str) -> AppResult<()> {
        if self.saw_terminal {
            return Err(AppError::MalformedStream);
        }
        match (self.provider, reason) {
            (_, "stop") => {
                self.saw_terminal = true;
                self.finish_reason = Some(MessageFinishReason::Stop);
            }
            (_, "length") => {
                self.saw_terminal = true;
                self.finish_reason = Some(MessageFinishReason::OutputLimit);
            }
            (ProviderId::Zai, "sensitive")
            | (ProviderId::DeepSeek | ProviderId::Nvidia, "content_filter") => {
                return Err(AppError::ProviderContentRejected);
            }
            (ProviderId::Zai, "model_context_window_exceeded") => {
                return Err(AppError::ProviderContextLimit);
            }
            (ProviderId::Zai, "network_error")
            | (ProviderId::DeepSeek, "insufficient_system_resource") => {
                return Err(AppError::ProviderUnavailable);
            }
            (_, "tool_calls" | "function_call") => {
                return Err(AppError::UnsupportedProviderCapability);
            }
            _ => return Err(AppError::MalformedStream),
        }
        Ok(())
    }

    fn process_google_event(&mut self, event: &str, outputs: &mut Vec<SseOutput>) -> AppResult<()> {
        if event.trim() == "[DONE]" || self.saw_terminal {
            return Err(AppError::MalformedStream);
        }
        let terminal_before_event = self.saw_terminal;
        let value: Value = serde_json::from_str(event).map_err(|_| AppError::MalformedStream)?;
        let object = value.as_object().ok_or(AppError::MalformedStream)?;
        if let Some(feedback) = object.get("promptFeedback") {
            let feedback = feedback.as_object().ok_or(AppError::MalformedStream)?;
            if let Some(reason) = feedback.get("blockReason") {
                let reason = reason.as_str().ok_or(AppError::MalformedStream)?;
                match reason {
                    "SAFETY" | "OTHER" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "IMAGE_SAFETY" => {
                        return Err(AppError::ProviderContentRejected);
                    }
                    "BLOCK_REASON_UNSPECIFIED" => return Err(AppError::MalformedStream),
                    _ => return Err(AppError::MalformedStream),
                }
            }
        }
        let candidates = match object.get("candidates") {
            Some(value) => value
                .as_array()
                .map(Vec::as_slice)
                .ok_or(AppError::MalformedStream)?,
            None => &[],
        };
        if candidates.len() > 1 {
            return Err(AppError::MalformedStream);
        }
        if let Some(candidate) = candidates.first() {
            let candidate = candidate.as_object().ok_or(AppError::MalformedStream)?;
            if let Some(content) = candidate.get("content") {
                let parts = content
                    .get("parts")
                    .and_then(Value::as_array)
                    .ok_or(AppError::MalformedStream)?;
                for part in parts {
                    let part = part.as_object().ok_or(AppError::MalformedStream)?;
                    if part.is_empty() {
                        return Err(AppError::MalformedStream);
                    }
                    for key in part.keys() {
                        match key.as_str() {
                            "text" | "thought" | "thoughtSignature" => {}
                            "functionCall"
                            | "functionResponse"
                            | "executableCode"
                            | "codeExecutionResult"
                            | "fileData"
                            | "inlineData" => {
                                return Err(AppError::UnsupportedProviderCapability);
                            }
                            _ => return Err(AppError::UnsupportedProviderCapability),
                        }
                    }
                    let text = match part.get("text") {
                        Some(Value::String(text)) => Some(text.as_str()),
                        None => None,
                        Some(_) => return Err(AppError::MalformedStream),
                    };
                    if part
                        .get("thoughtSignature")
                        .is_some_and(|value| !value.is_string())
                    {
                        return Err(AppError::MalformedStream);
                    }
                    let hidden_thought = match part.get("thought") {
                        Some(Value::Bool(value)) => *value,
                        None => false,
                        Some(_) => return Err(AppError::MalformedStream),
                    };
                    if hidden_thought {
                        continue;
                    }
                    if let Some(text) = text {
                        self.push_visible_delta(text, outputs)?;
                    }
                    // Thought signatures are never emitted or persisted.
                }
            }
            if let Some(reason) = candidate
                .get("finishReason")
                .filter(|value| !value.is_null())
            {
                self.map_google_finish(reason.as_str().ok_or(AppError::MalformedStream)?)?;
            }
        }
        let terminalized_this_event = !terminal_before_event && self.saw_terminal;
        if let Some(raw_usage) = object.get("usageMetadata").filter(|value| !value.is_null()) {
            if !terminalized_this_event || self.saw_usage {
                return Err(AppError::MalformedStream);
            }
            self.saw_usage = true;
            self.usage = normalize_usage(ProviderId::Google, raw_usage);
        }
        Ok(())
    }

    fn map_google_finish(&mut self, reason: &str) -> AppResult<()> {
        match reason {
            "STOP" => {
                self.saw_terminal = true;
                self.finish_reason = Some(MessageFinishReason::Stop);
            }
            "MAX_TOKENS" => {
                self.saw_terminal = true;
                self.finish_reason = Some(MessageFinishReason::OutputLimit);
            }
            "SAFETY"
            | "RECITATION"
            | "LANGUAGE"
            | "BLOCKLIST"
            | "PROHIBITED_CONTENT"
            | "SPII"
            | "IMAGE_SAFETY"
            | "IMAGE_PROHIBITED_CONTENT"
            | "IMAGE_OTHER"
            | "NO_IMAGE"
            | "IMAGE_RECITATION" => return Err(AppError::ProviderContentRejected),
            "MALFORMED_FUNCTION_CALL"
            | "UNEXPECTED_TOOL_CALL"
            | "TOO_MANY_TOOL_CALLS"
            | "MISSING_THOUGHT_SIGNATURE" => {
                return Err(AppError::UnsupportedProviderCapability);
            }
            "OTHER" | "MALFORMED_RESPONSE" | "FINISH_REASON_UNSPECIFIED" => {
                return Err(AppError::MalformedStream);
            }
            _ => return Err(AppError::MalformedStream),
        }
        Ok(())
    }

    fn push_visible_delta(&mut self, content: &str, outputs: &mut Vec<SseOutput>) -> AppResult<()> {
        if content.len() > MAX_DELTA_BYTES
            || content.chars().any(|character| {
                character == '\0'
                    || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            })
        {
            return Err(AppError::MalformedStream);
        }
        self.content_bytes = self
            .content_bytes
            .checked_add(content.len())
            .filter(|length| *length <= MAX_CONTENT_BYTES)
            .ok_or(AppError::MalformedStream)?;
        if !content.is_empty() {
            outputs.push(SseOutput::Delta(content.to_owned()));
        }
        Ok(())
    }
}

#[derive(Default)]
struct SseFraming {
    line_buffer: Vec<u8>,
    event_data: Vec<u8>,
    transport_bytes: usize,
}

impl SseFraming {
    fn push(&mut self, chunk: &[u8]) -> AppResult<Vec<String>> {
        self.transport_bytes = self
            .transport_bytes
            .checked_add(chunk.len())
            .filter(|length| *length <= MAX_STREAM_BYTES)
            .ok_or(AppError::MalformedStream)?;
        self.line_buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        let mut consumed = 0usize;
        while let Some(relative) = self.line_buffer[consumed..]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            let end = consumed + relative;
            if end - consumed > MAX_SSE_LINE_BYTES {
                return Err(AppError::MalformedStream);
            }
            let line = self.line_buffer[consumed..end].to_vec();
            consumed = end + 1;
            self.process_line(&line, &mut events)?;
        }
        if consumed != 0 {
            self.line_buffer.drain(..consumed);
        }
        if self.line_buffer.len() > MAX_SSE_LINE_BYTES {
            return Err(AppError::MalformedStream);
        }
        Ok(events)
    }

    fn finish(&mut self) -> AppResult<Vec<String>> {
        let mut events = Vec::new();
        if !self.line_buffer.is_empty() {
            let line = std::mem::take(&mut self.line_buffer);
            self.process_line(&line, &mut events)?;
        }
        if !self.event_data.is_empty() {
            events.push(self.take_event()?);
        }
        Ok(events)
    }

    fn process_line(&mut self, raw: &[u8], events: &mut Vec<String>) -> AppResult<()> {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let line = std::str::from_utf8(raw).map_err(|_| AppError::MalformedStream)?;
        if line.is_empty() {
            if !self.event_data.is_empty() {
                events.push(self.take_event()?);
            }
            return Ok(());
        }
        if line.starts_with(':') {
            return Ok(());
        }
        let data = line
            .strip_prefix("data:")
            .ok_or(AppError::MalformedStream)?
            .strip_prefix(' ')
            .unwrap_or_else(|| line.strip_prefix("data:").unwrap_or_default());
        let separator = usize::from(!self.event_data.is_empty());
        let next = self
            .event_data
            .len()
            .checked_add(separator)
            .and_then(|length| length.checked_add(data.len()))
            .filter(|length| *length <= MAX_SSE_EVENT_BYTES)
            .ok_or(AppError::MalformedStream)?;
        let _ = next;
        if separator == 1 {
            self.event_data.push(b'\n');
        }
        self.event_data.extend_from_slice(data.as_bytes());
        Ok(())
    }

    fn take_event(&mut self) -> AppResult<String> {
        String::from_utf8(std::mem::take(&mut self.event_data))
            .map_err(|_| AppError::MalformedStream)
    }
}

fn optional_json_safe_u64(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, ()> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .filter(|value| *value <= MAX_SAFE_INTEGER)
            .map(Some)
            .ok_or(()),
    }
}

fn checked_usage_add(left: u64, right: u64) -> Option<u64> {
    left.checked_add(right)
        .filter(|value| *value <= MAX_SAFE_INTEGER)
}

fn normalize_usage(provider: ProviderId, raw: &Value) -> Option<TokenUsage> {
    let object = raw.as_object()?;
    let usage = match provider {
        ProviderId::Zai | ProviderId::AlibabaUs | ProviderId::Nvidia => {
            let prompt = optional_json_safe_u64(object, "prompt_tokens").ok()?;
            let cached = match object.get("prompt_tokens_details") {
                None | Some(Value::Null) => None,
                Some(value) => optional_json_safe_u64(value.as_object()?, "cached_tokens").ok()?,
            };
            let completion = optional_json_safe_u64(object, "completion_tokens").ok()?;
            let total = optional_json_safe_u64(object, "total_tokens").ok()?;
            if let (Some(prompt), Some(completion)) = (prompt, completion) {
                let computed_total = checked_usage_add(prompt, completion)?;
                if total.is_some_and(|total| total != computed_total) {
                    return None;
                }
            }
            let input = match (prompt, cached) {
                (Some(prompt), Some(cached)) => Some(prompt.checked_sub(cached)?),
                _ => None,
            };
            TokenUsage {
                input_tokens: input,
                cached_input_tokens: cached,
                output_tokens: completion,
                total_tokens: total,
            }
        }
        ProviderId::DeepSeek => {
            let prompt = optional_json_safe_u64(object, "prompt_tokens").ok()?;
            let miss = optional_json_safe_u64(object, "prompt_cache_miss_tokens").ok()?;
            let hit = optional_json_safe_u64(object, "prompt_cache_hit_tokens").ok()?;
            let completion = optional_json_safe_u64(object, "completion_tokens").ok()?;
            let total = optional_json_safe_u64(object, "total_tokens").ok()?;
            let computed_prompt = match (miss, hit) {
                (Some(miss), Some(hit)) => Some(checked_usage_add(miss, hit)?),
                _ => None,
            };
            if let Some(computed_prompt) = computed_prompt
                && prompt.is_some_and(|prompt| prompt != computed_prompt)
            {
                return None;
            }
            if let (Some(prompt), Some(completion)) = (prompt.or(computed_prompt), completion) {
                let computed_total = checked_usage_add(prompt, completion)?;
                if total.is_some_and(|total| total != computed_total) {
                    return None;
                }
            }
            TokenUsage {
                input_tokens: miss,
                cached_input_tokens: hit,
                output_tokens: completion,
                total_tokens: total,
            }
        }
        ProviderId::Google => {
            let prompt = optional_json_safe_u64(object, "promptTokenCount").ok()?;
            let cached = optional_json_safe_u64(object, "cachedContentTokenCount").ok()?;
            let candidates = optional_json_safe_u64(object, "candidatesTokenCount").ok()?;
            let thoughts = optional_json_safe_u64(object, "thoughtsTokenCount").ok()?;
            let total = optional_json_safe_u64(object, "totalTokenCount").ok()?;
            let input = match (prompt, cached) {
                (Some(prompt), Some(cached)) => Some(prompt.checked_sub(cached)?),
                _ => None,
            };
            let output = match (candidates, thoughts) {
                (Some(candidates), Some(thoughts)) => {
                    Some(checked_usage_add(candidates, thoughts)?)
                }
                _ => None,
            };
            if let (Some(prompt), Some(candidates), Some(thoughts)) = (prompt, candidates, thoughts)
            {
                let computed_total =
                    checked_usage_add(checked_usage_add(prompt, candidates)?, thoughts)?;
                if total.is_some_and(|total| total != computed_total) {
                    return None;
                }
            }
            TokenUsage {
                input_tokens: input,
                cached_input_tokens: cached,
                output_tokens: output,
                total_tokens: total,
            }
        }
    };
    usage.validate().ok()?;
    (!usage.is_empty()).then_some(usage)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBalance {
    pub is_available: bool,
    pub balance_infos: Vec<BalanceInfo>,
}

#[derive(Debug, Deserialize)]
struct RawBalance {
    is_available: bool,
    balance_infos: Vec<RawBalanceInfo>,
}

#[derive(Debug, Deserialize)]
struct RawBalanceInfo {
    currency: String,
    total_balance: String,
    granted_balance: String,
    topped_up_balance: String,
}

pub(crate) fn parse_deepseek_balance(body: &[u8]) -> AppResult<ParsedBalance> {
    if body.len() > MAX_BALANCE_BYTES {
        return Err(AppError::MalformedBalance);
    }
    validate_bounded_json_nesting(body, 8).map_err(|_| AppError::MalformedBalance)?;
    let raw: RawBalance = serde_json::from_slice(body).map_err(|_| AppError::MalformedBalance)?;
    if raw.balance_infos.len() > 16 {
        return Err(AppError::MalformedBalance);
    }
    let mut currencies = HashSet::new();
    let mut balance_infos = Vec::with_capacity(raw.balance_infos.len());
    for entry in raw.balance_infos {
        if !matches!(entry.currency.as_str(), "CNY" | "USD")
            || !currencies.insert(entry.currency.clone())
        {
            return Err(AppError::MalformedBalance);
        }
        let total = parse_decimal(&entry.total_balance)?;
        let granted = parse_decimal(&entry.granted_balance)?;
        let topped = parse_decimal(&entry.topped_up_balance)?;
        if granted.checked_add(topped) != Some(total) {
            return Err(AppError::MalformedBalance);
        }
        balance_infos.push(BalanceInfo {
            currency: entry.currency,
            total_balance: entry.total_balance,
            granted_balance: entry.granted_balance,
            topped_up_balance: entry.topped_up_balance,
        });
    }
    Ok(ParsedBalance {
        is_available: raw.is_available,
        balance_infos,
    })
}

fn validate_bounded_json_nesting(body: &[u8], maximum: usize) -> AppResult<()> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in body {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1).ok_or(AppError::MalformedBalance)?;
                if depth > maximum {
                    return Err(AppError::MalformedBalance);
                }
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or(AppError::MalformedBalance)?;
            }
            _ => {}
        }
    }
    if depth != 0 || in_string {
        return Err(AppError::MalformedBalance);
    }
    Ok(())
}

fn parse_decimal(value: &str) -> AppResult<u128> {
    if value.is_empty() || value.len() > 37 {
        return Err(AppError::MalformedBalance);
    }
    let (whole, fraction) = value.split_once('.').map_or((value, ""), |parts| parts);
    if whole.is_empty()
        || whole.len() > 18
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || (whole.len() > 1 && whole.starts_with('0'))
        || fraction.len() > 18
        || (value.contains('.') && fraction.is_empty())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AppError::MalformedBalance);
    }
    let whole = whole
        .parse::<u128>()
        .map_err(|_| AppError::MalformedBalance)?;
    let fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u128>()
            .map_err(|_| AppError::MalformedBalance)?
            .checked_mul(10u128.pow(18 - fraction.len() as u32))
            .ok_or(AppError::MalformedBalance)?
    };
    whole
        .checked_mul(10u128.pow(18))
        .and_then(|scaled| scaled.checked_add(fraction_value))
        .ok_or(AppError::MalformedBalance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MessageRole;
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    fn messages() -> Vec<ProviderMessage> {
        vec![ProviderMessage {
            role: MessageRole::User,
            content: "Hello".to_owned(),
        }]
    }

    fn unmistakably_fake_key() -> String {
        ["test", "key", "not", "a", "secret"].join("_")
    }

    #[test]
    fn catalog_is_exactly_five_providers_and_seventeen_models() {
        let catalog = model_catalog();
        assert_eq!(catalog.version, 2);
        assert_eq!(catalog.default_selection.provider_id, ProviderId::Zai);
        assert_eq!(catalog.default_selection.model_id, "glm-5.1");
        assert_eq!(catalog.providers.len(), 5);
        assert_eq!(
            catalog
                .providers
                .iter()
                .map(|provider| provider.models.len())
                .sum::<usize>(),
            17
        );
        let pairs = catalog
            .providers
            .iter()
            .flat_map(|provider| {
                provider
                    .models
                    .iter()
                    .map(move |model| (provider.id.as_str(), model.id))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pairs,
            vec![
                ("zai", "glm-4.7"),
                ("zai", "glm-5"),
                ("zai", "glm-5.1"),
                ("zai", "glm-5.2"),
                ("deepseek", "deepseek-v4-flash"),
                ("deepseek", "deepseek-v4-pro"),
                ("alibaba-us", "qwen3.5-plus"),
                ("alibaba-us", "qwen3.5-flash"),
                ("alibaba-us", "qwen3.6-plus"),
                ("alibaba-us", "qwen3.6-flash"),
                ("alibaba-us", "qwen3.7-plus"),
                ("alibaba-us", "qwen3.7-max"),
                ("google", "gemini-2.5-flash"),
                ("google", "gemini-2.5-flash-lite"),
                ("google", "gemini-2.5-pro"),
                ("nvidia", "nvidia/nemotron-3-super-120b-a12b"),
                ("nvidia", "nvidia/nemotron-3-ultra-550b-a55b"),
            ]
        );
        assert!(validate_selection(ProviderId::Zai, "GLM-5.1").is_err());
        assert!(validate_selection(ProviderId::Google, "glm-5.1").is_err());
    }

    #[test]
    fn exact_request_golden_contracts() {
        let cases = [
            (
                ProviderId::Zai,
                "glm-5.1",
                ResponseProfile::Standard,
                json!({"model":"glm-5.1","messages":[{"role":"user","content":"Hello"}],"stream":true,"thinking":{"type":"enabled"},"max_tokens":8192}),
            ),
            (
                ProviderId::DeepSeek,
                "deepseek-v4-pro",
                ResponseProfile::Deep,
                json!({"model":"deepseek-v4-pro","messages":[{"role":"user","content":"Hello"}],"stream":true,"stream_options":{"include_usage":true},"thinking":{"type":"enabled"},"max_tokens":16384,"reasoning_effort":"max"}),
            ),
            (
                ProviderId::AlibabaUs,
                "qwen3.7-max",
                ResponseProfile::Fast,
                json!({"model":"qwen3.7-max","messages":[{"role":"user","content":"Hello"}],"stream":true,"stream_options":{"include_usage":true},"enable_thinking":false,"max_completion_tokens":4096}),
            ),
            (
                ProviderId::Google,
                "gemini-2.5-pro",
                ResponseProfile::Fast,
                json!({"contents":[{"role":"user","parts":[{"text":"Hello"}]}],"generationConfig":{"thinkingConfig":{"thinkingBudget":128},"maxOutputTokens":4096}}),
            ),
            (
                ProviderId::Nvidia,
                "nvidia/nemotron-3-ultra-550b-a55b",
                ResponseProfile::Standard,
                json!({"model":"nvidia/nemotron-3-ultra-550b-a55b","messages":[{"role":"user","content":"Hello"}],"stream":true,"temperature":1.0,"top_p":0.95,"max_tokens":8192,"chat_template_kwargs":{"enable_thinking":true},"reasoning_budget":4096}),
            ),
        ];
        for (provider, model, profile, expected) in cases {
            let request = build_request(provider, model, profile, &messages()).expect("request");
            let actual: Value = serde_json::from_slice(&request.body).expect("json");
            assert_eq!(actual, expected);
        }
        let deepseek_fast = build_request(
            ProviderId::DeepSeek,
            "deepseek-v4-flash",
            ResponseProfile::Fast,
            &messages(),
        )
        .expect("request");
        let body: Value = serde_json::from_slice(&deepseek_fast.body).expect("json");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn every_catalog_pair_supports_the_exact_profile_matrix() {
        for provider in model_catalog().providers {
            for model in provider.models {
                for profile in ResponseProfile::ALL {
                    let request = build_request(provider.id, model.id, profile, &messages())
                        .expect("catalog pair and profile must build");
                    let body: Value = serde_json::from_slice(&request.body).expect("request JSON");
                    let object = body.as_object().unwrap();
                    let cap = u64::from(profile.max_output_tokens());
                    match provider.id {
                        ProviderId::Zai => {
                            assert_eq!(request.url, ZAI_URL);
                            assert_eq!(request.authentication, Authentication::Bearer);
                            assert_eq!(object.len(), 5);
                            assert_eq!(body["model"], model.id);
                            assert_eq!(body["max_tokens"], cap);
                            assert_eq!(body["thinking"]["type"], profile.thinking_type());
                        }
                        ProviderId::DeepSeek => {
                            assert_eq!(request.url, DEEPSEEK_URL);
                            assert_eq!(request.authentication, Authentication::Bearer);
                            assert_eq!(
                                object.len(),
                                usize::from(profile != ResponseProfile::Fast) + 6
                            );
                            assert_eq!(body["model"], model.id);
                            assert_eq!(body["stream_options"], json!({"include_usage": true}));
                            assert_eq!(body["max_tokens"], cap);
                            assert_eq!(body["thinking"]["type"], profile.thinking_type());
                            let effort = match profile {
                                ResponseProfile::Fast => None,
                                ResponseProfile::Standard => Some("high"),
                                ResponseProfile::Deep => Some("max"),
                            };
                            assert_eq!(
                                body.get("reasoning_effort").and_then(Value::as_str),
                                effort
                            );
                        }
                        ProviderId::AlibabaUs => {
                            assert_eq!(request.url, ALIBABA_US_URL);
                            assert_eq!(request.authentication, Authentication::Bearer);
                            assert_eq!(object.len(), 6);
                            assert_eq!(body["model"], model.id);
                            assert_eq!(body["stream_options"], json!({"include_usage": true}));
                            assert_eq!(body["enable_thinking"], profile != ResponseProfile::Fast);
                            assert_eq!(body["max_completion_tokens"], cap);
                        }
                        ProviderId::Google => {
                            assert_eq!(
                                request.url,
                                format!("{GOOGLE_PREFIX}{}{GOOGLE_SUFFIX}", model.id)
                            );
                            assert_eq!(request.authentication, Authentication::GoogleApiKey);
                            assert_eq!(object.len(), 2);
                            assert_eq!(body["generationConfig"]["maxOutputTokens"], cap);
                            let budget = match (model.id, profile) {
                                ("gemini-2.5-pro", ResponseProfile::Fast) => 128,
                                ("gemini-2.5-pro", ResponseProfile::Standard) => 4_096,
                                ("gemini-2.5-pro", ResponseProfile::Deep) => 16_384,
                                (_, ResponseProfile::Fast) => 0,
                                (_, ResponseProfile::Standard) => 1_024,
                                (_, ResponseProfile::Deep) => 8_192,
                            };
                            assert_eq!(
                                body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
                                budget
                            );
                        }
                        ProviderId::Nvidia => {
                            assert_eq!(request.url, NVIDIA_URL);
                            assert_eq!(request.authentication, Authentication::Bearer);
                            assert_eq!(
                                object.len(),
                                usize::from(profile != ResponseProfile::Fast) + 7
                            );
                            assert_eq!(body["model"], model.id);
                            assert_eq!(body["max_tokens"], cap);
                            assert_eq!(body["temperature"], 1.0);
                            assert_eq!(body["top_p"], 0.95);
                            assert_eq!(
                                body["chat_template_kwargs"]["enable_thinking"],
                                profile != ResponseProfile::Fast
                            );
                            let budget = match profile {
                                ResponseProfile::Fast => None,
                                ResponseProfile::Standard => Some(4_096),
                                ResponseProfile::Deep => Some(16_384),
                            };
                            assert_eq!(
                                body.get("reasoning_budget").and_then(Value::as_u64),
                                budget
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn openai_and_google_streams_have_distinct_terminal_semantics() {
        let mut zai = StreamParser::new(ProviderId::Zai);
        let fixture = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Public\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"hidden\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":8,\"prompt_tokens_details\":{\"cached_tokens\":2},\"completion_tokens\":32,\"total_tokens\":40}}\n\n",
            "data: [DONE]\n\n"
        );
        let outputs = zai.push(fixture.as_bytes()).expect("valid Z.AI stream");
        assert_eq!(outputs.len(), 1);
        let (_, outcome) = zai.finish().expect("Z.AI needs DONE");
        assert_eq!(outcome.finish_reason, MessageFinishReason::Stop);
        assert_eq!(
            outcome.usage,
            Some(TokenUsage::new(Some(6), Some(2), Some(32), Some(40)).unwrap())
        );

        let mut google = StreamParser::new(ProviderId::Google);
        let fixture = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"private\",\"thought\":true},{\"text\":\"Visible\",\"thoughtSignature\":\"discard\"}]},\"finishReason\":\"STOP\"}],",
            "\"usageMetadata\":{\"promptTokenCount\":10,\"cachedContentTokenCount\":2,\"candidatesTokenCount\":5,\"thoughtsTokenCount\":3,\"totalTokenCount\":18}}\n\n"
        );
        let outputs = google
            .push(fixture.as_bytes())
            .expect("valid Gemini stream");
        assert_eq!(outputs.len(), 1);
        let (_, outcome) = google.finish().expect("Gemini ends at EOF");
        assert_eq!(outcome.finish_reason, MessageFinishReason::Stop);
        assert_eq!(
            outcome.usage,
            Some(TokenUsage::new(Some(8), Some(2), Some(8), Some(18)).unwrap())
        );
    }

    #[test]
    fn every_adapter_maps_output_limit_into_the_typed_outcome() {
        for provider in [
            ProviderId::Zai,
            ProviderId::DeepSeek,
            ProviderId::AlibabaUs,
            ProviderId::Nvidia,
        ] {
            let mut parser = StreamParser::new(provider);
            parser
                .push(
                    concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"Partial\"},",
                        "\"finish_reason\":\"length\"}]}\n\n",
                        "data: [DONE]\n\n"
                    )
                    .as_bytes(),
                )
                .unwrap();
            let (_, outcome) = parser.finish().unwrap();
            assert_eq!(
                outcome.finish_reason,
                MessageFinishReason::OutputLimit,
                "{}",
                provider.as_str()
            );
        }

        let mut google = StreamParser::new(ProviderId::Google);
        google
            .push(
                br#"data: {"candidates":[{"content":{"parts":[{"text":"Partial"}]},"finishReason":"MAX_TOKENS"}]}

"#,
            )
            .unwrap();
        let (_, outcome) = google.finish().unwrap();
        assert_eq!(outcome.finish_reason, MessageFinishReason::OutputLimit);
    }

    #[test]
    fn out_of_order_usage_fails_at_each_providers_nonterminal_position() {
        for provider in [ProviderId::Zai, ProviderId::Nvidia] {
            let mut parser = StreamParser::new(provider);
            let error = parser
                .push(
                    concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"Answer\"}}],",
                        "\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n",
                        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                        "data: [DONE]\n\n"
                    )
                    .as_bytes(),
                )
                .unwrap_err();
            assert_eq!(error.public().code, "malformed_stream");
        }

        for provider in [ProviderId::DeepSeek, ProviderId::AlibabaUs] {
            let mut parser = StreamParser::new(provider);
            let error = parser
                .push(
                    concat!(
                        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n",
                        "data: {\"choices\":[{\"delta\":{\"content\":\"Answer\"},\"finish_reason\":\"stop\"}]}\n\n",
                        "data: [DONE]\n\n"
                    )
                    .as_bytes(),
                )
                .unwrap_err();
            assert_eq!(error.public().code, "malformed_stream");
        }

        let mut google = StreamParser::new(ProviderId::Google);
        let error = google
            .push(
                concat!(
                    "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Answer\"}]}}],",
                    "\"usageMetadata\":{\"promptTokenCount\":5,\"cachedContentTokenCount\":1,\"candidatesTokenCount\":2,\"thoughtsTokenCount\":0,\"totalTokenCount\":7}}\n\n",
                    "data: {\"candidates\":[{\"finishReason\":\"STOP\"}]}\n\n"
                )
                .as_bytes(),
            )
            .unwrap_err();
        assert_eq!(error.public().code, "malformed_stream");
    }

    #[test]
    fn choices_empty_usage_after_terminal_is_authoritative_for_enabled_adapters() {
        let cases = [
            (
                ProviderId::DeepSeek,
                "{\"prompt_tokens\":7,\"prompt_cache_miss_tokens\":5,\"prompt_cache_hit_tokens\":2,\"completion_tokens\":3,\"total_tokens\":10}",
            ),
            (
                ProviderId::AlibabaUs,
                "{\"prompt_tokens\":7,\"prompt_tokens_details\":{\"cached_tokens\":2},\"completion_tokens\":3,\"total_tokens\":10}",
            ),
        ];
        for (provider, usage) in cases {
            let fixture = format!(
                concat!(
                    "data: {{\"choices\":[{{\"delta\":{{\"content\":\"Answer\"}},\"finish_reason\":\"stop\"}}]}}\n\n",
                    "data: {{\"choices\":[],\"usage\":{usage}}}\n\n",
                    "data: [DONE]\n\n"
                ),
                usage = usage
            );
            let mut parser = StreamParser::new(provider);
            parser.push(fixture.as_bytes()).unwrap();
            let (_, outcome) = parser.finish().unwrap();
            assert_eq!(
                outcome.usage,
                Some(TokenUsage::new(Some(5), Some(2), Some(3), Some(10)).unwrap()),
                "{}",
                provider.as_str()
            );
        }
    }

    #[test]
    fn unknown_delta_and_part_capabilities_fail_closed() {
        for provider in [
            ProviderId::Zai,
            ProviderId::DeepSeek,
            ProviderId::AlibabaUs,
            ProviderId::Nvidia,
        ] {
            for field in ["audio", "file", "refusal", "unknownCapability"] {
                let fixture =
                    format!("data: {{\"choices\":[{{\"delta\":{{\"{field}\":{{}}}}}}]}}\n\n");
                let mut parser = StreamParser::new(provider);
                assert_eq!(
                    parser.push(fixture.as_bytes()).unwrap_err().public().code,
                    "unsupported_provider_capability",
                    "{} {field}",
                    provider.as_str()
                );
            }
        }

        for field in ["audioData", "fileData", "functionCall", "unknownCapability"] {
            let fixture = format!(
                "data: {{\"candidates\":[{{\"content\":{{\"parts\":[{{\"{field}\":{{}}}}]}}}}]}}\n\n"
            );
            let mut parser = StreamParser::new(ProviderId::Google);
            assert_eq!(
                parser.push(fixture.as_bytes()).unwrap_err().public().code,
                "unsupported_provider_capability",
                "{field}"
            );
        }
    }

    #[test]
    fn openai_stream_discriminators_reject_present_null_wrong_types_and_values() {
        for provider in [
            ProviderId::Zai,
            ProviderId::DeepSeek,
            ProviderId::AlibabaUs,
            ProviderId::Nvidia,
        ] {
            let mut valid = StreamParser::new(provider);
            valid
                .push(
                    concat!(
                        "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Answer\"},",
                        "\"finish_reason\":\"stop\"}]}\n\n",
                        "data: [DONE]\n\n"
                    )
                    .as_bytes(),
                )
                .unwrap();
            valid.finish().unwrap();

            for index in ["null", "\"0\"", "1", "0.0"] {
                let fixture = format!(
                    concat!(
                        "data: {{\"choices\":[{{\"index\":{index},\"delta\":{{\"content\":\"Answer\"}},",
                        "\"finish_reason\":\"stop\"}}]}}\n\n",
                        "data: [DONE]\n\n"
                    ),
                    index = index
                );
                let mut parser = StreamParser::new(provider);
                assert_eq!(
                    parser.push(fixture.as_bytes()).unwrap_err().public().code,
                    "malformed_stream",
                    "{} index={index}",
                    provider.as_str()
                );
            }

            for role in ["null", "42", "\"user\""] {
                let fixture = format!(
                    concat!(
                        "data: {{\"choices\":[{{\"delta\":{{\"role\":{role},\"content\":\"Answer\"}},",
                        "\"finish_reason\":\"stop\"}}]}}\n\n",
                        "data: [DONE]\n\n"
                    ),
                    role = role
                );
                let mut parser = StreamParser::new(provider);
                assert_eq!(
                    parser.push(fixture.as_bytes()).unwrap_err().public().code,
                    "malformed_stream",
                    "{} role={role}",
                    provider.as_str()
                );
            }
        }
    }

    #[test]
    fn finish_reason_allowlists_are_provider_specific_and_fail_closed() {
        for provider in [
            ProviderId::Zai,
            ProviderId::DeepSeek,
            ProviderId::AlibabaUs,
            ProviderId::Nvidia,
        ] {
            let mut stopped = StreamParser::new(provider);
            stopped.map_openai_finish("stop").unwrap();
            assert!(stopped.saw_terminal);
            assert_eq!(stopped.finish_reason, Some(MessageFinishReason::Stop));

            let mut limited = StreamParser::new(provider);
            limited.map_openai_finish("length").unwrap();
            assert!(limited.saw_terminal);
            assert_eq!(
                limited.finish_reason,
                Some(MessageFinishReason::OutputLimit)
            );

            let mut unknown = StreamParser::new(provider);
            assert_eq!(
                unknown
                    .map_openai_finish("unknown")
                    .unwrap_err()
                    .public()
                    .code,
                "malformed_stream"
            );
        }

        let openai_errors = [
            (ProviderId::Zai, "sensitive", "provider_content_rejected"),
            (
                ProviderId::Zai,
                "model_context_window_exceeded",
                "provider_context_limit",
            ),
            (ProviderId::Zai, "network_error", "provider_unavailable"),
            (
                ProviderId::Zai,
                "tool_calls",
                "unsupported_provider_capability",
            ),
            (
                ProviderId::DeepSeek,
                "content_filter",
                "provider_content_rejected",
            ),
            (
                ProviderId::DeepSeek,
                "insufficient_system_resource",
                "provider_unavailable",
            ),
            (
                ProviderId::DeepSeek,
                "tool_calls",
                "unsupported_provider_capability",
            ),
            (
                ProviderId::AlibabaUs,
                "tool_calls",
                "unsupported_provider_capability",
            ),
            (
                ProviderId::Nvidia,
                "content_filter",
                "provider_content_rejected",
            ),
            (
                ProviderId::Nvidia,
                "tool_calls",
                "unsupported_provider_capability",
            ),
            (
                ProviderId::Nvidia,
                "function_call",
                "unsupported_provider_capability",
            ),
        ];
        for (provider, reason, code) in openai_errors {
            let mut parser = StreamParser::new(provider);
            assert_eq!(
                parser.map_openai_finish(reason).unwrap_err().public().code,
                code
            );
        }

        for (reason, terminal, finish_reason) in [
            ("STOP", true, MessageFinishReason::Stop),
            ("MAX_TOKENS", true, MessageFinishReason::OutputLimit),
        ] {
            let mut parser = StreamParser::new(ProviderId::Google);
            parser.map_google_finish(reason).unwrap();
            assert_eq!(parser.saw_terminal, terminal);
            assert_eq!(parser.finish_reason, Some(finish_reason));
        }
        let google_errors = [
            ("SAFETY", "provider_content_rejected"),
            ("RECITATION", "provider_content_rejected"),
            ("LANGUAGE", "provider_content_rejected"),
            ("BLOCKLIST", "provider_content_rejected"),
            ("PROHIBITED_CONTENT", "provider_content_rejected"),
            ("SPII", "provider_content_rejected"),
            ("IMAGE_SAFETY", "provider_content_rejected"),
            ("IMAGE_PROHIBITED_CONTENT", "provider_content_rejected"),
            ("IMAGE_OTHER", "provider_content_rejected"),
            ("NO_IMAGE", "provider_content_rejected"),
            ("IMAGE_RECITATION", "provider_content_rejected"),
            ("MALFORMED_FUNCTION_CALL", "unsupported_provider_capability"),
            ("UNEXPECTED_TOOL_CALL", "unsupported_provider_capability"),
            ("TOO_MANY_TOOL_CALLS", "unsupported_provider_capability"),
            (
                "MISSING_THOUGHT_SIGNATURE",
                "unsupported_provider_capability",
            ),
            ("OTHER", "malformed_stream"),
            ("MALFORMED_RESPONSE", "malformed_stream"),
            ("FINISH_REASON_UNSPECIFIED", "malformed_stream"),
            ("unknown", "malformed_stream"),
        ];
        for (reason, code) in google_errors {
            let mut parser = StreamParser::new(ProviderId::Google);
            assert_eq!(
                parser.map_google_finish(reason).unwrap_err().public().code,
                code
            );
        }
    }

    #[test]
    fn every_openai_compatible_provider_requires_done_after_terminal_choice() {
        for provider in [
            ProviderId::Zai,
            ProviderId::DeepSeek,
            ProviderId::AlibabaUs,
            ProviderId::Nvidia,
        ] {
            let terminal = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Visible\"},",
                "\"finish_reason\":\"stop\"}]}\n\n"
            );
            let mut without_done = StreamParser::new(provider);
            without_done.push(terminal.as_bytes()).unwrap();
            assert!(without_done.finish().is_err());

            let mut complete = StreamParser::new(provider);
            complete
                .push(format!("{terminal}data: [DONE]\n\n").as_bytes())
                .unwrap();
            complete.finish().unwrap();
        }
    }

    #[test]
    fn malformed_google_discriminators_fail_on_the_first_event() {
        for fixture in [
            r#"data: {"candidates":"not-an-array"}

"#,
            r#"data: {"promptFeedback":{"blockReason":42}}

"#,
            r#"data: {"candidates":[{"content":{"parts":[{"text":"hidden","thought":"true"}]}}]}

"#,
            r#"data: {"candidates":[{"finishReason":42}]}

"#,
        ] {
            let mut parser = StreamParser::new(ProviderId::Google);
            assert!(parser.push(fixture.as_bytes()).is_err());
        }
    }

    #[test]
    fn malformed_usage_is_partial_without_corrupting_valid_answer() {
        let mut parser = StreamParser::new(ProviderId::AlibabaUs);
        parser
            .push(concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Answer\"},\"finish_reason\":\"stop\"}]}\n\n",
                "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":4,\"total_tokens\":999}}\n\n",
                "data: [DONE]\n\n"
            ).as_bytes())
            .expect("usage inconsistency is non-fatal");
        let (_, outcome) = parser.finish().expect("answer remains valid");
        assert_eq!(outcome.usage, None);
    }

    #[test]
    fn invalid_dependent_usage_arithmetic_discards_the_whole_observation() {
        for provider in [ProviderId::Zai, ProviderId::AlibabaUs, ProviderId::Nvidia] {
            assert_eq!(
                normalize_usage(
                    provider,
                    &json!({
                        "prompt_tokens": 5,
                        "prompt_tokens_details": {"cached_tokens": 6},
                        "completion_tokens": 2,
                        "total_tokens": 7
                    })
                ),
                None,
                "{} cached input exceeds prompt",
                provider.as_str()
            );
            assert_eq!(
                normalize_usage(
                    provider,
                    &json!({
                        "prompt_tokens": MAX_SAFE_INTEGER + 1,
                        "completion_tokens": 2
                    })
                ),
                None,
                "{} unsafe partial field",
                provider.as_str()
            );
        }

        assert_eq!(
            normalize_usage(
                ProviderId::DeepSeek,
                &json!({
                    "prompt_tokens": 6,
                    "prompt_cache_miss_tokens": 2,
                    "prompt_cache_hit_tokens": 3,
                    "completion_tokens": 1
                })
            ),
            None
        );
        assert_eq!(
            normalize_usage(
                ProviderId::DeepSeek,
                &json!({
                    "prompt_cache_miss_tokens": 2,
                    "prompt_cache_hit_tokens": 3,
                    "completion_tokens": 1,
                    "total_tokens": 99
                })
            ),
            None
        );
        assert_eq!(
            normalize_usage(
                ProviderId::DeepSeek,
                &json!({
                    "prompt_cache_miss_tokens": MAX_SAFE_INTEGER,
                    "prompt_cache_hit_tokens": 1
                })
            ),
            None
        );
        assert_eq!(
            normalize_usage(
                ProviderId::Google,
                &json!({
                    "promptTokenCount": 5,
                    "cachedContentTokenCount": 6,
                    "candidatesTokenCount": 1,
                    "thoughtsTokenCount": 1,
                    "totalTokenCount": 7
                })
            ),
            None
        );
        assert_eq!(
            normalize_usage(
                ProviderId::Google,
                &json!({
                    "candidatesTokenCount": MAX_SAFE_INTEGER,
                    "thoughtsTokenCount": 1
                })
            ),
            None
        );
        assert_eq!(
            normalize_usage(
                ProviderId::Google,
                &json!({"promptTokenCount": 1.5, "totalTokenCount": 1})
            ),
            None
        );
    }

    #[test]
    fn deepseek_balance_has_exact_decimal_and_currency_policy() {
        let valid = br#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"10.25","granted_balance":"1.05","topped_up_balance":"9.20"},{"currency":"CNY","total_balance":"0","granted_balance":"0.0","topped_up_balance":"0.00"}]}"#;
        let parsed = parse_deepseek_balance(valid).expect("valid exact balance");
        assert!(parsed.is_available);
        assert_eq!(parsed.balance_infos.len(), 2);
        for invalid in [
            br#"{"is_available":true,"balance_infos":[{"currency":"EUR","total_balance":"1","granted_balance":"1","topped_up_balance":"0"}]}"#.as_slice(),
            br#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"01","granted_balance":"1","topped_up_balance":"0"}]}"#.as_slice(),
            br#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"2","granted_balance":"1","topped_up_balance":"0"}]}"#.as_slice(),
            br#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"1","granted_balance":"1","topped_up_balance":"0"},{"currency":"USD","total_balance":"1","granted_balance":"1","topped_up_balance":"0"}]}"#.as_slice(),
        ] {
            assert!(parse_deepseek_balance(invalid).is_err());
        }

        let unknown_fields = br#"{"is_available":true,"ignored":{"safe":true},"balance_infos":[{"currency":"USD","total_balance":"1.000000000000000001","granted_balance":"0.000000000000000001","topped_up_balance":"1.000000000000000000","ignored":"safe"}]}"#;
        assert!(parse_deepseek_balance(unknown_fields).is_ok());
        let duplicate_required = br#"{"is_available":true,"is_available":true,"balance_infos":[]}"#;
        assert!(parse_deepseek_balance(duplicate_required).is_err());
        let too_many = json!({
            "is_available": true,
            "balance_infos": (0..17).map(|_| json!({
                "currency": "USD",
                "total_balance": "0",
                "granted_balance": "0",
                "topped_up_balance": "0"
            })).collect::<Vec<_>>()
        });
        assert!(parse_deepseek_balance(&serde_json::to_vec(&too_many).unwrap()).is_err());
    }

    #[test]
    fn account_map_is_the_exact_closed_eleven_action_set() {
        let expected = [
            (
                ProviderId::Zai,
                ProviderAccountAction::Billing,
                "https://z.ai/manage-apikey/billing",
            ),
            (
                ProviderId::Zai,
                ProviderAccountAction::AddCredits,
                "https://z.ai/manage-apikey/billing",
            ),
            (
                ProviderId::DeepSeek,
                ProviderAccountAction::Usage,
                "https://platform.deepseek.com/usage",
            ),
            (
                ProviderId::DeepSeek,
                ProviderAccountAction::AddCredits,
                "https://platform.deepseek.com/top_up",
            ),
            (
                ProviderId::AlibabaUs,
                ProviderAccountAction::Usage,
                "https://modelstudio.console.alibabacloud.com/?tab=costing-balance",
            ),
            (
                ProviderId::AlibabaUs,
                ProviderAccountAction::Billing,
                "https://usercenter2-intl.console.alibabacloud.com/finance/expense-report/expense-detail-by-instance",
            ),
            (
                ProviderId::AlibabaUs,
                ProviderAccountAction::AddCredits,
                "https://billing-cost-intl.aliyun.com/fortune/billing-account",
            ),
            (
                ProviderId::Google,
                ProviderAccountAction::Usage,
                "https://aistudio.google.com/usage",
            ),
            (
                ProviderId::Google,
                ProviderAccountAction::Billing,
                "https://aistudio.google.com/billing",
            ),
            (
                ProviderId::Google,
                ProviderAccountAction::Spend,
                "https://aistudio.google.com/spend",
            ),
            (
                ProviderId::Nvidia,
                ProviderAccountAction::Deployment,
                "https://build.nvidia.com/",
            ),
        ];
        let observed = ProviderId::ALL
            .into_iter()
            .flat_map(|provider| {
                ProviderAccountAction::ALL
                    .into_iter()
                    .filter_map(move |action| {
                        account_url(provider, action)
                            .ok()
                            .map(|url| (provider, action, url))
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
        assert!(
            observed
                .iter()
                .all(|(_, _, url)| url.starts_with("https://"))
        );
    }

    #[tokio::test]
    async fn cancellation_at_the_provider_stage_uses_only_a_controlled_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("controlled listener");
        let endpoint = format!("http://{}/chat", listener.local_addr().unwrap());
        let provider = ProviderClient::for_controlled_server(endpoint).expect("provider client");
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let history = messages();
        let test_api_key = unmistakably_fake_key();
        let result = provider
            .stream_chat(
                ProviderChatRequest {
                    provider: ProviderId::Zai,
                    model: "glm-5.1",
                    profile: ResponseProfile::Standard,
                    messages: &history,
                    api_key: &test_api_key,
                    cancellation: &cancellation,
                },
                |_| Ok(()),
            )
            .await;
        assert!(matches!(result, Err(AppError::Cancelled)));
    }

    #[tokio::test]
    async fn transient_provider_failure_is_not_retried() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("controlled listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let endpoint = format!("http://{}/chat", listener.local_addr().unwrap());
        let attempts = Arc::new(AtomicUsize::new(0));
        let server_attempts = Arc::clone(&attempts);
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(750);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        server_attempts.fetch_add(1, Ordering::SeqCst);
                        socket.set_nonblocking(false).unwrap();
                        socket
                            .set_read_timeout(Some(Duration::from_millis(500)))
                            .unwrap();
                        let mut request = Vec::new();
                        loop {
                            let mut chunk = [0_u8; 4096];
                            let read = socket.read(&mut chunk).unwrap();
                            if read == 0 {
                                break;
                            }
                            request.extend_from_slice(&chunk[..read]);
                            let Some(header_end) =
                                request.windows(4).position(|window| window == b"\r\n\r\n")
                            else {
                                continue;
                            };
                            let headers = String::from_utf8_lossy(&request[..header_end]);
                            let content_length = headers
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(str::trim)
                                        .and_then(|value| value.parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if request.len() >= header_end + 4 + content_length {
                                break;
                            }
                        }
                        socket
                            .write_all(
                                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .unwrap();
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("controlled server failed: {error}"),
                }
            }
        });

        let provider = ProviderClient::for_controlled_server(endpoint).expect("provider client");
        let history = messages();
        let cancellation = CancellationToken::new();
        let test_api_key = unmistakably_fake_key();
        let result = provider
            .stream_chat(
                ProviderChatRequest {
                    provider: ProviderId::Zai,
                    model: "glm-5.1",
                    profile: ResponseProfile::Standard,
                    messages: &history,
                    api_key: &test_api_key,
                    cancellation: &cancellation,
                },
                |_| Ok(()),
            )
            .await;
        server.join().expect("controlled server");
        assert!(matches!(result, Err(AppError::ProviderUnavailable)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
