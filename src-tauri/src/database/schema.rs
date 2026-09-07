use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use rusqlite::limits::Limit;
use rusqlite::{Connection, MAIN_DB, OpenFlags, Transaction, params};

use super::{
    DEFAULT_MODEL, MAX_MESSAGES_PER_CONVERSATION, MAX_STORED_MESSAGE_BYTES, normalize_title,
    validate_message_content, validate_stored_timestamp, validate_terminal_content, validate_uuid,
};
use crate::error::{AppError, AppResult};
use crate::models::{MAX_SAFE_INTEGER, MessageRole, MessageStatus, ResponseProfile};

const MAX_SQLITE_VALUE_BYTES: i32 = 3 * 1024 * 1024;
const MIGRATION_CHECKSUM: &str = "aster-schema-v2-2026-07-14-r3";

const CONNECTION_PRAGMAS: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA secure_delete = ON;
PRAGMA trusted_schema = OFF;
PRAGMA temp_store = MEMORY;
"#;

const V2_SCHEMA: &str = r#"
CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 256),
    provider_id TEXT NOT NULL CHECK(provider_id IN ('zai','deepseek','alibaba-us','google','nvidia')),
    model_id TEXT NOT NULL,
    response_profile TEXT NOT NULL CHECK(response_profile IN ('fast','standard','deep')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(
        (provider_id = 'zai' AND model_id IN ('glm-4.7','glm-5','glm-5.1','glm-5.2')) OR
        (provider_id = 'deepseek' AND model_id IN ('deepseek-v4-flash','deepseek-v4-pro')) OR
        (provider_id = 'alibaba-us' AND model_id IN ('qwen3.5-plus','qwen3.5-flash','qwen3.6-plus','qwen3.6-flash','qwen3.7-plus','qwen3.7-max')) OR
        (provider_id = 'google' AND model_id IN ('gemini-2.5-flash','gemini-2.5-flash-lite','gemini-2.5-pro')) OR
        (provider_id = 'nvidia' AND model_id IN ('nvidia/nemotron-3-super-120b-a12b','nvidia/nemotron-3-ultra-550b-a55b'))
    )
) STRICT;

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position >= 0),
    role TEXT NOT NULL CHECK(role IN ('user','assistant')),
    content TEXT NOT NULL CHECK(length(content) <= 2097152),
    input_tokens INTEGER CHECK(input_tokens IS NULL OR input_tokens BETWEEN 0 AND 9007199254740991),
    cached_input_tokens INTEGER CHECK(cached_input_tokens IS NULL OR cached_input_tokens BETWEEN 0 AND 9007199254740991),
    output_tokens INTEGER CHECK(output_tokens IS NULL OR output_tokens BETWEEN 0 AND 9007199254740991),
    total_tokens INTEGER CHECK(total_tokens IS NULL OR total_tokens BETWEEN 0 AND 9007199254740991),
    created_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('complete','cancelled','error')),
    finish_reason TEXT CHECK(finish_reason IN ('stop','output_limit','unknown')),
    CHECK(NOT(input_tokens IS NOT NULL AND cached_input_tokens IS NOT NULL AND output_tokens IS NOT NULL AND total_tokens IS NOT NULL)
          OR total_tokens = input_tokens + cached_input_tokens + output_tokens),
    CHECK(role = 'assistant' OR status = 'complete'),
    CHECK((role = 'assistant' AND status = 'complete') OR
          (input_tokens IS NULL AND cached_input_tokens IS NULL AND output_tokens IS NULL AND total_tokens IS NULL)),
    CHECK((role = 'assistant' AND status = 'complete') = (finish_reason IS NOT NULL)),
    UNIQUE(conversation_id, position)
) STRICT;

