mod schema;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

use crate::api::validate_selection;
use crate::error::{AppError, AppResult};
use crate::models::{
    AdvisoryBudget, Conversation, ConversationSummary, ImportBundle, ImportBundleV1,
    ImportBundleV2, MAX_SAFE_INTEGER, Message, MessageFinishReason, MessageRole, MessageStatus,
    ProviderId, ProviderMessage, ResponseProfile, TokenUsage, UsageSummary,
};

pub const DEFAULT_PROVIDER: ProviderId = ProviderId::Zai;
pub const DEFAULT_MODEL: &str = "glm-5.1";
const DEFAULT_TITLE: &str = "New conversation";
const MAX_TITLE_CHARS: usize = 80;
const MAX_USER_MESSAGE_BYTES: usize = 256 * 1024;
const MAX_STORED_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PROVIDER_HISTORY_BYTES: usize = 512 * 1024;
const MAX_PROVIDER_HISTORY_MESSAGES: usize = 200;
const MAX_IMPORT_CONVERSATIONS: usize = 100;
const MAX_IMPORT_MESSAGES: usize = 10_000;
const MAX_MESSAGES_PER_CONVERSATION: usize = 10_000;

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

pub struct PreparedGeneration {
    pub history: Vec<ProviderMessage>,
    pub provider_id: ProviderId,
    pub model_id: String,
    pub response_profile: ResponseProfile,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        Ok(Self {
            connection: Arc::new(Mutex::new(schema::open_path(path)?)),
        })
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> AppResult<Self> {
        Ok(Self {
            connection: Arc::new(Mutex::new(schema::open_memory()?)),
        })
    }

    fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| AppError::Internal)
    }

    pub fn create_conversation(
        &self,
        title: Option<String>,
        provider_id: Option<ProviderId>,
        model_id: Option<String>,
    ) -> AppResult<Conversation> {
        let (provider_id, model_id) = match (provider_id, model_id) {
            (None, None) => (DEFAULT_PROVIDER, DEFAULT_MODEL.to_owned()),
            (Some(provider), Some(model)) => {
                validate_selection(provider, &model)?;
                (provider, model)
            }
            _ => {
                return Err(AppError::Validation(
                    "Provider and model must be supplied together.",
                ));
            }
        };
        let title = normalize_title(title.as_deref().unwrap_or(DEFAULT_TITLE))?;
        let id = Uuid::new_v4().to_string();
        let now = now_utc();
        self.lock()?.execute(
            "INSERT INTO conversations
             (id,title,provider_id,model_id,response_profile,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?6)",
            params![
                id,
                title,
                provider_id.as_str(),
                model_id,
                ResponseProfile::Standard.as_str(),
                now
            ],
        )?;
        self.get_conversation(&id)
    }

    pub fn update_conversation_selection(
        &self,
        conversation_id: &str,
        provider_id: ProviderId,
        model_id: String,
    ) -> AppResult<Conversation> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        validate_selection(provider_id, &model_id)?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let current = transaction
            .query_row(
                "SELECT provider_id,model_id,
                        (SELECT COUNT(*) FROM messages WHERE conversation_id=conversations.id)
                 FROM conversations WHERE id=?1",
                [conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(AppError::NotFound("Conversation not found."))?;
        if current.0 == provider_id.as_str() && current.1 == model_id {
            transaction.commit()?;
            drop(connection);
            return self.get_conversation(conversation_id);
        }
        if current.2 != 0 {
            return Err(AppError::ConversationModelLocked);
        }
        transaction.execute(
            "UPDATE conversations SET provider_id=?1,model_id=?2,updated_at=?3 WHERE id=?4",
            params![provider_id.as_str(), model_id, now_utc(), conversation_id],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_conversation(conversation_id)
    }

    pub fn list_conversations(&self) -> AppResult<Vec<ConversationSummary>> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT c.id,c.title,c.provider_id,c.model_id,c.response_profile,c.created_at,c.updated_at,
                    COUNT(m.id)
             FROM conversations c LEFT JOIN messages m ON m.conversation_id=c.id
             GROUP BY c.id ORDER BY c.updated_at DESC,c.id DESC",
        )?;
        statement
            .query_map([], summary_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_conversation(&self, conversation_id: &str) -> AppResult<Conversation> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        let connection = self.lock()?;
        conversation_from_connection(&connection, conversation_id)
    }

    pub fn get_conversation_for_export(
        &self,
        conversation_id: &str,
        maximum_serialized_bytes: usize,
    ) -> AppResult<Conversation> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        let connection = self.lock()?;
        let (count, bytes): (i64, i64) = connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(length(CAST(content AS BLOB))),0)
             FROM messages WHERE conversation_id=?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let count = usize::try_from(count).map_err(|_| AppError::DatabaseIntegrity)?;
        let bytes = usize::try_from(bytes).map_err(|_| AppError::DatabaseIntegrity)?;
        let estimate = 4_096usize
            .checked_add(bytes)
            .and_then(|value| value.checked_add(count.checked_mul(640)?))
            .ok_or(AppError::Validation(
                "The conversation export exceeds the 32 MiB limit.",
            ))?;
        if estimate > maximum_serialized_bytes {
            return Err(AppError::Validation(
                "The conversation export exceeds the 32 MiB limit.",
            ));
        }
        conversation_from_connection(&connection, conversation_id)
    }

    pub fn rename_conversation(
        &self,
        conversation_id: &str,
        title: String,
    ) -> AppResult<ConversationSummary> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        let title = normalize_title(&title)?;
        let changed = self.lock()?.execute(
            "UPDATE conversations SET title=?1,updated_at=?2 WHERE id=?3",
            params![title, now_utc(), conversation_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound("Conversation not found."));
        }
        Ok(self.get_conversation(conversation_id)?.summary())
    }

    pub fn delete_conversation(&self, conversation_id: &str) -> AppResult<()> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        let changed = self
            .lock()?
            .execute("DELETE FROM conversations WHERE id=?1", [conversation_id])?;
        if changed == 0 {
            return Err(AppError::NotFound("Conversation not found."));
        }
        Ok(())
    }

    pub fn validate_generation_request(
        &self,
        conversation_id: &str,
        content: &str,
        regenerate_from_message_id: Option<&str>,
    ) -> AppResult<()> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        validate_message_content(content, MAX_USER_MESSAGE_BYTES)?;
        if let Some(message_id) = regenerate_from_message_id {
            validate_uuid(message_id, "Message ID is invalid.")?;
        }
        let exists: bool = self.lock()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
            [conversation_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound("Conversation not found."));
        }
        Ok(())
    }

    pub fn prepare_generation<T>(
        &self,
        operation_id: &str,
        conversation_id: &str,
        content: String,
        response_profile: ResponseProfile,
        regenerate_from_message_id: Option<String>,
        pre_network: impl FnOnce(&PreparedGeneration) -> AppResult<T>,
    ) -> AppResult<(PreparedGeneration, T)> {
        validate_uuid(operation_id, "Operation ID is invalid.")?;
        self.validate_generation_request(
            conversation_id,
            &content,
            regenerate_from_message_id.as_deref(),
        )?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let operation_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM usage_observations WHERE operation_id=?1)",
            [operation_id],
            |row| row.get(0),
        )?;
        if operation_exists {
            return Err(AppError::Conflict(
                "This generation request has already been started.",
            ));
        }
        let (provider, model): (String, String) = transaction
            .query_row(
                "SELECT provider_id,model_id FROM conversations WHERE id=?1",
                [conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(AppError::NotFound("Conversation not found."))?;
        let provider_id = ProviderId::parse(&provider).ok_or(AppError::DatabaseIntegrity)?;
        validate_selection(provider_id, &model).map_err(|_| AppError::DatabaseIntegrity)?;

        match regenerate_from_message_id.as_deref() {
            None => insert_new_user_message(&transaction, conversation_id, &content)?,
            Some(message_id) => {
                revise_or_regenerate(&transaction, conversation_id, message_id, &content)?
            }
        }
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        if count < 0 || count as usize >= MAX_MESSAGES_PER_CONVERSATION {
            return Err(AppError::Validation(
                "This conversation has reached the 10000-message limit.",
            ));
        }
        transaction.execute(
            "UPDATE conversations SET response_profile=?1,updated_at=?2 WHERE id=?3",
            params![response_profile.as_str(), now_utc(), conversation_id],
        )?;
        maybe_derive_title(&transaction, conversation_id)?;
        let history = provider_history(&transaction, conversation_id)?;
        if history.is_empty()
            || history
                .last()
                .is_none_or(|message| message.role != MessageRole::User)
        {
            return Err(AppError::Validation(
                "Generation requires a persisted user message.",
            ));
        }
        let prepared = PreparedGeneration {
            history,
            provider_id,
            model_id: model,
            response_profile,
        };
        let pre_network_result = pre_network(&prepared)?;
        transaction.execute(
            "INSERT INTO usage_observations
             (operation_id,provider_id,model_id,observed_at,input_tokens,cached_input_tokens,output_tokens,total_tokens,partial)
             VALUES(?1,?2,?3,?4,NULL,NULL,NULL,NULL,1)",
            params![
                operation_id,
                prepared.provider_id.as_str(),
                prepared.model_id,
                now_utc()
            ],
        )?;
        transaction.commit()?;
        Ok((prepared, pre_network_result))
    }

    #[cfg(test)]
    pub fn begin_usage_observation(
        &self,
        operation_id: &str,
        provider_id: ProviderId,
        model_id: &str,
    ) -> AppResult<()> {
        validate_uuid(operation_id, "Operation ID is invalid.")?;
        validate_selection(provider_id, model_id)?;
        self.lock()?.execute(
            "INSERT INTO usage_observations
             (operation_id,provider_id,model_id,observed_at,input_tokens,cached_input_tokens,output_tokens,total_tokens,partial)
             VALUES(?1,?2,?3,?4,NULL,NULL,NULL,NULL,1)",
            params![operation_id, provider_id.as_str(), model_id, now_utc()],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn complete_usage_observation(
        &self,
        operation_id: &str,
        usage: &TokenUsage,
    ) -> AppResult<()> {
        validate_uuid(operation_id, "Operation ID is invalid.")?;
        usage.validate()?;
        if usage.is_empty() {
            return Err(AppError::Validation("Token usage is empty."));
        }
        let connection = self.lock()?;
        finalize_usage_record(&connection, operation_id, Some(usage), &now_utc())
    }

    pub fn persist_generation_terminal(
        &self,
        operation_id: &str,
        conversation_id: &str,
        content: String,
        status: MessageStatus,
        finish_reason: Option<MessageFinishReason>,
        usage: Option<TokenUsage>,
    ) -> AppResult<Message> {
        validate_uuid(operation_id, "Operation ID is invalid.")?;
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        match status {
            MessageStatus::Complete => {
                validate_message_content(&content, MAX_STORED_MESSAGE_BYTES)?;
                if !matches!(
                    finish_reason,
                    Some(MessageFinishReason::Stop | MessageFinishReason::OutputLimit)
                ) {
                    return Err(AppError::Validation(
                        "A completed provider response requires a verified finish reason.",
                    ));
                }
            }
            MessageStatus::Cancelled | MessageStatus::Error => {
                validate_terminal_content(&content, MAX_STORED_MESSAGE_BYTES)?;
                if usage.is_some() || finish_reason.is_some() {
                    return Err(AppError::Validation(
                        "Cancelled or failed messages cannot store usage or a finish reason.",
                    ));
                }
            }
        }
        if let Some(usage) = &usage {
            usage.validate()?;
        }
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let completed_at = now_utc();
        finalize_usage_record(&transaction, operation_id, usage.as_ref(), &completed_at)?;
        ensure_conversation_exists(&transaction, conversation_id)?;
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position)+1,0) FROM messages WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        if position < 0 || position as usize >= MAX_MESSAGES_PER_CONVERSATION {
            return Err(AppError::Validation(
                "This conversation has reached the 10000-message limit.",
            ));
        }
        let message = Message {
            // Reusing the opaque operation identity makes terminal persistence
            // idempotent even when authoritative usage is absent: a duplicate
            // message insert fails and rolls the observation update back in
            // this same transaction without adding ledger-only state.
            id: operation_id.to_owned(),
            conversation_id: conversation_id.to_owned(),
            role: MessageRole::Assistant,
            content,
            created_at: completed_at,
            status,
            finish_reason,
            usage,
        };
        let usage = message.usage.as_ref();
        transaction.execute(
            "INSERT INTO messages
             (id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status,finish_reason)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                message.id,
                message.conversation_id,
                position,
                message.role.as_str(),
                message.content,
                to_sql_token(usage.and_then(|value| value.input_tokens))?,
                to_sql_token(usage.and_then(|value| value.cached_input_tokens))?,
                to_sql_token(usage.and_then(|value| value.output_tokens))?,
                to_sql_token(usage.and_then(|value| value.total_tokens))?,
                message.created_at,
                message.status.as_str(),
                message.finish_reason.map(MessageFinishReason::as_str)
            ],
        )?;
        transaction.execute(
            "UPDATE conversations SET updated_at=?1 WHERE id=?2",
            params![message.created_at, conversation_id],
        )?;
        transaction.commit()?;
        Ok(message)
    }

    pub fn acknowledge_notice(
        &self,
        provider_id: ProviderId,
        notice_version: u32,
    ) -> AppResult<()> {
        if notice_version == 0 || notice_version > i64::MAX as u32 {
            return Err(AppError::Validation("Provider notice version is invalid."));
        }
        self.lock()?.execute(
            "INSERT INTO provider_preferences(provider_id,weekly_token_budget,notice_version)
             VALUES(?1,NULL,?2)
             ON CONFLICT(provider_id) DO UPDATE SET notice_version=excluded.notice_version",
            params![provider_id.as_str(), i64::from(notice_version)],
        )?;
        Ok(())
    }

    pub fn notice_acknowledged(
        &self,
        provider_id: ProviderId,
        notice_version: u32,
    ) -> AppResult<bool> {
        let stored = self
            .lock()?
            .query_row(
                "SELECT notice_version FROM provider_preferences WHERE provider_id=?1",
                [provider_id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        Ok(stored == Some(i64::from(notice_version)))
    }

    pub fn set_usage_budget(
        &self,
        provider_id: ProviderId,
        token_budget: Option<u64>,
    ) -> AppResult<()> {
        let token_budget = token_budget
            .map(|value| {
                if value == 0 || value > MAX_SAFE_INTEGER {
                    return Err(AppError::Validation(
                        "The token budget must be a positive JavaScript-safe integer.",
                    ));
                }
                i64::try_from(value)
                    .map_err(|_| AppError::Validation("Token budget is out of range."))
            })
            .transpose()?;
        self.lock()?.execute(
            "INSERT INTO provider_preferences(provider_id,weekly_token_budget,notice_version)
             VALUES(?1,?2,0)
             ON CONFLICT(provider_id) DO UPDATE SET weekly_token_budget=excluded.weekly_token_budget",
            params![provider_id.as_str(), token_budget],
        )?;
        Ok(())
    }

    pub fn usage_summary(
        &self,
        provider_id: ProviderId,
        model_id: Option<&str>,
    ) -> AppResult<UsageSummary> {
        self.usage_summary_at(provider_id, model_id, Utc::now())
    }

    fn usage_summary_at(
        &self,
        provider_id: ProviderId,
        model_id: Option<&str>,
        now: DateTime<Utc>,
    ) -> AppResult<UsageSummary> {
        if let Some(model) = model_id {
            validate_selection(provider_id, model)?;
        }
        let start = now - Duration::days(7);
        let window_start = canonical_utc(start);
        let window_end = canonical_utc(now);
        let connection = self.lock()?;
        validate_usage_ledger(&connection)?;
        let mut statement = connection.prepare(
            "SELECT input_tokens,cached_input_tokens,output_tokens,total_tokens,partial
             FROM usage_observations
             WHERE provider_id=?1 AND (?2 IS NULL OR model_id=?2)
               AND observed_at>=?3 AND observed_at<=?4
             ORDER BY observed_at,operation_id",
        )?;
        let mut rows = statement.query(params![
            provider_id.as_str(),
            model_id,
            window_start,
            window_end
        ])?;
        let mut input = Aggregate::default();
        let mut cached = Aggregate::default();
        let mut output = Aggregate::default();
        let mut total = Aggregate::default();
        let mut complete_observations = 0u64;
        let mut partial_observations = 0u64;
        while let Some(row) = rows.next()? {
            input.add(row.get(0)?)?;
            cached.add(row.get(1)?)?;
            output.add(row.get(2)?)?;
            total.add(row.get(3)?)?;
            match row.get::<_, i64>(4)? {
                0 => complete_observations = complete_observations.saturating_add(1),
                1 => partial_observations = partial_observations.saturating_add(1),
                _ => return Err(AppError::DatabaseIntegrity),
            }
        }
        let overflow = input.overflow || cached.overflow || output.overflow || total.overflow;
        if overflow {
            partial_observations = partial_observations.saturating_add(1);
        }
        let budget_total = if model_id.is_some() {
            let mut provider_total = Aggregate::default();
            let mut statement = connection.prepare(
                "SELECT total_tokens FROM usage_observations
                 WHERE provider_id=?1 AND observed_at>=?2 AND observed_at<=?3
                 ORDER BY observed_at,operation_id",
            )?;
            let mut rows =
                statement.query(params![provider_id.as_str(), window_start, window_end])?;
            while let Some(row) = rows.next()? {
                provider_total.add(row.get(0)?)?;
            }
            provider_total
        } else {
            total.clone()
        };
        if budget_total.overflow && !overflow {
            partial_observations = partial_observations.saturating_add(1);
        }
        let budget_value = connection
            .query_row(
                "SELECT weekly_token_budget FROM provider_preferences WHERE provider_id=?1",
                [provider_id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        let budget = match budget_value {
            Some(raw_budget) => {
                let token_budget =
                    u64::try_from(raw_budget).map_err(|_| AppError::DatabaseIntegrity)?;
                if token_budget == 0 || token_budget > MAX_SAFE_INTEGER {
                    return Err(AppError::DatabaseIntegrity);
                }
                let known_used =
                    (!budget_total.overflow).then(|| budget_total.value().unwrap_or(0));
                let remaining = known_used
                    .map(|value| token_budget.saturating_sub(value))
                    .unwrap_or(0);
                let state = if known_used.is_none() || remaining == 0 {
                    "exhausted"
                } else if remaining <= token_budget / 10 {
                    "low"
                } else {
                    "normal"
                };
                Some(AdvisoryBudget {
                    token_budget,
                    known_used_tokens: known_used,
                    remaining_tokens: remaining,
                    remaining_percentage: if known_used.is_none() {
                        0.0
                    } else {
                        (remaining as f64 / token_budget as f64) * 100.0
                    },
                    state,
                })
            }
            _ => None,
        };
        let mut usage = TokenUsage {
            input_tokens: input.value(),
            cached_input_tokens: cached.value(),
            output_tokens: output.value(),
            total_tokens: total.value(),
        };
        if usage.validate().is_err() {
            usage.total_tokens = None;
            partial_observations = partial_observations.saturating_add(1);
        }
        let coverage = if complete_observations == 0 && partial_observations == 0 {
            "empty"
        } else if partial_observations == 0 {
            "complete"
        } else {
            "partial"
        };
        Ok(UsageSummary {
            provider_id,
            model_id: model_id.map(str::to_owned),
            window_start,
            window_end: window_end.clone(),
            observed_at: window_end,
            usage,
            complete_observations,
            partial_observations,
            coverage,
            budget,
        })
    }

    pub fn import(&self, bundle: ImportBundle) -> AppResult<Vec<ConversationSummary>> {
        let normalized = normalize_import(bundle)?;
        let total_messages = normalized.iter().try_fold(0usize, |total, conversation| {
            total
                .checked_add(conversation.messages.len())
                .ok_or(AppError::Validation(
                    "The import contains too many messages.",
                ))
        })?;
        if normalized.is_empty() || normalized.len() > MAX_IMPORT_CONVERSATIONS {
            return Err(AppError::Validation(
                "The import must contain between 1 and 100 conversations.",
            ));
        }
        if total_messages > MAX_IMPORT_MESSAGES {
            return Err(AppError::Validation(
                "The import contains more than 10000 messages.",
            ));
        }
        validate_normalized_import(&normalized)?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let mut imported_ids = Vec::with_capacity(normalized.len());
        for conversation in normalized {
            let conversation_id = Uuid::new_v4().to_string();
            transaction.execute(
                "INSERT INTO conversations
                 (id,title,provider_id,model_id,response_profile,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    conversation_id,
                    conversation.title,
                    conversation.provider_id.as_str(),
                    conversation.model_id,
                    conversation.response_profile.as_str(),
                    conversation.created_at,
                    conversation.updated_at
                ],
            )?;
            for (position, message) in conversation.messages.into_iter().enumerate() {
                let usage = message.usage.as_ref();
                transaction.execute(
                    "INSERT INTO messages
                     (id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status,finish_reason)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    params![
                        Uuid::new_v4().to_string(),
                        conversation_id,
                        position as i64,
                        message.role.as_str(),
                        message.content,
                        to_sql_token(usage.and_then(|value| value.input_tokens))?,
                        to_sql_token(usage.and_then(|value| value.cached_input_tokens))?,
                        to_sql_token(usage.and_then(|value| value.output_tokens))?,
                        to_sql_token(usage.and_then(|value| value.total_tokens))?,
                        message.created_at,
                        message.status.as_str(),
                        message.finish_reason.map(MessageFinishReason::as_str)
                    ],
                )?;
            }
            imported_ids.push(conversation_id);
        }
        transaction.commit()?;
        drop(connection);
        imported_ids
            .iter()
            .map(|id| {
                self.get_conversation(id)
                    .map(|conversation| conversation.summary())
            })
            .collect()
    }
}

#[derive(Clone, Default)]
struct Aggregate {
    seen: bool,
    total: u64,
    overflow: bool,
}

impl Aggregate {
    fn add(&mut self, value: Option<i64>) -> AppResult<()> {
        let Some(value) = value else {
            return Ok(());
        };
        let value = u64::try_from(value).map_err(|_| AppError::DatabaseIntegrity)?;
        if value > MAX_SAFE_INTEGER {
            return Err(AppError::DatabaseIntegrity);
        }
        self.seen = true;
        match self.total.checked_add(value) {
            Some(sum) if sum <= MAX_SAFE_INTEGER => self.total = sum,
            _ => self.overflow = true,
        }
        Ok(())
    }

    fn value(&self) -> Option<u64> {
        (self.seen && !self.overflow).then_some(self.total)
    }
}

fn finalize_usage_record(
    connection: &Connection,
    operation_id: &str,
    usage: Option<&TokenUsage>,
    completed_at: &str,
) -> AppResult<()> {
    let changed = if let Some(usage) = usage {
        connection.execute(
            "UPDATE usage_observations
             SET observed_at=?1,input_tokens=?2,cached_input_tokens=?3,output_tokens=?4,total_tokens=?5,partial=?6
             WHERE operation_id=?7
               AND partial=1
               AND input_tokens IS NULL
               AND cached_input_tokens IS NULL
               AND output_tokens IS NULL
               AND total_tokens IS NULL",
            params![
                completed_at,
                to_sql_token(usage.input_tokens)?,
                to_sql_token(usage.cached_input_tokens)?,
                to_sql_token(usage.output_tokens)?,
                to_sql_token(usage.total_tokens)?,
                i64::from(!usage.is_complete()),
                operation_id
            ],
        )?
    } else {
        connection.execute(
            "UPDATE usage_observations SET observed_at=?1
             WHERE operation_id=?2",
            params![completed_at, operation_id],
        )?
    };
    if changed != 1 {
        return Err(AppError::NotFound("The usage observation was not found."));
    }
    Ok(())
}

fn validate_usage_ledger(connection: &Connection) -> AppResult<()> {
    let mut statement = connection.prepare(
        "SELECT operation_id,provider_id,model_id,observed_at,
                input_tokens,cached_input_tokens,output_tokens,total_tokens,partial
         FROM usage_observations ORDER BY operation_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let operation_id: String = row.get(0)?;
        let provider: String = row.get(1)?;
        let model_id: String = row.get(2)?;
        let observed_at: String = row.get(3)?;
        let provider_id = ProviderId::parse(&provider).ok_or(AppError::DatabaseIntegrity)?;
        if validate_uuid(&operation_id, "invalid").is_err()
            || validate_selection(provider_id, &model_id).is_err()
            || validate_stored_timestamp(&observed_at).is_none()
        {
            return Err(AppError::DatabaseIntegrity);
        }
        let usage = TokenUsage::new(
            token_from_sql(row.get(4)?).map_err(|_| AppError::DatabaseIntegrity)?,
            token_from_sql(row.get(5)?).map_err(|_| AppError::DatabaseIntegrity)?,
            token_from_sql(row.get(6)?).map_err(|_| AppError::DatabaseIntegrity)?,
            token_from_sql(row.get(7)?).map_err(|_| AppError::DatabaseIntegrity)?,
        )
        .map_err(|_| AppError::DatabaseIntegrity)?;
        let partial: i64 = row.get(8)?;
        if !matches!(partial, 0 | 1) || (partial == 0) != usage.is_complete() {
            return Err(AppError::DatabaseIntegrity);
        }
    }
    Ok(())
}

fn ensure_conversation_exists(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> AppResult<()> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
        [conversation_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::NotFound("Conversation not found."));
    }
    Ok(())
}

fn insert_new_user_message(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    content: &str,
) -> AppResult<()> {
    let position: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(position)+1,0) FROM messages WHERE conversation_id=?1",
        [conversation_id],
        |row| row.get(0),
    )?;
    if position < 0 || position as usize >= MAX_MESSAGES_PER_CONVERSATION {
        return Err(AppError::Validation(
            "This conversation has reached the 10000-message limit.",
        ));
    }
    transaction.execute(
        "INSERT INTO messages
         (id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status)
         VALUES(?1,?2,?3,'user',?4,NULL,NULL,NULL,NULL,?5,'complete')",
        params![Uuid::new_v4().to_string(), conversation_id, position, content, now_utc()],
    )?;
    Ok(())
}

