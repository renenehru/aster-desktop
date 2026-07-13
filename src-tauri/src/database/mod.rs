use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::limits::Limit;
use rusqlite::types::Type;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    Conversation, ConversationSummary, ImportBundle, Message, MessageRole, MessageStatus,
    ProviderMessage, ReasoningMode,
};

pub const MODEL: &str = "glm-5.1";
const DEFAULT_TITLE: &str = "New conversation";
const MAX_TITLE_CHARS: usize = 80;
const MAX_USER_MESSAGE_BYTES: usize = 256 * 1024;
const MAX_STORED_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PROVIDER_HISTORY_BYTES: usize = 512 * 1024;
const MAX_PROVIDER_HISTORY_MESSAGES: usize = 200;
const MAX_IMPORT_CONVERSATIONS: usize = 100;
const MAX_IMPORT_MESSAGES: usize = 10_000;
const MAX_MESSAGES_PER_CONVERSATION: usize = 10_000;
const MAX_TOKEN_USAGE: u64 = i64::MAX as u64;
const MAX_SQLITE_VALUE_BYTES: i32 = 3 * 1024 * 1024;

const CONNECTION_PRAGMAS: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA secure_delete = ON;
PRAGMA trusted_schema = OFF;
PRAGMA temp_store = MEMORY;
"#;

const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 256),
    model TEXT NOT NULL CHECK(model = 'glm-5.1'),
    reasoning_mode TEXT NOT NULL CHECK(reasoning_mode IN ('fast', 'standard', 'deep')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position >= 0),
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content TEXT NOT NULL CHECK(length(content) <= 2097152),
    token_usage INTEGER CHECK(token_usage IS NULL OR token_usage >= 0),
    created_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('complete', 'cancelled', 'error')),
    UNIQUE(conversation_id, position)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_conversations_updated
    ON conversations(updated_at DESC, id);
CREATE INDEX IF NOT EXISTS idx_messages_conversation_position
    ON messages(conversation_id, position);

PRAGMA user_version = 1;
"#;

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