CREATE TABLE usage_observations (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK(
        length(operation_id) = 36 AND lower(operation_id) = operation_id AND
        substr(operation_id,9,1) = '-' AND substr(operation_id,14,1) = '-' AND
        substr(operation_id,19,1) = '-' AND substr(operation_id,24,1) = '-' AND
        length(replace(operation_id,'-','')) = 32 AND
        replace(operation_id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    provider_id TEXT NOT NULL CHECK(provider_id IN ('zai','deepseek','alibaba-us','google','nvidia')),
    model_id TEXT NOT NULL,
    observed_at TEXT NOT NULL CHECK(
        length(observed_at) = 24 AND observed_at GLOB
        '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9]Z'
    ),
    input_tokens INTEGER CHECK(input_tokens IS NULL OR input_tokens BETWEEN 0 AND 9007199254740991),
    cached_input_tokens INTEGER CHECK(cached_input_tokens IS NULL OR cached_input_tokens BETWEEN 0 AND 9007199254740991),
    output_tokens INTEGER CHECK(output_tokens IS NULL OR output_tokens BETWEEN 0 AND 9007199254740991),
    total_tokens INTEGER CHECK(total_tokens IS NULL OR total_tokens BETWEEN 0 AND 9007199254740991),
    partial INTEGER NOT NULL CHECK(partial IN (0,1)),
    CHECK(
        (provider_id = 'zai' AND model_id IN ('glm-4.7','glm-5','glm-5.1','glm-5.2')) OR
        (provider_id = 'deepseek' AND model_id IN ('deepseek-v4-flash','deepseek-v4-pro')) OR
        (provider_id = 'alibaba-us' AND model_id IN ('qwen3.5-plus','qwen3.5-flash','qwen3.6-plus','qwen3.6-flash','qwen3.7-plus','qwen3.7-max')) OR
        (provider_id = 'google' AND model_id IN ('gemini-2.5-flash','gemini-2.5-flash-lite','gemini-2.5-pro')) OR
        (provider_id = 'nvidia' AND model_id IN ('nvidia/nemotron-3-super-120b-a12b','nvidia/nemotron-3-ultra-550b-a55b'))
    ),
    CHECK(
        (partial = 0 AND input_tokens IS NOT NULL AND cached_input_tokens IS NOT NULL AND output_tokens IS NOT NULL AND total_tokens = input_tokens + cached_input_tokens + output_tokens) OR
        (partial = 1 AND NOT(input_tokens IS NOT NULL AND cached_input_tokens IS NOT NULL AND output_tokens IS NOT NULL AND total_tokens IS NOT NULL))
    )
) STRICT;

CREATE TABLE provider_preferences (
    provider_id TEXT PRIMARY KEY NOT NULL CHECK(provider_id IN ('zai','deepseek','alibaba-us','google','nvidia')),
    weekly_token_budget INTEGER CHECK(weekly_token_budget IS NULL OR weekly_token_budget BETWEEN 1 AND 9007199254740991),
    notice_version INTEGER NOT NULL DEFAULT 0 CHECK(notice_version >= 0)
) STRICT;

CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL CHECK(version = 2),
    applied_at TEXT NOT NULL,
    checksum TEXT NOT NULL CHECK(length(checksum) BETWEEN 1 AND 128)
) STRICT;

CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC, id);
CREATE INDEX idx_messages_conversation_position ON messages(conversation_id, position);
CREATE INDEX idx_usage_provider_model_time ON usage_observations(provider_id, model_id, observed_at);

CREATE TRIGGER lock_conversation_pair
BEFORE UPDATE OF provider_id, model_id ON conversations
WHEN OLD.provider_id <> NEW.provider_id OR OLD.model_id <> NEW.model_id
BEGIN
    SELECT CASE WHEN EXISTS(SELECT 1 FROM messages WHERE conversation_id = OLD.id)
        THEN RAISE(ABORT, 'conversation_model_locked') END;
END;
"#;

pub(super) fn open_path(path: &Path) -> AppResult<Connection> {
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    prepare_connection(&connection)?;
    let version = user_version(&connection)?;
    match version {
        0 => create_v2(&mut connection)?,
        1 => migrate_v1(path, &mut connection)?,
        2 => {
            verify_v2(&mut connection)?;
            clean_verified_orphan(path, &connection)?;
        }
        _ => return Err(AppError::DatabaseIntegrity),
    }
    verify_integrity(&connection)?;
    Ok(connection)
}

#[cfg(test)]
pub(super) fn open_memory() -> AppResult<Connection> {
    let mut connection = Connection::open_in_memory()?;
    prepare_connection(&connection)?;
    create_v2(&mut connection)?;
    Ok(connection)
}

fn prepare_connection(connection: &Connection) -> AppResult<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    configure_limits(connection)?;
    connection.execute_batch(CONNECTION_PRAGMAS)?;
    Ok(())
}

