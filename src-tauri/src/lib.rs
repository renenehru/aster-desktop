#![forbid(unsafe_code)]

mod api;
mod credentials;
mod database;
mod error;
mod models;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI8, Ordering};
use std::sync::{Arc, Mutex};

use api::ProviderClient;
use aster_credential_prompt::PromptOutcome;
use chrono::{SecondsFormat, Utc};
use credentials::CredentialStore;
use database::{Database, PreparedGeneration};
use error::{AppError, AppResult};
use models::{
    AppStatus, Conversation, ConversationSummary, CredentialPromptResult, CredentialStatus,
    ExportBundle, ExportResult, ImportBundle, MessageStatus, ReasoningMode, SendMessageResult,
    StreamEvent,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rfd::AsyncFileDialog;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request};
use tauri::{Emitter, Manager, State, WebviewWindow};
use tokio_util::sync::CancellationToken;
use url::{Host, Url};
use uuid::Uuid;
use zeroize::Zeroizing;

const MAIN_WINDOW: &str = "main";
const STREAM_EVENT: &str = "chat-stream";
const DATABASE_FILE_NAME: &str = "aster.sqlite3";
const EXPORT_FORMAT: &str = "aster-conversation";
const EXPORT_VERSION: u32 = 1;
const MAX_TRANSFER_BYTES: usize = 32 * 1024 * 1024;
const MAX_JSON_NESTING: usize = 8;
const MAX_IPC_BODY_BYTES: usize = 320 * 1024;
const MAX_EXTERNAL_URL_BYTES: usize = 2_048;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConversationIdArgs {
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateConversationArgs {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenameConversationArgs {
    conversation_id: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendMessageArgs {
    conversation_id: String,
    content: String,
    reasoning_mode: ReasoningMode,
    regenerate_from_message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestIdArgs {
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalUrlArgs {
    url: String,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    database: Database,
    credentials: CredentialStore,
    provider: ProviderClient,
    active: Mutex<HashMap<String, ActiveGeneration>>,
    shutdown_started: AtomicBool,
    shutdown_ready: AtomicBool,
    external_processing_acknowledged: AtomicBool,
    credential_prompt_active: AtomicBool,
    provider_reachability: AtomicI8,
}

struct ActiveGeneration {
    conversation_id: String,
    cancellation: CancellationToken,
    terminal_claimed: bool,
}

struct CredentialMutationLease {
    inner: Arc<AppStateInner>,
}

impl Drop for CredentialMutationLease {
    fn drop(&mut self) {
        self.inner
            .credential_prompt_active
            .store(false, Ordering::Release);
    }
}

impl AppState {
    fn new(database: Database, provider: ProviderClient) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                database,
                credentials: CredentialStore,
                provider,
                active: Mutex::new(HashMap::new()),
                shutdown_started: AtomicBool::new(false),
                shutdown_ready: AtomicBool::new(false),
                external_processing_acknowledged: AtomicBool::new(false),
                credential_prompt_active: AtomicBool::new(false),
                provider_reachability: AtomicI8::new(0),
            }),
        }
    }

    fn database(&self) -> &Database {
        &self.inner.database
    }

    fn credentials(&self) -> CredentialStore {
        self.inner.credentials
    }

    fn provider(&self) -> &ProviderClient {
        &self.inner.provider
    }

    fn reserve_credential_mutation(
        &self,
        require_inactive_generations: bool,
    ) -> AppResult<CredentialMutationLease> {
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        if require_inactive_generations && !active.is_empty() {
            return Err(AppError::Conflict(
                "Stop active generation before replacing the API key.",
            ));
        }
        self.inner
            .credential_prompt_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| AppError::CredentialPromptBusy)?;
        let lease = CredentialMutationLease {
            inner: self.inner.clone(),
        };
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            drop(lease);
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        drop(active);
        Ok(lease)
    }

    fn reserve_generation(&self, conversation_id: &str) -> AppResult<(String, CancellationToken)> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        let mut active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        if self.inner.credential_prompt_active.load(Ordering::Acquire) {
            return Err(AppError::CredentialPromptBusy);
        }
        if active
            .values()
            .any(|generation| generation.conversation_id == conversation_id)
        {
            return Err(AppError::Conflict(
                "This conversation already has an active generation.",
            ));
        }
        let request_id = Uuid::new_v4().to_string();
        let cancellation = CancellationToken::new();
        active.insert(
            request_id.clone(),
            ActiveGeneration {
                conversation_id: conversation_id.to_owned(),
                cancellation: cancellation.clone(),
                terminal_claimed: false,
            },
        );
        Ok((request_id, cancellation))
    }

    fn reserve_generation_for_send(
        &self,
        conversation_id: &str,
    ) -> AppResult<(String, CancellationToken)> {
        self.require_external_processing_acknowledged()?;
        self.reserve_generation(conversation_id)
    }

    fn release_generation(&self, request_id: &str) {
        if let Ok(mut active) = self.inner.active.lock() {
            active.remove(request_id);
        }
    }

    fn claim_terminal(&self, request_id: &str) -> AppResult<Option<bool>> {
        let mut active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        let Some(generation) = active.get_mut(request_id) else {
            return Ok(None);
        };
        if generation.terminal_claimed {
            return Ok(None);
        }
        generation.terminal_claimed = true;
        Ok(Some(generation.cancellation.is_cancelled()))
    }

    fn cancel_generation(&self, request_id: &str) -> AppResult<()> {
        validate_uuid(request_id, "Request ID is invalid.")?;
        let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if let Some(generation) = active.get(request_id)
            && !generation.terminal_claimed
        {
            generation.cancellation.cancel();
        }
        Ok(())
    }

    fn ensure_conversation_inactive(&self, conversation_id: &str) -> AppResult<()> {
        let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if active
            .values()
            .any(|generation| generation.conversation_id == conversation_id)
        {
            return Err(AppError::Conflict(
                "Stop the active generation before deleting this conversation.",
            ));
        }
        Ok(())
    }

    fn cancel_all(&self) {
        if let Ok(active) = self.inner.active.lock() {
            for generation in active.values() {
                if !generation.terminal_claimed {
                    generation.cancellation.cancel();
                }
            }
        }
    }

    fn request_shutdown(&self) -> (bool, bool) {
        if self.inner.shutdown_ready.load(Ordering::Acquire) {
            return (false, false);
        }
        let first = !self.inner.shutdown_started.swap(true, Ordering::AcqRel);
        if first {
            self.cancel_all();
        }
        let has_active_generation = self
            .inner
            .active
            .lock()
            .map(|active| !active.is_empty())
            .unwrap_or(true);
        let has_active =
            has_active_generation || self.inner.credential_prompt_active.load(Ordering::Acquire);
        if !has_active {
            self.inner.shutdown_ready.store(true, Ordering::Release);
        }
        (first, has_active)
    }

    fn generation_tasks_finished(&self) -> bool {
        self.inner
            .active
            .lock()
            .map(|active| active.is_empty())
            .unwrap_or(false)
            && !self.inner.credential_prompt_active.load(Ordering::Acquire)
    }

    fn mark_shutdown_ready(&self) {
        self.inner.shutdown_ready.store(true, Ordering::Release);
    }

    fn provider_reachability(&self) -> (&'static str, bool) {
        match self.inner.provider_reachability.load(Ordering::Acquire) {
            1 => ("reachable", true),
            -1 => ("unreachable", false),
            _ => ("unknown", false),
        }
    }

    fn acknowledge_external_processing(&self) {
        self.inner
            .external_processing_acknowledged
            .store(true, Ordering::Release);
    }

    fn external_processing_acknowledged(&self) -> bool {
        self.inner
            .external_processing_acknowledged
            .load(Ordering::Acquire)
    }

    fn require_external_processing_acknowledged(&self) -> AppResult<()> {
        if self.external_processing_acknowledged() {
            Ok(())
        } else {
            Err(AppError::ExternalProcessingNoticeRequired)
        }
    }

    fn record_provider_result(&self, result: &AppResult<api::ProviderOutcome>) {
        let value = match result {
            Ok(_) => 1,
            Err(AppError::Network | AppError::Timeout) => -1,
            Err(AppError::Cancelled) => return,
            Err(_) => 1,
        };
        self.inner
            .provider_reachability
            .store(value, Ordering::Release);
    }
}

