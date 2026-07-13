use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Client, StatusCode, redirect};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

use crate::database::MODEL;
use crate::error::{AppError, AppResult};
use crate::models::{ProviderMessage, ReasoningMode};

const PROVIDER_URL: &str = "https://api.z.ai/api/paas/v4/chat/completions";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const OVERALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: usize = 2;
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_HEADERS: usize = 64;
const MAX_RESPONSE_HEADER_BYTES: usize = 32 * 1024;
const MAX_STREAM_BYTES: usize = 8 * 1024 * 1024;
const MAX_SSE_LINE_BYTES: usize = 64 * 1024;
const MAX_SSE_EVENT_BYTES: usize = 128 * 1024;
const MAX_DELTA_BYTES: usize = 64 * 1024;
const MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOKEN_USAGE: u64 = i64::MAX as u64;

#[derive(Clone)]
pub struct ProviderClient {
    client: Client,
    retry_counter: Arc<AtomicU64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOutcome {
    pub token_usage: Option<u64>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'static str,
    messages: &'a [ProviderMessage],
    stream: bool,
    thinking: Thinking,
    max_tokens: u32,
}

#[derive(Serialize)]
struct Thinking {
    #[serde(rename = "type")]
    kind: &'static str,
}

impl ProviderClient {
    pub fn new() -> AppResult<Self> {
        let client = Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(IDLE_TIMEOUT)
            .timeout(OVERALL_TIMEOUT)
            .user_agent(concat!("Aster/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| AppError::Internal)?;
        Ok(Self {
            client,
            retry_counter: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn stream_chat<F>(
        &self,
        messages: &[ProviderMessage],
        reasoning_mode: ReasoningMode,
        api_key: &str,
        cancellation: &CancellationToken,
        on_delta: F,
    ) -> AppResult<ProviderOutcome>
    where
        F: FnMut(String) -> AppResult<()>,
    {
        let request = ChatRequest {
            model: MODEL,
            messages,
            stream: true,
            thinking: Thinking {
                kind: reasoning_mode.thinking_type(),
            },
            max_tokens: reasoning_mode.max_tokens(),
        };
        let request_body = serde_json::to_vec(&request).map_err(|_| AppError::Serialization)?;
        if request_body.len() > MAX_REQUEST_BYTES {
            return Err(AppError::Validation(
                "The provider request context is too large.",
            ));
        }

        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AppError::Cancelled),
            result = timeout(
                OVERALL_TIMEOUT,
                self.stream_with_retries(request_body, api_key, cancellation, on_delta),
            ) => result.unwrap_or(Err(AppError::Timeout)),
        }
    }

    async fn stream_with_retries<F>(
        &self,
        request_body: Vec<u8>,
        api_key: &str,
        cancellation: &CancellationToken,
        mut on_delta: F,
    ) -> AppResult<ProviderOutcome>
    where
        F: FnMut(String) -> AppResult<()>,
    {
        let retry_sequence = self.retry_counter.fetch_add(1, Ordering::Relaxed);
        for attempt in 0..MAX_ATTEMPTS {
            match self
                .stream_attempt(request_body.clone(), api_key, cancellation, &mut on_delta)
                .await
            {
                Ok(outcome) => return Ok(outcome),
                Err(failure)
                    if attempt + 1 < MAX_ATTEMPTS && failure.transient && !failure.surfaced =>
                {
                    let delay = retry_delay(failure.retry_after, retry_sequence, attempt);
                    tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => return Err(AppError::Cancelled),
                        () = sleep(delay) => {}
                    }
                }
                Err(failure) => return Err(failure.error),
            }
        }
        Err(AppError::Internal)
    }

    async fn stream_attempt<F>(
        &self,
        request_body: Vec<u8>,
        api_key: &str,
        cancellation: &CancellationToken,
        on_delta: &mut F,
    ) -> Result<ProviderOutcome, AttemptFailure>
    where
        F: FnMut(String) -> AppResult<()>,
    {
        let send = self
            .client
            .post(PROVIDER_URL)
            .header(ACCEPT, "text/event-stream")
            .header(CONTENT_TYPE, "application/json")
            .bearer_auth(api_key)
            .body(request_body)
            .send();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AttemptFailure::cancelled()),
            response = send => response.map_err(AttemptFailure::from_transport)?,
        };

        validate_response_headers(&response).map_err(AttemptFailure::terminal)?;
        let status = response.status();
        if !status.is_success() {
            let retry_guidance = parse_retry_after(response.headers().get(RETRY_AFTER));
            return Err(AttemptFailure::from_status(status, retry_guidance));
        }
        validate_event_stream_content_type(&response).map_err(AttemptFailure::terminal)?;

        let mut parser = SseParser::default();
        let mut stream = response.bytes_stream();
        let mut surfaced = false;
        while let Some(chunk) = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AttemptFailure::cancelled_with(surfaced)),
            chunk = timeout(IDLE_TIMEOUT, stream.next()) => {
                chunk.map_err(|_| AttemptFailure::timeout(surfaced))?
            }
        } {
            let chunk = chunk.map_err(|error| AttemptFailure::transport(error, surfaced))?;
            for transport_line in chunk.split_inclusive(|byte| *byte == b'\n') {
                let outputs = parser
                    .push(transport_line)
                    .map_err(|error| AttemptFailure::from_stream(error, surfaced))?;
                for output in outputs {
                    let SseOutput::Delta(delta) = output;
                    surfaced = true;
                    on_delta(delta)
                        .map_err(|error| AttemptFailure::error(error, surfaced, false, None))?;
                }
            }
            if parser.is_done() {
                return parser
                    .finish()
                    .map_err(|error| AttemptFailure::from_stream(error, surfaced));
            }
        }
        let outcome = parser
            .finish()
            .map_err(|error| AttemptFailure::from_stream(error, surfaced))?;
        Ok(outcome)
    }
}