fn configure_limits(connection: &Connection) -> AppResult<()> {
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

fn create_v2(connection: &mut Connection) -> AppResult<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(V2_SCHEMA)?;
    record_migration(&transaction)?;
    transaction.pragma_update(None, "user_version", 2)?;
    transaction.commit()?;
    verify_v2(connection)
}

fn migrate_v1(path: &Path, connection: &mut Connection) -> AppResult<()> {
    verify_v1(connection)?;
    let backup_path = backup_path(path)?;
    if backup_path.exists() {
        let backup = open_verified_v1_backup(&backup_path)?;
        compare_v1_databases(connection, &backup)?;
    } else {
        connection.backup(MAIN_DB, &backup_path, None)?;
        let backup = open_verified_v1_backup(&backup_path)?;
        compare_v1_databases(connection, &backup)?;
    }
    validate_v1_rows(connection)?;

    let transaction = connection.transaction()?;
    transaction.execute_batch(
        r#"
        DROP INDEX idx_messages_conversation_position;
        DROP INDEX idx_conversations_updated;
        ALTER TABLE messages RENAME TO messages_v1;
        ALTER TABLE conversations RENAME TO conversations_v1;
        "#,
    )?;
    transaction.execute_batch(V2_SCHEMA)?;
    transaction.execute(
        "INSERT INTO conversations
         (id,title,provider_id,model_id,response_profile,created_at,updated_at)
         SELECT id,title,'zai','glm-5.1',reasoning_mode,created_at,updated_at
         FROM conversations_v1",
        [],
    )?;
    transaction.execute(
        "INSERT INTO messages
         (id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status,finish_reason)
         SELECT id,conversation_id,position,role,content,NULL,NULL,NULL,token_usage,created_at,status,
                CASE WHEN role='assistant' AND status='complete' THEN 'unknown' ELSE NULL END
         FROM messages_v1",
        [],
    )?;
    transaction.execute_batch("DROP TABLE messages_v1; DROP TABLE conversations_v1;")?;
    record_migration(&transaction)?;
    transaction.pragma_update(None, "user_version", 2)?;
    transaction.commit()?;

    verify_v2(connection)?;
    verify_integrity(connection)?;
    let backup = open_verified_v1_backup(&backup_path)?;
    compare_v1_to_v2(&backup, connection)?;
    drop(backup);
    fs::remove_file(&backup_path)?;
    Ok(())
}

fn record_migration(transaction: &Transaction<'_>) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO schema_migrations(version,applied_at,checksum) VALUES(2,?1,?2)",
        params![now_utc(), MIGRATION_CHECKSUM],
    )?;
    Ok(())
}

fn backup_path(path: &Path) -> AppResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or(AppError::DatabaseIntegrity)?;
    Ok(path.with_file_name(format!("{file_name}.v1-backup.sqlite3")))
}

fn open_verified_v1_backup(path: &Path) -> AppResult<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(5))?;
    verify_v1(&connection)?;
    verify_integrity(&connection)?;
    validate_v1_rows(&connection)?;
    Ok(connection)
}

fn clean_verified_orphan(path: &Path, v2: &Connection) -> AppResult<()> {
    let backup_path = backup_path(path)?;
    if !backup_path.exists() {
        return Ok(());
    }
    let backup = open_verified_v1_backup(&backup_path)?;
    compare_v1_to_v2(&backup, v2)?;
    drop(backup);
    fs::remove_file(backup_path)?;
    Ok(())
}

fn user_version(connection: &Connection) -> AppResult<u32> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

fn verify_integrity(connection: &Connection) -> AppResult<()> {
    let integrity: String = connection.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
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
    Ok(())
}

fn verify_v1(connection: &Connection) -> AppResult<()> {
    if user_version(connection)? != 1 {
        return Err(AppError::DatabaseIntegrity);
    }
    verify_objects(
        connection,
        &[
            ("index", "idx_conversations_updated", "conversations"),
            ("index", "idx_messages_conversation_position", "messages"),
            ("table", "conversations", "conversations"),
            ("table", "messages", "messages"),
        ],
    )?;
    verify_columns(
        connection,
        "conversations",
        &[
            "id",
            "title",
            "model",
            "reasoning_mode",
            "created_at",
            "updated_at",
        ],
    )?;
    verify_columns(
        connection,
        "messages",
        &[
            "id",
            "conversation_id",
            "position",
            "role",
            "content",
            "token_usage",
            "created_at",
            "status",
        ],
    )?;
    verify_sql_fragments(
        connection,
        "conversations",
        &["strict", "check(model = 'glm-5.1')", "reasoning_mode"],
    )?;
    verify_sql_fragments(
        connection,
        "messages",
        &["strict", "unique(conversation_id, position)", "token_usage"],
    )
}