async fn wait_for_generation_tasks(state: &AppState) {
    while !state.generation_tasks_finished() {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    state.mark_shutdown_ready();
}

fn ensure_main_window(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != MAIN_WINDOW {
        return Err(AppError::Conflict(
            "This command is available only to the main application window.",
        ));
    }
    Ok(())
}

fn main_window_owner_handle(window: &WebviewWindow) -> AppResult<isize> {
    let handle = window
        .window_handle()
        .map_err(|_| AppError::CredentialPrompt)?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get()),
        _ => Err(AppError::CredentialPrompt),
    }
}

fn validate_ipc_request(request: &Request<'_>, allowed_keys: &[&str]) -> AppResult<()> {
    match request.body() {
        InvokeBody::Raw(bytes) => validate_ipc_bytes(bytes, allowed_keys),
        InvokeBody::Json(_) => Err(AppError::Validation(
            "Command arguments must use the bounded raw JSON IPC format.",
        )),
    }
}

fn parse_ipc_request<T: DeserializeOwned>(
    request: &Request<'_>,
    allowed_keys: &[&str],
) -> AppResult<T> {
    match request.body() {
        InvokeBody::Raw(bytes) => parse_ipc_bytes(bytes, allowed_keys),
        InvokeBody::Json(_) => Err(AppError::Validation(
            "Command arguments must use the bounded raw JSON IPC format.",
        )),
    }
}

fn parse_ipc_bytes<T: DeserializeOwned>(bytes: &[u8], allowed_keys: &[&str]) -> AppResult<T> {
    validate_ipc_bytes(bytes, allowed_keys)?;
    serde_json::from_slice(bytes)
        .map_err(|_| AppError::Validation("Command arguments are missing or have an invalid type."))
}

fn validate_ipc_bytes(bytes: &[u8], allowed_keys: &[&str]) -> AppResult<()> {
    if bytes.len() > MAX_IPC_BODY_BYTES {
        return Err(AppError::Validation(
            "Command arguments exceed the 320 KiB limit.",
        ));
    }
    validate_ipc_json_nesting(bytes)?;
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::Validation("Command arguments must be a valid JSON object."))?;
    validate_ipc_json(&value, allowed_keys)
}