pub struct PreparedGeneration {
    pub history: Vec<ProviderMessage>,
    pub reasoning_mode: ReasoningMode,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::initialize(connection)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> AppResult<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(mut connection: Connection) -> AppResult<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        configure_sqlite_limits(&connection)?;
        connection.execute_batch(CONNECTION_PRAGMAS)?;
        let schema_version: u32 =
            connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match schema_version {
            0 => {
                let transaction = connection.transaction()?;
                transaction.execute_batch(MIGRATION_1)?;
                transaction.commit()?;
            }
            1 => {}
            _ => return Err(AppError::DatabaseIntegrity),
        }
        verify_schema(&mut connection)?;
        let integrity: String =
            connection.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(AppError::DatabaseIntegrity);
        }
        let has_foreign_key_violation = {
            let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
            statement.query([])?.next()?.is_some()
        };
        if has_foreign_key_violation {
            return Err(AppError::DatabaseIntegrity);
        }
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    fn lock(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| AppError::Internal)
    }

    pub fn create_conversation(&self, title: Option<String>) -> AppResult<Conversation> {
        let title = normalize_title(title.as_deref().unwrap_or(DEFAULT_TITLE))?;
        let id = Uuid::new_v4().to_string();
        let now = now_utc();
        self.lock()?.execute(
            "INSERT INTO conversations
             (id, title, model, reasoning_mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, title, MODEL, ReasoningMode::Standard.as_str(), now],
        )?;
        self.get_conversation(&id)
    }

    pub fn list_conversations(&self) -> AppResult<Vec<ConversationSummary>> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT length(CAST(c.id AS BLOB)), length(CAST(c.title AS BLOB)),
                    length(CAST(c.model AS BLOB)), length(CAST(c.reasoning_mode AS BLOB)),
                    length(CAST(c.created_at AS BLOB)), length(CAST(c.updated_at AS BLOB)),
                    c.id, c.title, c.model, c.reasoning_mode, c.created_at, c.updated_at, COUNT(m.id)
             FROM conversations c
             LEFT JOIN messages m ON m.conversation_id = c.id
             GROUP BY c.id
             ORDER BY c.updated_at DESC, c.id DESC",
        )?;
        let mut rows = statement.query([])?;
        let mut conversations = Vec::new();
        while let Some(row) = rows.next()? {
            conversations.push(summary_from_row(row)?);
        }
        Ok(conversations)
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
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        ensure_conversation_exists(&transaction, conversation_id)?;
        let (message_count, content_bytes): (i64, i64) = transaction.query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(content AS BLOB))), 0)
             FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let message_count =
            usize::try_from(message_count).map_err(|_| AppError::Database(invalid_db_value(0)))?;
        let content_bytes =
            usize::try_from(content_bytes).map_err(|_| AppError::Database(invalid_db_value(1)))?;
        let estimated_bytes = 4_096usize
            .checked_add(content_bytes)
            .and_then(|value| value.checked_add(message_count.checked_mul(512)?))
            .ok_or(AppError::Validation(
                "The conversation export exceeds the 32 MiB limit.",
            ))?;
        if estimated_bytes > maximum_serialized_bytes {
            return Err(AppError::Validation(
                "The conversation export exceeds the 32 MiB limit.",
            ));
        }
        let conversation = conversation_from_connection(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(conversation)
    }

    pub fn rename_conversation(
        &self,
        conversation_id: &str,
        title: String,
    ) -> AppResult<ConversationSummary> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        let title = normalize_title(&title)?;
        let changed = self.lock()?.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
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
            .execute("DELETE FROM conversations WHERE id = ?1", [conversation_id])?;
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
        let exists = self.lock()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(AppError::NotFound("Conversation not found."));
        }
        Ok(())
    }

    pub fn prepare_generation(
        &self,
        conversation_id: &str,
        content: String,
        reasoning_mode: ReasoningMode,
        regenerate_from_message_id: Option<String>,
    ) -> AppResult<PreparedGeneration> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        validate_message_content(&content, MAX_USER_MESSAGE_BYTES)?;
        if let Some(message_id) = regenerate_from_message_id.as_deref() {
            validate_uuid(message_id, "Message ID is invalid.")?;
        }

        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        ensure_conversation_exists(&transaction, conversation_id)?;

        match regenerate_from_message_id.as_deref() {
            None => insert_new_user_message(&transaction, conversation_id, &content)?,
            Some(message_id) => {
                revise_or_regenerate(&transaction, conversation_id, message_id, &content)?
            }
        }
        let persisted_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        if persisted_count < 0 || persisted_count as usize >= MAX_MESSAGES_PER_CONVERSATION {
            return Err(AppError::Validation(
                "This conversation has reached the 10000-message limit.",
            ));
        }

        transaction.execute(
            "UPDATE conversations
             SET reasoning_mode = ?1, updated_at = ?2
             WHERE id = ?3",
            params![reasoning_mode.as_str(), now_utc(), conversation_id],
        )?;
        maybe_derive_title(&transaction, conversation_id)?;
        let history = provider_history(&transaction, conversation_id)?;
        if history.is_empty() || history.last().is_none_or(|m| m.role != MessageRole::User) {
            return Err(AppError::Validation(
                "Generation requires a persisted user message.",
            ));
        }
        transaction.commit()?;
        Ok(PreparedGeneration {
            history,
            reasoning_mode,
        })
    }

    pub fn append_assistant_message(
        &self,
        conversation_id: &str,
        content: String,
        status: MessageStatus,
        token_usage: Option<u64>,
    ) -> AppResult<Message> {
        validate_uuid(conversation_id, "Conversation ID is invalid.")?;
        match status {
            MessageStatus::Complete => {
                validate_message_content(&content, MAX_STORED_MESSAGE_BYTES)?;
            }
            MessageStatus::Cancelled | MessageStatus::Error => {
                validate_terminal_content(&content, MAX_STORED_MESSAGE_BYTES)?;
            }
        }
        let token_usage = token_usage
            .map(i64::try_from)
            .transpose()
            .map_err(|_| AppError::Validation("Token usage is out of range."))?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        ensure_conversation_exists(&transaction, conversation_id)?;
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        if position < 0 || position as usize >= MAX_MESSAGES_PER_CONVERSATION {
            return Err(AppError::Validation(
                "This conversation has reached the 10000-message limit.",
            ));
        }
        let message = Message {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_owned(),
            role: MessageRole::Assistant,
            content,
            created_at: now_utc(),
            status,
            token_usage: token_usage.map(|value| value as u64),
        };
        transaction.execute(
            "INSERT INTO messages
             (id, conversation_id, position, role, content, token_usage, created_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                message.id,
                message.conversation_id,
                position,
                message.role.as_str(),
                message.content,
                token_usage,
                message.created_at,
                message.status.as_str()
            ],
        )?;
        transaction.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![message.created_at, conversation_id],
        )?;
        transaction.commit()?;
        Ok(message)
    }

    pub fn import(&self, bundle: ImportBundle) -> AppResult<Vec<ConversationSummary>> {
        validate_import_header(&bundle)?;
        let total_messages =
            bundle
                .conversations
                .iter()
                .try_fold(0usize, |total, conversation| {
                    total
                        .checked_add(conversation.messages.len())
                        .ok_or(AppError::Validation(
                            "The import contains too many messages.",
                        ))
                })?;
        if total_messages > MAX_IMPORT_MESSAGES {
            return Err(AppError::Validation(
                "The import contains more than 10000 messages.",
            ));
        }

        validate_import_conversations(&bundle)?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let mut imported_ids = Vec::with_capacity(bundle.conversations.len());

        for conversation in bundle.conversations {
            let new_conversation_id = Uuid::new_v4().to_string();
            let title = normalize_title(&conversation.title)?;
            let conversation_created_at = normalize_timestamp(&conversation.created_at)?;
            let conversation_updated_at = normalize_timestamp(&conversation.updated_at)?;
            transaction.execute(
                "INSERT INTO conversations
                 (id, title, model, reasoning_mode, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    new_conversation_id,
                    title,
                    MODEL,
                    conversation.reasoning_mode.as_str(),
                    conversation_created_at,
                    conversation_updated_at
                ],
            )?;
            for (position, message) in conversation.messages.into_iter().enumerate() {
                let token_usage = message
                    .token_usage
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|_| AppError::Validation("Token usage is out of range."))?;
                let message_created_at = normalize_timestamp(&message.created_at)?;
                transaction.execute(
                    "INSERT INTO messages
                     (id, conversation_id, position, role, content, token_usage, created_at, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        Uuid::new_v4().to_string(),
                        new_conversation_id,
                        position as i64,
                        message.role.as_str(),
                        message.content,
                        token_usage,
                        message_created_at,
                        message.status.as_str()
                    ],
                )?;
            }
            imported_ids.push(new_conversation_id);
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

fn configure_sqlite_limits(connection: &Connection) -> AppResult<()> {
    connection.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_SQLITE_VALUE_BYTES)?;
    connection.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 128 * 1024)?;
    connection.set_limit(Limit::SQLITE_LIMIT_COLUMN, 64)?;
    connection.set_limit(Limit::SQLITE_LIMIT_EXPR_DEPTH, 64)?;
    connection.set_limit(Limit::SQLITE_LIMIT_COMPOUND_SELECT, 8)?;
    connection.set_limit(Limit::SQLITE_LIMIT_FUNCTION_ARG, 32)?;
    connection.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    connection.set_limit(Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 256 * 1024)?;
    connection.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 32)?;
    connection.set_limit(Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 4)?;
    connection.set_limit(Limit::SQLITE_LIMIT_WORKER_THREADS, 0)?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct ColumnDefinition {
    name: String,
    data_type: String,
    not_null: bool,
    primary_key: bool,
    hidden: bool,
}