fn validate_v1_rows(connection: &Connection) -> AppResult<()> {
    let mut conversations = connection.prepare(
        "SELECT id,title,model,reasoning_mode,created_at,updated_at,
                (SELECT COUNT(*) FROM messages WHERE conversation_id=conversations.id)
         FROM conversations ORDER BY id",
    )?;
    let mut rows = conversations.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let model: String = row.get(2)?;
        let profile: String = row.get(3)?;
        let created_at: String = row.get(4)?;
        let updated_at: String = row.get(5)?;
        let message_count: i64 = row.get(6)?;
        let normalized_title = normalize_title(&title).map_err(|_| AppError::DatabaseIntegrity)?;
        let created = validate_stored_timestamp(&created_at);
        let updated = validate_stored_timestamp(&updated_at);
        if validate_uuid(&id, "invalid").is_err()
            || normalized_title != title
            || model != DEFAULT_MODEL
            || ResponseProfile::parse(&profile).is_none()
            || created.is_none()
            || updated.is_none()
            || updated < created
            || message_count < 0
            || message_count as usize > MAX_MESSAGES_PER_CONVERSATION
        {
            return Err(AppError::DatabaseIntegrity);
        }
    }

    let mut messages = connection.prepare(
        "SELECT id,conversation_id,position,role,content,token_usage,created_at,status
         FROM messages ORDER BY conversation_id,position",
    )?;
    let mut rows = messages.query([])?;
    let mut current_conversation = None::<String>;
    let mut expected_position = 0i64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let conversation_id: String = row.get(1)?;
        let position: i64 = row.get(2)?;
        let role: String = row.get(3)?;
        let content: String = row.get(4)?;
        let token_usage: Option<i64> = row.get(5)?;
        let created_at: String = row.get(6)?;
        let status: String = row.get(7)?;
        if current_conversation.as_deref() != Some(conversation_id.as_str()) {
            current_conversation = Some(conversation_id.clone());
            expected_position = 0;
        }
        let role = MessageRole::parse(&role).ok_or(AppError::DatabaseIntegrity)?;
        let status = MessageStatus::parse(&status).ok_or(AppError::DatabaseIntegrity)?;
        let content_is_valid = match status {
            MessageStatus::Complete => {
                validate_message_content(&content, MAX_STORED_MESSAGE_BYTES).is_ok()
            }
            MessageStatus::Cancelled | MessageStatus::Error => {
                validate_terminal_content(&content, MAX_STORED_MESSAGE_BYTES).is_ok()
            }
        };
        let token_is_valid = match token_usage {
            Some(value) => {
                value >= 0
                    && u64::try_from(value).is_ok_and(|value| value <= MAX_SAFE_INTEGER)
                    && role == MessageRole::Assistant
                    && status == MessageStatus::Complete
            }
            None => true,
        };
        if validate_uuid(&id, "invalid").is_err()
            || validate_uuid(&conversation_id, "invalid").is_err()
            || position != expected_position
            || validate_stored_timestamp(&created_at).is_none()
            || !content_is_valid
            || (role == MessageRole::User && status != MessageStatus::Complete)
            || !token_is_valid
        {
            return Err(AppError::DatabaseIntegrity);
        }
        expected_position = expected_position
            .checked_add(1)
            .ok_or(AppError::DatabaseIntegrity)?;
    }
    Ok(())
}