fn validate_response_headers(response: &reqwest::Response) -> AppResult<()> {
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
        if length > MAX_STREAM_BYTES as u64 {
            return Err(AppError::MalformedStream);
        }
    }
    Ok(())
}

fn validate_event_stream_content_type(response: &reqwest::Response) -> AppResult<()> {
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
    if !media_type.eq_ignore_ascii_case("text/event-stream") {
        return Err(AppError::MalformedStream);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RetryGuidance {
    delay: Option<Duration>,
    permitted: bool,
}

fn parse_retry_after(value: Option<&reqwest::header::HeaderValue>) -> RetryGuidance {
    let Some(value) = value.and_then(|value| value.to_str().ok()).map(str::trim) else {
        return RetryGuidance {
            delay: None,
            permitted: true,
        };
    };
    if let Ok(seconds) = value.parse::<u64>() {
        let delay = Duration::from_secs(seconds);
        return RetryGuidance {
            delay: (delay <= MAX_RETRY_DELAY).then_some(delay),
            permitted: delay <= MAX_RETRY_DELAY,
        };
    }
    let Some(retry_at) = DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
    else {
        return RetryGuidance {
            delay: None,
            permitted: true,
        };
    };
    let wait = retry_at
        .signed_duration_since(Utc::now())
        .to_std()
        .unwrap_or(Duration::ZERO);
    RetryGuidance {
        delay: (wait <= MAX_RETRY_DELAY).then_some(wait),
        permitted: wait <= MAX_RETRY_DELAY,
    }
}

fn retry_delay(retry_after: Option<Duration>, sequence: u64, attempt: usize) -> Duration {
    let base = retry_after.unwrap_or_else(|| Duration::from_millis(250 << attempt));
    let mixed = sequence
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add((attempt as u64).wrapping_mul(1_442_695_040_888_963_407));
    let jitter = Duration::from_millis(25 + mixed % 100);
    base.saturating_add(jitter).min(MAX_RETRY_DELAY)
}

struct AttemptFailure {
    error: AppError,
    surfaced: bool,
    transient: bool,
    retry_after: Option<Duration>,
}

impl AttemptFailure {
    fn error(
        error: AppError,
        surfaced: bool,
        transient: bool,
        retry_after: Option<Duration>,
    ) -> Self {
        Self {
            error,
            surfaced,
            transient,
            retry_after,
        }
    }

    fn terminal(error: AppError) -> Self {
        Self::error(error, false, false, None)
    }

    fn cancelled() -> Self {
        Self::cancelled_with(false)
    }

    fn cancelled_with(surfaced: bool) -> Self {
        Self::error(AppError::Cancelled, surfaced, false, None)
    }

    fn timeout(surfaced: bool) -> Self {
        Self::error(AppError::Timeout, surfaced, true, None)
    }

    fn from_transport(error: reqwest::Error) -> Self {
        Self::transport(error, false)
    }

    fn transport(error: reqwest::Error, surfaced: bool) -> Self {
        let app_error = if error.is_timeout() {
            AppError::Timeout
        } else {
            AppError::Network
        };
        Self::error(app_error, surfaced, true, None)
    }

    fn from_status(status: StatusCode, guidance: RetryGuidance) -> Self {
        match status.as_u16() {
            401 | 403 => Self::terminal(AppError::ProviderAuthentication),
            400 | 422 => Self::terminal(AppError::ProviderContract),
            408 | 425 => Self::error(
                AppError::ProviderUnavailable,
                false,
                guidance.permitted,
                guidance.delay,
            ),
            429 => Self::error(
                AppError::ProviderRateLimited,
                false,
                guidance.permitted,
                guidance.delay,
            ),
            500..=599 => Self::error(
                AppError::ProviderUnavailable,
                false,
                guidance.permitted,
                guidance.delay,
            ),
            _ => Self::terminal(AppError::ProviderRejected),
        }
    }

    fn from_stream(error: AppError, surfaced: bool) -> Self {
        let transient = matches!(&error, AppError::ProviderUnavailable);
        Self::error(error, surfaced, transient, None)
    }
}

#[derive(Debug, Deserialize)]
struct ProviderChunk {
    #[serde(default)]
    choices: Option<Vec<ChunkChoice>>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkChoice {
    #[serde(default)]
    index: Option<u32>,
    #[serde(default)]
    delta: Option<ChunkDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    tool_calls: bool,
    #[serde(default, deserialize_with = "deserialize_present")]
    function_call: bool,
}

fn deserialize_present<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(true)
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: u64,
}

#[derive(Debug)]
enum SseOutput {
    Delta(String),
}

#[derive(Default)]
struct SseParser {
    line_buffer: Vec<u8>,
    event_data: Vec<u8>,
    transport_bytes: usize,
    content_bytes: usize,
    saw_finish: bool,
    saw_done: bool,
    token_usage: Option<u64>,
}

impl SseParser {
    fn is_done(&self) -> bool {
        self.saw_done
    }

    fn push(&mut self, chunk: &[u8]) -> AppResult<Vec<SseOutput>> {
        self.transport_bytes = self
            .transport_bytes
            .checked_add(chunk.len())
            .ok_or(AppError::MalformedStream)?;
        if self.transport_bytes > MAX_STREAM_BYTES {
            return self.malformed();
        }
        self.line_buffer.extend_from_slice(chunk);

        let mut outputs = Vec::new();
        let mut consumed = 0usize;
        while let Some(relative_end) = self.line_buffer[consumed..]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            let end = consumed + relative_end;
            if end - consumed > MAX_SSE_LINE_BYTES {
                return self.malformed();
            }
            let line = self.line_buffer[consumed..end].to_vec();
            consumed = end + 1;
            self.process_line(&line, &mut outputs)?;
        }
        if consumed > 0 {
            self.line_buffer.drain(..consumed);
        }
        if self.line_buffer.len() > MAX_SSE_LINE_BYTES {
            return self.malformed();
        }
        Ok(outputs)
    }

    fn finish(mut self) -> AppResult<ProviderOutcome> {
        let mut outputs = Vec::new();
        if !self.line_buffer.is_empty() {
            let line = std::mem::take(&mut self.line_buffer);
            self.process_line(&line, &mut outputs)?;
        }
        if !self.event_data.is_empty() {
            self.dispatch_event(&mut outputs)?;
        }
        if !outputs.is_empty() || !self.saw_done || !self.saw_finish || self.content_bytes == 0 {
            return self.malformed();
        }
        Ok(ProviderOutcome {
            token_usage: self.token_usage,
        })
    }

    fn process_line(&mut self, raw_line: &[u8], outputs: &mut Vec<SseOutput>) -> AppResult<()> {
        let raw_line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let line = std::str::from_utf8(raw_line).map_err(|_| AppError::MalformedStream)?;
        if line.is_empty() {
            if !self.event_data.is_empty() {
                self.dispatch_event(outputs)?;
            }
            return Ok(());
        }
        if self.saw_done {
            return self.malformed();
        }
        if line.starts_with(':') {
            return Ok(());
        }
        let Some(data) = line.strip_prefix("data:") else {
            return self.malformed();
        };
        let data = data.strip_prefix(' ').unwrap_or(data).as_bytes();
        let separator = usize::from(!self.event_data.is_empty());
        let next_size = self
            .event_data
            .len()
            .checked_add(separator)
            .and_then(|size| size.checked_add(data.len()))
            .ok_or(AppError::MalformedStream)?;
        if next_size > MAX_SSE_EVENT_BYTES {
            return self.malformed();
        }
        if separator == 1 {
            self.event_data.push(b'\n');
        }
        self.event_data.extend_from_slice(data);
        Ok(())
    }

    fn dispatch_event(&mut self, outputs: &mut Vec<SseOutput>) -> AppResult<()> {
        let event = std::mem::take(&mut self.event_data);
        let event = std::str::from_utf8(&event).map_err(|_| AppError::MalformedStream)?;
        if event.trim() == "[DONE]" {
            if !self.saw_finish {
                return self.malformed();
            }
            self.saw_done = true;
            return Ok(());
        }
        let chunk: ProviderChunk =
            serde_json::from_str(event).map_err(|_| AppError::MalformedStream)?;
        let choices = chunk.choices.as_deref().unwrap_or_default();
        if choices.len() > 1 || (choices.is_empty() && chunk.usage.is_none()) {
            return self.malformed();
        }
        if self.saw_finish && !choices.is_empty() {
            return self.malformed();
        }
        if let Some(choice) = choices.first() {
            if choice.index.is_some_and(|index| index != 0) {
                return self.malformed();
            }
            if choice.logprobs.is_some() {
                return self.malformed();
            }
            match choice.delta.as_ref() {
                Some(delta) => {
                    if delta.tool_calls
                        || delta.function_call
                        || delta
                            .role
                            .as_deref()
                            .is_some_and(|role| role != "assistant")
                    {
                        return self.malformed();
                    }
                    let has_supported_delta = delta.role.is_some()
                        || delta.content.is_some()
                        || delta.reasoning_content.is_some();
                    if !has_supported_delta && choice.finish_reason.is_none() {
                        return self.malformed();
                    }
                    if let Some(content) = delta.content.as_ref() {
                        self.validate_delta(content)?;
                        if !content.is_empty() {
                            outputs.push(SseOutput::Delta(content.clone()));
                        }
                    }
                }
                None if choice.finish_reason.is_none() => return self.malformed(),
                None => {}
            }
            if let Some(reason) = choice.finish_reason.as_deref() {
                if self.saw_finish {
                    return self.malformed();
                }
                match reason {
                    "stop" | "length" => self.saw_finish = true,
                    "sensitive" => return Err(AppError::ProviderContentRejected),
                    "model_context_window_exceeded" => {
                        return Err(AppError::ProviderContextLimit);
                    }
                    "network_error" => return Err(AppError::ProviderUnavailable),
                    _ => return self.malformed(),
                }
            }
        }
        if let Some(usage) = chunk.usage {
            if usage.total_tokens > MAX_TOKEN_USAGE {
                return self.malformed();
            }
            if self
                .token_usage
                .is_some_and(|existing| existing != usage.total_tokens)
            {
                return self.malformed();
            }
            self.token_usage = Some(usage.total_tokens);
        }
        Ok(())
    }

    fn validate_delta(&mut self, delta: &str) -> AppResult<()> {
        if delta.len() > MAX_DELTA_BYTES {
            return self.malformed();
        }
        if delta.chars().any(|character| {
            character == '\0'
                || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        }) {
            return self.malformed();
        }
        self.content_bytes = self
            .content_bytes
            .checked_add(delta.len())
            .ok_or(AppError::MalformedStream)?;
        if self.content_bytes > MAX_CONTENT_BYTES {
            return self.malformed();
        }
        Ok(())
    }

    fn malformed<T>(&self) -> AppResult<T> {
        Err(AppError::MalformedStream)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::models::MessageRole;

    fn messages() -> Vec<ProviderMessage> {
        vec![ProviderMessage {
            role: MessageRole::User,
            content: "Hello".to_owned(),
        }]
    }

    #[test]
    fn request_contract_maps_all_reasoning_modes_exactly() {
        for (mode, thinking, max_tokens) in [
            (ReasoningMode::Fast, "disabled", 4_096),
            (ReasoningMode::Standard, "enabled", 8_192),
            (ReasoningMode::Deep, "enabled", 16_384),
        ] {
            let messages = messages();
            let value = serde_json::to_value(ChatRequest {
                model: MODEL,
                messages: &messages,
                stream: true,
                thinking: Thinking {
                    kind: mode.thinking_type(),
                },
                max_tokens: mode.max_tokens(),
            })
            .expect("request should serialize");
            assert_eq!(
                value,
                json!({
                    "model": "glm-5.1",
                    "messages": [{"role": "user", "content": "Hello"}],
                    "stream": true,
                    "thinking": {"type": thinking},
                    "max_tokens": max_tokens
                })
            );
        }
    }

    #[test]
    fn parser_accepts_split_utf8_and_sse_lines() {
        let fixture = concat!(
            ": keep-alive\r\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\r\n\r\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello ✨\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":12}}\n\n",
            "data: [DONE]\n\n"
        );
        let mut parser = SseParser::default();
        let bytes = fixture.as_bytes();
        let mut deltas = Vec::new();
        for chunk in bytes.chunks(7) {
            deltas.extend(
                parser
                    .push(chunk)
                    .expect("fragmented fixture should parse")
                    .into_iter()
                    .map(|output| match output {
                        SseOutput::Delta(delta) => delta,
                    }),
            );
        }
        let outcome = parser.finish().expect("stream should finish");
        assert_eq!(deltas, ["Hello ✨"]);
        assert_eq!(outcome.token_usage, Some(12));
    }

    #[test]
    fn parser_never_emits_reasoning_content() {
        let fixture = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"private\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Public\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let mut parser = SseParser::default();
        let deltas = parser
            .push(fixture.as_bytes())
            .expect("fixture should parse");
        assert_eq!(deltas.len(), 1);
        assert!(matches!(&deltas[0], SseOutput::Delta(value) if value == "Public"));
        parser.finish().expect("stream should finish");
    }

    #[test]
    fn parser_rejects_malformed_or_oversized_input() {
        let mut malformed = SseParser::default();
        assert!(malformed.push(b"event: hostile\n\n").is_err());

        let mut oversized = SseParser::default();
        let long_line = vec![b'x'; MAX_SSE_LINE_BYTES + 1];
        assert!(oversized.push(&long_line).is_err());
    }

    #[test]
    fn parser_maps_documented_provider_terminal_reasons() {
        for (reason, expected) in [
            ("sensitive", "provider_content_rejected"),
            ("model_context_window_exceeded", "provider_context_limit"),
            ("network_error", "provider_unavailable"),
            ("unknown_reason", "malformed_stream"),
        ] {
            let fixture = format!(
                "data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"{reason}\"}}]}}\n\n"
            );
            let mut parser = SseParser::default();
            let error = parser
                .push(fixture.as_bytes())
                .expect_err("terminal error reason should fail")
                .public();
            assert_eq!(error.code, expected);
        }
    }

    #[test]
    fn parser_rejects_tools_and_content_after_finish() {
        let mut tools = SseParser::default();
        let tool_event = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"x\"}]}}]}\n\n";
        assert!(matches!(
            tools.push(tool_event.as_bytes()),
            Err(AppError::MalformedStream)
        ));

        let mut after_finish = SseParser::default();
        after_finish
            .push(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"Done\"},\"finish_reason\":\"stop\"}]}\n\n",
            )
            .expect("terminal content is valid");
        assert!(matches!(
            after_finish.push(b"data: {\"choices\":[{\"delta\":{\"content\":\"late\"}}]}\n\n"),
            Err(AppError::MalformedStream)
        ));

        let mut premature_done = SseParser::default();
        assert!(matches!(
            premature_done.push(b"data: [DONE]\n\n"),
            Err(AppError::MalformedStream)
        ));

        let mut absurd_usage = SseParser::default();
        assert!(matches!(
            absurd_usage.push(
                b"data: {\"choices\":[],\"usage\":{\"total_tokens\":18446744073709551615}}\n\n"
            ),
            Err(AppError::MalformedStream)
        ));
    }

    #[tokio::test]
    async fn pre_cancelled_request_never_starts_network_work() {
        let provider = ProviderClient::new().expect("provider should initialize");
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let result = provider
            .stream_chat(
                &messages(),
                ReasoningMode::Standard,
                "test_key_not_a_secret",
                &cancellation,
                |_| Ok(()),
            )
            .await;
        assert!(matches!(result, Err(AppError::Cancelled)));
    }

    #[test]
    fn provider_origin_is_exact_https_and_not_configurable() {
        assert_eq!(
            PROVIDER_URL,
            "https://api.z.ai/api/paas/v4/chat/completions"
        );
        assert!(PROVIDER_URL.starts_with("https://api.z.ai/"));
    }

    #[test]
    fn retry_after_is_honored_only_within_the_bounded_retry_window() {
        let short = reqwest::header::HeaderValue::from_static("2");
        let short = parse_retry_after(Some(&short));
        assert!(short.permitted);
        assert_eq!(short.delay, Some(Duration::from_secs(2)));

        let long = reqwest::header::HeaderValue::from_static("60");
        let long = parse_retry_after(Some(&long));
        assert!(!long.permitted);
        assert_eq!(long.delay, None);

        let malformed = reqwest::header::HeaderValue::from_static("later");
        let malformed = parse_retry_after(Some(&malformed));
        assert!(malformed.permitted);
        assert_eq!(malformed.delay, None);
    }
}