fn verify_schema(connection: &mut Connection) -> AppResult<()> {
    let objects = {
        let mut statement = connection.prepare(
            "SELECT type, name, tbl_name FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let expected_objects = vec![
        (
            "index".to_owned(),
            "idx_conversations_updated".to_owned(),
            "conversations".to_owned(),
        ),
        (
            "index".to_owned(),
            "idx_messages_conversation_position".to_owned(),
            "messages".to_owned(),
        ),
        (
            "table".to_owned(),
            "conversations".to_owned(),
            "conversations".to_owned(),
        ),
        (
            "table".to_owned(),
            "messages".to_owned(),
            "messages".to_owned(),
        ),
    ];
    if objects != expected_objects {
        return Err(AppError::DatabaseIntegrity);
    }

    if table_columns(connection, "PRAGMA table_xinfo(conversations)")?
        != expected_columns(&[
            ("id", "TEXT", true, true),
            ("title", "TEXT", true, false),
            ("model", "TEXT", true, false),
            ("reasoning_mode", "TEXT", true, false),
            ("created_at", "TEXT", true, false),
            ("updated_at", "TEXT", true, false),
        ])
        || table_columns(connection, "PRAGMA table_xinfo(messages)")?
            != expected_columns(&[
                ("id", "TEXT", true, true),
                ("conversation_id", "TEXT", true, false),
                ("position", "INTEGER", true, false),
                ("role", "TEXT", true, false),
                ("content", "TEXT", true, false),
                ("token_usage", "INTEGER", false, false),
                ("created_at", "TEXT", true, false),
                ("status", "TEXT", true, false),
            ])
    {
        return Err(AppError::DatabaseIntegrity);
    }

    verify_schema_sql(
        connection,
        "conversations",
        &[
            "check(length(title) between 1 and 256)",
            "check(model = 'glm-5.1')",
            "check(reasoning_mode in ('fast', 'standard', 'deep'))",
        ],
    )?;
    verify_schema_sql(
        connection,
        "messages",
        &[
            "check(position >= 0)",
            "check(role in ('user', 'assistant'))",
            "check(length(content) <= 2097152)",
            "check(token_usage is null or token_usage >= 0)",
            "check(status in ('complete', 'cancelled', 'error'))",
            "unique(conversation_id, position)",
        ],
    )?;

    let foreign_keys = {
        let mut statement = connection.prepare("PRAGMA foreign_key_list(messages)")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    if foreign_keys
        != vec![(
            "conversations".to_owned(),
            "conversation_id".to_owned(),
            "id".to_owned(),
            "CASCADE".to_owned(),
        )]
    {
        return Err(AppError::DatabaseIntegrity);
    }
    if index_columns(connection, "idx_conversations_updated")? != ["updated_at", "id"]
        || index_columns(connection, "idx_messages_conversation_position")?
            != ["conversation_id", "position"]
    {
        return Err(AppError::DatabaseIntegrity);
    }
    let unique_indexes = {
        let mut statement = connection
            .prepare("SELECT name FROM pragma_index_list('messages') WHERE \"unique\" = 1")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    if !unique_indexes.into_iter().any(|name| {
        index_columns(connection, &name)
            .is_ok_and(|columns| columns == ["conversation_id", "position"])
    }) {
        return Err(AppError::DatabaseIntegrity);
    }

    verify_constraint_behavior(connection)
}

fn table_columns(connection: &Connection, pragma: &str) -> AppResult<Vec<ColumnDefinition>> {
    let mut statement = connection.prepare(pragma)?;
    let columns = statement
        .query_map([], |row| {
            Ok(ColumnDefinition {
                name: row.get(1)?,
                data_type: row.get(2)?,
                not_null: row.get::<_, i64>(3)? == 1,
                primary_key: row.get::<_, i64>(5)? == 1,
                hidden: row.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns)
}

fn expected_columns(definitions: &[(&str, &str, bool, bool)]) -> Vec<ColumnDefinition> {
    definitions
        .iter()
        .map(
            |(name, data_type, not_null, primary_key)| ColumnDefinition {
                name: (*name).to_owned(),
                data_type: (*data_type).to_owned(),
                not_null: *not_null,
                primary_key: *primary_key,
                hidden: false,
            },
        )
        .collect()
}

fn verify_schema_sql(
    connection: &Connection,
    table: &str,
    required_fragments: &[&str],
) -> AppResult<()> {
    let sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    let normalized = sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if !normalized.ends_with("strict")
        || required_fragments
            .iter()
            .any(|fragment| !normalized.contains(fragment))
    {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

fn index_columns(connection: &Connection, index_name: &str) -> AppResult<Vec<String>> {
    let mut statement =
        connection.prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")?;
    Ok(statement
        .query_map([index_name], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn verify_constraint_behavior(connection: &mut Connection) -> AppResult<()> {
    let transaction = connection.transaction()?;
    let conversation_id = Uuid::new_v4().to_string();
    let timestamp = "2026-01-01T00:00:00.000Z";
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO conversations VALUES (?1, '', ?2, 'standard', ?3, ?3)",
        params![Uuid::new_v4().to_string(), MODEL, timestamp],
    ))?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO conversations VALUES (?1, 'Probe', 'wrong-model', 'standard', ?2, ?2)",
        params![Uuid::new_v4().to_string(), timestamp],
    ))?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO conversations VALUES (?1, 'Probe', ?2, 'unsupported', ?3, ?3)",
        params![Uuid::new_v4().to_string(), MODEL, timestamp],
    ))?;
    transaction.execute(
        "INSERT INTO conversations VALUES (?1, 'Probe', ?2, 'standard', ?3, ?3)",
        params![conversation_id, MODEL, timestamp],
    )?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, -1, 'user', 'x', NULL, ?3, 'complete')",
        params![Uuid::new_v4().to_string(), conversation_id, timestamp],
    ))?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, 0, 'system', 'x', NULL, ?3, 'complete')",
        params![Uuid::new_v4().to_string(), conversation_id, timestamp],
    ))?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, 0, 'user', 'x', -1, ?3, 'complete')",
        params![Uuid::new_v4().to_string(), conversation_id, timestamp],
    ))?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, 0, 'user', 'x', NULL, ?3, 'streaming')",
        params![Uuid::new_v4().to_string(), conversation_id, timestamp],
    ))?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, 0, 'user', 'x', NULL, ?3, 'complete')",
        params![
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            timestamp
        ],
    ))?;
    transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, 0, 'user', 'x', NULL, ?3, 'complete')",
        params![Uuid::new_v4().to_string(), conversation_id, timestamp],
    )?;
    expect_constraint_rejection(transaction.execute(
        "INSERT INTO messages VALUES (?1, ?2, 0, 'assistant', 'x', NULL, ?3, 'complete')",
        params![Uuid::new_v4().to_string(), conversation_id, timestamp],
    ))?;
    transaction.execute(
        "DELETE FROM conversations WHERE id = ?1",
        [&conversation_id],
    )?;
    let remaining: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
        [&conversation_id],
        |row| row.get(0),
    )?;
    if remaining != 0 {
        return Err(AppError::DatabaseIntegrity);
    }
    transaction.rollback()?;
    Ok(())
}