fn verify_v2(connection: &mut Connection) -> AppResult<()> {
    if user_version(connection)? != 2 {
        return Err(AppError::DatabaseIntegrity);
    }
    verify_objects(
        connection,
        &[
            ("index", "idx_conversations_updated", "conversations"),
            ("index", "idx_messages_conversation_position", "messages"),
            (
                "index",
                "idx_usage_provider_model_time",
                "usage_observations",
            ),
            ("table", "conversations", "conversations"),
            ("table", "messages", "messages"),
            ("table", "provider_preferences", "provider_preferences"),
            ("table", "schema_migrations", "schema_migrations"),
            ("table", "usage_observations", "usage_observations"),
            ("trigger", "lock_conversation_pair", "conversations"),
        ],
    )?;
    verify_columns(
        connection,
        "conversations",
        &[
            "id",
            "title",
            "provider_id",
            "model_id",
            "response_profile",
            "created_at",
            "updated_at",
        ],
    )?;
    verify_columns(
        connection,
        "messages",
        &[
            "id",
            "conversation_id",
            "position",
            "role",
            "content",
            "input_tokens",
            "cached_input_tokens",
            "output_tokens",
            "total_tokens",
            "created_at",
            "status",
            "finish_reason",
        ],
    )?;
    verify_columns(
        connection,
        "usage_observations",
        &[
            "operation_id",
            "provider_id",
            "model_id",
            "observed_at",
            "input_tokens",
            "cached_input_tokens",
            "output_tokens",
            "total_tokens",
            "partial",
        ],
    )?;
    verify_columns(
        connection,
        "provider_preferences",
        &["provider_id", "weekly_token_budget", "notice_version"],
    )?;
    verify_columns(
        connection,
        "schema_migrations",
        &["version", "applied_at", "checksum"],
    )?;
    verify_sql_fragments(
        connection,
        "conversations",
        &["strict", "provider_id", "model_id", "gemini-2.5-pro"],
    )?;
    verify_sql_fragments(
        connection,
        "messages",
        &["strict", "cached_input_tokens", "9007199254740991"],
    )?;
    let migration: (String, String) = connection.query_row(
        "SELECT applied_at, checksum FROM schema_migrations WHERE version = 2",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if migration.1 != MIGRATION_CHECKSUM
        || chrono::DateTime::parse_from_rfc3339(&migration.0).is_err()
    {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

fn verify_objects(connection: &Connection, expected: &[(&str, &str, &str)]) -> AppResult<()> {
    let mut statement = connection.prepare(
        "SELECT type,name,tbl_name FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
    )?;
    let actual = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let expected = expected
        .iter()
        .map(|(kind, name, table)| ((*kind).to_owned(), (*name).to_owned(), (*table).to_owned()))
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

fn verify_columns(connection: &Connection, table: &str, expected: &[&str]) -> AppResult<()> {
    let sql = format!("PRAGMA table_xinfo({table})");
    let mut statement = connection.prepare(&sql)?;
    let actual = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if actual != expected {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

fn verify_sql_fragments(connection: &Connection, table: &str, fragments: &[&str]) -> AppResult<()> {
    let sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
        [table],
        |row| row.get(0),
    )?;
    let normalized = sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if fragments
        .iter()
        .any(|fragment| !normalized.contains(&fragment.to_ascii_lowercase()))
    {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

type V1ConversationRow = (String, String, String, String, String, String);
type V1MessageRow = (
    String,
    String,
    i64,
    String,
    String,
    Option<i64>,
    String,
    String,
);
type V2MessageRow = (
    String,
    String,
    i64,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    Option<String>,
);

fn compare_v1_databases(left: &Connection, right: &Connection) -> AppResult<()> {
    let conversations_sql =
        "SELECT id,title,model,reasoning_mode,created_at,updated_at FROM conversations ORDER BY id";
    compare_rows::<V1ConversationRow>(left, right, conversations_sql, |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ))
    })?;
    let messages_sql = "SELECT id,conversation_id,position,role,content,token_usage,created_at,status FROM messages ORDER BY id";
    compare_rows::<V1MessageRow>(left, right, messages_sql, |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
        ))
    })
}

fn compare_rows<T>(
    left: &Connection,
    right: &Connection,
    sql: &str,
    mapper: fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> AppResult<()>
where
    T: PartialEq,
{
    let mut left_statement = left.prepare(sql)?;
    let mut right_statement = right.prepare(sql)?;
    let mut left_rows = left_statement.query([])?;
    let mut right_rows = right_statement.query([])?;
    loop {
        match (left_rows.next()?, right_rows.next()?) {
            (None, None) => return Ok(()),
            (Some(left), Some(right)) if mapper(left)? == mapper(right)? => {}
            _ => return Err(AppError::DatabaseIntegrity),
        }
    }
}

fn compare_v1_to_v2(v1: &Connection, v2: &Connection) -> AppResult<()> {
    let v1_count: i64 = v1.query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))?;
    let v2_count: i64 = v2.query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))?;
    let v1_messages: i64 = v1.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
    let v2_messages: i64 = v2.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
    let observations: i64 = v2.query_row("SELECT COUNT(*) FROM usage_observations", [], |row| {
        row.get(0)
    })?;
    let preferences: i64 =
        v2.query_row("SELECT COUNT(*) FROM provider_preferences", [], |row| {
            row.get(0)
        })?;
    if v1_count != v2_count || v1_messages != v2_messages || observations != 0 || preferences != 0 {
        return Err(AppError::DatabaseIntegrity);
    }

    let mut old = v1.prepare(
        "SELECT id,title,reasoning_mode,created_at,updated_at FROM conversations ORDER BY id",
    )?;
    let mut new = v2.prepare(
        "SELECT id,title,provider_id,model_id,response_profile,created_at,updated_at FROM conversations ORDER BY id",
    )?;
    let mut old_rows = old.query([])?;
    let mut new_rows = new.query([])?;
    loop {
        match (old_rows.next()?, new_rows.next()?) {
            (None, None) => break,
            (Some(old), Some(new)) => {
                let old_row: (String, String, String, String, String) = (
                    old.get(0)?,
                    old.get(1)?,
                    old.get(2)?,
                    old.get(3)?,
                    old.get(4)?,
                );
                let new_row: (String, String, String, String, String, String, String) = (
                    new.get(0)?,
                    new.get(1)?,
                    new.get(2)?,
                    new.get(3)?,
                    new.get(4)?,
                    new.get(5)?,
                    new.get(6)?,
                );
                if new_row
                    != (
                        old_row.0,
                        old_row.1,
                        "zai".to_owned(),
                        "glm-5.1".to_owned(),
                        old_row.2,
                        old_row.3,
                        old_row.4,
                    )
                {
                    return Err(AppError::DatabaseIntegrity);
                }
            }
            _ => return Err(AppError::DatabaseIntegrity),
        }
    }

    let mut old = v1.prepare(
        "SELECT id,conversation_id,position,role,content,token_usage,created_at,status FROM messages ORDER BY id",
    )?;
    let mut new = v2.prepare(
        "SELECT id,conversation_id,position,role,content,input_tokens,cached_input_tokens,output_tokens,total_tokens,created_at,status,finish_reason FROM messages ORDER BY id",
    )?;
    let mut old_rows = old.query([])?;
    let mut new_rows = new.query([])?;
    loop {
        match (old_rows.next()?, new_rows.next()?) {
            (None, None) => return Ok(()),
            (Some(old), Some(new)) => {
                let old_row: V1MessageRow = (
                    old.get(0)?,
                    old.get(1)?,
                    old.get(2)?,
                    old.get(3)?,
                    old.get(4)?,
                    old.get(5)?,
                    old.get(6)?,
                    old.get(7)?,
                );
                let new_row: V2MessageRow = (
                    new.get(0)?,
                    new.get(1)?,
                    new.get(2)?,
                    new.get(3)?,
                    new.get(4)?,
                    new.get(5)?,
                    new.get(6)?,
                    new.get(7)?,
                    new.get(8)?,
                    new.get(9)?,
                    new.get(10)?,
                    new.get(11)?,
                );
                let expected_finish_reason = (old_row.3 == "assistant" && old_row.7 == "complete")
                    .then(|| "unknown".to_owned());
                if new_row
                    != (
                        old_row.0,
                        old_row.1,
                        old_row.2,
                        old_row.3,
                        old_row.4,
                        None,
                        None,
                        None,
                        old_row.5,
                        old_row.6,
                        old_row.7,
                        expected_finish_reason,
                    )
                {
                    return Err(AppError::DatabaseIntegrity);
                }
            }
            _ => return Err(AppError::DatabaseIntegrity),
        }
    }
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
pub(super) fn v1_schema_for_tests(connection: &Connection) -> AppResult<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE conversations (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 256),
            model TEXT NOT NULL CHECK(model = 'glm-5.1'),
            reasoning_mode TEXT NOT NULL CHECK(reasoning_mode IN ('fast', 'standard', 'deep')),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        ) STRICT;
        CREATE TABLE messages (
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
        CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC, id);
        CREATE INDEX idx_messages_conversation_position ON messages(conversation_id, position);
        PRAGMA user_version = 1;
        "#,
    )?;
    Ok(())
}