fn revise_or_regenerate(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    message_id: &str,
    content: &str,
) -> AppResult<()> {
    let target = transaction
        .query_row(
            "SELECT role,position FROM messages WHERE id=?1 AND conversation_id=?2",
            params![message_id, conversation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((role, position)) = target else {
        return Err(AppError::NotFound("Message not found."));
    };
    match MessageRole::parse(&role).ok_or(AppError::DatabaseIntegrity)? {
        MessageRole::User => {
            transaction.execute(
                "DELETE FROM messages WHERE conversation_id=?1 AND position>?2",
                params![conversation_id, position],
            )?;
            transaction.execute(
                "UPDATE messages SET content=?1,status='complete',input_tokens=NULL,
                 cached_input_tokens=NULL,output_tokens=NULL,total_tokens=NULL
                 WHERE id=?2 AND conversation_id=?3",
                params![content, message_id, conversation_id],
            )?;
        }
        MessageRole::Assistant => {
            let latest: i64 = transaction.query_row(
                "SELECT MAX(position) FROM messages WHERE conversation_id=?1",
                [conversation_id],
                |row| row.get(0),
            )?;
            if latest != position {
                return Err(AppError::Conflict(
                    "Only the most recent assistant response can be regenerated.",
                ));
            }
            let preceding: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM messages
                 WHERE conversation_id=?1 AND role='user' AND position<?2)",
                params![conversation_id, position],
                |row| row.get(0),
            )?;
            if !preceding {
                return Err(AppError::Validation(
                    "The assistant response has no preceding user message.",
                ));
            }
            transaction.execute(
                "DELETE FROM messages WHERE conversation_id=?1 AND position>=?2",
                params![conversation_id, position],
            )?;
        }
    }
    Ok(())
}