fn expect_constraint_rejection(result: rusqlite::Result<usize>) -> AppResult<()> {
    if result.is_ok() {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

fn ensure_conversation_exists(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> AppResult<()> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
        [conversation_id],
        |row| row.get::<_, bool>(0),
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
        "SELECT COALESCE(MAX(position) + 1, 0) FROM messages WHERE conversation_id = ?1",
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
         (id, conversation_id, position, role, content, token_usage, created_at, status)
         VALUES (?1, ?2, ?3, 'user', ?4, NULL, ?5, 'complete')",
        params![
            Uuid::new_v4().to_string(),
            conversation_id,
            position,
            content,
            now_utc()
        ],
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
            "SELECT length(CAST(role AS BLOB)), role, position FROM messages
             WHERE id = ?1 AND conversation_id = ?2",
            params![message_id, conversation_id],
            |row| {
                let expected = bounded_row_length(row, 0, 16)?;
                let role: String = row.get(1)?;
                if role.len() != expected {
                    return Err(invalid_db_value(1));
                }
                Ok((role, row.get::<_, i64>(2)?))
            },
        )
        .optional()?;
    let Some((role, position)) = target else {
        return Err(AppError::NotFound("Message not found."));
    };
    let role = MessageRole::parse(&role).ok_or(AppError::Database(invalid_db_value(0)))?;

    match role {
        MessageRole::User => {
            transaction.execute(
                "DELETE FROM messages WHERE conversation_id = ?1 AND position > ?2",
                params![conversation_id, position],
            )?;
            transaction.execute(
                "UPDATE messages SET content = ?1, status = 'complete'
                 WHERE id = ?2 AND conversation_id = ?3",
                params![content, message_id, conversation_id],
            )?;
        }
        MessageRole::Assistant => {
            let latest_position: i64 = transaction.query_row(
                "SELECT MAX(position) FROM messages WHERE conversation_id = ?1",
                [conversation_id],
                |row| row.get(0),
            )?;
            if latest_position != position {
                return Err(AppError::Conflict(
                    "Only the most recent assistant response can be regenerated.",
                ));
            }
            let preceding_user_exists = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM messages
                    WHERE conversation_id = ?1 AND role = 'user' AND position < ?2
                 )",
                params![conversation_id, position],
                |row| row.get::<_, bool>(0),
            )?;
            if !preceding_user_exists {
                return Err(AppError::Validation(
                    "The assistant response has no preceding user message.",
                ));
            }
            transaction.execute(
                "DELETE FROM messages WHERE conversation_id = ?1 AND position >= ?2",
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
        "SELECT length(CAST(role AS BLOB)), length(CAST(content AS BLOB)), role, content
         FROM messages WHERE conversation_id = ?1
           AND (role = 'user' OR (role = 'assistant' AND status = 'complete'))
         ORDER BY position DESC
         LIMIT ?2",
    )?;
    let mut rows = statement.query(params![
        conversation_id,
        MAX_PROVIDER_HISTORY_MESSAGES as i64
    ])?;
    let mut reversed = Vec::new();
    let mut total_bytes = 0usize;
    while let Some(row) = rows.next()? {
        let role_bytes = bounded_row_length(row, 0, 16)?;
        let content_bytes = bounded_row_length(row, 1, MAX_STORED_MESSAGE_BYTES)?;
        let next_size = total_bytes.saturating_add(content_bytes);
        if next_size > MAX_PROVIDER_HISTORY_BYTES {
            break;
        }
        let role: String = row.get(2)?;
        let content: String = row.get(3)?;
        if role.len() != role_bytes || content.len() != content_bytes {
            return Err(AppError::DatabaseIntegrity);
        }
        validate_message_content(&content, MAX_STORED_MESSAGE_BYTES)
            .map_err(|_| AppError::DatabaseIntegrity)?;
        total_bytes = next_size;
        reversed.push(ProviderMessage {
            role: MessageRole::parse(&role).ok_or(AppError::Database(invalid_db_value(0)))?,
            content,
        });
    }
    reversed.reverse();
    Ok(reversed)
}

fn maybe_derive_title(transaction: &Transaction<'_>, conversation_id: &str) -> AppResult<()> {
    let title: String = transaction.query_row(
        "SELECT length(CAST(title AS BLOB)), title FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| {
            let expected = bounded_row_length(row, 0, MAX_TITLE_CHARS * 4)?;
            let title: String = row.get(1)?;
            if title.len() != expected || normalize_title(&title).is_err() {
                return Err(invalid_db_value(1));
            }
            Ok(title)
        },
    )?;
    if title != DEFAULT_TITLE && title != "New chat" {
        return Ok(());
    }
    let first_user_content: Option<String> = transaction
        .query_row(
            "SELECT length(CAST(content AS BLOB)), content FROM messages
             WHERE conversation_id = ?1 AND role = 'user'
             ORDER BY position ASC LIMIT 1",
            [conversation_id],
            |row| {
                let expected = bounded_row_length(row, 0, MAX_USER_MESSAGE_BYTES)?;
                let content: String = row.get(1)?;
                if content.len() != expected
                    || validate_message_content(&content, MAX_USER_MESSAGE_BYTES).is_err()
                {
                    return Err(invalid_db_value(1));
                }
                Ok(content)
            },
        )
        .optional()?;
    if let Some(content) = first_user_content {
        let derived = content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(60)
            .collect::<String>();
        if !derived.is_empty() {
            transaction.execute(
                "UPDATE conversations SET title = ?1 WHERE id = ?2",
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
            "SELECT length(CAST(c.id AS BLOB)), length(CAST(c.title AS BLOB)),
                    length(CAST(c.model AS BLOB)), length(CAST(c.reasoning_mode AS BLOB)),
                    length(CAST(c.created_at AS BLOB)), length(CAST(c.updated_at AS BLOB)),
                    c.id, c.title, c.model, c.reasoning_mode, c.created_at, c.updated_at,
                    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id)
             FROM conversations c WHERE c.id = ?1",
            [conversation_id],
            summary_from_row,
        )
        .optional()?
        .ok_or(AppError::NotFound("Conversation not found."))?;
    let mut statement = connection.prepare(
        "SELECT position, length(CAST(id AS BLOB)), length(CAST(conversation_id AS BLOB)),
                length(CAST(role AS BLOB)), length(CAST(content AS BLOB)),
                length(CAST(created_at AS BLOB)), length(CAST(status AS BLOB)),
                id, conversation_id, role, content, created_at, status, token_usage
         FROM messages WHERE conversation_id = ?1 ORDER BY position ASC",
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
        model: summary.model,
        reasoning_mode: summary.reasoning_mode,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        message_count: summary.message_count,
        messages,
    })
}

fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<ConversationSummary> {
    let id_bytes = bounded_row_length(row, 0, 36)?;
    let title_bytes = bounded_row_length(row, 1, MAX_TITLE_CHARS * 4)?;
    let model_bytes = bounded_row_length(row, 2, MODEL.len())?;
    let reasoning_bytes = bounded_row_length(row, 3, 16)?;
    let created_bytes = bounded_row_length(row, 4, 64)?;
    let updated_bytes = bounded_row_length(row, 5, 64)?;
    let id: String = row.get(6)?;
    let title: String = row.get(7)?;
    let model: String = row.get(8)?;
    let reasoning: String = row.get(9)?;
    let created_at: String = row.get(10)?;
    let updated_at: String = row.get(11)?;
    let message_count: i64 = row.get(12)?;
    if id.len() != id_bytes
        || title.len() != title_bytes
        || model.len() != model_bytes
        || reasoning.len() != reasoning_bytes
        || created_at.len() != created_bytes
        || updated_at.len() != updated_bytes
        || validate_uuid(&id, "invalid").is_err()
        || normalize_title(&title).is_err()
        || model != MODEL
    {
        return Err(invalid_db_value(0));
    }
    let created = validate_stored_timestamp(&created_at).ok_or_else(|| invalid_db_value(10))?;
    let updated = validate_stored_timestamp(&updated_at).ok_or_else(|| invalid_db_value(11))?;
    if updated < created
        || message_count < 0
        || message_count as usize > MAX_MESSAGES_PER_CONVERSATION
    {
        return Err(invalid_db_value(12));
    }
    Ok(ConversationSummary {
        id,
        title,
        model,
        reasoning_mode: ReasoningMode::parse(&reasoning).ok_or_else(|| invalid_db_value(3))?,
        created_at,
        updated_at,
        message_count: u64::try_from(message_count).map_err(|_| invalid_db_value(6))?,
    })
}