fn validate_ipc_json_nesting(serialized: &[u8]) -> AppResult<()> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in serialized {
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
                depth = depth.checked_add(1).ok_or(AppError::Validation(
                    "Command arguments are nested too deeply.",
                ))?;
                if depth > MAX_JSON_NESTING {
                    return Err(AppError::Validation(
                        "Command arguments are nested too deeply.",
                    ));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn validate_ipc_json(value: &serde_json::Value, allowed_keys: &[&str]) -> AppResult<()> {
    let serde_json::Value::Object(arguments) = value else {
        return Err(AppError::Validation(
            "Command arguments must be a bounded JSON object.",
        ));
    };
    if arguments
        .keys()
        .any(|key| !allowed_keys.contains(&key.as_str()))
    {
        return Err(AppError::Validation(
            "The command contains an unknown argument.",
        ));
    }
    let mut remaining = MAX_IPC_BODY_BYTES;
    measure_json_value(value, 0, &mut remaining)
}

fn measure_json_value(
    value: &serde_json::Value,
    depth: usize,
    remaining: &mut usize,
) -> AppResult<()> {
    if depth > MAX_JSON_NESTING {
        return Err(AppError::Validation(
            "Command arguments are nested too deeply.",
        ));
    }
    let own_bytes = match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) => 5,
        serde_json::Value::Number(_) => 32,
        serde_json::Value::String(value) => value.len().saturating_add(2),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => 2,
    };
    *remaining = remaining
        .checked_sub(own_bytes)
        .ok_or(AppError::Validation(
            "Command arguments exceed the 320 KiB limit.",
        ))?;
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                measure_json_value(value, depth + 1, remaining)?;
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                *remaining = remaining.checked_sub(key.len().saturating_add(3)).ok_or(
                    AppError::Validation("Command arguments exceed the 320 KiB limit."),
                )?;
                measure_json_value(value, depth + 1, remaining)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[tauri::command]
fn app_status(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<AppStatus> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    let (provider_reachability, online) = state.provider_reachability();
    Ok(AppStatus {
        mode: "desktop",
        version: env!("CARGO_PKG_VERSION"),
        online,
        provider_reachability,
        database_ready: true,
        external_processing_acknowledged: state.external_processing_acknowledged(),
    })
}

#[tauri::command]
fn acknowledge_external_processing(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<()> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    state.acknowledge_external_processing();
    Ok(())
}

#[tauri::command]
fn credential_status(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<CredentialStatus> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    state.credentials().status()
}

#[tauri::command]
async fn prompt_store_api_key(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<CredentialPromptResult> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    let state = state.inner().clone();
    let lease = state.reserve_credential_mutation(true)?;
    let owner_window = main_window_owner_handle(&window)?;
    let (lease, prompt_result) = tauri::async_runtime::spawn_blocking(move || {
        let result = aster_credential_prompt::prompt_api_key(owner_window);
        (lease, result)
    })
    .await
    .map_err(|_| AppError::CredentialPrompt)?;

    let result = match prompt_result {
        Ok(PromptOutcome::Submitted(api_key)) => {
            state
                .credentials()
                .store(api_key)
                .map(|status| CredentialPromptResult {
                    configured: status.configured,
                    source: status.source,
                    cancelled: false,
                })
        }
        Ok(PromptOutcome::Cancelled) => {
            state
                .credentials()
                .status()
                .map(|status| CredentialPromptResult {
                    configured: status.configured,
                    source: status.source,
                    cancelled: true,
                })
        }
        Err(aster_credential_prompt::PromptError::InvalidSecret) => {
            Err(AppError::CredentialInvalid)
        }
        Err(
            aster_credential_prompt::PromptError::InvalidOwner
            | aster_credential_prompt::PromptError::Unavailable,
        ) => Err(AppError::CredentialPrompt),
    };
    drop(lease);
    result
}

#[tauri::command]
fn delete_api_key(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<CredentialStatus> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    let lease = state.reserve_credential_mutation(false)?;
    state.cancel_all();
    let result = state.credentials().delete();
    drop(lease);
    result
}

#[tauri::command]
fn list_conversations(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<Vec<ConversationSummary>> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    state.database().list_conversations()
}

#[tauri::command]
fn get_conversation(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<Conversation> {
    ensure_main_window(&window)?;
    let arguments: ConversationIdArgs = parse_ipc_request(&request, &["conversationId"])?;
    state
        .database()
        .get_conversation(&arguments.conversation_id)
}

#[tauri::command]
fn create_conversation(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<Conversation> {
    ensure_main_window(&window)?;
    let arguments: CreateConversationArgs = parse_ipc_request(&request, &["title"])?;
    state.database().create_conversation(arguments.title)
}

#[tauri::command]
fn rename_conversation(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<ConversationSummary> {
    ensure_main_window(&window)?;
    let arguments: RenameConversationArgs =
        parse_ipc_request(&request, &["conversationId", "title"])?;
    state
        .database()
        .rename_conversation(&arguments.conversation_id, arguments.title)
}

#[tauri::command]
fn delete_conversation(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<()> {
    ensure_main_window(&window)?;
    let arguments: ConversationIdArgs = parse_ipc_request(&request, &["conversationId"])?;
    state.ensure_conversation_inactive(&arguments.conversation_id)?;
    state
        .database()
        .delete_conversation(&arguments.conversation_id)
}

#[tauri::command]
async fn send_message(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<SendMessageResult> {
    ensure_main_window(&window)?;
    let arguments: SendMessageArgs = parse_ipc_request(
        &request,
        &[
            "conversationId",
            "content",
            "reasoningMode",
            "regenerateFromMessageId",
        ],
    )?;
    let state = state.inner().clone();
    let (request_id, cancellation) =
        state.reserve_generation_for_send(&arguments.conversation_id)?;

    if let Err(error) = state.database().validate_generation_request(
        &arguments.conversation_id,
        &arguments.content,
        arguments.regenerate_from_message_id.as_deref(),
    ) {
        state.release_generation(&request_id);
        return Err(error);
    }

    let api_key = match state.credentials().load() {
        Ok(value) => value,
        Err(error) => {
            state.release_generation(&request_id);
            return Err(error);
        }
    };
    let prepared = match prepare_generation(
        &state,
        &arguments.conversation_id,
        arguments.content,
        arguments.reasoning_mode,
        arguments.regenerate_from_message_id,
    ) {
        Ok(value) => value,
        Err(error) => {
            state.release_generation(&request_id);
            return Err(error);
        }
    };

    let task_request_id = request_id.clone();
    tauri::async_runtime::spawn(run_generation(
        window,
        state,
        task_request_id,
        arguments.conversation_id,
        prepared,
        api_key,
        cancellation,
    ));
    Ok(SendMessageResult { request_id })
}

fn prepare_generation(
    state: &AppState,
    conversation_id: &str,
    content: String,
    reasoning_mode: ReasoningMode,
    regenerate_from_message_id: Option<String>,
) -> AppResult<PreparedGeneration> {
    state.database().prepare_generation(
        conversation_id,
        content,
        reasoning_mode,
        regenerate_from_message_id,
    )
}

async fn run_generation(
    window: WebviewWindow,
    state: AppState,
    request_id: String,
    conversation_id: String,
    prepared: PreparedGeneration,
    api_key: Zeroizing<String>,
    cancellation: CancellationToken,
) {
    let mut sequence = 0_u64;
    if emit_stream_event(
        &window,
        StreamEvent {
            request_id: request_id.clone(),
            conversation_id: conversation_id.clone(),
            sequence,
            kind: "started",
            delta: None,
            message: None,
            error: None,
            error_code: None,
            retryable: None,
        },
    )
    .is_err()
    {
        cancellation.cancel();
    }

    let mut content = String::new();
    let result = state
        .provider()
        .stream_chat(
            &prepared.history,
            prepared.reasoning_mode,
            api_key.as_str(),
            &cancellation,
            |delta| {
                if cancellation.is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                content.push_str(&delta);
                sequence = next_stream_sequence(sequence)?;
                emit_stream_event(
                    &window,
                    StreamEvent {
                        request_id: request_id.clone(),
                        conversation_id: conversation_id.clone(),
                        sequence,
                        kind: "delta",
                        delta: Some(delta),
                        message: None,
                        error: None,
                        error_code: None,
                        retryable: None,
                    },
                )
            },
        )
        .await;
    drop(api_key);
    state.record_provider_result(&result);

    let cancelled = match state.claim_terminal(&request_id) {
        Ok(Some(value)) => value || matches!(&result, Err(AppError::Cancelled)),
        Ok(None) | Err(_) => return,
    };
    let terminal_sequence = match next_stream_sequence(sequence) {
        Ok(value) => value,
        Err(_) => {
            state.release_generation(&request_id);
            return;
        }
    };

    if cancelled {
        emit_persisted_terminal(
            &window,
            &state,
            &request_id,
            &conversation_id,
            content,
            MessageStatus::Cancelled,
            None,
            None,
            terminal_sequence,
        );
        state.release_generation(&request_id);
        return;
    }

    match result {
        Ok(outcome) => emit_persisted_terminal(
            &window,
            &state,
            &request_id,
            &conversation_id,
            content,
            MessageStatus::Complete,
            outcome.token_usage,
            None,
            terminal_sequence,
        ),
        Err(error) => emit_persisted_terminal(
            &window,
            &state,
            &request_id,
            &conversation_id,
            content,
            MessageStatus::Error,
            None,
            Some(error),
            terminal_sequence,
        ),
    }
    state.release_generation(&request_id);
}

#[allow(clippy::too_many_arguments)]
fn emit_persisted_terminal(
    window: &WebviewWindow,
    state: &AppState,
    request_id: &str,
    conversation_id: &str,
    content: String,
    status: MessageStatus,
    token_usage: Option<u64>,
    source_error: Option<AppError>,
    sequence: u64,
) {
    match state
        .database()
        .append_assistant_message(conversation_id, content, status, token_usage)
    {
        Ok(message) => {
            let (kind, error, error_code, retryable) = match source_error.as_ref() {
                Some(source_error) => {
                    let public = source_error.public();
                    (
                        "error",
                        Some(public.message),
                        Some(public.code),
                        public.retryable,
                    )
                }
                None if status == MessageStatus::Cancelled => ("cancelled", None, None, false),
                None => ("completed", None, None, false),
            };
            let _ = emit_stream_event(
                window,
                StreamEvent {
                    request_id: request_id.to_owned(),
                    conversation_id: conversation_id.to_owned(),
                    sequence,
                    kind,
                    delta: None,
                    message: Some(message),
                    error,
                    error_code,
                    retryable: (kind == "error").then_some(retryable),
                },
            );
        }
        Err(database_error) => {
            let public = database_error.public();
            let _ = emit_stream_event(
                window,
                StreamEvent {
                    request_id: request_id.to_owned(),
                    conversation_id: conversation_id.to_owned(),
                    sequence,
                    kind: "error",
                    delta: None,
                    message: None,
                    error: Some(public.message),
                    error_code: Some(public.code),
                    retryable: Some(public.retryable),
                },
            );
        }
    }
}

fn emit_stream_event(window: &WebviewWindow, event: StreamEvent) -> AppResult<()> {
    ensure_main_window(window)?;
    window
        .emit(STREAM_EVENT, event)
        .map_err(|_| AppError::Internal)
}

fn next_stream_sequence(sequence: u64) -> AppResult<u64> {
    sequence.checked_add(1).ok_or(AppError::Internal)
}

fn validate_external_url(value: &str) -> AppResult<Url> {
    if value.is_empty()
        || value.len() > MAX_EXTERNAL_URL_BYTES
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AppError::Validation(
            "External links must be bounded HTTPS URLs without whitespace.",
        ));
    }
    let url = Url::parse(value)
        .map_err(|_| AppError::Validation("External links must be valid absolute HTTPS URLs."))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host().is_none()
    {
        return Err(AppError::Validation(
            "External links must use HTTPS and cannot contain credentials.",
        ));
    }
    match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            let local_name = domain == "localhost"
                || domain.ends_with(".localhost")
                || domain.ends_with(".local")
                || domain.ends_with(".localdomain")
                || domain.ends_with(".internal")
                || domain.ends_with(".home")
                || domain.ends_with(".lan")
                || !domain.contains('.');
            if domain.is_empty() || local_name {
                return Err(AppError::Validation(
                    "External links cannot target local or private host names.",
                ));
            }
        }
        Some(Host::Ipv4(_)) | Some(Host::Ipv6(_)) | None => {
            return Err(AppError::Validation(
                "External links cannot target IP address literals.",
            ));
        }
    }
    Ok(url)
}

#[tauri::command]
fn open_external_url(request: Request<'_>, window: WebviewWindow) -> AppResult<()> {
    ensure_main_window(&window)?;
    let arguments: ExternalUrlArgs = parse_ipc_request(&request, &["url"])?;
    let url = validate_external_url(&arguments.url)?;
    tauri_plugin_opener::open_url(url.as_str(), None::<&str>)
        .map_err(|_| AppError::ExternalNavigation)
}

#[tauri::command]
fn cancel_generation(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<()> {
    ensure_main_window(&window)?;
    let arguments: RequestIdArgs = parse_ipc_request(&request, &["requestId"])?;
    state.cancel_generation(&arguments.request_id)
}

#[tauri::command]
async fn export_conversation(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    ensure_main_window(&window)?;
    let arguments: ConversationIdArgs = parse_ipc_request(&request, &["conversationId"])?;
    let conversation = state
        .database()
        .get_conversation_for_export(&arguments.conversation_id, MAX_TRANSFER_BYTES)?;
    let bundle = ExportBundle {
        format: EXPORT_FORMAT,
        version: EXPORT_VERSION,
        exported_at: now_utc(),
        conversations: vec![conversation.into()],
    };
    let serialized = serialize_bounded(&bundle)?;

    let Some(file) = AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Export Aster conversation")
        .set_file_name("aster-conversation.json")
        .add_filter("Aster conversation JSON", &["json"])
        .save_file()
        .await
    else {
        return Ok(ExportResult {
            cancelled: true,
            file_name: None,
        });
    };
    file.write(&serialized).await?;
    Ok(ExportResult {
        cancelled: false,
        file_name: bounded_file_name(file.file_name()),
    })
}

#[tauri::command]
async fn import_conversations(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<Vec<ConversationSummary>> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    let Some(file) = AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Import Aster conversations")
        .add_filter("Aster conversation JSON", &["json"])
        .pick_file()
        .await
    else {
        return Ok(Vec::new());
    };

    let path = file.path().to_path_buf();
    let serialized = tauri::async_runtime::spawn_blocking(move || read_bounded(&path))
        .await
        .map_err(|_| AppError::Internal)??;
    validate_json_nesting(&serialized)?;
    let bundle: ImportBundle =
        serde_json::from_slice(&serialized).map_err(|_| AppError::Serialization)?;
    state.database().import(bundle)
}

fn serialize_bounded<T: Serialize>(value: &T) -> AppResult<Vec<u8>> {
    let mut writer = BoundedBuffer::new(MAX_TRANSFER_BYTES);
    if serde_json::to_writer(&mut writer, value).is_err() {
        return if writer.exceeded {
            Err(AppError::Validation(
                "The conversation export exceeds the 32 MiB limit.",
            ))
        } else {
            Err(AppError::Serialization)
        };
    }
    Ok(writer.bytes)
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedBuffer {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(buffer.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("bounded serializer capacity exceeded"));
        };
        if next_len > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("bounded serializer capacity exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn read_bounded(path: &Path) -> AppResult<Vec<u8>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(AppError::Validation(
            "The selected import must be a regular file.",
        ));
    }
    if metadata.len() > MAX_TRANSFER_BYTES as u64 {
        return Err(AppError::Validation(
            "The selected import exceeds the 32 MiB limit.",
        ));
    }
    let expected = usize::try_from(metadata.len())
        .unwrap_or(MAX_TRANSFER_BYTES)
        .min(MAX_TRANSFER_BYTES);
    let mut bytes = Vec::with_capacity(expected);
    file.take((MAX_TRANSFER_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TRANSFER_BYTES {
        return Err(AppError::Validation(
            "The selected import exceeds the 32 MiB limit.",
        ));
    }
    Ok(bytes)
}

fn validate_json_nesting(serialized: &[u8]) -> AppResult<()> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in serialized {
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
                depth = depth.checked_add(1).ok_or(AppError::Validation(
                    "The import JSON is nested too deeply.",
                ))?;
                if depth > MAX_JSON_NESTING {
                    return Err(AppError::Validation(
                        "The import JSON is nested more than 8 levels.",
                    ));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn bounded_file_name(file_name: String) -> Option<String> {
    if file_name.is_empty() {
        return None;
    }
    Some(file_name.chars().take(255).collect())
}

fn validate_uuid(value: &str, message: &'static str) -> AppResult<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| AppError::Validation(message))?;
    if parsed.to_string() != value.to_ascii_lowercase() {
        return Err(AppError::Validation(message));
    }
    Ok(())
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn run() {
    let application = tauri::Builder::default()
        .setup(|app| {
            let app_data_directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_directory)?;
            let database = Database::open(&app_data_directory.join(DATABASE_FILE_NAME))?;
            let provider = ProviderClient::new()?;
            app.manage(AppState::new(database, provider));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            credential_status,
            acknowledge_external_processing,
            prompt_store_api_key,
            delete_api_key,
            list_conversations,
            get_conversation,
            create_conversation,
            rename_conversation,
            delete_conversation,
            send_message,
            cancel_generation,
            export_conversation,
            import_conversations,
            open_external_url,
        ])
        .build(tauri::generate_context!())
        .expect("Aster startup configuration is invalid");

    application.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            let state = app.state::<AppState>().inner().clone();
            let (first, has_active) = state.request_shutdown();
            if has_active {
                api.prevent_exit();
                if first {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        wait_for_generation_tasks(&state).await;
                        app.exit(0);
                    });
                }
            }
        }
        tauri::RunEvent::Exit => app.state::<AppState>().cancel_all(),
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::Value;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn bounded_serializer_stops_before_the_limit_is_exceeded() {
        let payload = "x".repeat(256);
        let mut writer = BoundedBuffer::new(32);
        assert!(serde_json::to_writer(&mut writer, &payload).is_err());
        assert!(writer.exceeded);
        assert!(writer.bytes.len() <= 32);
    }

    #[test]
    fn bounded_reader_rejects_oversized_files_before_allocating_them() {
        let file = NamedTempFile::new().expect("temporary file should open");
        file.as_file()
            .set_len((MAX_TRANSFER_BYTES + 1) as u64)
            .expect("temporary file should resize");
        assert!(matches!(
            read_bounded(file.path()),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn import_nesting_scan_ignores_brackets_inside_strings_and_rejects_depth_nine() {
        validate_json_nesting(br#"{"content":"[[[[[[[[["}"#)
            .expect("string brackets do not affect nesting");
        assert!(matches!(
            validate_json_nesting(b"[[[[[[[[[]]]]]]]]]"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn raw_ipc_validator_rejects_malformed_unknown_oversized_and_deep_arguments() {
        assert!(validate_ipc_bytes(b"{}", &[]).is_ok());
        assert!(matches!(
            validate_ipc_bytes(br#"{"unexpected":true}"#, &[]),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_ipc_bytes(b"{", &[]),
            Err(AppError::Validation(_))
        ));
        let oversized = vec![b'x'; MAX_IPC_BODY_BYTES + 1];
        assert!(matches!(
            validate_ipc_bytes(&oversized, &["content"]),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_ipc_bytes(br#"{"content":[[[[[[[[["x"]]]]]]]]]}"#, &["content"]),
            Err(AppError::Validation(_))
        ));
        validate_ipc_bytes(br#"{"content":"[[[[[[[[["}"#, &["content"])
            .expect("brackets inside a string do not affect raw nesting");

        let allowed_keys = [
            "conversationId",
            "content",
            "reasoningMode",
            "regenerateFromMessageId",
        ];
        let valid_bytes = serde_json::to_vec(&serde_json::json!({
            "conversationId": Uuid::new_v4().to_string(),
            "content": "Bounded prompt",
            "reasoningMode": "standard"
        }))
        .expect("serialize fixture");
        let valid: SendMessageArgs =
            parse_ipc_bytes(&valid_bytes, &allowed_keys).expect("valid typed arguments");
        assert_eq!(valid.content, "Bounded prompt");

        for malformed in [
            serde_json::json!({
                "conversationId": Uuid::new_v4().to_string(),
                "reasoningMode": "standard"
            }),
            serde_json::json!({
                "conversationId": Uuid::new_v4().to_string(),
                "content": 42,
                "reasoningMode": "standard"
            }),
            serde_json::json!({
                "conversationId": Uuid::new_v4().to_string(),
                "content": "Prompt",
                "reasoningMode": "unsupported"
            }),
        ] {
            let malformed = serde_json::to_vec(&malformed).expect("serialize malformed fixture");
            let result: AppResult<SendMessageArgs> = parse_ipc_bytes(&malformed, &allowed_keys);
            assert!(matches!(result, Err(AppError::Validation(_))));
        }

        let duplicate = format!(
            "{{\"conversationId\":\"{}\",\"content\":\"first\",\"content\":\"second\",\"reasoningMode\":\"standard\"}}",
            Uuid::new_v4()
        );
        let result: AppResult<SendMessageArgs> =
            parse_ipc_bytes(duplicate.as_bytes(), &allowed_keys);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn generation_registry_enforces_one_request_through_terminal_persistence() {
        let database = Database::open_in_memory().expect("database");
        let conversation = database.create_conversation(None).expect("conversation");
        let state = AppState::new(database, ProviderClient::new().expect("provider"));

        let (request_id, cancellation) = state
            .reserve_generation(&conversation.id)
            .expect("first reservation");
        assert!(matches!(
            state.reserve_generation(&conversation.id),
            Err(AppError::Conflict(_))
        ));
        assert_eq!(
            state.claim_terminal(&request_id).expect("claim"),
            Some(false)
        );
        state
            .cancel_generation(&request_id)
            .expect("late cancellation is idempotent");
        assert!(!cancellation.is_cancelled());
        assert!(matches!(
            state.reserve_generation(&conversation.id),
            Err(AppError::Conflict(_))
        ));

        state.release_generation(&request_id);
        let (second_request, second_cancellation) = state
            .reserve_generation(&conversation.id)
            .expect("reservation after persistence");
        state
            .cancel_generation(&second_request)
            .expect("cancel active request");
        assert!(second_cancellation.is_cancelled());
        assert_eq!(
            state.claim_terminal(&second_request).expect("claim"),
            Some(true)
        );
        state.release_generation(&second_request);
    }

    #[test]
    fn external_processing_acknowledgement_is_required_before_reservation_and_is_session_only() {
        let database = Database::open_in_memory().expect("database");
        let conversation = database.create_conversation(None).expect("conversation");
        let state = AppState::new(database.clone(), ProviderClient::new().expect("provider"));

        assert!(matches!(
            state.reserve_generation_for_send(&conversation.id),
            Err(AppError::ExternalProcessingNoticeRequired)
        ));
        assert!(state.generation_tasks_finished());
        assert!(
            database
                .get_conversation(&conversation.id)
                .expect("unchanged conversation")
                .messages
                .is_empty()
        );

        state.acknowledge_external_processing();
        let (request_id, _) = state
            .reserve_generation_for_send(&conversation.id)
            .expect("acknowledged reservation");
        state.release_generation(&request_id);

        let fresh_session = AppState::new(database, ProviderClient::new().expect("provider"));
        assert!(!fresh_session.external_processing_acknowledged());
    }

    #[test]
    fn credential_mutation_lease_is_single_flight_across_threads() {
        let database = Database::open_in_memory().expect("database");
        let state = AppState::new(database, ProviderClient::new().expect("provider"));
        let worker_state = state.clone();
        let (acquired_sender, acquired_receiver) = std::sync::mpsc::sync_channel(0);
        let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(0);
        let worker = std::thread::spawn(move || {
            let lease = worker_state
                .reserve_credential_mutation(true)
                .expect("first credential mutation");
            acquired_sender.send(()).expect("signal acquisition");
            release_receiver.recv().expect("wait for release");
            drop(lease);
        });
        acquired_receiver.recv().expect("worker acquired lease");

        assert!(matches!(
            state.reserve_credential_mutation(true),
            Err(AppError::CredentialPromptBusy)
        ));
        release_sender.send(()).expect("release worker");
        worker.join().expect("worker should not panic");
        let lease = state
            .reserve_credential_mutation(true)
            .expect("lease should be available after drop");
        drop(lease);
    }

    #[test]
    fn credential_mutation_blocks_new_sends_and_replacement_requires_an_idle_generation() {
        let database = Database::open_in_memory().expect("database");
        let conversation = database.create_conversation(None).expect("conversation");
        let state = AppState::new(database, ProviderClient::new().expect("provider"));
        state.acknowledge_external_processing();

        let deletion_lease = state
            .reserve_credential_mutation(false)
            .expect("delete-style mutation lease");
        assert!(matches!(
            state.reserve_generation_for_send(&conversation.id),
            Err(AppError::CredentialPromptBusy)
        ));
        drop(deletion_lease);

        let (request_id, cancellation) = state
            .reserve_generation_for_send(&conversation.id)
            .expect("generation reservation");
        assert!(matches!(
            state.reserve_credential_mutation(true),
            Err(AppError::Conflict(_))
        ));
        let deletion_lease = state
            .reserve_credential_mutation(false)
            .expect("deletion serializes against new sends");
        state.cancel_all();
        assert!(cancellation.is_cancelled());
        drop(deletion_lease);
        state.release_generation(&request_id);
    }

    #[test]
    fn credential_prompt_errors_have_stable_safe_categories() {
        let invalid = AppError::CredentialInvalid.public();
        assert_eq!(invalid.code, "credential_invalid");
        assert_eq!(
            invalid.message,
            "The API key must contain 8 to 255 printable ASCII characters without whitespace."
        );
        assert!(!invalid.retryable);
        let unavailable = AppError::CredentialPrompt.public();
        assert_eq!(unavailable.code, "credential_prompt_unavailable");
        assert!(unavailable.retryable);
    }

    #[test]
    fn stream_sequence_is_strictly_monotonic_and_overflow_is_rejected() {
        let started = 0;
        let first_delta = next_stream_sequence(started).expect("first delta");
        let second_delta = next_stream_sequence(first_delta).expect("second delta");
        let terminal = next_stream_sequence(second_delta).expect("terminal");
        assert_eq!([started, first_delta, second_delta, terminal], [0, 1, 2, 3]);
        assert!(matches!(
            next_stream_sequence(u64::MAX),
            Err(AppError::Internal)
        ));
    }

    #[tokio::test]
    async fn shutdown_waits_until_terminal_persistence_releases_the_registry_entry() {
        let database = Database::open_in_memory().expect("database");
        let conversation = database.create_conversation(None).expect("conversation");
        let state = AppState::new(database, ProviderClient::new().expect("provider"));
        let (request_id, _) = state
            .reserve_generation(&conversation.id)
            .expect("reservation");
        assert_eq!(
            state.claim_terminal(&request_id).expect("terminal claim"),
            Some(false)
        );
        assert_eq!(state.request_shutdown(), (true, true));

        let waiter_state = state.clone();
        let waiter = tokio::spawn(async move {
            wait_for_generation_tasks(&waiter_state).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(75)).await;
        assert!(!state.inner.shutdown_ready.load(Ordering::Acquire));
        assert!(!waiter.is_finished());

        state.release_generation(&request_id);
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("shutdown waiter should finish after persistence")
            .expect("shutdown waiter should not panic");
        assert!(state.inner.shutdown_ready.load(Ordering::Acquire));
    }

    #[test]
    fn external_url_policy_allows_public_https_and_rejects_local_or_active_schemes() {
        assert!(validate_external_url("https://docs.z.ai/guides?q=streaming#sse").is_ok());
        for rejected in [
            "http://docs.z.ai/",
            "javascript:alert(1)",
            "https://user:password@example.com/",
            "https://localhost/",
            "https://service.local/",
            "https://intranet/",
            "https://127.0.0.1/",
            "https://[::1]/",
            " https://example.com/",
        ] {
            assert!(
                validate_external_url(rejected).is_err(),
                "URL should be rejected: {rejected}"
            );
        }
        assert!(
            validate_external_url(&format!(
                "https://example.com/{}",
                "x".repeat(MAX_EXTERNAL_URL_BYTES)
            ))
            .is_err()
        );
    }

    #[test]
    fn production_csp_and_capability_are_least_privilege() {
        let config: Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("production CSP");
        assert!(!csp.contains("unsafe-eval"));
        assert!(!csp.contains("https:"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("frame-src 'none'"));
        assert_eq!(config["plugins"], serde_json::json!({}));
        let build_script = include_str!("../build.rs");
        for forbidden in ["open_path", "reveal_item_in_dir", "plugin:opener"] {
            assert!(!build_script.contains(forbidden));
        }
        assert!(build_script.contains("\"prompt_store_api_key\""));
        let retired_secret_command = concat!("\"store_", "api_key\"");
        assert!(!build_script.contains(retired_secret_command));
        let generated_acl = include_str!("../gen/schemas/acl-manifests.json");
        assert!(generated_acl.contains("\"prompt_store_api_key\""));
        assert!(!generated_acl.contains(retired_secret_command));

        let capability: Value = serde_json::from_str(include_str!("../capabilities/main.json"))
            .expect("valid capability");
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(capability["platforms"], serde_json::json!(["windows"]));
        let permissions = capability["permissions"]
            .as_array()
            .expect("permission list")
            .iter()
            .map(|permission| permission.as_str().expect("string permission"))
            .collect::<BTreeSet<_>>();
        let expected = [
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "core:window:allow-minimize",
            "core:window:allow-toggle-maximize",
            "core:window:allow-close",
            "core:window:allow-start-dragging",
            "allow-app-status",
            "allow-credential-status",
            "allow-acknowledge-external-processing",
            "allow-prompt-store-api-key",
            "allow-delete-api-key",
            "allow-list-conversations",
            "allow-get-conversation",
            "allow-create-conversation",
            "allow-rename-conversation",
            "allow-delete-conversation",
            "allow-send-message",
            "allow-cancel-generation",
            "allow-export-conversation",
            "allow-import-conversations",
            "allow-open-external-url",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(permissions, expected);
        let window_permissions = permissions
            .iter()
            .copied()
            .filter(|permission| permission.starts_with("core:window:"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            window_permissions,
            [
                "core:window:allow-close",
                "core:window:allow-minimize",
                "core:window:allow-start-dragging",
                "core:window:allow-toggle-maximize",
            ]
            .into_iter()
            .collect()
        );
        assert!(
            permissions
                .iter()
                .all(|permission| !permission.contains('*'))
        );
        assert!(permissions.iter().all(|permission| {
            !["shell:", "process:", "fs:", "http:", "opener:", "dialog:"]
                .iter()
                .any(|forbidden| permission.starts_with(forbidden))
        }));
    }
}
