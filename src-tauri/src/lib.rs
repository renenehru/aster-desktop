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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use api::{ParsedBalance, PreparedProviderRequest, ProviderClient};
use aster_credential_prompt::{PromptOutcome, PromptProvider};
use chrono::{SecondsFormat, Utc};
use credentials::CredentialStore;
use database::{Database, PreparedGeneration};
use error::{AppError, AppResult, PublicError};
use models::{
    AppStatus, BalanceInfo, Conversation, ConversationSummary, CredentialPromptResult,
    CredentialStatus, DeepSeekBalanceStatus, ExportBundle, ExportResult, ImportBundle,
    MessageStatus, ModelCatalog, ProviderAccountAction, ProviderId, ProviderStatus,
    ResponseProfile, SendMessageResult, StreamEvent, TokenUsage, UsageSummary,
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
const EXPORT_VERSION: u32 = 2;
const MAX_TRANSFER_BYTES: usize = 32 * 1024 * 1024;
const MAX_JSON_NESTING: usize = 8;
const MAX_IPC_BODY_BYTES: usize = 320 * 1024;
const MAX_EXTERNAL_URL_BYTES: usize = 2_048;
const MAX_STREAM_EVENTS: u64 = 65_536;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderArgs {
    provider_id: ProviderId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConversationIdArgs {
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateConversationArgs {
    title: Option<String>,
    provider_id: Option<ProviderId>,
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateConversationSelectionArgs {
    conversation_id: String,
    provider_id: ProviderId,
    model_id: String,
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
    response_profile: ResponseProfile,
    regenerate_from_message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestIdArgs {
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UsageSummaryArgs {
    provider_id: ProviderId,
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetUsageBudgetArgs {
    provider_id: ProviderId,
    token_budget: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderAccountArgs {
    provider_id: ProviderId,
    action: ProviderAccountAction,
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
    provider_reachability: Mutex<HashMap<ProviderId, i8>>,
    balance: Mutex<BalanceMemory>,
    balance_credential_epoch: AtomicU64,
    balance_operation: AtomicU64,
    shutdown_started: AtomicBool,
    shutdown_ready: AtomicBool,
    credential_prompt_active: AtomicBool,
}

struct ActiveGeneration {
    conversation_id: String,
    provider_id: ProviderId,
    model_id: String,
    cancellation: CancellationToken,
    terminal_claimed: bool,
}

struct GenerationTask {
    request_id: String,
    conversation_id: String,
    provider_id: ProviderId,
    model_id: String,
    provider_request: PreparedProviderRequest,
    api_key: Zeroizing<String>,
    cancellation: CancellationToken,
}

#[derive(Clone)]
struct BalanceSnapshot {
    observed_at: String,
    is_available: bool,
    balance_infos: Vec<BalanceInfo>,
}

#[derive(Default)]
struct BalanceMemory {
    success: Option<BalanceSnapshot>,
    error: Option<PublicError>,
}

#[derive(Clone, Copy)]
struct BalanceAuthority {
    credential_epoch: u64,
    operation: u64,
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
        let provider_reachability = ProviderId::ALL
            .into_iter()
            .map(|provider| (provider, 0))
            .collect();
        Self {
            inner: Arc::new(AppStateInner {
                database,
                credentials: CredentialStore,
                provider,
                active: Mutex::new(HashMap::new()),
                provider_reachability: Mutex::new(provider_reachability),
                balance: Mutex::new(BalanceMemory::default()),
                balance_credential_epoch: AtomicU64::new(0),
                balance_operation: AtomicU64::new(0),
                shutdown_started: AtomicBool::new(false),
                shutdown_ready: AtomicBool::new(false),
                credential_prompt_active: AtomicBool::new(false),
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
        provider_id: ProviderId,
    ) -> AppResult<CredentialMutationLease> {
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if active
            .values()
            .any(|generation| generation.provider_id == provider_id)
        {
            return Err(AppError::Conflict(
                "Stop active generation for this provider before changing its API key.",
            ));
        }
        self.inner
            .credential_prompt_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| AppError::CredentialPromptBusy)?;
        drop(active);
        Ok(CredentialMutationLease {
            inner: self.inner.clone(),
        })
    }

    fn reserve_generation(
        &self,
        conversation_id: &str,
    ) -> AppResult<(String, CancellationToken, Conversation)> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        if self.inner.shutdown_started.load(Ordering::Acquire) {
            return Err(AppError::Conflict("Aster is shutting down."));
        }
        let mut active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if active
            .values()
            .any(|generation| generation.conversation_id == conversation_id)
        {
            return Err(AppError::Conflict(
                "This conversation already has an active generation.",
            ));
        }
        if self.inner.credential_prompt_active.load(Ordering::Acquire) {
            return Err(AppError::CredentialPromptBusy);
        }
        let conversation = self.database().get_conversation(conversation_id)?;
        let request_id = Uuid::new_v4().to_string();
        let cancellation = CancellationToken::new();
        active.insert(
            request_id.clone(),
            ActiveGeneration {
                conversation_id: conversation_id.to_owned(),
                provider_id: conversation.provider_id,
                model_id: conversation.model_id.clone(),
                cancellation: cancellation.clone(),
                terminal_claimed: false,
            },
        );
        Ok((request_id, cancellation, conversation))
    }

    fn release_generation(&self, request_id: &str) {
        if let Ok(mut active) = self.inner.active.lock() {
            active.remove(request_id);
        }
    }

    fn validate_and_start_generation<T>(
        &self,
        request_id: &str,
        conversation_id: &str,
        provider_id: ProviderId,
        model_id: &str,
        start: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        {
            let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
            let generation = active.get(request_id).ok_or(AppError::Internal)?;
            if generation.terminal_claimed
                || generation.conversation_id != conversation_id
                || generation.provider_id != provider_id
                || generation.model_id != model_id
            {
                return Err(AppError::Internal);
            }
            if generation.cancellation.is_cancelled() {
                return Err(AppError::Cancelled);
            }
        }

        // Do not hold `active` while emitting `started`: the renderer must be
        // able to call the real cancellation path from that event. The
        // preflight's post-emission check is the final pre-attempt boundary.
        // Once it succeeds, the database marker is committed and the request
        // task is spawned even if cancellation arrives immediately afterward.
        start()
    }

    fn claim_terminal(
        &self,
        request_id: &str,
        conversation_id: &str,
        provider_id: ProviderId,
        model_id: &str,
    ) -> AppResult<Option<bool>> {
        let mut active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        let Some(generation) = active.get_mut(request_id) else {
            return Ok(None);
        };
        if generation.terminal_claimed {
            return Ok(None);
        }
        if generation.conversation_id != conversation_id
            || generation.provider_id != provider_id
            || generation.model_id != model_id
        {
            return Err(AppError::Internal);
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

    fn update_conversation_selection(
        &self,
        conversation_id: &str,
        provider_id: ProviderId,
        model_id: String,
    ) -> AppResult<Conversation> {
        let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if active
            .values()
            .any(|generation| generation.conversation_id == conversation_id)
        {
            return Err(AppError::Conflict(
                "Stop the active generation before changing this conversation.",
            ));
        }
        let result =
            self.database()
                .update_conversation_selection(conversation_id, provider_id, model_id);
        drop(active);
        result
    }

    fn delete_conversation(&self, conversation_id: &str) -> AppResult<()> {
        let active = self.inner.active.lock().map_err(|_| AppError::Internal)?;
        if active
            .values()
            .any(|generation| generation.conversation_id == conversation_id)
        {
            return Err(AppError::Conflict(
                "Stop the active generation before deleting this conversation.",
            ));
        }
        let result = self.database().delete_conversation(conversation_id);
        drop(active);
        result
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

    fn reachability(&self, provider_id: ProviderId) -> &'static str {
        match self
            .inner
            .provider_reachability
            .lock()
            .ok()
            .and_then(|values| values.get(&provider_id).copied())
            .unwrap_or(0)
        {
            1 => "reachable",
            -1 => "unreachable",
            _ => "unknown",
        }
    }

    fn record_provider_result<T>(&self, provider_id: ProviderId, result: &AppResult<T>) {
        if let Some(value) = Self::provider_result_reachability(result)
            && let Ok(mut values) = self.inner.provider_reachability.lock()
        {
            values.insert(provider_id, value);
        }
    }

    fn provider_result_reachability<T>(result: &AppResult<T>) -> Option<i8> {
        match result {
            Ok(_) => Some(1),
            Err(error) => Self::provider_error_reachability(error),
        }
    }

    fn provider_error_reachability(error: &AppError) -> Option<i8> {
        match error {
            AppError::Network | AppError::Timeout => Some(-1),
            AppError::Cancelled => None,
            _ => Some(1),
        }
    }

    fn balance_status(&self) -> AppResult<DeepSeekBalanceStatus> {
        let balance = self.inner.balance.lock().map_err(|_| AppError::Internal)?;
        Ok(match (&balance.success, &balance.error) {
            (None, None) => DeepSeekBalanceStatus::not_checked(),
            (None, Some(error)) => DeepSeekBalanceStatus {
                status: "error",
                observed_at: None,
                is_available: None,
                balance_infos: Vec::new(),
                error: Some(error.clone()),
            },
            (Some(success), None) => DeepSeekBalanceStatus {
                status: "current",
                observed_at: Some(success.observed_at.clone()),
                is_available: Some(success.is_available),
                balance_infos: success.balance_infos.clone(),
                error: None,
            },
            (Some(success), Some(error)) => DeepSeekBalanceStatus {
                status: "stale",
                observed_at: Some(success.observed_at.clone()),
                is_available: Some(success.is_available),
                balance_infos: success.balance_infos.clone(),
                error: Some(error.clone()),
            },
        })
    }

    fn begin_balance_refresh(&self) -> AppResult<BalanceAuthority> {
        let balance = self.inner.balance.lock().map_err(|_| AppError::Internal)?;
        let credential_epoch = self.inner.balance_credential_epoch.load(Ordering::Acquire);
        let operation = self.inner.balance_operation.fetch_add(1, Ordering::AcqRel) + 1;
        let authority = BalanceAuthority {
            credential_epoch,
            operation,
        };
        drop(balance);
        Ok(authority)
    }

    fn invalidate_deepseek_balance(&self) -> AppResult<()> {
        let mut balance = self.inner.balance.lock().map_err(|_| AppError::Internal)?;
        self.inner
            .balance_credential_epoch
            .fetch_add(1, Ordering::AcqRel);
        self.inner.balance_operation.fetch_add(1, Ordering::AcqRel);
        *balance = BalanceMemory::default();
        self.inner
            .provider_reachability
            .lock()
            .map_err(|_| AppError::Internal)?
            .insert(ProviderId::DeepSeek, 0);
        drop(balance);
        Ok(())
    }

    fn authority_is_current(&self, authority: BalanceAuthority) -> bool {
        self.inner.balance_credential_epoch.load(Ordering::Acquire) == authority.credential_epoch
            && self.inner.balance_operation.load(Ordering::Acquire) == authority.operation
    }

    fn record_balance_success(
        &self,
        authority: BalanceAuthority,
        parsed: ParsedBalance,
    ) -> AppResult<DeepSeekBalanceStatus> {
        let mut balance = self.inner.balance.lock().map_err(|_| AppError::Internal)?;
        if !self.authority_is_current(authority) {
            drop(balance);
            return self.balance_status();
        }
        self.inner
            .provider_reachability
            .lock()
            .map_err(|_| AppError::Internal)?
            .insert(ProviderId::DeepSeek, 1);
        balance.success = Some(BalanceSnapshot {
            observed_at: now_utc(),
            is_available: parsed.is_available,
            balance_infos: parsed.balance_infos,
        });
        balance.error = None;
        drop(balance);
        self.balance_status()
    }

    fn record_balance_error(
        &self,
        authority: BalanceAuthority,
        error: &AppError,
        clear_success: bool,
        network_attempted: bool,
    ) -> AppResult<DeepSeekBalanceStatus> {
        let mut balance = self.inner.balance.lock().map_err(|_| AppError::Internal)?;
        if !self.authority_is_current(authority) {
            drop(balance);
            return self.balance_status();
        }
        if network_attempted && let Some(value) = Self::provider_error_reachability(error) {
            self.inner
                .provider_reachability
                .lock()
                .map_err(|_| AppError::Internal)?
                .insert(ProviderId::DeepSeek, value);
        }
        if clear_success {
            balance.success = None;
        }
        balance.error = Some(error.public());
        drop(balance);
        self.balance_status()
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

fn prompt_provider(provider: ProviderId) -> PromptProvider {
    match provider {
        ProviderId::Zai => PromptProvider::Zai,
        ProviderId::DeepSeek => PromptProvider::DeepSeek,
        ProviderId::AlibabaUs => PromptProvider::AlibabaUs,
        ProviderId::Google => PromptProvider::Google,
        ProviderId::Nvidia => PromptProvider::Nvidia,
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
    _state: State<'_, AppState>,
) -> AppResult<AppStatus> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    Ok(AppStatus {
        mode: "desktop",
        version: env!("CARGO_PKG_VERSION"),
        // Aster does not perform a global connectivity probe. Provider-specific
        // reachability changes only after a real provider request.
        online: false,
        database_ready: true,
    })
}

#[tauri::command]
fn model_catalog(request: Request<'_>, window: WebviewWindow) -> AppResult<ModelCatalog> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    Ok(api::model_catalog())
}

#[tauri::command]
fn provider_statuses(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<Vec<ProviderStatus>> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    ProviderId::ALL
        .into_iter()
        .map(|provider_id| {
            let configured = state.credentials().status(provider_id)?.configured;
            let notice_version = api::provider_notice_version(provider_id);
            Ok(ProviderStatus {
                provider_id,
                configured,
                reachability: state.reachability(provider_id),
                notice_version,
                notice_acknowledged: state
                    .database()
                    .notice_acknowledged(provider_id, notice_version)?,
            })
        })
        .collect()
}

#[tauri::command]
fn acknowledge_external_processing(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<()> {
    ensure_main_window(&window)?;
    let arguments: ProviderArgs = parse_ipc_request(&request, &["providerId"])?;
    state.database().acknowledge_notice(
        arguments.provider_id,
        api::provider_notice_version(arguments.provider_id),
    )
}

#[tauri::command]
async fn prompt_store_api_key(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<CredentialPromptResult> {
    ensure_main_window(&window)?;
    let arguments: ProviderArgs = parse_ipc_request(&request, &["providerId"])?;
    let state = state.inner().clone();
    let lease = state.reserve_credential_mutation(arguments.provider_id)?;
    let owner_window = main_window_owner_handle(&window)?;
    let provider_id = arguments.provider_id;
    let native_provider = prompt_provider(provider_id);
    let (lease, prompt_result) = tauri::async_runtime::spawn_blocking(move || {
        let result = aster_credential_prompt::prompt_api_key(owner_window, native_provider);
        (lease, result)
    })
    .await
    .map_err(|_| AppError::CredentialPrompt)?;
    let result = match prompt_result {
        Ok(PromptOutcome::Submitted(api_key)) => state
            .credentials()
            .store(provider_id, api_key)
            .and_then(|status| {
                if provider_id == ProviderId::DeepSeek {
                    state.invalidate_deepseek_balance()?;
                }
                Ok(CredentialPromptResult {
                    provider_id,
                    configured: status.configured,
                    source: status.source,
                    cancelled: false,
                })
            }),
        Ok(PromptOutcome::Cancelled) => {
            state
                .credentials()
                .status(provider_id)
                .map(|status| CredentialPromptResult {
                    provider_id,
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
    let arguments: ProviderArgs = parse_ipc_request(&request, &["providerId"])?;
    let lease = state.reserve_credential_mutation(arguments.provider_id)?;
    let result = state
        .credentials()
        .delete(arguments.provider_id)
        .and_then(|status| {
            if arguments.provider_id == ProviderId::DeepSeek {
                state.invalidate_deepseek_balance()?;
            }
            Ok(status)
        });
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
    let arguments: CreateConversationArgs =
        parse_ipc_request(&request, &["title", "providerId", "modelId"])?;
    state
        .database()
        .create_conversation(arguments.title, arguments.provider_id, arguments.model_id)
}

#[tauri::command]
fn update_conversation_selection(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<Conversation> {
    ensure_main_window(&window)?;
    let arguments: UpdateConversationSelectionArgs =
        parse_ipc_request(&request, &["conversationId", "providerId", "modelId"])?;
    state.update_conversation_selection(
        &arguments.conversation_id,
        arguments.provider_id,
        arguments.model_id,
    )
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
    state.delete_conversation(&arguments.conversation_id)
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
            "responseProfile",
            "regenerateFromMessageId",
        ],
    )?;
    let state = state.inner().clone();
    state.database().validate_generation_request(
        &arguments.conversation_id,
        &arguments.content,
        arguments.regenerate_from_message_id.as_deref(),
    )?;
    let (request_id, cancellation, conversation) =
        state.reserve_generation(&arguments.conversation_id)?;
    let notice_version = api::provider_notice_version(conversation.provider_id);
    let notice_acknowledged = match state
        .database()
        .notice_acknowledged(conversation.provider_id, notice_version)
    {
        Ok(value) => value,
        Err(error) => {
            state.release_generation(&request_id);
            return Err(error);
        }
    };
    if !notice_acknowledged {
        state.release_generation(&request_id);
        return Err(AppError::ExternalProcessingNoticeRequired);
    }
    let api_key = match state.credentials().load(conversation.provider_id) {
        Ok(api_key) => api_key,
        Err(error) => {
            state.release_generation(&request_id);
            return Err(error);
        }
    };
    let prepared = match state.validate_and_start_generation(
        &request_id,
        &conversation.id,
        conversation.provider_id,
        &conversation.model_id,
        || {
            state.database().prepare_generation(
                &request_id,
                &conversation.id,
                arguments.content,
                arguments.response_profile,
                arguments.regenerate_from_message_id,
                |prepared| {
                    if prepared.provider_id != conversation.provider_id
                        || prepared.model_id != conversation.model_id
                    {
                        return Err(AppError::DatabaseIntegrity);
                    }
                    if cancellation.is_cancelled() {
                        return Err(AppError::Cancelled);
                    }
                    let provider_request = state.provider().prepare_chat(
                        prepared.provider_id,
                        &prepared.model_id,
                        prepared.response_profile,
                        &prepared.history,
                    )?;
                    complete_generation_preflight(&cancellation, || {
                        emit_stream_event(
                            &window,
                            StreamEvent {
                                request_id: request_id.clone(),
                                conversation_id: conversation.id.clone(),
                                provider_id: prepared.provider_id,
                                model_id: prepared.model_id.clone(),
                                sequence: 0,
                                kind: "started",
                                delta: None,
                                message: None,
                                error: None,
                                error_code: None,
                                retryable: None,
                            },
                        )
                    })?;
                    Ok(provider_request)
                },
            )
        },
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            state.release_generation(&request_id);
            return Err(error);
        }
    };
    let (prepared, provider_request) = prepared;
    let task_request_id = request_id.clone();
    let PreparedGeneration {
        provider_id,
        model_id,
        ..
    } = prepared;
    tauri::async_runtime::spawn(run_generation(
        window,
        state,
        GenerationTask {
            request_id: task_request_id,
            conversation_id: conversation.id,
            provider_id,
            model_id,
            provider_request,
            api_key,
            cancellation,
        },
    ));
    Ok(SendMessageResult { request_id })
}

async fn run_generation(window: WebviewWindow, state: AppState, task: GenerationTask) {
    let GenerationTask {
        request_id,
        conversation_id,
        provider_id,
        model_id,
        provider_request,
        api_key,
        cancellation,
    } = task;
    let mut sequence = 0u64;
    let mut content = String::new();
    let result = state
        .provider()
        .stream_prepared_chat(provider_request, api_key.as_str(), &cancellation, |delta| {
            if cancellation.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            accept_stream_delta(&mut sequence, &mut content, &delta)?;
            emit_stream_event(
                &window,
                StreamEvent {
                    request_id: request_id.clone(),
                    conversation_id: conversation_id.clone(),
                    provider_id,
                    model_id: model_id.clone(),
                    sequence,
                    kind: "delta",
                    delta: Some(delta),
                    message: None,
                    error: None,
                    error_code: None,
                    retryable: None,
                },
            )
        })
        .await;
    drop(api_key);
    state.record_provider_result(provider_id, &result);
    let cancelled =
        match state.claim_terminal(&request_id, &conversation_id, provider_id, &model_id) {
            Ok(Some(value)) => value || matches!(&result, Err(AppError::Cancelled)),
            Ok(None) | Err(_) => return,
        };
    let terminal_sequence = match next_terminal_stream_sequence(sequence) {
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
            provider_id,
            &model_id,
            content,
            MessageStatus::Cancelled,
            None,
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
            provider_id,
            &model_id,
            content,
            MessageStatus::Complete,
            Some(outcome.finish_reason),
            outcome.usage,
            None,
            terminal_sequence,
        ),
        Err(error) => {
            if matches!(
                error,
                AppError::ProviderContentRejected | AppError::UnsupportedProviderCapability
            ) {
                content.clear();
            }
            emit_persisted_terminal(
                &window,
                &state,
                &request_id,
                &conversation_id,
                provider_id,
                &model_id,
                content,
                MessageStatus::Error,
                None,
                None,
                Some(error),
                terminal_sequence,
            );
        }
    }
    state.release_generation(&request_id);
}

#[allow(clippy::too_many_arguments)]
fn emit_persisted_terminal(
    window: &WebviewWindow,
    state: &AppState,
    request_id: &str,
    conversation_id: &str,
    provider_id: ProviderId,
    model_id: &str,
    content: String,
    status: MessageStatus,
    finish_reason: Option<models::MessageFinishReason>,
    usage: Option<TokenUsage>,
    source_error: Option<AppError>,
    sequence: u64,
) {
    match state.database().persist_generation_terminal(
        request_id,
        conversation_id,
        content,
        status,
        finish_reason,
        usage,
    ) {
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
                    provider_id,
                    model_id: model_id.to_owned(),
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
                    provider_id,
                    model_id: model_id.to_owned(),
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

fn complete_generation_preflight(
    cancellation: &CancellationToken,
    emit_started: impl FnOnce() -> AppResult<()>,
) -> AppResult<()> {
    if cancellation.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    emit_started()?;
    if cancellation.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    Ok(())
}

fn next_stream_sequence(sequence: u64) -> AppResult<u64> {
    sequence.checked_add(1).ok_or(AppError::Internal)
}

fn next_delta_stream_sequence(sequence: u64) -> AppResult<u64> {
    let next = next_stream_sequence(sequence)?;
    if next >= MAX_STREAM_EVENTS - 1 {
        return Err(AppError::MalformedStream);
    }
    Ok(next)
}

fn accept_stream_delta(sequence: &mut u64, content: &mut String, delta: &str) -> AppResult<()> {
    let next = next_delta_stream_sequence(*sequence)?;
    content.push_str(delta);
    *sequence = next;
    Ok(())
}

fn next_terminal_stream_sequence(sequence: u64) -> AppResult<u64> {
    let next = next_stream_sequence(sequence)?;
    if next >= MAX_STREAM_EVENTS {
        return Err(AppError::MalformedStream);
    }
    Ok(next)
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
fn usage_summary(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<UsageSummary> {
    ensure_main_window(&window)?;
    let arguments: UsageSummaryArgs = parse_ipc_request(&request, &["providerId", "modelId"])?;
    state
        .database()
        .usage_summary(arguments.provider_id, arguments.model_id.as_deref())
}

#[tauri::command]
fn set_usage_budget(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<UsageSummary> {
    ensure_main_window(&window)?;
    let arguments: SetUsageBudgetArgs =
        parse_ipc_request(&request, &["providerId", "tokenBudget"])?;
    state
        .database()
        .set_usage_budget(arguments.provider_id, arguments.token_budget)?;
    state.database().usage_summary(arguments.provider_id, None)
}

#[tauri::command]
fn deepseek_balance_status(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<DeepSeekBalanceStatus> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    state.balance_status()
}

#[tauri::command]
async fn refresh_deepseek_balance(
    request: Request<'_>,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> AppResult<DeepSeekBalanceStatus> {
    ensure_main_window(&window)?;
    validate_ipc_request(&request, &[])?;
    let state = state.inner().clone();
    let authority = state.begin_balance_refresh()?;
    let api_key = match state.credentials().load(ProviderId::DeepSeek) {
        Ok(api_key) => api_key,
        Err(error) => return state.record_balance_error(authority, &error, true, false),
    };
    let cancellation = CancellationToken::new();
    let result = state
        .provider()
        .refresh_deepseek_balance(api_key.as_str(), &cancellation)
        .await;
    drop(api_key);
    match result {
        Ok(parsed) => state.record_balance_success(authority, parsed),
        Err(error) => state.record_balance_error(authority, &error, false, true),
    }
}

#[tauri::command]
fn open_provider_account(request: Request<'_>, window: WebviewWindow) -> AppResult<()> {
    ensure_main_window(&window)?;
    let arguments: ProviderAccountArgs = parse_ipc_request(&request, &["providerId", "action"])?;
    let url = api::account_url(arguments.provider_id, arguments.action)?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|_| AppError::ExternalNavigation)
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
            model_catalog,
            provider_statuses,
            acknowledge_external_processing,
            prompt_store_api_key,
            delete_api_key,
            list_conversations,
            get_conversation,
            create_conversation,
            update_conversation_selection,
            rename_conversation,
            delete_conversation,
            send_message,
            cancel_generation,
            usage_summary,
            set_usage_budget,
            deepseek_balance_status,
            refresh_deepseek_balance,
            open_provider_account,
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
    use super::*;

    const APPLICATION_PERMISSIONS: [&str; 22] = [
        "allow-app-status",
        "allow-model-catalog",
        "allow-provider-statuses",
        "allow-acknowledge-external-processing",
        "allow-prompt-store-api-key",
        "allow-delete-api-key",
        "allow-list-conversations",
        "allow-get-conversation",
        "allow-create-conversation",
        "allow-update-conversation-selection",
        "allow-rename-conversation",
        "allow-delete-conversation",
        "allow-send-message",
        "allow-cancel-generation",
        "allow-usage-summary",
        "allow-set-usage-budget",
        "allow-deepseek-balance-status",
        "allow-refresh-deepseek-balance",
        "allow-open-provider-account",
        "allow-export-conversation",
        "allow-import-conversations",
        "allow-open-external-url",
    ];

    #[test]
    fn capability_and_csp_are_exact_least_privilege_boundaries() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let permissions = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|permission| permission.starts_with("allow-"))
            .collect::<Vec<_>>();
        assert_eq!(permissions, APPLICATION_PERMISSIONS);
        let serialized_capability = capability.to_string();
        for forbidden in [
            "credential-status",
            "opener:",
            "shell:",
            "http:",
            "fs:",
            "dialog:",
            "process:",
            "*",
        ] {
            assert!(!serialized_capability.contains(forbidden), "{forbidden}");
        }

        let configuration: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(configuration["version"], "0.2.0");
        assert_eq!(configuration["build"]["removeUnusedCommands"], true);
        assert_eq!(configuration["app"]["windows"][0]["devtools"], false);
        let csp = configuration["app"]["security"]["csp"].as_str().unwrap();
        assert!(csp.contains("connect-src 'self' ipc: http://ipc.localhost;"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("frame-src 'none'"));
        assert!(!csp.contains("unsafe-eval"));
        assert!(!csp.contains("https:"));

        let build_manifest = include_str!("../build.rs");
        for permission in APPLICATION_PERMISSIONS {
            let command = permission.trim_start_matches("allow-").replace('-', "_");
            assert!(build_manifest.contains(&format!("\"{command}\"")));
        }
        assert!(!build_manifest.contains("credential_status"));
    }

    #[test]
    fn raw_ipc_rejects_unknown_keys_wrong_types_and_unsafe_budget_numbers() {
        assert!(
            parse_ipc_bytes::<ProviderArgs>(br#"{"providerId":"zai"}"#, &["providerId"]).is_ok()
        );
        assert!(
            parse_ipc_bytes::<ProviderArgs>(br#"{"providerId":"ZAI"}"#, &["providerId"]).is_err()
        );
        assert!(
            parse_ipc_bytes::<ProviderArgs>(
                br#"{"providerId":"zai","url":"https://evil.example"}"#,
                &["providerId"]
            )
            .is_err()
        );
        assert!(
            parse_ipc_bytes::<SetUsageBudgetArgs>(
                br#"{"providerId":"zai","tokenBudget":1.5}"#,
                &["providerId", "tokenBudget"]
            )
            .is_err()
        );
        assert!(
            parse_ipc_bytes::<SetUsageBudgetArgs>(
                br#"{"providerId":"zai","tokenBudget":9007199254740992}"#,
                &["providerId", "tokenBudget"]
            )
            .is_ok()
        );
        // The database semantic boundary rejects the syntactically valid out-of-range u64.
        let database = Database::open_in_memory().unwrap();
        assert!(
            database
                .set_usage_budget(ProviderId::Zai, Some(9_007_199_254_740_992))
                .is_err()
        );
    }

    #[test]
    fn active_generation_binds_conversation_provider_and_model() {
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            ProviderClient::new().unwrap(),
        );
        let conversation = state
            .database()
            .create_conversation(
                None,
                Some(ProviderId::Google),
                Some("gemini-2.5-pro".to_owned()),
            )
            .unwrap();
        let (request_id, _, _) = state.reserve_generation(&conversation.id).unwrap();
        let active = state.inner.active.lock().unwrap();
        let generation = active.get(&request_id).unwrap();
        assert_eq!(generation.provider_id, ProviderId::Google);
        assert_eq!(generation.model_id, "gemini-2.5-pro");
    }

    #[test]
    fn cancellation_from_started_callback_rolls_back_before_request_attempt() {
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            ProviderClient::new().unwrap(),
        );
        let conversation = state
            .database()
            .create_conversation(None, None, None)
            .unwrap();
        let (request_id, cancellation, reserved) =
            state.reserve_generation(&conversation.id).unwrap();
        let cancelling_state = state.clone();
        let cancelling_request = request_id.clone();
        let task_would_spawn = AtomicBool::new(false);

        let result = state.validate_and_start_generation(
            &request_id,
            &reserved.id,
            reserved.provider_id,
            &reserved.model_id,
            || {
                state.database().prepare_generation(
                    &request_id,
                    &reserved.id,
                    "Cancel from started".to_owned(),
                    ResponseProfile::Standard,
                    None,
                    |_| {
                        complete_generation_preflight(&cancellation, || {
                            cancelling_state.cancel_generation(&cancelling_request)
                        })
                    },
                )
            },
        );
        if result.is_ok() {
            task_would_spawn.store(true, Ordering::Release);
        }

        assert!(matches!(result, Err(AppError::Cancelled)));
        assert!(!task_would_spawn.load(Ordering::Acquire));
        assert!(
            state
                .database()
                .get_conversation(&conversation.id)
                .unwrap()
                .messages
                .is_empty()
        );
        assert_eq!(
            state
                .database()
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .coverage,
            "empty"
        );
        state.release_generation(&request_id);
    }

    #[test]
    fn provider_account_ipc_has_no_model_or_url_field() {
        assert!(
            parse_ipc_bytes::<ProviderAccountArgs>(
                br#"{"providerId":"deepseek","action":"addCredits"}"#,
                &["providerId", "action"]
            )
            .is_ok()
        );
        assert!(
            parse_ipc_bytes::<ProviderAccountArgs>(
                br#"{"providerId":"deepseek","action":"addCredits","url":"https://evil.example"}"#,
                &["providerId", "action"]
            )
            .is_err()
        );
    }

    #[test]
    fn external_content_links_reject_local_targets_and_credentials() {
        assert!(validate_external_url("https://example.com/path").is_ok());
        for value in [
            "http://example.com",
            "https://localhost/test",
            "https://127.0.0.1/test",
            "https://user:password@example.com/",
            "javascript:alert(1)",
        ] {
            assert!(validate_external_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn balance_memory_is_not_checked_then_current_then_stale() {
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            ProviderClient::new().unwrap(),
        );
        assert_eq!(state.balance_status().unwrap().status, "notChecked");
        let success_authority = state.begin_balance_refresh().unwrap();
        let current = state
            .record_balance_success(success_authority, fixture_balance("1"))
            .unwrap();
        assert_eq!(current.status, "current");
        let error_authority = state.begin_balance_refresh().unwrap();
        let stale = state
            .record_balance_error(error_authority, &AppError::Network, false, true)
            .unwrap();
        assert_eq!(stale.status, "stale");
        assert_eq!(stale.balance_infos.len(), 1);
    }

    #[test]
    fn credential_mutation_invalidates_balance_and_rejects_old_completion() {
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            ProviderClient::new().unwrap(),
        );
        let old_authority = state.begin_balance_refresh().unwrap();
        assert_eq!(
            state
                .record_balance_success(old_authority, fixture_balance("1"))
                .unwrap()
                .status,
            "current"
        );
        assert_eq!(state.reachability(ProviderId::DeepSeek), "reachable");

        state.invalidate_deepseek_balance().unwrap();
        assert_eq!(state.balance_status().unwrap().status, "notChecked");
        assert_eq!(state.reachability(ProviderId::DeepSeek), "unknown");
        assert_eq!(
            state
                .record_balance_success(old_authority, fixture_balance("999"))
                .unwrap()
                .status,
            "notChecked"
        );
        assert_eq!(state.reachability(ProviderId::DeepSeek), "unknown");

        let new_authority = state.begin_balance_refresh().unwrap();
        let current = state
            .record_balance_success(new_authority, fixture_balance("2"))
            .unwrap();
        assert_eq!(current.status, "current");
        assert_eq!(current.balance_infos[0].total_balance, "2");
        assert_eq!(state.reachability(ProviderId::DeepSeek), "reachable");
    }

    #[test]
    fn latest_balance_refresh_wins_and_credential_load_error_clears_success() {
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            ProviderClient::new().unwrap(),
        );
        let older = state.begin_balance_refresh().unwrap();
        let newer = state.begin_balance_refresh().unwrap();
        assert_eq!(
            state
                .record_balance_success(older, fixture_balance("1"))
                .unwrap()
                .status,
            "notChecked"
        );
        assert_eq!(
            state
                .record_balance_success(newer, fixture_balance("2"))
                .unwrap()
                .balance_infos[0]
                .total_balance,
            "2"
        );

        let missing_credential = state.begin_balance_refresh().unwrap();
        let error = state
            .record_balance_error(
                missing_credential,
                &AppError::CredentialNotConfigured,
                true,
                false,
            )
            .unwrap();
        assert_eq!(error.status, "error");
        assert!(error.balance_infos.is_empty());
        assert_eq!(
            error.error.expect("credential error").code,
            "credential_not_configured"
        );
    }

    #[test]
    fn balance_begin_is_serialized_with_authority_check_and_commit() {
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            ProviderClient::new().unwrap(),
        );
        let older = state.begin_balance_refresh().unwrap();
        let balance_guard = state.inner.balance.lock().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let concurrent_state = state.clone();
        let worker = std::thread::spawn(move || {
            sender
                .send(concurrent_state.begin_balance_refresh())
                .unwrap();
        });

        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        assert!(state.authority_is_current(older));
        drop(balance_guard);

        let newer = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        worker.join().unwrap();
        assert!(!state.authority_is_current(older));
        assert!(state.authority_is_current(newer));
        assert_eq!(
            state
                .record_balance_success(older, fixture_balance("stale"))
                .unwrap()
                .status,
            "notChecked"
        );
        assert_eq!(state.reachability(ProviderId::DeepSeek), "unknown");
    }

    #[test]
    fn stream_event_ceiling_reserves_one_terminal_event() {
        let mut sequence = 0;
        let mut content = String::new();
        for _ in 0..(MAX_STREAM_EVENTS - 2) {
            accept_stream_delta(&mut sequence, &mut content, "x").unwrap();
        }

        assert_eq!(sequence, MAX_STREAM_EVENTS - 2);
        assert_eq!(content.len() as u64, MAX_STREAM_EVENTS - 2);
        assert_eq!(
            next_terminal_stream_sequence(sequence).unwrap(),
            MAX_STREAM_EVENTS - 1
        );
        assert!(matches!(
            accept_stream_delta(&mut sequence, &mut content, "rejected"),
            Err(AppError::MalformedStream)
        ));
        assert_eq!(sequence, MAX_STREAM_EVENTS - 2);
        assert_eq!(content.len() as u64, MAX_STREAM_EVENTS - 2);
    }

    fn fixture_balance(total: &str) -> ParsedBalance {
        ParsedBalance {
            is_available: true,
            balance_infos: vec![BalanceInfo {
                currency: "USD".to_owned(),
                total_balance: total.to_owned(),
                granted_balance: total.to_owned(),
                topped_up_balance: "0".to_owned(),
            }],
        }
    }
}