fn message_from_row(row: &Row<'_>, expected_conversation_id: &str) -> rusqlite::Result<Message> {
    let id_bytes = bounded_row_length(row, 1, 36)?;
    let conversation_id_bytes = bounded_row_length(row, 2, 36)?;
    let role_bytes = bounded_row_length(row, 3, 16)?;
    let content_bytes = bounded_row_length(row, 4, MAX_STORED_MESSAGE_BYTES)?;
    let created_bytes = bounded_row_length(row, 5, 64)?;
    let status_bytes = bounded_row_length(row, 6, 16)?;
    let id: String = row.get(7)?;
    let conversation_id: String = row.get(8)?;
    let role: String = row.get(9)?;
    let content: String = row.get(10)?;
    let created_at: String = row.get(11)?;
    let status: String = row.get(12)?;
    let token_usage: Option<i64> = row.get(13)?;
    if id.len() != id_bytes
        || conversation_id.len() != conversation_id_bytes
        || role.len() != role_bytes
        || content.len() != content_bytes
        || created_at.len() != created_bytes
        || status.len() != status_bytes
        || conversation_id != expected_conversation_id
        || validate_uuid(&id, "invalid").is_err()
        || validate_uuid(&conversation_id, "invalid").is_err()
        || validate_stored_timestamp(&created_at).is_none()
    {
        return Err(invalid_db_value(0));
    }
    let role = MessageRole::parse(&role).ok_or_else(|| invalid_db_value(9))?;
    let status = MessageStatus::parse(&status).ok_or_else(|| invalid_db_value(12))?;
    let content_valid = match status {
        MessageStatus::Complete => validate_message_content(&content, MAX_STORED_MESSAGE_BYTES),
        MessageStatus::Cancelled | MessageStatus::Error => {
            validate_terminal_content(&content, MAX_STORED_MESSAGE_BYTES)
        }
    };
    if content_valid.is_err() || (role == MessageRole::User && status != MessageStatus::Complete) {
        return Err(invalid_db_value(10));
    }
    Ok(Message {
        id,
        conversation_id,
        role,
        content,
        created_at,
        status,
        token_usage: token_usage
            .map(u64::try_from)
            .transpose()
            .map_err(|_| invalid_db_value(6))?,
    })
}

fn validate_import_header(bundle: &ImportBundle) -> AppResult<()> {
    if bundle.format != "aster-conversation" || bundle.version != 1 {
        return Err(AppError::Validation(
            "The import format or schema version is not supported.",
        ));
    }
    validate_timestamp(&bundle.exported_at)?;
    if bundle.conversations.is_empty() || bundle.conversations.len() > MAX_IMPORT_CONVERSATIONS {
        return Err(AppError::Validation(
            "The import must contain between 1 and 100 conversations.",
        ));
    }
    Ok(())
}