fn provider_history(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> AppResult<Vec<ProviderMessage>> {
    let mut statement = transaction.prepare(
        "SELECT role,content FROM messages WHERE conversation_id=?1
           AND (role='user' OR (role='assistant' AND status='complete'))
         ORDER BY position DESC LIMIT ?2",
    )?;
    let mut rows = statement.query(params![
        conversation_id,
        MAX_PROVIDER_HISTORY_MESSAGES as i64
    ])?;
    let mut reversed = Vec::new();
    let mut total_bytes = 0usize;
    while let Some(row) = rows.next()? {
        let role: String = row.get(0)?;
        let content: String = row.get(1)?;
        validate_message_content(&content, MAX_STORED_MESSAGE_BYTES)
            .map_err(|_| AppError::DatabaseIntegrity)?;
        let next = total_bytes.saturating_add(content.len());
        if next > MAX_PROVIDER_HISTORY_BYTES {
            break;
        }
        total_bytes = next;
        reversed.push(ProviderMessage {
            role: MessageRole::parse(&role).ok_or(AppError::DatabaseIntegrity)?,
            content,
        });
    }
    reversed.reverse();
    Ok(reversed)
}

fn maybe_derive_title(transaction: &Transaction<'_>, conversation_id: &str) -> AppResult<()> {
    let title: String = transaction.query_row(
        "SELECT title FROM conversations WHERE id=?1",
        [conversation_id],
        |row| row.get(0),
    )?;
    if title != DEFAULT_TITLE && title != "New chat" {
        return Ok(());
    }
    let content = transaction
        .query_row(
            "SELECT content FROM messages WHERE conversation_id=?1 AND role='user'
             ORDER BY position LIMIT 1",
            [conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(content) = content {
        let derived = content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(60)
            .collect::<String>();
        if !derived.is_empty() {
            transaction.execute(
                "UPDATE conversations SET title=?1 WHERE id=?2",
                params![derived, conversation_id],
            )?;
        }
    }
    Ok(())
}

fn conversation_from_connection(
    connection: &Connection,
    conversation_id: &str,
) -> AppResult<Conversation> {
    let summary = connection
        .query_row(
            "SELECT c.id,c.title,c.provider_id,c.model_id,c.response_profile,c.created_at,c.updated_at,
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id=c.id)
             FROM conversations c WHERE c.id=?1",
            [conversation_id],
            summary_from_row,
        )
        .optional()?
        .ok_or(AppError::NotFound("Conversation not found."))?;
    let mut statement = connection.prepare(
        "SELECT position,id,conversation_id,role,content,input_tokens,cached_input_tokens,
                output_tokens,total_tokens,created_at,status,finish_reason
         FROM messages WHERE conversation_id=?1 ORDER BY position",
    )?;
    let mut rows = statement.query([conversation_id])?;
    let mut messages = Vec::new();
    while let Some(row) = rows.next()? {
        let position: i64 = row.get(0)?;
        if position != messages.len() as i64 {
            return Err(AppError::DatabaseIntegrity);
        }
        messages.push(message_from_row(row, conversation_id)?);
    }
    Ok(Conversation {
        id: summary.id,
        title: summary.title,
        provider_id: summary.provider_id,
        model_id: summary.model_id,
        response_profile: summary.response_profile,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        message_count: summary.message_count,
        messages,
    })
}

fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<ConversationSummary> {
    let id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let provider: String = row.get(2)?;
    let model_id: String = row.get(3)?;
    let profile: String = row.get(4)?;
    let created_at: String = row.get(5)?;
    let updated_at: String = row.get(6)?;
    let message_count: i64 = row.get(7)?;
    let provider_id = ProviderId::parse(&provider).ok_or_else(invalid_db_value)?;
    if validate_uuid(&id, "invalid").is_err()
        || normalize_title(&title).is_err()
        || validate_selection(provider_id, &model_id).is_err()
        || validate_stored_timestamp(&created_at).is_none()
        || validate_stored_timestamp(&updated_at).is_none()
        || message_count < 0
        || message_count as usize > MAX_MESSAGES_PER_CONVERSATION
    {
        return Err(invalid_db_value());
    }
    Ok(ConversationSummary {
        id,
        title,
        provider_id,
        model_id,
        response_profile: ResponseProfile::parse(&profile).ok_or_else(invalid_db_value)?,
        created_at,
        updated_at,
        message_count: message_count as u64,
    })
}

fn message_from_row(row: &Row<'_>, expected_conversation_id: &str) -> AppResult<Message> {
    let id: String = row.get(1)?;
    let conversation_id: String = row.get(2)?;
    let role: String = row.get(3)?;
    let content: String = row.get(4)?;
    let input = token_from_sql(row.get(5)?).map_err(|_| AppError::DatabaseIntegrity)?;
    let cached = token_from_sql(row.get(6)?).map_err(|_| AppError::DatabaseIntegrity)?;
    let output = token_from_sql(row.get(7)?).map_err(|_| AppError::DatabaseIntegrity)?;
    let total = token_from_sql(row.get(8)?).map_err(|_| AppError::DatabaseIntegrity)?;
    let created_at: String = row.get(9)?;
    let status: String = row.get(10)?;
    let finish_reason: Option<String> = row.get(11)?;
    if conversation_id != expected_conversation_id
        || validate_uuid(&id, "invalid").is_err()
        || validate_uuid(&conversation_id, "invalid").is_err()
        || validate_stored_timestamp(&created_at).is_none()
    {
        return Err(AppError::DatabaseIntegrity);
    }
    let role = MessageRole::parse(&role).ok_or(AppError::DatabaseIntegrity)?;
    let status = MessageStatus::parse(&status).ok_or(AppError::DatabaseIntegrity)?;
    let finish_reason = match finish_reason.as_deref() {
        Some(value) => Some(MessageFinishReason::parse(value).ok_or(AppError::DatabaseIntegrity)?),
        None => None,
    };
    let content_result = match status {
        MessageStatus::Complete => validate_message_content(&content, MAX_STORED_MESSAGE_BYTES),
        MessageStatus::Cancelled | MessageStatus::Error => {
            validate_terminal_content(&content, MAX_STORED_MESSAGE_BYTES)
        }
    };
    if content_result.is_err()
        || (role == MessageRole::User && status != MessageStatus::Complete)
        || ((role == MessageRole::Assistant && status == MessageStatus::Complete)
            != finish_reason.is_some())
    {
        return Err(AppError::DatabaseIntegrity);
    }
    let usage =
        TokenUsage::new(input, cached, output, total).map_err(|_| AppError::DatabaseIntegrity)?;
    if !usage.is_empty() && !(role == MessageRole::Assistant && status == MessageStatus::Complete) {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(Message {
        id,
        conversation_id,
        role,
        content,
        created_at,
        status,
        finish_reason,
        usage: (!usage.is_empty()).then_some(usage),
    })
}

struct NormalizedConversation {
    title: String,
    provider_id: ProviderId,
    model_id: String,
    response_profile: ResponseProfile,
    created_at: String,
    updated_at: String,
    messages: Vec<NormalizedMessage>,
}

struct NormalizedMessage {
    role: MessageRole,
    content: String,
    created_at: String,
    status: MessageStatus,
    finish_reason: Option<MessageFinishReason>,
    usage: Option<TokenUsage>,
}

fn normalize_import(bundle: ImportBundle) -> AppResult<Vec<NormalizedConversation>> {
    match bundle {
        ImportBundle::V1(bundle) => normalize_v1(bundle),
        ImportBundle::V2(bundle) => normalize_v2(bundle),
    }
}

fn normalize_v1(bundle: ImportBundleV1) -> AppResult<Vec<NormalizedConversation>> {
    validate_import_header(&bundle.format, bundle.version, &bundle.exported_at, 1)?;
    bundle
        .conversations
        .into_iter()
        .map(|conversation| {
            if conversation.model != DEFAULT_MODEL {
                return Err(AppError::Validation(
                    "The version 1 import uses an unsupported model.",
                ));
            }
            let messages = conversation
                .messages
                .into_iter()
                .map(|message| {
                    let finish_reason = (message.role == MessageRole::Assistant
                        && message.status == MessageStatus::Complete)
                        .then_some(MessageFinishReason::Unknown);
                    let usage = message
                        .token_usage
                        .map(|total| TokenUsage::new(None, None, None, Some(total)))
                        .transpose()?;
                    Ok(NormalizedMessage {
                        role: message.role,
                        content: message.content,
                        created_at: normalize_timestamp(&message.created_at)?,
                        status: message.status,
                        finish_reason,
                        usage,
                    })
                })
                .collect::<AppResult<Vec<_>>>()?;
            Ok(NormalizedConversation {
                title: normalize_title(&conversation.title)?,
                provider_id: DEFAULT_PROVIDER,
                model_id: DEFAULT_MODEL.to_owned(),
                response_profile: conversation.reasoning_mode,
                created_at: normalize_timestamp(&conversation.created_at)?,
                updated_at: normalize_timestamp(&conversation.updated_at)?,
                messages,
            })
        })
        .collect()
}

fn normalize_v2(bundle: ImportBundleV2) -> AppResult<Vec<NormalizedConversation>> {
    validate_import_header(&bundle.format, bundle.version, &bundle.exported_at, 2)?;
    bundle
        .conversations
        .into_iter()
        .map(|conversation| {
            validate_selection(conversation.provider_id, &conversation.model_id)?;
            let messages = conversation
                .messages
                .into_iter()
                .map(|message| {
                    if let Some(usage) = &message.usage {
                        usage.validate()?;
                    }
                    let finish_reason = if message.role == MessageRole::Assistant
                        && message.status == MessageStatus::Complete
                    {
                        Some(
                            message
                                .finish_reason
                                .unwrap_or(MessageFinishReason::Unknown),
                        )
                    } else {
                        message.finish_reason
                    };
                    Ok(NormalizedMessage {
                        role: message.role,
                        content: message.content,
                        created_at: normalize_timestamp(&message.created_at)?,
                        status: message.status,
                        finish_reason,
                        usage: message.usage,
                    })
                })
                .collect::<AppResult<Vec<_>>>()?;
            Ok(NormalizedConversation {
                title: normalize_title(&conversation.title)?,
                provider_id: conversation.provider_id,
                model_id: conversation.model_id,
                response_profile: conversation.response_profile,
                created_at: normalize_timestamp(&conversation.created_at)?,
                updated_at: normalize_timestamp(&conversation.updated_at)?,
                messages,
            })
        })
        .collect()
}

fn validate_import_header(
    format: &str,
    version: u32,
    exported_at: &str,
    expected_version: u32,
) -> AppResult<()> {
    if format != "aster-conversation" || version != expected_version {
        return Err(AppError::Validation(
            "The import format or schema version is not supported.",
        ));
    }
    validate_timestamp(exported_at)
}

fn validate_normalized_import(conversations: &[NormalizedConversation]) -> AppResult<()> {
    for conversation in conversations {
        validate_selection(conversation.provider_id, &conversation.model_id)?;
        let created = parse_timestamp(&conversation.created_at)?;
        let updated = parse_timestamp(&conversation.updated_at)?;
        if updated < created {
            return Err(AppError::Validation(
                "An imported conversation update timestamp precedes its creation timestamp.",
            ));
        }
        if conversation.messages.len() > MAX_MESSAGES_PER_CONVERSATION {
            return Err(AppError::Validation(
                "An imported conversation has too many messages.",
            ));
        }
        for message in &conversation.messages {
            validate_timestamp(&message.created_at)?;
            match message.status {
                MessageStatus::Complete => {
                    validate_message_content(&message.content, MAX_STORED_MESSAGE_BYTES)?;
                }
                MessageStatus::Cancelled | MessageStatus::Error => {
                    validate_terminal_content(&message.content, MAX_STORED_MESSAGE_BYTES)?;
                }
            }
            if message.role == MessageRole::User && message.status != MessageStatus::Complete {
                return Err(AppError::Validation(
                    "Imported user messages must have complete status.",
                ));
            }
            if (message.role == MessageRole::Assistant && message.status == MessageStatus::Complete)
                != message.finish_reason.is_some()
            {
                return Err(AppError::Validation(
                    "Imported finish reasons are allowed only on complete assistant messages.",
                ));
            }
            if message.usage.is_some()
                && (message.role != MessageRole::Assistant
                    || message.status != MessageStatus::Complete)
            {
                return Err(AppError::Validation(
                    "Imported token usage is allowed only on complete assistant messages.",
                ));
            }
            if let Some(usage) = &message.usage {
                usage.validate()?;
                if usage.is_empty() {
                    return Err(AppError::Validation(
                        "Imported token usage must contain at least one known count.",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn normalize_title(value: &str) -> AppResult<String> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::Validation(
            "Conversation titles must contain between 1 and 80 characters.",
        ));
    }
    if title.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Conversation titles cannot contain control characters.",
        ));
    }
    Ok(title.to_owned())
}

fn validate_message_content(content: &str, maximum: usize) -> AppResult<()> {
    if content.trim().is_empty() {
        return Err(AppError::Validation("Message content cannot be empty."));
    }
    validate_terminal_content(content, maximum)
}

fn validate_terminal_content(content: &str, maximum: usize) -> AppResult<()> {
    if content.len() > maximum {
        return Err(AppError::Validation("Message content is too large."));
    }
    if content.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err(AppError::Validation(
            "Message content contains unsupported control characters.",
        ));
    }
    Ok(())
}

fn validate_uuid(value: &str, message: &'static str) -> AppResult<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| AppError::Validation(message))?;
    if parsed.to_string() != value {
        return Err(AppError::Validation(message));
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> AppResult<()> {
    parse_timestamp(value).map(|_| ())
}

fn parse_timestamp(value: &str) -> AppResult<DateTime<chrono::FixedOffset>> {
    if value.len() > 64 {
        return Err(AppError::Validation("An imported timestamp is invalid."));
    }
    DateTime::parse_from_rfc3339(value)
        .map_err(|_| AppError::Validation("An imported timestamp is invalid."))
}

fn normalize_timestamp(value: &str) -> AppResult<String> {
    Ok(canonical_utc(parse_timestamp(value)?.with_timezone(&Utc)))
}

fn validate_stored_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    if parsed.offset().local_minus_utc() != 0 {
        return None;
    }
    let utc = parsed.with_timezone(&Utc);
    (canonical_utc(utc) == value).then_some(utc)
}

fn canonical_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn now_utc() -> String {
    canonical_utc(Utc::now())
}

fn to_sql_token(value: Option<u64>) -> AppResult<Option<i64>> {
    value
        .map(|value| {
            if value > MAX_SAFE_INTEGER {
                return Err(AppError::Validation("Token usage is out of range."));
            }
            i64::try_from(value).map_err(|_| AppError::Validation("Token usage is out of range."))
        })
        .transpose()
}

fn token_from_sql(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value
        .map(|value| {
            u64::try_from(value)
                .ok()
                .filter(|value| *value <= MAX_SAFE_INTEGER)
                .ok_or_else(invalid_db_value)
        })
        .transpose()
}

fn invalid_db_value() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ProviderClient;
    use chrono::TimeZone;
    use rusqlite::Connection;
    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn v1_migration_uses_backup_and_preserves_only_legacy_total_on_message() {
        let directory = tempdir().expect("temp directory");
        let path = directory.path().join("aster.sqlite3");
        let legacy = Connection::open(&path).expect("legacy database");
        legacy
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
            .unwrap();
        schema::v1_schema_for_tests(&legacy).expect("v1 schema");
        let conversation_id = Uuid::new_v4().to_string();
        let message_id = Uuid::new_v4().to_string();
        let timestamp = "2026-07-13T10:00:00.000Z";
        legacy
            .execute(
                "INSERT INTO conversations VALUES(?1,'Legacy','glm-5.1','deep',?2,?2)",
                params![conversation_id, timestamp],
            )
            .unwrap();
        legacy
            .execute(
                "INSERT INTO messages VALUES(?1,?2,0,'assistant','Legacy answer',42,?3,'complete')",
                params![message_id, conversation_id, timestamp],
            )
            .unwrap();

        let database = Database::open(&path).expect("migration should succeed");
        let conversation = database.get_conversation(&conversation_id).unwrap();
        assert_eq!(conversation.provider_id, ProviderId::Zai);
        assert_eq!(conversation.model_id, "glm-5.1");
        assert_eq!(conversation.response_profile, ResponseProfile::Deep);
        assert_eq!(
            conversation.messages[0].usage,
            Some(TokenUsage::total_only(42))
        );
        assert_eq!(
            conversation.messages[0].finish_reason,
            Some(MessageFinishReason::Unknown)
        );
        let summary = database.usage_summary(ProviderId::Zai, None).unwrap();
        assert_eq!(summary.coverage, "empty");
        assert!(
            !path
                .with_file_name("aster.sqlite3.v1-backup.sqlite3")
                .exists()
        );
    }

    #[test]
    fn conflicting_backup_blocks_migration_without_mutating_v1() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("aster.sqlite3");
        let legacy = Connection::open(&path).unwrap();
        schema::v1_schema_for_tests(&legacy).unwrap();
        std::fs::write(
            path.with_file_name("aster.sqlite3.v1-backup.sqlite3"),
            b"not sqlite",
        )
        .unwrap();
        assert!(Database::open(&path).is_err());
        let version: u32 = legacy
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn invalid_v1_title_timestamp_and_status_retain_backup_and_original() {
        for case in ["title", "timestamp", "status"] {
            let directory = tempdir().unwrap();
            let path = directory.path().join("aster.sqlite3");
            let legacy = Connection::open(&path).unwrap();
            schema::v1_schema_for_tests(&legacy).unwrap();
            let conversation_id = Uuid::new_v4().to_string();
            let message_id = Uuid::new_v4().to_string();
            let title = if case == "title" {
                "x".repeat(MAX_TITLE_CHARS + 1)
            } else {
                "Legacy".to_owned()
            };
            let timestamp = if case == "timestamp" {
                "not-a-timestamp"
            } else {
                "2026-07-13T10:00:00.000Z"
            };
            let (role, status, token_usage) = if case == "status" {
                ("user", "cancelled", None)
            } else {
                ("assistant", "complete", Some(42_i64))
            };
            legacy
                .execute(
                    "INSERT INTO conversations VALUES(?1,?2,'glm-5.1','standard',?3,?3)",
                    params![conversation_id, title, timestamp],
                )
                .unwrap();
            legacy
                .execute(
                    "INSERT INTO messages VALUES(?1,?2,0,?3,'Legacy content',?4,?5,?6)",
                    params![
                        message_id,
                        conversation_id,
                        role,
                        token_usage,
                        timestamp,
                        status
                    ],
                )
                .unwrap();
            let expected: (String, String, String, String) = legacy
                .query_row(
                    "SELECT c.title,c.created_at,m.role,m.status
                     FROM conversations c JOIN messages m ON m.conversation_id=c.id",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            drop(legacy);

            assert!(Database::open(&path).is_err(), "case {case}");
            let backup_path = path.with_file_name("aster.sqlite3.v1-backup.sqlite3");
            assert!(backup_path.exists(), "case {case}");
            for candidate in [&path, &backup_path] {
                let connection = Connection::open(candidate).unwrap();
                let version: u32 = connection
                    .pragma_query_value(None, "user_version", |row| row.get(0))
                    .unwrap();
                assert_eq!(version, 1, "case {case}");
                let actual: (String, String, String, String) = connection
                    .query_row(
                        "SELECT c.title,c.created_at,m.role,m.status
                         FROM conversations c JOIN messages m ON m.conversation_id=c.id",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .unwrap();
                assert_eq!(actual, expected, "case {case}");
            }
        }
    }

    #[test]
    fn provider_pair_is_mutable_only_while_conversation_is_empty() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let updated = database
            .update_conversation_selection(
                &conversation.id,
                ProviderId::Google,
                "gemini-2.5-pro".to_owned(),
            )
            .unwrap();
        assert_eq!(updated.provider_id, ProviderId::Google);
        database
            .prepare_generation(
                &Uuid::new_v4().to_string(),
                &conversation.id,
                "Hello".to_owned(),
                ResponseProfile::Fast,
                None,
                |_| Ok(()),
            )
            .unwrap();
        assert!(matches!(
            database.update_conversation_selection(
                &conversation.id,
                ProviderId::Zai,
                "glm-5.2".to_owned()
            ),
            Err(AppError::ConversationModelLocked)
        ));
    }

    #[test]
    fn usage_observation_is_single_fill_and_survives_conversation_deletion() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        let partial = database
            .usage_summary(ProviderId::Zai, Some("glm-5.1"))
            .unwrap();
        assert_eq!(partial.partial_observations, 1);
        let usage = TokenUsage::new(Some(8), Some(2), Some(30), Some(40)).unwrap();
        database
            .complete_usage_observation(&operation_id, &usage)
            .unwrap();
        assert!(
            database
                .complete_usage_observation(&operation_id, &usage)
                .is_err()
        );
        database.delete_conversation(&conversation.id).unwrap();
        let current = database.usage_summary(ProviderId::Zai, None).unwrap();
        assert_eq!(current.usage.total_tokens, Some(40));
        assert_eq!(current.complete_observations, 1);
    }

    #[test]
    fn terminal_message_and_usage_finalization_commit_atomically() {
        let database = Database::open_in_memory().unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        let usage = TokenUsage::new(Some(8), Some(2), Some(30), Some(40)).unwrap();
        assert!(
            database
                .persist_generation_terminal(
                    &operation_id,
                    &Uuid::new_v4().to_string(),
                    "Answer".to_owned(),
                    MessageStatus::Complete,
                    Some(MessageFinishReason::Stop),
                    Some(usage.clone()),
                )
                .is_err()
        );
        let after_rollback = database.usage_summary(ProviderId::Zai, None).unwrap();
        assert_eq!(after_rollback.partial_observations, 1);
        assert_eq!(after_rollback.usage.total_tokens, None);

        let conversation = database.create_conversation(None, None, None).unwrap();
        let message = database
            .persist_generation_terminal(
                &operation_id,
                &conversation.id,
                "Answer".to_owned(),
                MessageStatus::Complete,
                Some(MessageFinishReason::Stop),
                Some(usage.clone()),
            )
            .unwrap();
        assert_eq!(message.usage, Some(usage.clone()));
        assert!(
            database
                .persist_generation_terminal(
                    &operation_id,
                    &conversation.id,
                    "Duplicate".to_owned(),
                    MessageStatus::Complete,
                    Some(MessageFinishReason::Stop),
                    Some(usage),
                )
                .is_err()
        );
        let stored = database.get_conversation(&conversation.id).unwrap();
        assert_eq!(stored.messages.len(), 1);
        assert_eq!(
            database
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .complete_observations,
            1
        );
    }

    #[test]
    fn null_usage_terminalization_is_explicit_and_idempotent() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        let message = database
            .persist_generation_terminal(
                &operation_id,
                &conversation.id,
                "Partial answer".to_owned(),
                MessageStatus::Cancelled,
                None,
                None,
            )
            .unwrap();
        assert_eq!(message.finish_reason, None);
        assert!(message.usage.is_none());
        assert_eq!(message.id, operation_id);
        assert!(
            database
                .persist_generation_terminal(
                    &operation_id,
                    &conversation.id,
                    "Duplicate".to_owned(),
                    MessageStatus::Cancelled,
                    None,
                    None,
                )
                .is_err()
        );
        assert_eq!(
            database
                .get_conversation(&conversation.id)
                .unwrap()
                .messages
                .len(),
            1
        );
    }

    #[test]
    fn usage_observation_schema_is_the_closed_normative_set() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.lock().unwrap();
        let mut statement = connection
            .prepare("PRAGMA table_info(usage_observations)")
            .unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns,
            [
                "operation_id",
                "provider_id",
                "model_id",
                "observed_at",
                "input_tokens",
                "cached_input_tokens",
                "output_tokens",
                "total_tokens",
                "partial",
            ]
        );
    }

    #[test]
    fn output_limit_finish_reason_persists_with_partial_content_and_usage() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        let usage = TokenUsage::new(Some(8), Some(2), Some(30), Some(40)).unwrap();
        database
            .persist_generation_terminal(
                &operation_id,
                &conversation.id,
                "Response stopped at the provider output limit".to_owned(),
                MessageStatus::Complete,
                Some(MessageFinishReason::OutputLimit),
                Some(usage.clone()),
            )
            .unwrap();
        let stored = database.get_conversation(&conversation.id).unwrap();
        assert_eq!(
            stored.messages[0].finish_reason,
            Some(MessageFinishReason::OutputLimit)
        );
        assert_eq!(stored.messages[0].usage, Some(usage));
        let exported =
            serde_json::to_value(crate::models::ExportConversation::from(stored)).unwrap();
        assert_eq!(
            exported["messages"][0]["finishReason"],
            serde_json::json!("outputLimit")
        );
    }

    #[test]
    fn user_message_and_pre_network_usage_marker_commit_atomically() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        assert!(
            database
                .prepare_generation(
                    &operation_id,
                    &conversation.id,
                    "Must roll back".to_owned(),
                    ResponseProfile::Standard,
                    None,
                    |_| Ok(()),
                )
                .is_err()
        );
        assert!(
            database
                .get_conversation(&conversation.id)
                .unwrap()
                .messages
                .is_empty()
        );
    }

    #[test]
    fn oversized_built_request_is_rejected_without_message_or_usage_marker() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let escaped_content = "\\".repeat(MAX_USER_MESSAGE_BYTES);
        database
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO messages
                 (id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status,finish_reason)
                 VALUES(?1,?2,0,'assistant',?3,NULL,NULL,NULL,NULL,?4,'complete','stop')",
                params![
                    Uuid::new_v4().to_string(),
                    conversation.id,
                    escaped_content,
                    now_utc()
                ],
            )
            .unwrap();
        let provider = ProviderClient::new().unwrap();
        let operation_id = Uuid::new_v4().to_string();
        let result = database.prepare_generation(
            &operation_id,
            &conversation.id,
            escaped_content,
            ResponseProfile::Standard,
            None,
            |prepared| {
                provider
                    .prepare_chat(
                        prepared.provider_id,
                        &prepared.model_id,
                        prepared.response_profile,
                        &prepared.history,
                    )
                    .map(|_| ())
            },
        );
        assert!(matches!(result, Err(AppError::Validation(_))));
        assert_eq!(
            database
                .get_conversation(&conversation.id)
                .unwrap()
                .messages
                .len(),
            1
        );
        assert_eq!(
            database
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .coverage,
            "empty"
        );
    }

    #[test]
    fn started_event_failure_and_pre_network_cancel_leave_no_marker() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let cancellation = CancellationToken::new();
        let failed_emit = database.prepare_generation(
            &Uuid::new_v4().to_string(),
            &conversation.id,
            "No event".to_owned(),
            ResponseProfile::Standard,
            None,
            |_| crate::complete_generation_preflight(&cancellation, || Err(AppError::Internal)),
        );
        assert!(matches!(failed_emit, Err(AppError::Internal)));
        assert!(
            database
                .get_conversation(&conversation.id)
                .unwrap()
                .messages
                .is_empty()
        );
        assert_eq!(
            database
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .coverage,
            "empty"
        );

        let cancelled_during_emit = database.prepare_generation(
            &Uuid::new_v4().to_string(),
            &conversation.id,
            "Cancelled before network".to_owned(),
            ResponseProfile::Standard,
            None,
            |_| {
                crate::complete_generation_preflight(&cancellation, || {
                    cancellation.cancel();
                    Ok(())
                })
            },
        );
        assert!(matches!(cancelled_during_emit, Err(AppError::Cancelled)));
        assert!(
            database
                .get_conversation(&conversation.id)
                .unwrap()
                .messages
                .is_empty()
        );
        assert_eq!(
            database
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .coverage,
            "empty"
        );
    }

    #[test]
    fn advisory_budget_threshold_is_exact_and_never_blocks_usage() {
        let database = Database::open_in_memory().unwrap();
        database
            .set_usage_budget(ProviderId::Zai, Some(100))
            .unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        database
            .complete_usage_observation(
                &operation_id,
                &TokenUsage::new(Some(90), Some(0), Some(0), Some(90)).unwrap(),
            )
            .unwrap();
        let summary = database.usage_summary(ProviderId::Zai, None).unwrap();
        let budget = summary.budget.unwrap();
        assert_eq!(budget.remaining_tokens, 10);
        assert_eq!(budget.state, "low");
        database.set_usage_budget(ProviderId::Zai, None).unwrap();
        assert!(
            database
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .budget
                .is_none()
        );

        for (known_usage, expected_state) in
            [(89, "normal"), (90, "low"), (91, "low"), (100, "exhausted")]
        {
            let database = Database::open_in_memory().unwrap();
            database
                .set_usage_budget(ProviderId::Google, Some(100))
                .unwrap();
            let operation_id = Uuid::new_v4().to_string();
            database
                .begin_usage_observation(&operation_id, ProviderId::Google, "gemini-2.5-flash")
                .unwrap();
            database
                .complete_usage_observation(
                    &operation_id,
                    &TokenUsage::new(Some(known_usage), Some(0), Some(0), Some(known_usage))
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(
                database
                    .usage_summary(ProviderId::Google, None)
                    .unwrap()
                    .budget
                    .unwrap()
                    .state,
                expected_state
            );
        }

        let database = Database::open_in_memory().unwrap();
        assert!(database.set_usage_budget(ProviderId::Zai, Some(1)).is_ok());
        assert!(
            database
                .set_usage_budget(ProviderId::Zai, Some(MAX_SAFE_INTEGER))
                .is_ok()
        );
        assert!(database.set_usage_budget(ProviderId::Zai, Some(0)).is_err());
    }

    #[test]
    fn model_filter_never_narrows_provider_wide_budget_consumption() {
        let database = Database::open_in_memory().unwrap();
        database
            .set_usage_budget(ProviderId::Google, Some(100))
            .unwrap();
        for (model, total) in [("gemini-2.5-flash", 5), ("gemini-2.5-pro", 86)] {
            let operation_id = Uuid::new_v4().to_string();
            database
                .begin_usage_observation(&operation_id, ProviderId::Google, model)
                .unwrap();
            database
                .complete_usage_observation(
                    &operation_id,
                    &TokenUsage::new(Some(total), Some(0), Some(0), Some(total)).unwrap(),
                )
                .unwrap();
        }

        let filtered = database
            .usage_summary(ProviderId::Google, Some("gemini-2.5-flash"))
            .unwrap();
        assert_eq!(filtered.usage.total_tokens, Some(5));
        let budget = filtered.budget.unwrap();
        assert_eq!(budget.known_used_tokens, Some(91));
        assert_eq!(budget.remaining_tokens, 9);
        assert_eq!(budget.state, "low");
    }

    #[test]
    fn budget_threshold_is_integer_exact_and_aggregate_overflow_stays_exhausted() {
        let database = Database::open_in_memory().unwrap();
        let token_budget = 9_007_199_254_740_989;
        let known_used = 8_106_479_329_266_890;
        database
            .set_usage_budget(ProviderId::Zai, Some(token_budget))
            .unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        database
            .complete_usage_observation(
                &operation_id,
                &TokenUsage::new(Some(known_used), Some(0), Some(0), Some(known_used)).unwrap(),
            )
            .unwrap();

        let exact = database.usage_summary(ProviderId::Zai, None).unwrap();
        let exact_budget = exact.budget.unwrap();
        assert_eq!(exact_budget.known_used_tokens, Some(known_used));
        assert_eq!(exact_budget.remaining_tokens, 900_719_925_474_099);
        assert_eq!(exact_budget.state, "normal");

        let overflowed = Database::open_in_memory().unwrap();
        overflowed
            .set_usage_budget(ProviderId::Google, Some(100))
            .unwrap();
        for used in [MAX_SAFE_INTEGER, 1] {
            let operation_id = Uuid::new_v4().to_string();
            overflowed
                .begin_usage_observation(&operation_id, ProviderId::Google, "gemini-2.5-flash")
                .unwrap();
            overflowed
                .complete_usage_observation(
                    &operation_id,
                    &TokenUsage::new(Some(used), Some(0), Some(0), Some(used)).unwrap(),
                )
                .unwrap();
        }

        let overflow_summary = overflowed
            .usage_summary(ProviderId::Google, Some("gemini-2.5-flash"))
            .unwrap();
        let overflow_budget = overflow_summary.budget.unwrap();
        assert_eq!(overflow_summary.coverage, "partial");
        assert_eq!(overflow_summary.usage.total_tokens, None);
        assert_eq!(overflow_budget.known_used_tokens, None);
        assert_eq!(overflow_budget.remaining_tokens, 0);
        assert_eq!(overflow_budget.remaining_percentage, 0.0);
        assert_eq!(overflow_budget.state, "exhausted");
    }

    #[test]
    fn usage_window_has_exact_inclusive_seven_day_boundaries() {
        let database = Database::open_in_memory().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 7, 13, 12, 0, 0).unwrap();
        let start = now - Duration::days(7);
        let cases = [
            (start - Duration::milliseconds(1), 13_u64),
            (start, 11_u64),
            (now, 17_u64),
            (now + Duration::milliseconds(1), 19_u64),
        ];
        let connection = database.lock().unwrap();
        for (timestamp, total) in cases {
            connection
                .execute(
                    "INSERT INTO usage_observations
                     (operation_id,provider_id,model_id,observed_at,input_tokens,cached_input_tokens,output_tokens,total_tokens,partial)
                     VALUES(?1,'google','gemini-2.5-flash',?2,?3,0,0,?3,0)",
                    params![
                        Uuid::new_v4().to_string(),
                        canonical_utc(timestamp),
                        i64::try_from(total).unwrap()
                    ],
                )
                .unwrap();
        }
        drop(connection);
        let summary = database
            .usage_summary_at(ProviderId::Google, Some("gemini-2.5-flash"), now)
            .unwrap();
        assert_eq!(summary.usage.total_tokens, Some(28));
        assert_eq!(summary.complete_observations, 2);
        assert_eq!(summary.partial_observations, 0);
        assert_eq!(summary.window_start, canonical_utc(start));
        assert_eq!(summary.window_end, canonical_utc(now));
    }

    #[test]
    fn notice_acknowledgement_is_provider_and_version_scoped() {
        let database = Database::open_in_memory().unwrap();
        assert!(
            !database
                .notice_acknowledged(ProviderId::DeepSeek, 1)
                .unwrap()
        );
        database
            .acknowledge_notice(ProviderId::DeepSeek, 1)
            .unwrap();
        assert!(
            database
                .notice_acknowledged(ProviderId::DeepSeek, 1)
                .unwrap()
        );
        assert!(
            !database
                .notice_acknowledged(ProviderId::DeepSeek, 2)
                .unwrap()
        );
        assert!(!database.notice_acknowledged(ProviderId::Zai, 1).unwrap());
    }

    #[test]
    fn v1_and_v2_imports_map_pairs_without_seeding_usage_ledger() {
        let database = Database::open_in_memory().unwrap();
        let v1 = r#"{"format":"aster-conversation","version":1,"exportedAt":"2026-07-13T10:00:00Z","conversations":[{"title":"Legacy","model":"glm-5.1","reasoningMode":"standard","createdAt":"2026-07-13T10:00:00Z","updatedAt":"2026-07-13T10:00:00Z","messages":[{"role":"assistant","content":"Old","createdAt":"2026-07-13T10:00:00Z","status":"complete","tokenUsage":12}]}]}"#;
        let imported = database.import(serde_json::from_str(v1).unwrap()).unwrap();
        assert_eq!(imported[0].provider_id, ProviderId::Zai);
        assert_eq!(
            database.get_conversation(&imported[0].id).unwrap().messages[0].finish_reason,
            Some(MessageFinishReason::Unknown)
        );
        assert_eq!(
            database
                .usage_summary(ProviderId::Zai, None)
                .unwrap()
                .coverage,
            "empty"
        );

        let v2 = r#"{"format":"aster-conversation","version":2,"exportedAt":"2026-07-13T10:00:00Z","conversations":[{"title":"Gemini","provider":"google","model":"gemini-2.5-pro","responseProfile":"fast","createdAt":"2026-07-13T10:00:00Z","updatedAt":"2026-07-13T10:00:00Z","messages":[{"role":"assistant","content":"Imported answer","createdAt":"2026-07-13T10:00:00Z","status":"complete"}]}]}"#;
        let imported = database.import(serde_json::from_str(v2).unwrap()).unwrap();
        assert_eq!(imported[0].provider_id, ProviderId::Google);
        assert_eq!(imported[0].model_id, "gemini-2.5-pro");
        assert_eq!(
            database.get_conversation(&imported[0].id).unwrap().messages[0].finish_reason,
            Some(MessageFinishReason::Unknown)
        );
    }

    #[test]
    fn invalid_import_usage_role_or_status_is_rejected_atomically() {
        let database = Database::open_in_memory().unwrap();
        let invalid = r#"{"format":"aster-conversation","version":2,"exportedAt":"2026-07-13T10:00:00Z","conversations":[{"title":"Invalid usage","provider":"google","model":"gemini-2.5-pro","responseProfile":"fast","createdAt":"2026-07-13T10:00:00Z","updatedAt":"2026-07-13T10:00:00Z","messages":[{"role":"assistant","content":"Stopped","createdAt":"2026-07-13T10:00:00Z","status":"cancelled","usage":{"inputTokens":1,"cachedInputTokens":0,"outputTokens":1,"totalTokens":2}}]}]}"#;
        assert!(
            database
                .import(serde_json::from_str(invalid).unwrap())
                .is_err()
        );
        assert!(database.list_conversations().unwrap().is_empty());
        assert_eq!(
            database
                .usage_summary(ProviderId::Google, None)
                .unwrap()
                .coverage,
            "empty"
        );
    }

    #[test]
    fn tampered_user_message_usage_metadata_fails_closed_on_read() {
        let database = Database::open_in_memory().unwrap();
        let conversation = database.create_conversation(None, None, None).unwrap();
        let message_id = Uuid::new_v4().to_string();
        {
            let connection = database.lock().unwrap();
            let insert_message = || {
                connection.execute(
                    "INSERT INTO messages
                     (id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status,finish_reason)
                     VALUES(?1,?2,0,'user','Tampered usage',1,0,0,1,?3,'complete',NULL)",
                    params![message_id, conversation.id, now_utc()],
                )
            };
            assert!(insert_message().is_err());

            connection
                .execute_batch("PRAGMA ignore_check_constraints=ON;")
                .unwrap();
            insert_message().unwrap();
            connection
                .execute_batch("PRAGMA ignore_check_constraints=OFF;")
                .unwrap();
        }

        assert!(matches!(
            database.get_conversation(&conversation.id),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn usage_ledger_rejects_a_noncanonical_operation_uuid_on_read() {
        let database = Database::open_in_memory().unwrap();
        let operation_id = "550E8400-E29B-41D4-A716-446655440000";
        let observed_at = "2026-07-14T00:00:00.000Z";
        {
            let connection = database.lock().unwrap();
            let insert_observation = || {
                connection.execute(
                    "INSERT INTO usage_observations
                     (operation_id,provider_id,model_id,observed_at,input_tokens,cached_input_tokens,output_tokens,total_tokens,partial)
                     VALUES(?1,'zai','glm-5.1',?2,NULL,NULL,NULL,NULL,1)",
                    params![operation_id, observed_at],
                )
            };
            assert!(Uuid::parse_str(operation_id).is_ok());
            assert!(insert_observation().is_err());
            connection
                .execute_batch("PRAGMA ignore_check_constraints=ON;")
                .unwrap();
            insert_observation().unwrap();
            connection
                .execute_batch("PRAGMA ignore_check_constraints=OFF;")
                .unwrap();
        }

        assert!(matches!(
            database.usage_summary(ProviderId::Zai, None),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn usage_ledger_rejects_an_invalid_time_on_read() {
        let database = Database::open_in_memory().unwrap();
        database
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO usage_observations
                 (operation_id,provider_id,model_id,observed_at,input_tokens,cached_input_tokens,output_tokens,total_tokens,partial)
                 VALUES(?1,'zai','glm-5.1','2026-07-14T24:00:00.000Z',NULL,NULL,NULL,NULL,1)",
                [Uuid::new_v4().to_string()],
            )
            .expect("the storage shape constraint intentionally permits semantic validation");

        assert!(matches!(
            database.usage_summary(ProviderId::Zai, None),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn usage_ledger_rejects_a_valid_shape_but_invalid_date_on_read() {
        let database = Database::open_in_memory().unwrap();
        database
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO usage_observations
                 (operation_id,provider_id,model_id,observed_at,input_tokens,cached_input_tokens,output_tokens,total_tokens,partial)
                 VALUES(?1,'zai','glm-5.1','2026-02-30T00:00:00.000Z',NULL,NULL,NULL,NULL,1)",
                [Uuid::new_v4().to_string()],
            )
            .expect("the storage shape constraint intentionally permits semantic validation");

        assert!(matches!(
            database.usage_summary(ProviderId::Zai, None),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn canonical_usage_ledger_metadata_remains_readable() {
        let database = Database::open_in_memory().unwrap();
        let operation_id = Uuid::new_v4().to_string();
        database
            .begin_usage_observation(&operation_id, ProviderId::Zai, "glm-5.1")
            .unwrap();
        database
            .complete_usage_observation(
                &operation_id,
                &TokenUsage::new(Some(8), Some(2), Some(30), Some(40)).unwrap(),
            )
            .unwrap();

        let summary = database.usage_summary(ProviderId::Zai, None).unwrap();
        assert_eq!(summary.coverage, "complete");
        assert_eq!(summary.complete_observations, 1);
        assert_eq!(summary.partial_observations, 0);
        assert_eq!(summary.usage.total_tokens, Some(40));
    }

    #[test]
    fn v2_finish_reason_round_trips_and_wrong_role_is_rejected() {
        let database = Database::open_in_memory().unwrap();
        let valid = r#"{"format":"aster-conversation","version":2,"exportedAt":"2026-07-13T10:00:00Z","conversations":[{"title":"Limited","provider":"zai","model":"glm-5.1","responseProfile":"standard","createdAt":"2026-07-13T10:00:00Z","updatedAt":"2026-07-13T10:00:00Z","messages":[{"role":"assistant","content":"Partial answer","createdAt":"2026-07-13T10:00:00Z","status":"complete","finishReason":"outputLimit"}]}]}"#;
        let imported = database
            .import(serde_json::from_str(valid).unwrap())
            .unwrap();
        let conversation = database.get_conversation(&imported[0].id).unwrap();
        assert_eq!(
            conversation.messages[0].finish_reason,
            Some(MessageFinishReason::OutputLimit)
        );
        let exported =
            serde_json::to_value(crate::models::ExportConversation::from(conversation)).unwrap();
        assert_eq!(
            exported["messages"][0]["finishReason"],
            serde_json::json!("outputLimit")
        );

        let invalid = r#"{"format":"aster-conversation","version":2,"exportedAt":"2026-07-13T10:00:00Z","conversations":[{"title":"Invalid","provider":"zai","model":"glm-5.1","responseProfile":"standard","createdAt":"2026-07-13T10:00:00Z","updatedAt":"2026-07-13T10:00:00Z","messages":[{"role":"user","content":"Hello","createdAt":"2026-07-13T10:00:00Z","status":"complete","finishReason":"stop"}]}]}"#;
        assert!(
            database
                .import(serde_json::from_str(invalid).unwrap())
                .is_err()
        );
        assert_eq!(database.list_conversations().unwrap().len(), 1);
    }
}