fn validate_import_conversations(bundle: &ImportBundle) -> AppResult<()> {
    for conversation in &bundle.conversations {
        normalize_title(&conversation.title)?;
        let created_at = parse_timestamp(&conversation.created_at)?;
        let updated_at = parse_timestamp(&conversation.updated_at)?;
        if updated_at < created_at {
            return Err(AppError::Validation(
                "An imported conversation update timestamp precedes its creation timestamp.",
            ));
        }
        if conversation.model != MODEL {
            return Err(AppError::Validation(
                "The import uses a model that this Aster version does not support.",
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
            if message
                .token_usage
                .is_some_and(|usage| usage > MAX_TOKEN_USAGE)
            {
                return Err(AppError::Validation("Token usage is out of range."));
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

fn validate_message_content(content: &str, max_bytes: usize) -> AppResult<()> {
    if content.trim().is_empty() {
        return Err(AppError::Validation("Message content cannot be empty."));
    }
    if content.len() > max_bytes {
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

fn validate_terminal_content(content: &str, max_bytes: usize) -> AppResult<()> {
    if content.len() > max_bytes {
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
    if parsed.to_string() != value.to_ascii_lowercase() {
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
    Ok(parse_timestamp(value)?
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn validate_stored_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    if parsed.offset().local_minus_utc() != 0 {
        return None;
    }
    let utc = parsed.with_timezone(&Utc);
    if utc.to_rfc3339_opts(SecondsFormat::Millis, true) != value {
        return None;
    }
    Some(utc)
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn invalid_db_value(index: usize) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            "database value violates the application schema",
        )),
    )
}

fn bounded_row_length(row: &Row<'_>, index: usize, maximum: usize) -> rusqlite::Result<usize> {
    let length: i64 = row.get(index)?;
    let length = usize::try_from(length).map_err(|_| invalid_db_value(index))?;
    if length > maximum {
        return Err(invalid_db_value(index));
    }
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Database {
        Database::open_in_memory().expect("test database should open")
    }

    #[test]
    fn conversation_crud_and_messages_are_persistent() {
        let database = database();
        let conversation = database
            .create_conversation(None)
            .expect("conversation should be created");
        let prepared = database
            .prepare_generation(
                &conversation.id,
                "Review this threat model.".to_owned(),
                ReasoningMode::Deep,
                None,
            )
            .expect("generation should be prepared");
        assert_eq!(prepared.history.len(), 1);
        assert_eq!(prepared.reasoning_mode, ReasoningMode::Deep);

        database
            .append_assistant_message(
                &conversation.id,
                "Start with assets and trust boundaries.".to_owned(),
                MessageStatus::Complete,
                Some(42),
            )
            .expect("assistant message should be stored");
        let loaded = database
            .get_conversation(&conversation.id)
            .expect("conversation should load");
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.reasoning_mode, ReasoningMode::Deep);
        assert_eq!(loaded.title, "Review this threat model.");
        assert!(matches!(
            database.get_conversation_for_export(&conversation.id, 64),
            Err(AppError::Validation(_))
        ));
        assert!(
            database
                .get_conversation_for_export(&conversation.id, 1024 * 1024)
                .is_ok()
        );

        database
            .rename_conversation(&conversation.id, "Security review".to_owned())
            .expect("rename should work");
        assert_eq!(database.list_conversations().expect("list").len(), 1);
        database
            .delete_conversation(&conversation.id)
            .expect("delete should work");
        assert!(database.list_conversations().expect("list").is_empty());
    }

    #[test]
    fn editing_a_user_message_removes_all_descendants_atomically() {
        let database = database();
        let conversation = database.create_conversation(None).expect("create");
        database
            .prepare_generation(
                &conversation.id,
                "First prompt".to_owned(),
                ReasoningMode::Standard,
                None,
            )
            .expect("prepare");
        database
            .append_assistant_message(
                &conversation.id,
                "First answer".to_owned(),
                MessageStatus::Complete,
                None,
            )
            .expect("append");
        database
            .prepare_generation(
                &conversation.id,
                "Second prompt".to_owned(),
                ReasoningMode::Standard,
                None,
            )
            .expect("prepare");
        let before = database.get_conversation(&conversation.id).expect("load");
        let first_user_id = before.messages[0].id.clone();

        let prepared = database
            .prepare_generation(
                &conversation.id,
                "Revised first prompt".to_owned(),
                ReasoningMode::Fast,
                Some(first_user_id),
            )
            .expect("revision");
        assert_eq!(prepared.history.len(), 1);
        assert_eq!(prepared.history[0].content, "Revised first prompt");
        let after = database.get_conversation(&conversation.id).expect("load");
        assert_eq!(after.messages.len(), 1);
    }

    #[test]
    fn regenerating_latest_assistant_reuses_the_preceding_user_prompt() {
        let database = database();
        let conversation = database.create_conversation(None).expect("create");
        database
            .prepare_generation(
                &conversation.id,
                "Original prompt".to_owned(),
                ReasoningMode::Standard,
                None,
            )
            .expect("prepare");
        let assistant = database
            .append_assistant_message(
                &conversation.id,
                "Original answer".to_owned(),
                MessageStatus::Complete,
                None,
            )
            .expect("append");

        let prepared = database
            .prepare_generation(
                &conversation.id,
                "This caller content is not duplicated".to_owned(),
                ReasoningMode::Deep,
                Some(assistant.id),
            )
            .expect("regenerate");
        assert_eq!(prepared.history.len(), 1);
        assert_eq!(prepared.history[0].role, MessageRole::User);
        assert_eq!(prepared.history[0].content, "Original prompt");
        assert_eq!(
            database
                .get_conversation(&conversation.id)
                .expect("load")
                .messages
                .len(),
            1
        );
    }

    #[test]
    fn invalid_regeneration_history_rolls_back_descendant_deletion() {
        let database = database();
        let conversation = database.create_conversation(None).expect("create");
        database
            .prepare_generation(
                &conversation.id,
                "Prompt".to_owned(),
                ReasoningMode::Standard,
                None,
            )
            .expect("prepare");
        database
            .append_assistant_message(
                &conversation.id,
                "First adjacent assistant".to_owned(),
                MessageStatus::Complete,
                None,
            )
            .expect("append first");
        let latest = database
            .append_assistant_message(
                &conversation.id,
                "Second adjacent assistant".to_owned(),
                MessageStatus::Complete,
                None,
            )
            .expect("append second");
        assert!(
            database
                .prepare_generation(
                    &conversation.id,
                    "Ignored".to_owned(),
                    ReasoningMode::Standard,
                    Some(latest.id.clone()),
                )
                .is_err()
        );
        let after = database.get_conversation(&conversation.id).expect("load");
        assert_eq!(after.messages.len(), 3);
        assert_eq!(after.messages[2].id, latest.id);

        let oversized = "x".repeat(MAX_PROVIDER_HISTORY_BYTES + 1);
        let second = database.create_conversation(None).expect("create second");
        database
            .prepare_generation(
                &second.id,
                "Prompt".to_owned(),
                ReasoningMode::Standard,
                None,
            )
            .expect("prepare second");
        database
            .append_assistant_message(&second.id, oversized, MessageStatus::Complete, None)
            .expect("append oversized");
        let latest = database
            .append_assistant_message(
                &second.id,
                "Latest".to_owned(),
                MessageStatus::Complete,
                None,
            )
            .expect("append latest");
        assert!(
            database
                .prepare_generation(
                    &second.id,
                    "Ignored".to_owned(),
                    ReasoningMode::Standard,
                    Some(latest.id.clone()),
                )
                .is_err()
        );
        let after = database.get_conversation(&second.id).expect("load second");
        assert_eq!(after.messages.len(), 3);
        assert_eq!(after.messages[2].id, latest.id);
    }

    #[test]
    fn invalid_import_does_not_commit_partial_data() {
        let database = database();
        let malformed = ImportBundle {
            format: "aster-conversation".to_owned(),
            version: 1,
            exported_at: "2026-07-11T00:00:00Z".to_owned(),
            conversations: vec![crate::models::ImportConversation {
                title: "Imported".to_owned(),
                model: MODEL.to_owned(),
                reasoning_mode: ReasoningMode::Standard,
                created_at: "not-a-timestamp".to_owned(),
                updated_at: "2026-07-11T00:00:00Z".to_owned(),
                messages: Vec::new(),
            }],
        };
        assert!(database.import(malformed).is_err());
        assert!(database.list_conversations().expect("list").is_empty());
    }

    #[test]
    fn valid_import_generates_new_ids_and_normalizes_timestamps() {
        let database = database();
        let bundle = ImportBundle {
            format: "aster-conversation".to_owned(),
            version: 1,
            exported_at: "2026-07-11T01:00:00+01:00".to_owned(),
            conversations: vec![crate::models::ImportConversation {
                title: "Imported".to_owned(),
                model: MODEL.to_owned(),
                reasoning_mode: ReasoningMode::Fast,
                created_at: "2026-07-11T01:00:00+01:00".to_owned(),
                updated_at: "2026-07-11T02:00:00+01:00".to_owned(),
                messages: vec![crate::models::ImportMessage {
                    role: MessageRole::User,
                    content: "Imported prompt".to_owned(),
                    created_at: "2026-07-11T01:30:00+01:00".to_owned(),
                    status: MessageStatus::Complete,
                    token_usage: None,
                }],
            }],
        };
        let imported = database.import(bundle).expect("valid import");
        assert_eq!(imported.len(), 1);
        assert!(Uuid::parse_str(&imported[0].id).is_ok());
        assert_eq!(imported[0].created_at, "2026-07-11T00:00:00.000Z");
        let conversation = database
            .get_conversation(&imported[0].id)
            .expect("imported conversation");
        assert_eq!(conversation.messages.len(), 1);
        assert!(Uuid::parse_str(&conversation.messages[0].id).is_ok());
        assert_eq!(
            conversation.messages[0].created_at,
            "2026-07-11T00:30:00.000Z"
        );
    }

    #[test]
    fn newer_database_schema_is_rejected_without_replacement() {
        let connection = Connection::open_in_memory().expect("open");
        connection
            .pragma_update(None, "user_version", 2)
            .expect("set version");
        assert!(matches!(
            Database::initialize(connection),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn version_one_schema_with_weakened_constraints_is_rejected() {
        let connection = Connection::open_in_memory().expect("open");
        let weakened = MIGRATION_1.replace(
            "CHECK(length(content) <= 2097152)",
            "CHECK(length(content) <= 3145728)",
        );
        connection
            .execute_batch(&weakened)
            .expect("weakened schema should be syntactically valid");
        assert!(matches!(
            Database::initialize(connection),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn oversized_stored_content_is_rejected_before_content_is_read() {
        let database = database();
        let conversation = database.create_conversation(None).expect("conversation");
        let oversized = "x".repeat(MAX_STORED_MESSAGE_BYTES + 1);
        {
            let connection = database.lock().expect("connection");
            connection
                .execute_batch("PRAGMA ignore_check_constraints = ON")
                .expect("test-only constraint bypass");
            connection
                .execute(
                    "INSERT INTO messages
                     (id, conversation_id, position, role, content, token_usage, created_at, status)
                     VALUES (?1, ?2, 0, 'assistant', ?3, NULL, ?4, 'complete')",
                    params![
                        Uuid::new_v4().to_string(),
                        conversation.id,
                        oversized,
                        now_utc()
                    ],
                )
                .expect("test corruption should be inserted");
        }
        assert!(matches!(
            database.get_conversation(&conversation.id),
            Err(AppError::Database(_))
        ));
    }

    #[test]
    fn malformed_stored_conversation_metadata_is_rejected_on_read() {
        let database = database();
        let conversation = database.create_conversation(None).expect("conversation");
        {
            let connection = database.lock().expect("connection");
            connection
                .execute_batch("PRAGMA ignore_check_constraints = ON")
                .expect("test-only constraint bypass");
            connection
                .execute(
                    "UPDATE conversations SET model = 'unsupported' WHERE id = ?1",
                    [&conversation.id],
                )
                .expect("test corruption should be inserted");
        }
        assert!(matches!(
            database.list_conversations(),
            Err(AppError::Database(_))
        ));
    }

    #[test]
    fn preexisting_foreign_key_violation_is_rejected_on_open() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("foreign-key-violation.sqlite3");
        drop(Database::open(&path).expect("initialize database"));

        let connection = Connection::open(&path).expect("reopen without app pragmas");
        connection
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("disable foreign keys for corruption fixture");
        connection
            .execute(
                "INSERT INTO messages
                 (id, conversation_id, position, role, content, token_usage, created_at, status)
                 VALUES (?1, ?2, 0, 'user', 'orphan', NULL, ?3, 'complete')",
                params![
                    Uuid::new_v4().to_string(),
                    Uuid::new_v4().to_string(),
                    now_utc()
                ],
            )
            .expect("orphan fixture should be inserted");
        drop(connection);

        assert!(matches!(
            Database::open(&path),
            Err(AppError::DatabaseIntegrity)
        ));
    }
}
