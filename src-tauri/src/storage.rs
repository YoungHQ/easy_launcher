use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub type StorageState = Arc<Mutex<Storage>>;

const APP_DATA_DIR: &str = "EasyLauncher";
const WEB_SEARCH_QUERY_PLACEHOLDER: &str = "{query}";
const SELECTION_MARK_TEXT_MAX_CHARS: usize = 20_000;
const SELECTION_MARK_STORAGE_LIMIT: usize = 500;
const TODO_TEXT_MAX_CHARS: usize = 20_000;
pub const DEFAULT_AI_MODEL_PROFILE_ID: &str = "default-openai-compatible";
pub const DEFAULT_AI_PROVIDER_ID: &str = "default-openai-compatible-provider";
pub const DEFAULT_AI_ASSISTANT_ID: &str = "default-assistant";
pub const TRANSLATION_AI_ASSISTANT_ID: &str = "translation-assistant";
pub const SUMMARY_AI_ASSISTANT_ID: &str = "summary-assistant";
pub const PROFESSIONAL_EXPLANATION_AI_ASSISTANT_ID: &str = "professional-explanation-assistant";
pub const POLISH_AI_ASSISTANT_ID: &str = "polish-assistant";
pub const KEY_POINTS_AI_ASSISTANT_ID: &str = "key-points-assistant";

#[derive(Debug)]
pub enum StorageError {
    AppDataDirUnavailable,
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Validation(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::AppDataDirUnavailable => {
                write!(formatter, "无法获取 LocalAppData 目录")
            }
            StorageError::Io(error) => write!(formatter, "文件系统错误：{error}"),
            StorageError::Sqlite(error) => write!(formatter, "SQLite 错误：{error}"),
            StorageError::Validation(message) => write!(formatter, "{message}"),
        }
    }
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        StorageError::Io(error)
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        StorageError::Sqlite(error)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub data_dir: String,
    pub database_path: String,
    pub initialized: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCommand {
    pub id: String,
    pub name: String,
    pub command_type: String,
    pub target: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCommandInput {
    pub id: Option<String>,
    pub name: String,
    pub command_type: String,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Phrase {
    pub id: String,
    pub title: String,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
    pub use_count: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhraseInput {
    pub id: Option<String>,
    pub title: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionMark {
    pub id: String,
    pub text: String,
    pub source_app: Option<String>,
    pub created_at: String,
    pub use_count: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionMarkInput {
    pub text: String,
    pub source_app: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub source_app: Option<String>,
    pub remind_at: String,
    pub status: String,
    pub last_notified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoInput {
    pub text: String,
    pub source_app: Option<String>,
    pub remind_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoUpdateInput {
    pub id: String,
    pub text: Option<String>,
    pub remind_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchTemplate {
    pub id: String,
    pub keyword: String,
    pub name: String,
    pub url_template: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchTemplateInput {
    pub id: Option<String>,
    pub keyword: String,
    pub name: String,
    pub url_template: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub target: String,
    pub use_count: i64,
    pub last_used_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionRule {
    pub id: String,
    pub match_type: String,
    pub pattern: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionRuleInput {
    pub id: Option<String>,
    pub match_type: String,
    pub pattern: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedResult {
    pub result_id: String,
    pub kind: String,
    pub title: String,
    pub target: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedResultInput {
    pub result_id: String,
    pub kind: String,
    pub title: String,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultAlias {
    pub alias: String,
    pub normalized_alias: String,
    pub result_id: String,
    pub kind: String,
    pub title: String,
    pub target: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultAliasInput {
    pub alias: String,
    pub result_id: String,
    pub kind: String,
    pub title: String,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelProfile {
    pub id: String,
    pub provider_type: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub stream: bool,
    pub enabled: bool,
    pub sort_order: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelProfileInput {
    pub id: Option<String>,
    pub provider_type: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub stream: bool,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderInput {
    pub id: Option<String>,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderModel {
    pub id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub sort_order: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderModelInput {
    pub id: Option<String>,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSelectionAction {
    pub assistant_id: String,
    pub assistant_name: String,
    pub assistant_icon: String,
    pub assistant_description: String,
    pub assistant_model_profile_id: String,
    pub system_prompt: String,
    pub assistant_enabled: bool,
    pub show_in_selection: bool,
    pub selection_label: String,
    pub sort_order: i64,
    pub last_provider_id: Option<String>,
    pub last_model_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSelectionActionInput {
    pub assistant_id: String,
    pub show_in_selection: bool,
    pub selection_label: String,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAssistant {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub model_profile_id: String,
    pub system_prompt: String,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<i64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub stream: Option<bool>,
    pub enabled: bool,
    pub sort_order: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAssistantInput {
    pub id: Option<String>,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub model_profile_id: String,
    pub system_prompt: String,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConversation {
    pub id: String,
    pub assistant_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_message_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
}

pub struct Storage {
    database_path: PathBuf,
    connection: Connection,
}

impl Storage {
    pub fn initialize() -> Result<Self, StorageError> {
        let data_dir = data_dir()?;
        fs::create_dir_all(data_dir.join("exports"))?;
        fs::create_dir_all(data_dir.join("logs"))?;

        let database_path = data_dir.join("data.db");
        let connection = Connection::open(&database_path)?;
        initialize_schema(&connection)?;
        seed_defaults(&connection)?;

        Ok(Self {
            database_path,
            connection,
        })
    }

    pub fn status(&self) -> StorageStatus {
        let data_dir = self
            .database_path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default();

        StorageStatus {
            data_dir,
            database_path: self.database_path.display().to_string(),
            initialized: true,
        }
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = statement.query(params![key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![key, value],
        )?;

        Ok(())
    }

    pub fn export_settings(&self, keys: &[&str]) -> Result<HashMap<String, String>, StorageError> {
        let mut settings = HashMap::new();
        for key in keys {
            if let Some(value) = self.get_setting(key)? {
                settings.insert((*key).to_string(), value);
            }
        }

        Ok(settings)
    }

    pub fn record_recent_item(
        &self,
        id: &str,
        kind: &str,
        title: &str,
        target: &str,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO recent_items (id, kind, title, target, use_count, last_used_at)
             VALUES (?1, ?2, ?3, ?4, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                title = excluded.title,
                target = excluded.target,
                use_count = recent_items.use_count + 1,
                last_used_at = excluded.last_used_at",
            params![id, kind, title, target],
        )?;

        Ok(())
    }

    pub fn list_recent_items(&self, limit: usize) -> Result<Vec<RecentItem>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let bounded_limit = limit.min(100) as i64;
        let mut statement = self.connection.prepare(
            "SELECT id, kind, title, target, use_count, last_used_at
             FROM recent_items
             ORDER BY last_used_at DESC, use_count DESC, title COLLATE NOCASE ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![bounded_limit], recent_item_from_row)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn recent_scores(&self) -> Result<HashMap<String, f32>, StorageError> {
        self.recent_scores_at("now")
    }

    fn recent_scores_at(&self, now: &str) -> Result<HashMap<String, f32>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, use_count, COALESCE(julianday(?1) - julianday(last_used_at), 0)
                 FROM recent_items",
        )?;
        let rows = statement.query_map(params![now], |row| {
            let id: String = row.get(0)?;
            let use_count: i64 = row.get(1)?;
            let age_days: f64 = row.get(2)?;
            Ok((id, use_count as f32, age_days))
        })?;

        let mut scores = HashMap::new();
        for row in rows {
            let (id, use_count, age_days) = row?;
            scores.insert(
                id,
                use_count.min(20.0) / 20.0 * recent_score_decay(age_days),
            );
        }

        Ok(scores)
    }

    pub fn record_query_selection(
        &self,
        normalized_query: &str,
        result_id: &str,
    ) -> Result<bool, StorageError> {
        let normalized_query = normalized_query.trim();
        let result_id = result_id.trim();
        if normalized_query.is_empty() || result_id.is_empty() {
            return Ok(false);
        }

        self.connection.execute(
            "INSERT INTO query_selection_stats (normalized_query, result_id, use_count, last_used_at)
             VALUES (?1, ?2, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(normalized_query, result_id) DO UPDATE SET
                use_count = query_selection_stats.use_count + 1,
                last_used_at = excluded.last_used_at",
            params![normalized_query, result_id],
        )?;

        Ok(true)
    }

    pub fn query_selection_scores(
        &self,
        normalized_query: &str,
    ) -> Result<HashMap<String, f32>, StorageError> {
        let normalized_query = normalized_query.trim();
        if normalized_query.is_empty() {
            return Ok(HashMap::new());
        }

        let mut statement = self.connection.prepare(
            "SELECT result_id, use_count
             FROM query_selection_stats
             WHERE normalized_query = ?1",
        )?;
        let rows = statement.query_map(params![normalized_query], |row| {
            let result_id: String = row.get(0)?;
            let use_count: i64 = row.get(1)?;
            Ok((result_id, use_count as f32))
        })?;

        let mut scores = HashMap::new();
        for row in rows {
            let (result_id, use_count) = row?;
            scores.insert(result_id, use_count.min(10.0) / 10.0 * 0.35);
        }

        Ok(scores)
    }

    pub fn clear_recent_items(&self) -> Result<usize, StorageError> {
        self.connection
            .execute("DELETE FROM recent_items", [])
            .map_err(StorageError::from)
    }

    pub fn clear_query_selection_stats(&self) -> Result<usize, StorageError> {
        self.connection
            .execute("DELETE FROM query_selection_stats", [])
            .map_err(StorageError::from)
    }

    pub fn clear_ranking_learning(&self) -> Result<usize, StorageError> {
        let recent_count = self.clear_recent_items()?;
        let query_count = self.clear_query_selection_stats()?;
        Ok(recent_count + query_count)
    }

    pub fn list_pinned_results(&self) -> Result<Vec<PinnedResult>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT result_id, kind, title, target, created_at, updated_at
             FROM pinned_results
             ORDER BY updated_at DESC, title COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], pinned_result_from_row)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn pinned_result_scores(&self) -> Result<HashMap<String, f32>, StorageError> {
        let mut scores = HashMap::new();
        for result in self.list_pinned_results()? {
            scores.insert(result.result_id, 0.65);
        }
        Ok(scores)
    }

    pub fn set_result_pinned(
        &self,
        input: PinnedResultInput,
        pinned: bool,
    ) -> Result<Option<PinnedResult>, StorageError> {
        let result_id = input.result_id.trim();
        if result_id.is_empty() {
            return Err(StorageError::Validation("结果 ID 不能为空".into()));
        }

        if !pinned {
            self.delete_pinned_result(result_id)?;
            return Ok(None);
        }

        let kind = input.kind.trim();
        let title = input.title.trim();
        let target = input.target.trim();
        if kind.is_empty() || title.is_empty() {
            return Err(StorageError::Validation("固定结果缺少类型或标题".into()));
        }

        self.connection.execute(
            "INSERT INTO pinned_results (result_id, kind, title, target, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(result_id) DO UPDATE SET
                kind = excluded.kind,
                title = excluded.title,
                target = excluded.target,
                updated_at = excluded.updated_at",
            params![result_id, kind, title, target],
        )?;

        self.get_pinned_result(result_id)
    }

    pub fn delete_pinned_result(&self, result_id: &str) -> Result<bool, StorageError> {
        let affected = self.connection.execute(
            "DELETE FROM pinned_results WHERE result_id = ?1",
            params![result_id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_pinned_result(&self, result_id: &str) -> Result<Option<PinnedResult>, StorageError> {
        self.connection
            .query_row(
                "SELECT result_id, kind, title, target, created_at, updated_at
                 FROM pinned_results
                 WHERE result_id = ?1",
                params![result_id],
                pinned_result_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_result_aliases(&self) -> Result<Vec<ResultAlias>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT alias, normalized_alias, result_id, kind, title, target, created_at, updated_at
             FROM result_aliases
             ORDER BY alias COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], result_alias_from_row)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn upsert_result_alias(
        &self,
        input: ResultAliasInput,
    ) -> Result<ResultAlias, StorageError> {
        let alias = input.alias.trim();
        let normalized_alias = normalize_result_alias(alias)?;
        let result_id = input.result_id.trim();
        let kind = input.kind.trim();
        let title = input.title.trim();
        let target = input.target.trim();
        if result_id.is_empty() || kind.is_empty() || title.is_empty() {
            return Err(StorageError::Validation("Alias 目标结果不完整".into()));
        }

        let tool_menu_alias_conflict = self
            .get_setting("tools.menu.alias")?
            .map(|value| value.trim().eq_ignore_ascii_case(&normalized_alias))
            .unwrap_or(false);
        if tool_menu_alias_conflict {
            return Err(StorageError::Validation("Alias 与快捷入口冲突".into()));
        }

        let web_keyword_conflict = self
            .connection
            .query_row(
                "SELECT 1 FROM web_search_templates WHERE lower(keyword) = lower(?1)",
                params![normalized_alias],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if web_keyword_conflict {
            return Err(StorageError::Validation(
                "Alias 与网页搜索关键词冲突".into(),
            ));
        }

        let existing_result_id = self
            .connection
            .query_row(
                "SELECT result_id FROM result_aliases WHERE normalized_alias = ?1",
                params![normalized_alias],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if existing_result_id
            .as_deref()
            .is_some_and(|existing| existing != result_id)
        {
            return Err(StorageError::Validation("Alias 已被其他结果使用".into()));
        }

        self.connection.execute(
            "INSERT INTO result_aliases (
                alias, normalized_alias, result_id, kind, title, target, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(normalized_alias) DO UPDATE SET
                alias = excluded.alias,
                result_id = excluded.result_id,
                kind = excluded.kind,
                title = excluded.title,
                target = excluded.target,
                updated_at = excluded.updated_at",
            params![alias, normalized_alias, result_id, kind, title, target],
        )?;

        self.get_result_alias(&normalized_alias)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_result_alias(&self, normalized_alias: &str) -> Result<bool, StorageError> {
        let normalized_alias = normalize_result_alias(normalized_alias)?;
        let affected = self.connection.execute(
            "DELETE FROM result_aliases WHERE normalized_alias = ?1",
            params![normalized_alias],
        )?;
        Ok(affected > 0)
    }

    pub fn get_result_alias(
        &self,
        normalized_alias: &str,
    ) -> Result<Option<ResultAlias>, StorageError> {
        self.connection
            .query_row(
                "SELECT alias, normalized_alias, result_id, kind, title, target, created_at, updated_at
                 FROM result_aliases
                 WHERE normalized_alias = ?1",
                params![normalized_alias],
                result_alias_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_custom_commands(&self) -> Result<Vec<CustomCommand>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, command_type, target, created_at, updated_at
             FROM custom_commands
             ORDER BY name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], custom_command_from_row)?;

        let mut commands = Vec::new();
        for row in rows {
            commands.push(row?);
        }

        Ok(commands)
    }

    pub fn upsert_custom_command(
        &self,
        input: CustomCommandInput,
    ) -> Result<CustomCommand, StorageError> {
        let name = input.name.trim();
        let command_type = input.command_type.trim();
        let target = input.target.trim();
        validate_custom_command_fields(name, command_type, target)
            .map_err(StorageError::Validation)?;

        let existing_id = self
            .connection
            .query_row(
                "SELECT id FROM custom_commands WHERE lower(name) = lower(?1)",
                params![name],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let (Some(existing_id), Some(input_id)) = (existing_id.as_deref(), input.id.as_deref()) {
            if existing_id != input_id {
                return Err(StorageError::Validation("自定义命令名称已存在".into()));
            }
        } else if existing_id.is_some() && input.id.is_none() {
            return Err(StorageError::Validation("自定义命令名称已存在".into()));
        }

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("custom-command:{}", stable_command_slug(name)));

        self.connection.execute(
            "INSERT INTO custom_commands (id, name, command_type, target, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                command_type = excluded.command_type,
                target = excluded.target,
                updated_at = excluded.updated_at",
            params![id, name, command_type, target],
        )?;

        self.get_custom_command(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_custom_command(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM custom_commands WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn get_custom_command(&self, id: &str) -> Result<Option<CustomCommand>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, name, command_type, target, created_at, updated_at
                 FROM custom_commands
                 WHERE id = ?1",
                params![id],
                custom_command_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_phrases(&self) -> Result<Vec<Phrase>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, title, text, created_at, updated_at, use_count
             FROM phrases
             ORDER BY title COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], phrase_from_row)?;

        let mut phrases = Vec::new();
        for row in rows {
            phrases.push(row?);
        }

        Ok(phrases)
    }

    pub fn upsert_phrase(&self, input: PhraseInput) -> Result<Phrase, StorageError> {
        let title = input.title.trim();
        let text = input.text.trim();
        validate_phrase_fields(title, text).map_err(StorageError::Validation)?;

        let existing_id = self
            .connection
            .query_row(
                "SELECT id FROM phrases WHERE lower(title) = lower(?1)",
                params![title],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let (Some(existing_id), Some(input_id)) = (existing_id.as_deref(), input.id.as_deref()) {
            if existing_id != input_id {
                return Err(StorageError::Validation("快捷短语标题已存在".into()));
            }
        } else if existing_id.is_some() && input.id.is_none() {
            return Err(StorageError::Validation("快捷短语标题已存在".into()));
        }

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("phrase:{}", stable_command_slug(title)));

        self.connection.execute(
            "INSERT INTO phrases (id, title, text, created_at, updated_at, use_count)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                text = excluded.text,
                updated_at = excluded.updated_at",
            params![id, title, text],
        )?;

        self.get_phrase(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_phrase(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM phrases WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn get_phrase(&self, id: &str) -> Result<Option<Phrase>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, title, text, created_at, updated_at, use_count
                 FROM phrases
                 WHERE id = ?1",
                params![id],
                phrase_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn mark_phrase_used(&self, id: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE phrases
             SET use_count = use_count + 1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id],
        )?;

        Ok(())
    }

    pub fn list_selection_marks(&self) -> Result<Vec<SelectionMark>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, text, source_app, created_at, use_count
             FROM selection_marks
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], selection_mark_from_row)?;

        let mut marks = Vec::new();
        for row in rows {
            marks.push(row?);
        }

        Ok(marks)
    }

    pub fn save_selection_mark(
        &self,
        input: SelectionMarkInput,
    ) -> Result<SelectionMark, StorageError> {
        self.save_selection_mark_with_limit(input, SELECTION_MARK_STORAGE_LIMIT)
    }

    fn save_selection_mark_with_limit(
        &self,
        input: SelectionMarkInput,
        limit: usize,
    ) -> Result<SelectionMark, StorageError> {
        let text = input.text.trim();
        validate_selection_mark_text(text).map_err(StorageError::Validation)?;
        let source_app = normalize_optional_text(input.source_app);
        let id = generated_record_id("mark");

        self.connection.execute(
            "INSERT INTO selection_marks (id, text, source_app, created_at, use_count)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0)",
            params![id, text, source_app],
        )?;
        self.prune_selection_marks(limit)?;

        self.get_selection_mark(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_selection_mark(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM selection_marks WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn clear_selection_marks(&self) -> Result<usize, StorageError> {
        let affected = self.connection.execute("DELETE FROM selection_marks", [])?;
        Ok(affected)
    }

    pub fn mark_selection_mark_used(&self, id: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE selection_marks
             SET use_count = use_count + 1
             WHERE id = ?1",
            params![id],
        )?;

        Ok(())
    }

    pub fn get_selection_mark(&self, id: &str) -> Result<Option<SelectionMark>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, text, source_app, created_at, use_count
                 FROM selection_marks
                 WHERE id = ?1",
                params![id],
                selection_mark_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    fn prune_selection_marks(&self, limit: usize) -> Result<(), StorageError> {
        if limit == 0 {
            self.connection.execute("DELETE FROM selection_marks", [])?;
            return Ok(());
        }

        let count =
            self.connection
                .query_row("SELECT COUNT(*) FROM selection_marks", [], |row| {
                    row.get::<_, i64>(0)
                })?;
        let overflow = count - limit as i64;
        if overflow <= 0 {
            return Ok(());
        }

        self.connection.execute(
            "DELETE FROM selection_marks
             WHERE id IN (
                SELECT id
                FROM selection_marks
                ORDER BY created_at ASC, rowid ASC
                LIMIT ?1
             )",
            params![overflow],
        )?;
        Ok(())
    }

    pub fn list_todos(
        &self,
        status: Option<&str>,
        query: Option<&str>,
    ) -> Result<Vec<Todo>, StorageError> {
        let status = normalize_todo_list_status(status).map_err(StorageError::Validation)?;
        let query = query.unwrap_or_default().trim().to_lowercase();
        let query_pattern = format!("%{}%", escape_like_query(&query));
        let mut statement = self.connection.prepare(
            "SELECT id, text, source_app, remind_at, status, last_notified_at, created_at, updated_at
             FROM todos
             WHERE (?1 = 'all' OR status = ?1)
               AND (?2 = '' OR lower(text) LIKE ?3 ESCAPE '\\')
             ORDER BY
               CASE WHEN status = 'pending' AND remind_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') THEN 0 ELSE 1 END ASC,
               remind_at ASC,
               created_at DESC",
        )?;
        let rows = statement.query_map(params![status, query, query_pattern], todo_from_row)?;

        let mut todos = Vec::new();
        for row in rows {
            todos.push(row?);
        }

        Ok(todos)
    }

    pub fn save_todo(&self, input: TodoInput) -> Result<Todo, StorageError> {
        let text = input.text.trim();
        validate_todo_text(text).map_err(StorageError::Validation)?;
        let remind_at =
            parse_future_utc_time(&input.remind_at).map_err(StorageError::Validation)?;
        let source_app = normalize_optional_text(input.source_app);
        let id = generated_record_id("todo");

        self.connection.execute(
            "INSERT INTO todos (id, text, source_app, remind_at, status, last_notified_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', NULL, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![id, text, source_app, remind_at],
        )?;

        self.get_todo(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn update_todo(&self, input: TodoUpdateInput) -> Result<Todo, StorageError> {
        let id = input.id;
        let existing = self
            .get_todo(&id)?
            .ok_or_else(|| StorageError::Validation("待办不存在".into()))?;
        let text = input
            .text
            .as_deref()
            .map(str::trim)
            .unwrap_or(existing.text.as_str());
        validate_todo_text(text).map_err(StorageError::Validation)?;
        let remind_at = match input.remind_at.as_deref() {
            Some(value) => parse_future_utc_time(value).map_err(StorageError::Validation)?,
            None => existing.remind_at,
        };

        self.connection.execute(
            "UPDATE todos
             SET text = ?2,
                 remind_at = ?3,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id, text, remind_at],
        )?;

        self.get_todo(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn complete_todo(&self, id: &str) -> Result<Todo, StorageError> {
        self.connection.execute(
            "UPDATE todos
             SET status = 'done',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id],
        )?;

        self.get_todo(id)?
            .ok_or_else(|| StorageError::Validation("待办不存在".into()))
    }

    pub fn snooze_todo(&self, id: &str, minutes: i64) -> Result<Todo, StorageError> {
        if !(1..=1440).contains(&minutes) {
            return Err(StorageError::Validation(
                "稍后提醒时间必须在 1 到 1440 分钟之间".into(),
            ));
        }
        let remind_at = utc_time_string(Utc::now() + ChronoDuration::minutes(minutes));
        self.connection.execute(
            "UPDATE todos
             SET remind_at = ?2,
                 status = 'pending',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id, remind_at],
        )?;

        self.get_todo(id)?
            .ok_or_else(|| StorageError::Validation("待办不存在".into()))
    }

    pub fn delete_todo(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM todos WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn clear_completed_todos(&self) -> Result<usize, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM todos WHERE status = 'done'", [])?;
        Ok(affected)
    }

    pub fn due_todos(&self, now: DateTime<Utc>) -> Result<Vec<Todo>, StorageError> {
        let now = utc_time_string(now);
        let mut statement = self.connection.prepare(
            "SELECT id, text, source_app, remind_at, status, last_notified_at, created_at, updated_at
             FROM todos
             WHERE status = 'pending' AND remind_at <= ?1
             ORDER BY remind_at ASC, created_at ASC",
        )?;
        let rows = statement.query_map(params![now], todo_from_row)?;

        let mut todos = Vec::new();
        for row in rows {
            todos.push(row?);
        }

        Ok(todos)
    }

    pub fn mark_todo_notified(&self, id: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE todos
             SET last_notified_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id],
        )?;

        Ok(())
    }

    pub fn get_todo(&self, id: &str) -> Result<Option<Todo>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, text, source_app, remind_at, status, last_notified_at, created_at, updated_at
                 FROM todos
                 WHERE id = ?1",
                params![id],
                todo_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_web_search_templates(&self) -> Result<Vec<WebSearchTemplate>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, keyword, name, url_template, created_at, updated_at
             FROM web_search_templates
             ORDER BY keyword COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], web_search_template_from_row)?;

        let mut templates = Vec::new();
        for row in rows {
            templates.push(row?);
        }

        Ok(templates)
    }

    pub fn upsert_web_search_template(
        &self,
        input: WebSearchTemplateInput,
    ) -> Result<WebSearchTemplate, StorageError> {
        let keyword = input.keyword.trim().to_lowercase();
        let name = input.name.trim();
        let url_template = input.url_template.trim();
        validate_web_search_template_fields(&keyword, name, url_template)
            .map_err(StorageError::Validation)?;

        let existing_id = self
            .connection
            .query_row(
                "SELECT id FROM web_search_templates WHERE lower(keyword) = lower(?1)",
                params![keyword],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let (Some(existing_id), Some(input_id)) = (existing_id.as_deref(), input.id.as_deref()) {
            if existing_id != input_id {
                return Err(StorageError::Validation("网页搜索关键词已存在".into()));
            }
        } else if existing_id.is_some() && input.id.is_none() {
            return Err(StorageError::Validation("网页搜索关键词已存在".into()));
        }

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("web-search:{}", stable_command_slug(&keyword)));

        self.connection.execute(
            "INSERT INTO web_search_templates (id, keyword, name, url_template, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(id) DO UPDATE SET
                keyword = excluded.keyword,
                name = excluded.name,
                url_template = excluded.url_template,
                updated_at = excluded.updated_at",
            params![id, keyword, name, url_template],
        )?;

        self.get_web_search_template(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_web_search_template(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self.connection.execute(
            "DELETE FROM web_search_templates WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_web_search_template(
        &self,
        id: &str,
    ) -> Result<Option<WebSearchTemplate>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, keyword, name, url_template, created_at, updated_at
                 FROM web_search_templates
                 WHERE id = ?1",
                params![id],
                web_search_template_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_exclusion_rules(&self) -> Result<Vec<ExclusionRule>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, match_type, pattern, created_at, updated_at
             FROM exclusion_rules
             ORDER BY updated_at DESC, rowid DESC",
        )?;
        let rows = statement.query_map([], exclusion_rule_from_row)?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }

        Ok(rules)
    }

    pub fn upsert_exclusion_rule(
        &self,
        input: ExclusionRuleInput,
    ) -> Result<ExclusionRule, StorageError> {
        let match_type = input.match_type.trim();
        let pattern = input.pattern.trim();
        validate_exclusion_rule_fields(match_type, pattern).map_err(StorageError::Validation)?;

        let existing_id = self
            .connection
            .query_row(
                "SELECT id FROM exclusion_rules
                 WHERE lower(match_type) = lower(?1) AND lower(pattern) = lower(?2)",
                params![match_type, pattern],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let (Some(existing_id), Some(input_id)) = (existing_id.as_deref(), input.id.as_deref()) {
            if existing_id != input_id {
                return Err(StorageError::Validation("排除规则已存在".into()));
            }
        } else if existing_id.is_some() && input.id.is_none() {
            return Err(StorageError::Validation("排除规则已存在".into()));
        }

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "exclusion-rule:{}:{}",
                    match_type,
                    stable_command_slug(&format!("{match_type}:{pattern}"))
                )
            });

        self.connection.execute(
            "INSERT INTO exclusion_rules (id, match_type, pattern, created_at, updated_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(id) DO UPDATE SET
                match_type = excluded.match_type,
                pattern = excluded.pattern,
                updated_at = excluded.updated_at",
            params![id, match_type, pattern],
        )?;

        self.get_exclusion_rule(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_exclusion_rule(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM exclusion_rules WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn get_exclusion_rule(&self, id: &str) -> Result<Option<ExclusionRule>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, match_type, pattern, created_at, updated_at
                 FROM exclusion_rules
                 WHERE id = ?1",
                params![id],
                exclusion_rule_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_ai_providers(&self) -> Result<Vec<AiProvider>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, provider_type, base_url, api_key, enabled, sort_order, created_at, updated_at
             FROM ai_providers
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], ai_provider_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn get_ai_provider(&self, id: &str) -> Result<Option<AiProvider>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, name, provider_type, base_url, api_key, enabled, sort_order, created_at, updated_at
                 FROM ai_providers
                 WHERE id = ?1",
                params![id],
                ai_provider_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_ai_provider(&self, input: AiProviderInput) -> Result<AiProvider, StorageError> {
        let provider_type = input.provider_type.trim();
        validate_ai_provider_fields(provider_type).map_err(StorageError::Validation)?;

        let base_url = input.base_url.trim();
        let api_key = input.api_key.trim();
        let inferred_name = infer_provider_name_from_base_url(base_url);
        let name = if input.name.trim().is_empty() {
            if inferred_name.is_empty() {
                "OpenAI 兼容接口"
            } else {
                inferred_name.as_str()
            }
        } else {
            input.name.trim()
        };
        let id = match input.id.filter(|id| !id.trim().is_empty()) {
            Some(id) => id,
            None => self
                .reusable_empty_default_ai_provider_id()?
                .unwrap_or_else(|| format!("ai-provider:{}", stable_command_slug(name))),
        };

        self.connection.execute(
            "INSERT INTO ai_providers (
                id, name, provider_type, base_url, api_key, enabled, sort_order, created_at, updated_at
             )
             VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                provider_type = excluded.provider_type,
                base_url = excluded.base_url,
                api_key = excluded.api_key,
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![
                id,
                name,
                provider_type,
                base_url,
                api_key,
                bool_to_i64(input.enabled),
                input.sort_order,
            ],
        )?;

        self.get_ai_provider(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    fn reusable_empty_default_ai_provider_id(&self) -> Result<Option<String>, StorageError> {
        let provider_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM ai_providers", [], |row| row.get(0))?;
        if provider_count != 1 {
            return Ok(None);
        }

        self.connection
            .query_row(
                "SELECT id
                 FROM ai_providers
                 WHERE id = ?1
                   AND trim(base_url) = ''
                   AND trim(api_key) = ''
                   AND NOT EXISTS (
                        SELECT 1
                        FROM ai_provider_models
                        WHERE provider_id = ai_providers.id
                   )",
                params![DEFAULT_AI_PROVIDER_ID],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn delete_ai_provider(&self, id: &str) -> Result<bool, StorageError> {
        self.connection.execute(
            "DELETE FROM ai_provider_models WHERE provider_id = ?1",
            params![id],
        )?;
        self.connection.execute(
            "UPDATE ai_selection_actions
             SET last_provider_id = NULL, last_model_name = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE last_provider_id = ?1",
            params![id],
        )?;
        let affected = self
            .connection
            .execute("DELETE FROM ai_providers WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn list_ai_provider_models(
        &self,
        provider_id: Option<&str>,
    ) -> Result<Vec<AiProviderModel>, StorageError> {
        let sql = if provider_id.is_some() {
            "SELECT id, provider_id, model_name, enabled, sort_order, last_used_at, created_at, updated_at
             FROM ai_provider_models
             WHERE provider_id = ?1
             ORDER BY enabled DESC, sort_order ASC, model_name COLLATE NOCASE ASC"
        } else {
            "SELECT id, provider_id, model_name, enabled, sort_order, last_used_at, created_at, updated_at
             FROM ai_provider_models
             ORDER BY provider_id ASC, enabled DESC, sort_order ASC, model_name COLLATE NOCASE ASC"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = if let Some(provider_id) = provider_id {
            statement.query_map(params![provider_id], ai_provider_model_from_row)?
        } else {
            statement.query_map([], ai_provider_model_from_row)?
        };
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn list_enabled_ai_provider_models(&self) -> Result<Vec<AiProviderModel>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT model.id, model.provider_id, model.model_name, model.enabled,
                    model.sort_order, model.last_used_at, model.created_at, model.updated_at
             FROM ai_provider_models model
             JOIN ai_providers provider ON provider.id = model.provider_id
             WHERE provider.enabled = 1 AND model.enabled = 1
             ORDER BY provider.sort_order ASC, model.sort_order ASC, model.model_name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], ai_provider_model_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn get_ai_provider_model(
        &self,
        provider_id: &str,
        model_name: &str,
    ) -> Result<Option<AiProviderModel>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, provider_id, model_name, enabled, sort_order, last_used_at, created_at, updated_at
                 FROM ai_provider_models
                 WHERE provider_id = ?1 AND model_name = ?2",
                params![provider_id, model_name.trim()],
                ai_provider_model_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_ai_provider_model(
        &self,
        input: AiProviderModelInput,
    ) -> Result<AiProviderModel, StorageError> {
        let provider_id = input.provider_id.trim();
        let model_name = input.model_name.trim();
        validate_ai_provider_model_fields(provider_id, model_name)
            .map_err(StorageError::Validation)?;
        if self.get_ai_provider(provider_id)?.is_none() {
            return Err(StorageError::Validation("供应商不存在".into()));
        }

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "ai-provider-model:{}",
                    stable_command_slug(&format!("{provider_id}:{model_name}"))
                )
            });

        self.connection.execute(
            "INSERT INTO ai_provider_models (
                id, provider_id, model_name, enabled, sort_order, created_at, updated_at
             )
             VALUES (
                ?1, ?2, ?3, ?4, ?5,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )
             ON CONFLICT(provider_id, model_name) DO UPDATE SET
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![
                id,
                provider_id,
                model_name,
                bool_to_i64(input.enabled),
                input.sort_order,
            ],
        )?;

        self.get_ai_provider_model(provider_id, model_name)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn mark_ai_provider_model_used(
        &self,
        provider_id: &str,
        model_name: &str,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE ai_provider_models
             SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE provider_id = ?1 AND model_name = ?2",
            params![provider_id, model_name],
        )?;
        Ok(())
    }

    pub fn list_ai_model_profiles(&self) -> Result<Vec<AiModelProfile>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, provider_type, name, base_url, api_key, model_name,
                    temperature, top_p, max_tokens, presence_penalty, frequency_penalty,
                    stream, enabled, sort_order, last_used_at, created_at, updated_at
             FROM ai_model_profiles
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], ai_model_profile_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn get_ai_model_profile(&self, id: &str) -> Result<Option<AiModelProfile>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, provider_type, name, base_url, api_key, model_name,
                        temperature, top_p, max_tokens, presence_penalty, frequency_penalty,
                        stream, enabled, sort_order, last_used_at, created_at, updated_at
                 FROM ai_model_profiles
                 WHERE id = ?1",
                params![id],
                ai_model_profile_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_ai_model_profile(
        &self,
        input: AiModelProfileInput,
    ) -> Result<AiModelProfile, StorageError> {
        let provider_type = input.provider_type.trim();
        let name = input.name.trim();
        let base_url = input.base_url.trim();
        let api_key = input.api_key.trim();
        let model_name = input.model_name.trim();
        validate_ai_model_profile_fields(provider_type, name, input.max_tokens)
            .map_err(StorageError::Validation)?;

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("ai-model-profile:{}", stable_command_slug(name)));

        self.connection.execute(
            "INSERT INTO ai_model_profiles (
                id, provider_type, name, base_url, api_key, model_name,
                temperature, top_p, max_tokens, presence_penalty, frequency_penalty,
                stream, enabled, sort_order, created_at, updated_at
             )
             VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )
             ON CONFLICT(id) DO UPDATE SET
                provider_type = excluded.provider_type,
                name = excluded.name,
                base_url = excluded.base_url,
                api_key = excluded.api_key,
                model_name = excluded.model_name,
                temperature = excluded.temperature,
                top_p = excluded.top_p,
                max_tokens = excluded.max_tokens,
                presence_penalty = excluded.presence_penalty,
                frequency_penalty = excluded.frequency_penalty,
                stream = excluded.stream,
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![
                id,
                provider_type,
                name,
                base_url,
                api_key,
                model_name,
                input.temperature,
                input.top_p,
                input.max_tokens,
                input.presence_penalty,
                input.frequency_penalty,
                bool_to_i64(input.stream),
                bool_to_i64(input.enabled),
                input.sort_order,
            ],
        )?;

        self.get_ai_model_profile(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_ai_model_profile(&self, id: &str) -> Result<bool, StorageError> {
        let assistant_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM ai_assistants WHERE model_profile_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        if assistant_count > 0 {
            return Err(StorageError::Validation(
                "模型配置仍被助手使用，不能删除".into(),
            ));
        }

        let affected = self
            .connection
            .execute("DELETE FROM ai_model_profiles WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn list_ai_assistants(&self) -> Result<Vec<AiAssistant>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, icon, description, model_profile_id, system_prompt,
                    temperature, top_p, max_tokens, presence_penalty, frequency_penalty,
                    stream, enabled, sort_order, last_used_at, created_at, updated_at
             FROM ai_assistants
             ORDER BY last_used_at IS NULL ASC, last_used_at DESC, sort_order ASC, name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], ai_assistant_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn get_ai_assistant(&self, id: &str) -> Result<Option<AiAssistant>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, name, icon, description, model_profile_id, system_prompt,
                        temperature, top_p, max_tokens, presence_penalty, frequency_penalty,
                        stream, enabled, sort_order, last_used_at, created_at, updated_at
                 FROM ai_assistants
                 WHERE id = ?1",
                params![id],
                ai_assistant_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_ai_assistant(
        &self,
        input: AiAssistantInput,
    ) -> Result<AiAssistant, StorageError> {
        let name = input.name.trim();
        let icon = input.icon.trim();
        let description = input.description.trim();
        let model_profile_id = input.model_profile_id.trim();
        let system_prompt = input.system_prompt.trim();
        validate_ai_assistant_fields(name, icon, model_profile_id)
            .map_err(StorageError::Validation)?;

        if self.get_ai_model_profile(model_profile_id)?.is_none() {
            return Err(StorageError::Validation("绑定的模型配置不存在".into()));
        }

        let id = input
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("ai-assistant:{}", stable_command_slug(name)));

        self.connection.execute(
            "INSERT INTO ai_assistants (
                id, name, icon, description, model_profile_id, system_prompt,
                enabled, sort_order, created_at, updated_at
             )
             VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                icon = excluded.icon,
                description = excluded.description,
                model_profile_id = excluded.model_profile_id,
                system_prompt = excluded.system_prompt,
                enabled = excluded.enabled,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![
                id,
                name,
                icon,
                description,
                model_profile_id,
                system_prompt,
                bool_to_i64(input.enabled),
                input.sort_order,
            ],
        )?;

        self.get_ai_assistant(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn delete_ai_assistant(&self, id: &str) -> Result<bool, StorageError> {
        if is_builtin_selection_assistant(id) {
            return Err(StorageError::Validation("内置划词助手不能删除".into()));
        }

        let affected = self
            .connection
            .execute("DELETE FROM ai_assistants WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn mark_ai_assistant_used(&self, id: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE ai_assistants
             SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn list_ai_selection_actions(&self) -> Result<Vec<AiSelectionAction>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT action.assistant_id, assistant.name, assistant.icon, assistant.description,
                    assistant.model_profile_id, assistant.system_prompt, assistant.enabled, action.show_in_selection,
                    action.selection_label, action.sort_order, action.last_provider_id,
                    action.last_model_name
             FROM ai_selection_actions action
             JOIN ai_assistants assistant ON assistant.id = action.assistant_id
             ORDER BY action.sort_order ASC, action.selection_label COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], ai_selection_action_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn list_visible_ai_selection_actions(
        &self,
    ) -> Result<Vec<AiSelectionAction>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT action.assistant_id, assistant.name, assistant.icon, assistant.description,
                    assistant.model_profile_id, assistant.system_prompt, assistant.enabled, action.show_in_selection,
                    action.selection_label, action.sort_order, action.last_provider_id,
                    action.last_model_name
             FROM ai_selection_actions action
             JOIN ai_assistants assistant ON assistant.id = action.assistant_id
             WHERE action.show_in_selection = 1 AND assistant.enabled = 1
             ORDER BY action.sort_order ASC, action.selection_label COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], ai_selection_action_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn get_ai_selection_action(
        &self,
        assistant_id: &str,
    ) -> Result<Option<AiSelectionAction>, StorageError> {
        self.connection
            .query_row(
                "SELECT action.assistant_id, assistant.name, assistant.icon, assistant.description,
                        assistant.model_profile_id, assistant.system_prompt, assistant.enabled, action.show_in_selection,
                        action.selection_label, action.sort_order, action.last_provider_id,
                        action.last_model_name
                 FROM ai_selection_actions action
                 JOIN ai_assistants assistant ON assistant.id = action.assistant_id
                 WHERE action.assistant_id = ?1",
                params![assistant_id],
                ai_selection_action_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn upsert_ai_selection_action(
        &self,
        input: AiSelectionActionInput,
    ) -> Result<AiSelectionAction, StorageError> {
        let assistant_id = input.assistant_id.trim();
        let selection_label = input.selection_label.trim();
        validate_ai_selection_action_fields(assistant_id, selection_label)
            .map_err(StorageError::Validation)?;
        if self.get_ai_assistant(assistant_id)?.is_none() {
            return Err(StorageError::Validation("划词助手不存在".into()));
        }

        self.connection.execute(
            "INSERT INTO ai_selection_actions (
                assistant_id, show_in_selection, selection_label, sort_order,
                last_provider_id, last_model_name, created_at, updated_at
             )
             VALUES (
                ?1, ?2, ?3, ?4, NULL, NULL,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )
             ON CONFLICT(assistant_id) DO UPDATE SET
                show_in_selection = excluded.show_in_selection,
                selection_label = excluded.selection_label,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at",
            params![
                assistant_id,
                bool_to_i64(input.show_in_selection),
                selection_label,
                input.sort_order,
            ],
        )?;

        self.get_ai_selection_action(assistant_id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn set_ai_selection_action_model(
        &self,
        assistant_id: &str,
        provider_id: &str,
        model_name: &str,
    ) -> Result<AiSelectionAction, StorageError> {
        let assistant_id = assistant_id.trim();
        let provider_id = provider_id.trim();
        let model_name = model_name.trim();
        validate_ai_selection_action_fields(assistant_id, "model")
            .map_err(StorageError::Validation)?;
        if provider_id.is_empty() || model_name.is_empty() {
            return Err(StorageError::Validation("划词模型不能为空".into()));
        }
        if self.get_ai_selection_action(assistant_id)?.is_none() {
            return Err(StorageError::Validation("划词助手设置不存在".into()));
        }
        if self.get_ai_provider(provider_id)?.is_none() {
            return Err(StorageError::Validation("供应商不存在".into()));
        }
        if self
            .get_ai_provider_model(provider_id, model_name)?
            .is_none()
        {
            return Err(StorageError::Validation("模型不存在".into()));
        }

        self.connection.execute(
            "UPDATE ai_selection_actions
             SET last_provider_id = ?2,
                 last_model_name = ?3,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE assistant_id = ?1",
            params![assistant_id, provider_id, model_name],
        )?;

        self.get_ai_selection_action(assistant_id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn list_ai_conversations(
        &self,
        assistant_id: Option<&str>,
    ) -> Result<Vec<AiConversation>, StorageError> {
        let sql = if assistant_id.is_some() {
            "SELECT id, assistant_id, title, created_at, updated_at, last_message_at
             FROM ai_conversations
             WHERE assistant_id = ?1
             ORDER BY COALESCE(last_message_at, updated_at) DESC, created_at DESC"
        } else {
            "SELECT id, assistant_id, title, created_at, updated_at, last_message_at
             FROM ai_conversations
             ORDER BY COALESCE(last_message_at, updated_at) DESC, created_at DESC"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = if let Some(assistant_id) = assistant_id {
            statement.query_map(params![assistant_id], ai_conversation_from_row)?
        } else {
            statement.query_map([], ai_conversation_from_row)?
        };
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn create_ai_conversation(
        &self,
        assistant_id: &str,
        title: &str,
    ) -> Result<AiConversation, StorageError> {
        if self.get_ai_assistant(assistant_id)?.is_none() {
            return Err(StorageError::Validation("助手不存在".into()));
        }
        let id = format!("ai-conversation:{}", random_hex_id(&self.connection)?);
        self.connection.execute(
            "INSERT INTO ai_conversations (id, assistant_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![id, assistant_id, title.trim()],
        )?;
        self.get_ai_conversation(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn get_ai_conversation(&self, id: &str) -> Result<Option<AiConversation>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, assistant_id, title, created_at, updated_at, last_message_at
                 FROM ai_conversations
                 WHERE id = ?1",
                params![id],
                ai_conversation_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn rename_ai_conversation(&self, id: &str, title: &str) -> Result<(), StorageError> {
        self.connection.execute(
            "UPDATE ai_conversations
             SET title = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![id, title.trim()],
        )?;
        Ok(())
    }

    pub fn delete_ai_conversation(&self, id: &str) -> Result<bool, StorageError> {
        self.connection.execute(
            "DELETE FROM ai_messages WHERE conversation_id = ?1",
            params![id],
        )?;
        let affected = self
            .connection
            .execute("DELETE FROM ai_conversations WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn list_ai_messages(&self, conversation_id: &str) -> Result<Vec<AiMessage>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, conversation_id, role, content, status, error, created_at
             FROM ai_messages
             WHERE conversation_id = ?1
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = statement.query_map(params![conversation_id], ai_message_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn insert_ai_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<AiMessage, StorageError> {
        validate_ai_message_fields(role, status).map_err(StorageError::Validation)?;
        let id = format!("ai-message:{}", random_hex_id(&self.connection)?);
        self.connection.execute(
            "INSERT INTO ai_messages (id, conversation_id, role, content, status, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![id, conversation_id, role, content, status, error],
        )?;
        self.touch_ai_conversation_after_message(conversation_id, content)?;
        self.get_ai_message(&id)?
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn update_ai_message_status(
        &self,
        id: &str,
        content: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        validate_ai_message_fields("assistant", status).map_err(StorageError::Validation)?;
        self.connection.execute(
            "UPDATE ai_messages
             SET content = ?2, status = ?3, error = ?4
             WHERE id = ?1",
            params![id, content, status, error],
        )?;
        Ok(())
    }

    pub fn get_ai_message(&self, id: &str) -> Result<Option<AiMessage>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, conversation_id, role, content, status, error, created_at
                 FROM ai_messages
                 WHERE id = ?1",
                params![id],
                ai_message_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn delete_ai_message(&self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM ai_messages WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    fn touch_ai_conversation_after_message(
        &self,
        conversation_id: &str,
        user_content: &str,
    ) -> Result<(), StorageError> {
        let title = local_conversation_title(user_content);
        self.connection.execute(
            "UPDATE ai_conversations
             SET title = CASE WHEN title = '' THEN ?2 ELSE title END,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 last_message_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![conversation_id, title],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn from_connection_for_tests(connection: Connection) -> Self {
        Self {
            database_path: PathBuf::from("memory"),
            connection,
        }
    }
}

pub fn data_dir() -> Result<PathBuf, StorageError> {
    dirs::data_local_dir()
        .map(|dir| dir.join(APP_DATA_DIR))
        .ok_or(StorageError::AppDataDirUnavailable)
}

fn initialize_schema(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS recent_items (
            id TEXT PRIMARY KEY NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            target TEXT NOT NULL,
            use_count INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS app_index (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            launch_command TEXT NOT NULL,
            icon_path TEXT,
            source TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS query_selection_stats (
            normalized_query TEXT NOT NULL,
            result_id TEXT NOT NULL,
            use_count INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            PRIMARY KEY(normalized_query, result_id)
        );

        CREATE TABLE IF NOT EXISTS pinned_results (
            result_id TEXT PRIMARY KEY NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            target TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS result_aliases (
            normalized_alias TEXT PRIMARY KEY NOT NULL,
            alias TEXT NOT NULL,
            result_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            target TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS custom_commands (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL UNIQUE,
            command_type TEXT NOT NULL,
            target TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS phrases (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL UNIQUE,
            text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            use_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS selection_marks (
            id TEXT PRIMARY KEY NOT NULL,
            text TEXT NOT NULL,
            source_app TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            use_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY NOT NULL,
            text TEXT NOT NULL,
            source_app TEXT,
            remind_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'done')),
            last_notified_at TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_todos_due ON todos(status, remind_at);

        CREATE TABLE IF NOT EXISTS web_search_templates (
            id TEXT PRIMARY KEY NOT NULL,
            keyword TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            url_template TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS exclusion_rules (
            id TEXT PRIMARY KEY NOT NULL,
            match_type TEXT NOT NULL,
            pattern TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            UNIQUE(match_type, pattern)
        );

        CREATE TABLE IF NOT EXISTS ai_model_profiles (
            id TEXT PRIMARY KEY NOT NULL,
            provider_type TEXT NOT NULL,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            api_key TEXT NOT NULL DEFAULT '',
            model_name TEXT NOT NULL,
            temperature REAL,
            top_p REAL,
            max_tokens INTEGER,
            presence_penalty REAL,
            frequency_penalty REAL,
            stream INTEGER NOT NULL DEFAULT 1,
            enabled INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS ai_providers (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            provider_type TEXT NOT NULL,
            base_url TEXT NOT NULL,
            api_key TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS ai_provider_models (
            id TEXT PRIMARY KEY NOT NULL,
            provider_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            UNIQUE(provider_id, model_name)
        );

        CREATE TABLE IF NOT EXISTS ai_assistants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            icon TEXT NOT NULL DEFAULT 'AI',
            description TEXT NOT NULL DEFAULT '',
            model_profile_id TEXT NOT NULL,
            system_prompt TEXT NOT NULL DEFAULT '',
            temperature REAL,
            top_p REAL,
            max_tokens INTEGER,
            presence_penalty REAL,
            frequency_penalty REAL,
            stream INTEGER,
            enabled INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            last_used_at TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS ai_selection_actions (
            assistant_id TEXT PRIMARY KEY NOT NULL,
            show_in_selection INTEGER NOT NULL DEFAULT 1,
            selection_label TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            last_provider_id TEXT,
            last_model_name TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS ai_conversations (
            id TEXT PRIMARY KEY NOT NULL,
            assistant_id TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            last_message_at TEXT
        );

        CREATE TABLE IF NOT EXISTS ai_messages (
            id TEXT PRIMARY KEY NOT NULL,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'complete',
            error TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;
    connection.execute("DROP TABLE IF EXISTS clipboard_items", [])?;
    connection.execute(
        "DELETE FROM settings
         WHERE key IN (
            'clipboard.shortcut',
            'clipboard.history.enabled',
            'clipboard.history.limit',
            'clipboard.paste.restore_previous',
            'search.source.clipboard',
            'search.weight.clipboard'
         )",
        [],
    )?;
    migrate_ai_model_profiles_to_providers(connection)?;

    Ok(())
}

fn migrate_ai_model_profiles_to_providers(connection: &Connection) -> Result<(), StorageError> {
    connection.execute(
        "INSERT OR IGNORE INTO ai_providers (
            id, name, provider_type, base_url, api_key, enabled, sort_order, created_at, updated_at
         )
         SELECT 'ai-provider:' || id, name, provider_type, base_url, api_key, enabled, sort_order, created_at, updated_at
         FROM ai_model_profiles
         WHERE trim(base_url) <> '' OR trim(model_name) <> ''",
        [],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO ai_provider_models (
            id, provider_id, model_name, enabled, sort_order, last_used_at, created_at, updated_at
         )
         SELECT 'ai-provider-model:' || id, 'ai-provider:' || id, model_name, enabled, sort_order,
                last_used_at, created_at, updated_at
         FROM ai_model_profiles
         WHERE trim(model_name) <> ''",
        [],
    )?;

    Ok(())
}

fn migrate_selection_action_models_from_assistant_profiles(
    connection: &Connection,
) -> Result<(), StorageError> {
    connection.execute(
        "UPDATE ai_selection_actions
         SET last_provider_id = (
                SELECT 'ai-provider:' || assistant.model_profile_id
                FROM ai_assistants assistant
                JOIN ai_provider_models model
                  ON model.provider_id = 'ai-provider:' || assistant.model_profile_id
                WHERE assistant.id = ai_selection_actions.assistant_id
                  AND trim(model.model_name) <> ''
                LIMIT 1
             ),
             last_model_name = (
                SELECT model.model_name
                FROM ai_assistants assistant
                JOIN ai_provider_models model
                  ON model.provider_id = 'ai-provider:' || assistant.model_profile_id
                WHERE assistant.id = ai_selection_actions.assistant_id
                  AND trim(model.model_name) <> ''
                LIMIT 1
             ),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE last_provider_id IS NULL
           AND EXISTS (
                SELECT 1
                FROM ai_assistants assistant
                JOIN ai_provider_models model
                  ON model.provider_id = 'ai-provider:' || assistant.model_profile_id
                WHERE assistant.id = ai_selection_actions.assistant_id
                  AND trim(model.model_name) <> ''
           )",
        [],
    )?;

    Ok(())
}

#[cfg(test)]
pub(crate) fn initialize_schema_for_tests(connection: &Connection) {
    initialize_schema(connection).expect("initialize schema for tests");
}

fn custom_command_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CustomCommand> {
    Ok(CustomCommand {
        id: row.get(0)?,
        name: row.get(1)?,
        command_type: row.get(2)?,
        target: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn phrase_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Phrase> {
    Ok(Phrase {
        id: row.get(0)?,
        title: row.get(1)?,
        text: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        use_count: row.get(5)?,
    })
}

fn selection_mark_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SelectionMark> {
    Ok(SelectionMark {
        id: row.get(0)?,
        text: row.get(1)?,
        source_app: row.get(2)?,
        created_at: row.get(3)?,
        use_count: row.get(4)?,
    })
}

fn todo_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Todo> {
    Ok(Todo {
        id: row.get(0)?,
        text: row.get(1)?,
        source_app: row.get(2)?,
        remind_at: row.get(3)?,
        status: row.get(4)?,
        last_notified_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn web_search_template_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WebSearchTemplate> {
    Ok(WebSearchTemplate {
        id: row.get(0)?,
        keyword: row.get(1)?,
        name: row.get(2)?,
        url_template: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn recent_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecentItem> {
    Ok(RecentItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        target: row.get(3)?,
        use_count: row.get(4)?,
        last_used_at: row.get(5)?,
    })
}

fn exclusion_rule_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExclusionRule> {
    Ok(ExclusionRule {
        id: row.get(0)?,
        match_type: row.get(1)?,
        pattern: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn pinned_result_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PinnedResult> {
    Ok(PinnedResult {
        result_id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        target: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn result_alias_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResultAlias> {
    Ok(ResultAlias {
        alias: row.get(0)?,
        normalized_alias: row.get(1)?,
        result_id: row.get(2)?,
        kind: row.get(3)?,
        title: row.get(4)?,
        target: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn ai_provider_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiProvider> {
    Ok(AiProvider {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_type: row.get(2)?,
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        sort_order: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn ai_provider_model_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiProviderModel> {
    Ok(AiProviderModel {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        model_name: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        sort_order: row.get(4)?,
        last_used_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn ai_model_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiModelProfile> {
    Ok(AiModelProfile {
        id: row.get(0)?,
        provider_type: row.get(1)?,
        name: row.get(2)?,
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        model_name: row.get(5)?,
        temperature: row.get(6)?,
        top_p: row.get(7)?,
        max_tokens: row.get(8)?,
        presence_penalty: row.get(9)?,
        frequency_penalty: row.get(10)?,
        stream: row.get::<_, i64>(11)? != 0,
        enabled: row.get::<_, i64>(12)? != 0,
        sort_order: row.get(13)?,
        last_used_at: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn ai_assistant_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiAssistant> {
    let stream: Option<i64> = row.get(11)?;
    Ok(AiAssistant {
        id: row.get(0)?,
        name: row.get(1)?,
        icon: row.get(2)?,
        description: row.get(3)?,
        model_profile_id: row.get(4)?,
        system_prompt: row.get(5)?,
        temperature: row.get(6)?,
        top_p: row.get(7)?,
        max_tokens: row.get(8)?,
        presence_penalty: row.get(9)?,
        frequency_penalty: row.get(10)?,
        stream: stream.map(|value| value != 0),
        enabled: row.get::<_, i64>(12)? != 0,
        sort_order: row.get(13)?,
        last_used_at: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn ai_selection_action_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiSelectionAction> {
    Ok(AiSelectionAction {
        assistant_id: row.get(0)?,
        assistant_name: row.get(1)?,
        assistant_icon: row.get(2)?,
        assistant_description: row.get(3)?,
        assistant_model_profile_id: row.get(4)?,
        system_prompt: row.get(5)?,
        assistant_enabled: row.get::<_, i64>(6)? != 0,
        show_in_selection: row.get::<_, i64>(7)? != 0,
        selection_label: row.get(8)?,
        sort_order: row.get(9)?,
        last_provider_id: row.get(10)?,
        last_model_name: row.get(11)?,
    })
}

fn ai_conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiConversation> {
    Ok(AiConversation {
        id: row.get(0)?,
        assistant_id: row.get(1)?,
        title: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        last_message_at: row.get(5)?,
    })
}

fn ai_message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiMessage> {
    Ok(AiMessage {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        status: row.get(4)?,
        error: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn validate_custom_command_fields(
    name: &str,
    command_type: &str,
    target: &str,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("自定义命令名称不能为空".into());
    }
    if target.is_empty() {
        return Err("自定义命令目标不能为空".into());
    }
    if !matches!(command_type, "url" | "file" | "program") {
        return Err("自定义命令类型只能是 url、file 或 program".into());
    }
    if target.contains(WEB_SEARCH_QUERY_PLACEHOLDER) {
        return Err("自定义命令不支持 {query}，请使用网页搜索模板".into());
    }

    Ok(())
}

fn stable_command_slug(name: &str) -> String {
    let mut slug = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();

    if slug.is_empty() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        slug = format!("{:x}", hasher.finish());
    }

    slug
}

fn validate_phrase_fields(title: &str, text: &str) -> Result<(), String> {
    if title.is_empty() {
        return Err("快捷短语标题不能为空".into());
    }
    if text.is_empty() {
        return Err("快捷短语内容不能为空".into());
    }
    if text.chars().count() > 4000 {
        return Err("快捷短语内容不能超过 4000 个字符".into());
    }

    Ok(())
}

fn validate_selection_mark_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("划词记录内容不能为空".into());
    }
    if text.chars().count() > SELECTION_MARK_TEXT_MAX_CHARS {
        return Err(format!(
            "划词记录内容不能超过 {} 个字符",
            SELECTION_MARK_TEXT_MAX_CHARS
        ));
    }

    Ok(())
}

fn validate_todo_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("待办内容不能为空".into());
    }
    if text.chars().count() > TODO_TEXT_MAX_CHARS {
        return Err(format!("待办内容不能超过 {} 个字符", TODO_TEXT_MAX_CHARS));
    }

    Ok(())
}

fn parse_future_utc_time(value: &str) -> Result<String, String> {
    let remind_at = parse_utc_time(value)?;
    if remind_at <= Utc::now() {
        return Err("提醒时间必须晚于当前时间".into());
    }

    Ok(utc_time_string(remind_at))
}

fn parse_utc_time(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| "提醒时间格式无效".into())
}

fn utc_time_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn normalize_todo_list_status(status: Option<&str>) -> Result<&'static str, String> {
    match status
        .unwrap_or("pending")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "pending" => Ok("pending"),
        "done" => Ok("done"),
        "all" => Ok("all"),
        _ => Err("待办状态只能是 pending、done 或 all".into()),
    }
}

fn escape_like_query(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().chars().take(512).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn generated_record_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let mut rng = rand::rng();
    let suffix: u32 = rand::Rng::random(&mut rng);
    format!("{prefix}:{millis:x}-{suffix:08x}")
}

fn validate_web_search_template_fields(
    keyword: &str,
    name: &str,
    url_template: &str,
) -> Result<(), String> {
    if keyword.is_empty() {
        return Err("网页搜索关键词不能为空".into());
    }
    if !keyword
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("网页搜索关键词只能包含字母、数字、- 或 _".into());
    }
    if name.is_empty() {
        return Err("网页搜索名称不能为空".into());
    }
    if !url_template.contains(WEB_SEARCH_QUERY_PLACEHOLDER) {
        return Err("网页搜索 URL 模板必须包含 {query}".into());
    }
    if !(url_template.starts_with("https://") || url_template.starts_with("http://")) {
        return Err("网页搜索 URL 模板必须以 http:// 或 https:// 开头".into());
    }

    Ok(())
}

fn validate_exclusion_rule_fields(match_type: &str, pattern: &str) -> Result<(), String> {
    if !matches!(match_type, "result_id" | "path_pattern") {
        return Err("排除规则类型只能是 result_id 或 path_pattern".into());
    }
    if pattern.is_empty() {
        return Err("排除规则内容不能为空".into());
    }
    if pattern.chars().count() > 512 {
        return Err("排除规则内容不能超过 512 个字符".into());
    }

    Ok(())
}

fn validate_ai_provider_fields(provider_type: &str) -> Result<(), String> {
    if provider_type != "openai_compatible" {
        return Err("第一版仅支持 OpenAI 兼容接口供应商".into());
    }

    Ok(())
}

fn validate_ai_provider_model_fields(provider_id: &str, model_name: &str) -> Result<(), String> {
    if provider_id.is_empty() {
        return Err("供应商不能为空".into());
    }
    if model_name.is_empty() {
        return Err("模型名称不能为空".into());
    }

    Ok(())
}

fn validate_ai_model_profile_fields(
    provider_type: &str,
    name: &str,
    max_tokens: Option<i64>,
) -> Result<(), String> {
    if !matches!(
        provider_type,
        "openai" | "anthropic" | "google" | "openai_compatible"
    ) {
        return Err("模型提供方类型无效".into());
    }
    if name.is_empty() {
        return Err("模型配置名称不能为空".into());
    }
    if let Some(max_tokens) = max_tokens {
        if max_tokens <= 0 {
            return Err("max_tokens 必须大于 0".into());
        }
    }

    Ok(())
}

fn validate_ai_assistant_fields(
    name: &str,
    icon: &str,
    model_profile_id: &str,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("助手名称不能为空".into());
    }
    if icon.is_empty() {
        return Err("助手图标不能为空".into());
    }
    if model_profile_id.is_empty() {
        return Err("助手必须绑定模型配置".into());
    }

    Ok(())
}

fn validate_ai_selection_action_fields(
    assistant_id: &str,
    selection_label: &str,
) -> Result<(), String> {
    if assistant_id.is_empty() {
        return Err("划词助手不能为空".into());
    }
    if selection_label.is_empty() {
        return Err("划词动作名称不能为空".into());
    }

    Ok(())
}

fn validate_ai_message_fields(role: &str, status: &str) -> Result<(), String> {
    if !matches!(role, "user" | "assistant" | "system") {
        return Err("消息角色无效".into());
    }
    if !matches!(status, "streaming" | "complete" | "error") {
        return Err("消息状态无效".into());
    }

    Ok(())
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

pub fn local_conversation_title(content: &str) -> String {
    content.trim().chars().take(10).collect()
}

pub fn selection_conversation_title(action_label: &str, content: &str) -> String {
    let cleaned = cleaned_text_summary(content);
    let snippet: String = cleaned.chars().take(10).collect();
    if snippet.is_empty() {
        action_label.trim().to_string()
    } else {
        format!("{}：{}", action_label.trim(), snippet)
    }
}

fn cleaned_text_summary(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_builtin_selection_assistant(id: &str) -> bool {
    matches!(
        id,
        TRANSLATION_AI_ASSISTANT_ID
            | SUMMARY_AI_ASSISTANT_ID
            | PROFESSIONAL_EXPLANATION_AI_ASSISTANT_ID
            | POLISH_AI_ASSISTANT_ID
            | KEY_POINTS_AI_ASSISTANT_ID
    )
}

fn infer_provider_name_from_base_url(base_url: &str) -> String {
    let without_scheme = base_url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .last()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default();
    let parts = host
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("api") {
        parts[1].to_string()
    } else if parts.len() >= 2 {
        parts[parts.len() - 2].to_string()
    } else {
        host.to_string()
    }
}

fn random_hex_id(connection: &Connection) -> Result<String, StorageError> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(StorageError::from)
}

fn recent_score_decay(age_days: f64) -> f32 {
    if age_days <= 7.0 {
        1.0
    } else if age_days <= 30.0 {
        0.5
    } else {
        0.2
    }
}

fn normalize_result_alias(alias: &str) -> Result<String, StorageError> {
    let normalized = alias
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if normalized.is_empty() {
        return Err(StorageError::Validation("Alias 不能为空".into()));
    }
    if normalized.len() > 64 {
        return Err(StorageError::Validation("Alias 不能超过 64 个字符".into()));
    }
    if normalized
        .chars()
        .any(|character| character == '\n' || character == '\r')
    {
        return Err(StorageError::Validation("Alias 不能包含换行".into()));
    }
    if matches!(
        normalized.as_str(),
        "app"
            | "apps"
            | "a"
            | "file"
            | "files"
            | "f"
            | "cmd"
            | "command"
            | "commands"
            | "sys"
            | "system"
            | "calc"
            | "calculator"
            | "phrase"
            | "phrases"
            | "snippet"
            | "snippets"
            | "ai"
            | "gpt"
            | "web"
            | "www"
            | "enc"
            | "dec"
            | "pwd"
            | "time"
            | "tools"
    ) {
        return Err(StorageError::Validation("Alias 与内置关键词冲突".into()));
    }
    Ok(normalized)
}

fn seed_defaults(connection: &Connection) -> Result<(), StorageError> {
    let defaults = [
        ("launcher.shortcut", "Alt+1"),
        ("launcher.double_alt.enabled", "true"),
        ("ai.shortcut", "Alt+3"),
        ("selection.enabled", "true"),
        ("selection.trigger.mode", "ctrl_mouse"),
        ("file.editor.path", ""),
        ("folder.editor.path", ""),
        ("ui.language", "system"),
        ("startup.enabled", "false"),
        ("search.source.apps", "true"),
        ("search.source.files", "true"),
        ("search.source.calculator", "true"),
        ("search.source.system", "true"),
        ("search.source.ai", "true"),
        ("search.source.phrase", "true"),
        ("search.weight.apps", "1.00"),
        ("search.weight.files", "1.00"),
        ("search.weight.calculator", "1.00"),
        ("search.weight.system", "1.00"),
        ("search.weight.ai", "1.00"),
        ("search.weight.phrase", "1.00"),
        ("search.source.web_search", "true"),
        ("search.weight.web_search", "1.00"),
        ("search.source.tools", "true"),
        ("search.weight.tools", "1.00"),
        ("search.smart_ranking.enabled", "true"),
        ("everything.search.full_path", "false"),
        ("everything.search.content", "false"),
        ("tools.menu.alias", "/"),
        (
            "slash.board.scopes",
            r#"[{"id":"all","visible":true},{"id":"run","visible":true},{"id":"text","visible":true},{"id":"web","visible":true},{"id":"tools","visible":true},{"id":"recent","visible":true},{"id":"open","visible":true},{"id":"system","visible":true}]"#,
        ),
        ("tools.password.length", "16"),
        ("tools.password.uppercase", "true"),
        ("tools.password.lowercase", "true"),
        ("tools.password.digits", "true"),
        ("tools.password.hyphen", "false"),
        ("tools.password.underscore", "false"),
        ("tools.password.special", "true"),
        ("tools.password.brackets", "false"),
    ];

    for (key, value) in defaults {
        connection.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }

    connection.execute(
        "INSERT OR IGNORE INTO web_search_templates (id, keyword, name, url_template)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            "web-search:bing",
            "web",
            "Bing",
            "https://www.bing.com/search?q={query}"
        ],
    )?;

    connection.execute(
        "INSERT OR IGNORE INTO ai_model_profiles (
            id, provider_type, name, base_url, api_key, model_name, stream, enabled, sort_order
         )
         VALUES (?1, 'openai_compatible', ?2, '', '', '', 1, 1, 0)",
        params![DEFAULT_AI_MODEL_PROFILE_ID, "本地 OpenAI 兼容接口"],
    )?;

    connection.execute(
        "DELETE FROM ai_providers
         WHERE id = ?1
           AND trim(base_url) = ''
           AND trim(api_key) = ''
           AND EXISTS (
                SELECT 1
                FROM ai_providers real_provider
                WHERE real_provider.id <> ai_providers.id
           )
           AND NOT EXISTS (
                SELECT 1
                FROM ai_provider_models
                WHERE provider_id = ai_providers.id
           )",
        params![DEFAULT_AI_PROVIDER_ID],
    )?;

    connection.execute(
        "INSERT INTO ai_providers (
            id, name, provider_type, base_url, api_key, enabled, sort_order
         )
         SELECT ?1, ?2, 'openai_compatible', '', '', 1, 0
         WHERE NOT EXISTS (
            SELECT 1
            FROM ai_providers
         )",
        params![DEFAULT_AI_PROVIDER_ID, "本地 OpenAI 兼容接口"],
    )?;

    connection.execute(
        "INSERT OR IGNORE INTO ai_assistants (
            id, name, icon, description, model_profile_id, system_prompt, enabled, sort_order
         )
         VALUES (?1, ?2, 'AI', '', ?3, '', 1, 0)",
        params![
            DEFAULT_AI_ASSISTANT_ID,
            "默认助手",
            DEFAULT_AI_MODEL_PROFILE_ID
        ],
    )?;

    let selection_assistants = [
        (
            TRANSLATION_AI_ASSISTANT_ID,
            "翻译",
            "译",
            "划词翻译",
            "你是专业翻译助手。请自动判断用户提供文本的主要语言：英文翻译为简体中文，中文翻译为英文，其他语言默认翻译为简体中文。保留原意、术语、数字、代码和格式。只输出译文，不要解释。",
            0,
        ),
        (
            SUMMARY_AI_ASSISTANT_ID,
            "总结",
            "摘",
            "划词总结",
            "你是总结助手。请用简体中文总结用户提供的文本，提炼关键结论、事实和行动项，控制在 5 条要点以内。",
            1,
        ),
        (
            PROFESSIONAL_EXPLANATION_AI_ASSISTANT_ID,
            "专业解释",
            "解",
            "划词专业解释",
            "你是专业解释助手。请围绕用户提供的文本解释相关专业知识，说明背景、关键概念、原因和影响。表达要准确、清楚，适合非专业读者理解。",
            2,
        ),
        (
            POLISH_AI_ASSISTANT_ID,
            "润色",
            "润",
            "划词润色",
            "你是中文润色助手。请在不改变原意的前提下，让用户提供的文本更清晰、自然、专业。只输出润色后的文本。",
            3,
        ),
        (
            KEY_POINTS_AI_ASSISTANT_ID,
            "提取要点",
            "点",
            "划词提取要点",
            "你是要点提取助手。请从用户提供的文本中提取核心要点、重要数字、结论和待办事项，用简洁列表输出。",
            4,
        ),
    ];

    for (id, name, icon, description, prompt, sort_order) in selection_assistants {
        connection.execute(
            "INSERT OR IGNORE INTO ai_assistants (
                id, name, icon, description, model_profile_id, system_prompt, enabled, sort_order
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
            params![
                id,
                name,
                icon,
                description,
                DEFAULT_AI_MODEL_PROFILE_ID,
                prompt,
                sort_order + 1
            ],
        )?;
        connection.execute(
            "INSERT OR IGNORE INTO ai_selection_actions (
                assistant_id, show_in_selection, selection_label, sort_order
             )
             VALUES (?1, 1, ?2, ?3)",
            params![id, name, sort_order],
        )?;
    }

    connection.execute(
        "UPDATE ai_assistants
         SET name = '翻译',
             icon = '译',
             description = '划词翻译',
             system_prompt = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND (trim(system_prompt) = '' OR system_prompt = '请将用户输入翻译为简体中文，保留原意，直接输出译文。')",
        params![
            selection_assistants[0].4,
            TRANSLATION_AI_ASSISTANT_ID
        ],
    )?;

    migrate_selection_action_models_from_assistant_profiles(connection)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_schema_and_seed_settings() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");

        let shortcut: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'launcher.shortcut'",
                [],
                |row| row.get(0),
            )
            .expect("read default shortcut");

        assert_eq!(shortcut, "Alt+1");

        let ai_shortcut: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'ai.shortcut'",
                [],
                |row| row.get(0),
            )
            .expect("read default ai shortcut");

        assert_eq!(ai_shortcut, "Alt+3");

        let profile: (String, String, String) = connection
            .query_row(
                "SELECT provider_type, base_url, model_name
                 FROM ai_model_profiles
                 WHERE id = ?1",
                params![DEFAULT_AI_MODEL_PROFILE_ID],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read default ai model profile");

        assert_eq!(profile, ("openai_compatible".into(), "".into(), "".into()));

        let assistant_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_assistants", [], |row| row.get(0))
            .expect("count seeded assistants");

        assert_eq!(assistant_count, 6);

        let selection_action_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_selection_actions", [], |row| {
                row.get(0)
            })
            .expect("count selection actions");

        assert_eq!(selection_action_count, 5);
    }

    #[test]
    fn upserts_settings() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .set_setting("launcher.shortcut", "Alt+1")
            .expect("write setting");
        storage
            .set_setting("launcher.shortcut", "Ctrl+Space")
            .expect("update setting");

        let value = storage
            .get_setting("launcher.shortcut")
            .expect("read setting");

        assert_eq!(value.as_deref(), Some("Ctrl+Space"));
    }

    #[test]
    fn ai_model_profile_crud_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);

        let created = storage
            .upsert_ai_model_profile(AiModelProfileInput {
                id: None,
                provider_type: "openai_compatible".into(),
                name: "Local Qwen".into(),
                base_url: "http://127.0.0.1:11434".into(),
                api_key: "".into(),
                model_name: "qwen".into(),
                temperature: Some(0.2),
                top_p: None,
                max_tokens: Some(2048),
                presence_penalty: None,
                frequency_penalty: None,
                stream: true,
                enabled: true,
                sort_order: 3,
            })
            .expect("create ai model profile");

        assert_eq!(created.provider_type, "openai_compatible");
        assert_eq!(created.api_key, "");
        assert_eq!(created.max_tokens, Some(2048));

        let updated = storage
            .upsert_ai_model_profile(AiModelProfileInput {
                id: Some(created.id.clone()),
                provider_type: "openai_compatible".into(),
                name: "Local Qwen Updated".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                api_key: "secret".into(),
                model_name: "qwen2".into(),
                temperature: None,
                top_p: Some(0.9),
                max_tokens: None,
                presence_penalty: None,
                frequency_penalty: None,
                stream: false,
                enabled: false,
                sort_order: 4,
            })
            .expect("update ai model profile");

        assert_eq!(updated.name, "Local Qwen Updated");
        assert_eq!(updated.top_p, Some(0.9));
        assert!(!updated.stream);
        assert!(!updated.enabled);
        assert!(storage
            .delete_ai_model_profile(&created.id)
            .expect("delete unused ai model profile"));
    }

    #[test]
    fn ai_provider_and_model_roundtrip_deduplicates_models() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);

        let provider = storage
            .upsert_ai_provider(AiProviderInput {
                id: None,
                name: "".into(),
                provider_type: "openai_compatible".into(),
                base_url: "https://api.siliconflow.cn/v1".into(),
                api_key: "".into(),
                enabled: true,
                sort_order: 2,
            })
            .expect("create provider");

        assert_eq!(provider.id, DEFAULT_AI_PROVIDER_ID);
        assert_eq!(provider.name, "siliconflow");
        assert_eq!(
            storage.list_ai_providers().expect("list providers").len(),
            1
        );

        let model = storage
            .upsert_ai_provider_model(AiProviderModelInput {
                id: None,
                provider_id: provider.id.clone(),
                model_name: "qwen".into(),
                enabled: false,
                sort_order: 0,
            })
            .expect("create model");
        let enabled_model = storage
            .upsert_ai_provider_model(AiProviderModelInput {
                id: None,
                provider_id: provider.id.clone(),
                model_name: "qwen".into(),
                enabled: true,
                sort_order: 1,
            })
            .expect("enable existing model");

        assert_eq!(model.id, enabled_model.id);
        assert!(enabled_model.enabled);
        assert_eq!(
            storage
                .list_ai_provider_models(Some(&provider.id))
                .expect("list models")
                .len(),
            1
        );
    }

    #[test]
    fn seed_defaults_does_not_restore_empty_default_ai_provider_when_real_provider_exists() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);

        let provider = storage
            .upsert_ai_provider(AiProviderInput {
                id: None,
                name: "Local".into(),
                provider_type: "openai_compatible".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                api_key: "".into(),
                enabled: true,
                sort_order: 0,
            })
            .expect("reuse default provider");

        assert_eq!(provider.id, DEFAULT_AI_PROVIDER_ID);
        assert!(storage
            .delete_ai_provider(DEFAULT_AI_PROVIDER_ID)
            .expect("delete provider"));
        let replacement = storage
            .upsert_ai_provider(AiProviderInput {
                id: Some("ai-provider:local".into()),
                name: "Local".into(),
                provider_type: "openai_compatible".into(),
                base_url: "http://127.0.0.1:11434/v1".into(),
                api_key: "".into(),
                enabled: true,
                sort_order: 0,
            })
            .expect("create real provider");

        seed_defaults(&storage.connection).expect("seed defaults again");
        let providers = storage.list_ai_providers().expect("list providers");

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, replacement.id);
    }

    #[test]
    fn old_model_profiles_migrate_to_provider_models() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        connection
            .execute(
                "INSERT INTO ai_model_profiles (
                    id, provider_type, name, base_url, api_key, model_name, enabled, sort_order
                 )
                 VALUES ('legacy', 'openai_compatible', 'Legacy', 'https://api.openai.com/v1', 'secret', 'gpt-test', 1, 7)",
                [],
            )
            .expect("insert legacy profile");

        migrate_ai_model_profiles_to_providers(&connection).expect("migrate legacy profile");
        let storage = Storage::from_connection_for_tests(connection);
        let provider = storage
            .get_ai_provider("ai-provider:legacy")
            .expect("read provider")
            .expect("provider migrated");
        let model = storage
            .get_ai_provider_model(&provider.id, "gpt-test")
            .expect("read model")
            .expect("model migrated");

        assert_eq!(provider.api_key, "secret");
        assert!(model.enabled);
    }

    #[test]
    fn ai_assistant_crud_and_translation_delete_guard() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);

        let assistant = storage
            .upsert_ai_assistant(AiAssistantInput {
                id: None,
                name: "Coder".into(),
                icon: "CD".into(),
                description: "Code help".into(),
                model_profile_id: DEFAULT_AI_MODEL_PROFILE_ID.into(),
                system_prompt: "Be concise".into(),
                enabled: true,
                sort_order: 2,
            })
            .expect("create assistant");

        assert_eq!(assistant.system_prompt, "Be concise");
        storage
            .mark_ai_assistant_used(&assistant.id)
            .expect("mark assistant used");
        assert!(storage
            .get_ai_assistant(&assistant.id)
            .expect("get assistant")
            .expect("assistant exists")
            .last_used_at
            .is_some());
        assert!(storage
            .delete_ai_assistant(&assistant.id)
            .expect("delete assistant"));
        assert!(storage
            .delete_ai_assistant(TRANSLATION_AI_ASSISTANT_ID)
            .is_err());
    }

    #[test]
    fn selection_conversation_title_cleans_whitespace() {
        assert_eq!(
            selection_conversation_title("翻译", "  hello\n   world  again "),
            "翻译：hello worl"
        );
    }

    #[test]
    fn ai_message_delete_removes_one_message() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);
        let conversation = storage
            .create_ai_conversation(DEFAULT_AI_ASSISTANT_ID, "")
            .expect("create conversation");
        let first = storage
            .insert_ai_message(&conversation.id, "user", "hello", "complete", None)
            .expect("insert user message");
        storage
            .insert_ai_message(&conversation.id, "assistant", "hi", "complete", None)
            .expect("insert assistant message");

        assert!(storage
            .delete_ai_message(&first.id)
            .expect("delete ai message"));
        let messages = storage
            .list_ai_messages(&conversation.id)
            .expect("list ai messages");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
    }

    #[test]
    fn ai_conversation_delete_removes_messages() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);
        let conversation = storage
            .create_ai_conversation(DEFAULT_AI_ASSISTANT_ID, "")
            .expect("create conversation");
        storage
            .insert_ai_message(
                &conversation.id,
                "user",
                "0123456789abcdef",
                "complete",
                None,
            )
            .expect("insert user message");

        let renamed = storage
            .get_ai_conversation(&conversation.id)
            .expect("get conversation")
            .expect("conversation exists");
        assert_eq!(renamed.title, "0123456789");
        assert!(storage
            .delete_ai_conversation(&conversation.id)
            .expect("delete conversation"));
        assert!(storage
            .list_ai_messages(&conversation.id)
            .expect("list ai messages")
            .is_empty());
    }

    #[test]
    fn exports_only_requested_settings() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .set_setting("launcher.shortcut", "Alt+1")
            .expect("write shortcut");
        storage
            .set_setting("ai.api_key", "secret")
            .expect("write api key");

        let settings = storage
            .export_settings(&["launcher.shortcut"])
            .expect("export settings");

        assert_eq!(
            settings.get("launcher.shortcut").map(String::as_str),
            Some("Alt+1")
        );
        assert!(!settings.contains_key("ai.api_key"));
    }

    #[test]
    fn records_recent_items() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .record_recent_item("app:test", "app", "Test App", "C:\\Test.exe")
            .expect("record recent item");
        storage
            .record_recent_item("app:test", "app", "Test App", "C:\\Test.exe")
            .expect("record recent item again");

        let scores = storage.recent_scores().expect("read recent scores");

        assert_eq!(scores.get("app:test").copied(), Some(0.1));
    }

    #[test]
    fn lists_recent_items_by_last_used_with_limit() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .connection
            .execute(
                "INSERT INTO recent_items (id, kind, title, target, use_count, last_used_at)
                 VALUES
                    ('app:old', 'app', 'Old App', 'old.exe', 3, '2026-06-01T00:00:00.000Z'),
                    ('file:new', 'file', 'New Folder', 'C:\\New', 1, '2026-06-03T00:00:00.000Z'),
                    ('app:middle', 'app', 'Middle App', 'middle.exe', 2, '2026-06-02T00:00:00.000Z')",
                [],
            )
            .expect("insert recent items");

        let items = storage.list_recent_items(2).expect("list recent items");

        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["file:new", "app:middle"]
        );
        assert_eq!(items[0].kind, "file");
        assert_eq!(items[0].target, r"C:\New");
        assert_eq!(storage.list_recent_items(0).expect("list none").len(), 0);
    }

    #[test]
    fn recent_scores_decay_for_old_items() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .connection
            .execute(
                "INSERT INTO recent_items (id, kind, title, target, use_count, last_used_at)
                 VALUES
                    ('app:fresh', 'app', 'Fresh', 'fresh.exe', 10, '2026-06-01T00:00:00.000Z'),
                    ('app:stale', 'app', 'Stale', 'stale.exe', 10, '2026-05-01T00:00:00.000Z')",
                [],
            )
            .expect("insert recent items");

        let scores = storage
            .recent_scores_at("2026-06-02T00:00:00.000Z")
            .expect("read recent scores");

        assert_eq!(scores.get("app:fresh").copied(), Some(0.5));
        assert_eq!(scores.get("app:stale").copied(), Some(0.1));
    }

    #[test]
    fn records_query_specific_selection_scores() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .record_query_selection("code", "app:vscode")
            .expect("record query selection"));
        storage
            .record_query_selection("code", "app:vscode")
            .expect("record query selection again");
        storage
            .record_query_selection("terminal", "app:vscode")
            .expect("record other query selection");
        assert!(!storage
            .record_query_selection("", "app:vscode")
            .expect("ignore empty query"));

        let scores = storage
            .query_selection_scores("code")
            .expect("read query selection scores");

        assert_eq!(scores.get("app:vscode").copied(), Some(0.07));
        assert!(storage
            .query_selection_scores("terminal")
            .expect("read other query scores")
            .contains_key("app:vscode"));
        assert!(storage
            .query_selection_scores("missing")
            .expect("read missing query scores")
            .is_empty());
    }

    #[test]
    fn clears_ranking_learning_without_removing_manual_rules() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage::from_connection_for_tests(connection);

        storage
            .record_recent_item("app:vscode", "app", "Code", r"C:\Code.exe")
            .expect("record recent item");
        storage
            .record_query_selection("code", "app:vscode")
            .expect("record query selection");
        storage
            .set_result_pinned(
                PinnedResultInput {
                    result_id: "app:vscode".into(),
                    kind: "app".into(),
                    title: "Code".into(),
                    target: r"C:\Code.exe".into(),
                },
                true,
            )
            .expect("pin result");
        storage
            .upsert_result_alias(ResultAliasInput {
                alias: "ide".into(),
                result_id: "app:vscode".into(),
                kind: "app".into(),
                title: "Code".into(),
                target: r"C:\Code.exe".into(),
            })
            .expect("save alias");

        let cleared = storage
            .clear_ranking_learning()
            .expect("clear ranking learning");

        assert_eq!(cleared, 2);
        assert!(storage.recent_scores().expect("recent scores").is_empty());
        assert!(storage
            .query_selection_scores("code")
            .expect("query selection scores")
            .is_empty());
        assert_eq!(storage.list_pinned_results().expect("pinned").len(), 1);
        assert_eq!(storage.list_result_aliases().expect("aliases").len(), 1);
    }

    #[test]
    fn pinned_results_and_aliases_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage::from_connection_for_tests(connection);

        let pinned = storage
            .set_result_pinned(
                PinnedResultInput {
                    result_id: "app:vscode".into(),
                    kind: "app".into(),
                    title: "Code".into(),
                    target: r"C:\Code.exe".into(),
                },
                true,
            )
            .expect("pin result")
            .expect("pinned result");
        assert_eq!(pinned.result_id, "app:vscode");
        assert_eq!(
            storage
                .pinned_result_scores()
                .expect("pinned scores")
                .get("app:vscode")
                .copied(),
            Some(0.65)
        );

        let alias = storage
            .upsert_result_alias(ResultAliasInput {
                alias: "IDE".into(),
                result_id: "app:vscode".into(),
                kind: "app".into(),
                title: "Code".into(),
                target: r"C:\Code.exe".into(),
            })
            .expect("save alias");
        assert_eq!(alias.normalized_alias, "ide");

        assert!(storage.delete_result_alias("ide").expect("delete alias"));
        assert!(storage
            .set_result_pinned(
                PinnedResultInput {
                    result_id: "app:vscode".into(),
                    kind: "app".into(),
                    title: "Code".into(),
                    target: r"C:\Code.exe".into(),
                },
                false,
            )
            .expect("unpin result")
            .is_none());
    }

    #[test]
    fn result_aliases_reject_conflicts() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        seed_defaults(&connection).expect("seed defaults");
        let storage = Storage::from_connection_for_tests(connection);

        assert!(storage
            .upsert_result_alias(ResultAliasInput {
                alias: "app".into(),
                result_id: "app:vscode".into(),
                kind: "app".into(),
                title: "Code".into(),
                target: r"C:\Code.exe".into(),
            })
            .is_err());
        assert!(storage
            .upsert_result_alias(ResultAliasInput {
                alias: "web".into(),
                result_id: "app:vscode".into(),
                kind: "app".into(),
                title: "Code".into(),
                target: r"C:\Code.exe".into(),
            })
            .is_err());
        storage
            .set_setting("tools.menu.alias", "go")
            .expect("set tool menu alias");
        assert!(storage
            .upsert_result_alias(ResultAliasInput {
                alias: "go".into(),
                result_id: "app:vscode".into(),
                kind: "app".into(),
                title: "Code".into(),
                target: r"C:\Code.exe".into(),
            })
            .is_err());

        storage
            .upsert_result_alias(ResultAliasInput {
                alias: "ide".into(),
                result_id: "app:vscode".into(),
                kind: "app".into(),
                title: "Code".into(),
                target: r"C:\Code.exe".into(),
            })
            .expect("save alias");
        assert!(storage
            .upsert_result_alias(ResultAliasInput {
                alias: "ide".into(),
                result_id: "app:other".into(),
                kind: "app".into(),
                title: "Other".into(),
                target: r"C:\Other.exe".into(),
            })
            .is_err());
    }

    #[test]
    fn custom_command_add_update_delete_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let created = storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "Docs".into(),
                command_type: "url".into(),
                target: "https://example.com".into(),
            })
            .expect("create custom command");

        assert_eq!(created.name, "Docs");
        assert_eq!(
            storage.list_custom_commands().expect("list commands").len(),
            1
        );

        let updated = storage
            .upsert_custom_command(CustomCommandInput {
                id: Some(created.id.clone()),
                name: "Docs".into(),
                command_type: "file".into(),
                target: r"C:\Docs".into(),
            })
            .expect("update custom command");

        assert_eq!(updated.command_type, "file");
        assert!(storage
            .delete_custom_command(&created.id)
            .expect("delete custom command"));
        assert!(storage
            .list_custom_commands()
            .expect("list commands")
            .is_empty());
    }

    #[test]
    fn custom_command_rejects_invalid_fields() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "".into(),
                command_type: "url".into(),
                target: "https://example.com".into(),
            })
            .is_err());
        assert!(storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "Bad".into(),
                command_type: "shell".into(),
                target: "echo bad".into(),
            })
            .is_err());
        assert!(storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "Empty".into(),
                command_type: "program".into(),
                target: "".into(),
            })
            .is_err());
        assert!(storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "Query URL".into(),
                command_type: "url".into(),
                target: "https://example.com/search?q={query}".into(),
            })
            .is_err());
    }

    #[test]
    fn custom_command_rejects_duplicate_names() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "Docs".into(),
                command_type: "url".into(),
                target: "https://example.com".into(),
            })
            .expect("create command");

        assert!(storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "docs".into(),
                command_type: "url".into(),
                target: "https://example.org".into(),
            })
            .is_err());
    }

    #[test]
    fn phrase_add_update_delete_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let created = storage
            .upsert_phrase(PhraseInput {
                id: None,
                title: "Email".into(),
                text: "Thanks for the update.".into(),
            })
            .expect("create phrase");

        assert_eq!(created.title, "Email");
        assert_eq!(storage.list_phrases().expect("list phrases").len(), 1);

        let updated = storage
            .upsert_phrase(PhraseInput {
                id: Some(created.id.clone()),
                title: "Email".into(),
                text: "Thanks.".into(),
            })
            .expect("update phrase");

        assert_eq!(updated.text, "Thanks.");
        storage
            .mark_phrase_used(&created.id)
            .expect("mark phrase used");
        assert_eq!(
            storage
                .get_phrase(&created.id)
                .expect("get phrase")
                .expect("phrase exists")
                .use_count,
            1
        );
        assert!(storage.delete_phrase(&created.id).expect("delete phrase"));
        assert!(storage.list_phrases().expect("list phrases").is_empty());
    }

    #[test]
    fn phrase_rejects_empty_and_too_long_text() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .upsert_phrase(PhraseInput {
                id: None,
                title: "".into(),
                text: "hello".into(),
            })
            .is_err());
        assert!(storage
            .upsert_phrase(PhraseInput {
                id: None,
                title: "Empty".into(),
                text: "".into(),
            })
            .is_err());
        assert!(storage
            .upsert_phrase(PhraseInput {
                id: None,
                title: "Long".into(),
                text: "x".repeat(4001),
            })
            .is_err());
    }

    #[test]
    fn selection_mark_crud_allows_duplicate_text_and_tracks_use_count() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let first = storage
            .save_selection_mark(SelectionMarkInput {
                text: "  same text  ".into(),
                source_app: Some("  ".into()),
            })
            .expect("create first mark");
        let second = storage
            .save_selection_mark(SelectionMarkInput {
                text: "same text".into(),
                source_app: None,
            })
            .expect("create duplicate mark");

        assert_ne!(first.id, second.id);
        assert!(first.id.starts_with("mark:"));
        assert_eq!(first.text, "same text");
        assert_eq!(first.source_app, None);
        assert_eq!(storage.list_selection_marks().expect("list marks").len(), 2);

        storage
            .mark_selection_mark_used(&first.id)
            .expect("mark used");
        assert_eq!(
            storage
                .get_selection_mark(&first.id)
                .expect("get mark")
                .expect("mark exists")
                .use_count,
            1
        );
        assert!(storage
            .delete_selection_mark(&first.id)
            .expect("delete mark"));
        assert_eq!(storage.clear_selection_marks().expect("clear marks"), 1);
        assert!(storage
            .list_selection_marks()
            .expect("list marks")
            .is_empty());
    }

    #[test]
    fn selection_mark_rejects_empty_and_too_long_text() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .save_selection_mark(SelectionMarkInput {
                text: "   ".into(),
                source_app: None,
            })
            .is_err());
        assert!(storage
            .save_selection_mark(SelectionMarkInput {
                text: "x".repeat(SELECTION_MARK_TEXT_MAX_CHARS + 1),
                source_app: None,
            })
            .is_err());
    }

    #[test]
    fn selection_mark_limit_prunes_oldest_records() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let first = storage
            .save_selection_mark_with_limit(
                SelectionMarkInput {
                    text: "first".into(),
                    source_app: None,
                },
                2,
            )
            .expect("create first");
        let second = storage
            .save_selection_mark_with_limit(
                SelectionMarkInput {
                    text: "second".into(),
                    source_app: None,
                },
                2,
            )
            .expect("create second");
        let third = storage
            .save_selection_mark_with_limit(
                SelectionMarkInput {
                    text: "third".into(),
                    source_app: None,
                },
                2,
            )
            .expect("create third");

        let ids = storage
            .list_selection_marks()
            .expect("list marks")
            .into_iter()
            .map(|mark| mark.id)
            .collect::<Vec<_>>();

        assert!(!ids.contains(&first.id));
        assert!(ids.contains(&second.id));
        assert!(ids.contains(&third.id));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn todo_crud_complete_snooze_delete_and_clear_completed() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };
        let remind_at = utc_time_string(Utc::now() + ChronoDuration::hours(1));

        let created = storage
            .save_todo(TodoInput {
                text: "  follow up  ".into(),
                source_app: None,
                remind_at,
            })
            .expect("create todo");

        assert!(created.id.starts_with("todo:"));
        assert_eq!(created.text, "follow up");
        assert_eq!(created.status, "pending");
        assert_eq!(
            storage.list_todos(None, None).expect("list pending").len(),
            1
        );

        let updated = storage
            .update_todo(TodoUpdateInput {
                id: created.id.clone(),
                text: Some("follow up later".into()),
                remind_at: Some(utc_time_string(Utc::now() + ChronoDuration::hours(2))),
            })
            .expect("update todo");
        assert_eq!(updated.text, "follow up later");

        let snoozed = storage.snooze_todo(&created.id, 10).expect("snooze todo");
        assert_eq!(snoozed.status, "pending");

        let completed = storage.complete_todo(&created.id).expect("complete todo");
        assert_eq!(completed.status, "done");
        assert_eq!(
            storage
                .clear_completed_todos()
                .expect("clear completed todos"),
            1
        );
        assert!(storage
            .list_todos(Some("all"), None)
            .expect("list all")
            .is_empty());
    }

    #[test]
    fn todo_rejects_invalid_fields() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .save_todo(TodoInput {
                text: " ".into(),
                source_app: None,
                remind_at: utc_time_string(Utc::now() + ChronoDuration::hours(1)),
            })
            .is_err());
        assert!(storage
            .save_todo(TodoInput {
                text: "x".repeat(TODO_TEXT_MAX_CHARS + 1),
                source_app: None,
                remind_at: utc_time_string(Utc::now() + ChronoDuration::hours(1)),
            })
            .is_err());
        assert!(storage
            .save_todo(TodoInput {
                text: "todo".into(),
                source_app: None,
                remind_at: "not-a-time".into(),
            })
            .is_err());
        assert!(storage
            .save_todo(TodoInput {
                text: "todo".into(),
                source_app: None,
                remind_at: utc_time_string(Utc::now() - ChronoDuration::minutes(1)),
            })
            .is_err());
        assert!(storage
            .connection
            .execute(
                "INSERT INTO todos (id, text, remind_at, status)
                 VALUES ('todo:bad-status', 'todo', '2999-06-01T00:00:00.000Z', 'bad')",
                [],
            )
            .is_err());
        assert!(storage.list_todos(Some("bad"), None).is_err());
        assert!(storage.snooze_todo("todo:missing", 0).is_err());
    }

    #[test]
    fn todo_due_query_returns_only_pending_due_items_and_index_exists() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        storage
            .connection
            .execute(
                "INSERT INTO todos (id, text, remind_at, status, created_at, updated_at)
                 VALUES
                 ('todo:due', 'due alpha', '2026-06-01T00:00:00.000Z', 'pending', '2026-06-01T00:00:00.000Z', '2026-06-01T00:00:00.000Z'),
                 ('todo:future', 'future beta', '2026-06-03T00:00:00.000Z', 'pending', '2026-06-01T00:00:00.000Z', '2026-06-01T00:00:00.000Z'),
                 ('todo:done', 'done alpha', '2026-06-01T00:00:00.000Z', 'done', '2026-06-01T00:00:00.000Z', '2026-06-01T00:00:00.000Z')",
                [],
            )
            .expect("seed todos");

        let due = storage
            .due_todos(parse_utc_time("2026-06-02T00:00:00.000Z").expect("parse now"))
            .expect("due todos");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, "todo:due");

        let filtered = storage
            .list_todos(Some("all"), Some("alpha"))
            .expect("filter todos");
        assert_eq!(filtered.len(), 2);

        let index_exists: i64 = storage
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_todos_due'",
                [],
                |row| row.get(0),
            )
            .expect("query index");
        assert_eq!(index_exists, 1);
    }

    #[test]
    fn v2_regression_storage_crud_for_custom_commands_and_phrases() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let command = storage
            .upsert_custom_command(CustomCommandInput {
                id: None,
                name: "Docs".into(),
                command_type: "url".into(),
                target: "https://example.com".into(),
            })
            .expect("create command");
        let phrase = storage
            .upsert_phrase(PhraseInput {
                id: None,
                title: "Greeting".into(),
                text: "Hello there".into(),
            })
            .expect("create phrase");

        assert_eq!(storage.list_custom_commands().expect("commands").len(), 1);
        assert_eq!(storage.list_phrases().expect("phrases").len(), 1);

        storage
            .delete_custom_command(&command.id)
            .expect("delete command");
        storage.delete_phrase(&phrase.id).expect("delete phrase");

        assert!(storage.list_custom_commands().expect("commands").is_empty());
        assert!(storage.list_phrases().expect("phrases").is_empty());
    }

    #[test]
    fn web_search_template_add_update_delete_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let created = storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: None,
                keyword: "gh".into(),
                name: "GitHub".into(),
                url_template: "https://github.com/search?q={query}".into(),
            })
            .expect("create web search template");

        assert_eq!(created.keyword, "gh");
        assert_eq!(
            storage
                .list_web_search_templates()
                .expect("list templates")
                .len(),
            1
        );

        let updated = storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: Some(created.id.clone()),
                keyword: "gh".into(),
                name: "GitHub Code".into(),
                url_template: "https://github.com/search?q={query}&type=code".into(),
            })
            .expect("update web search template");

        assert_eq!(updated.name, "GitHub Code");
        assert!(storage
            .delete_web_search_template(&created.id)
            .expect("delete web search template"));
        assert!(storage
            .list_web_search_templates()
            .expect("list templates")
            .is_empty());
    }

    #[test]
    fn web_search_template_rejects_invalid_fields_and_duplicate_keywords() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: None,
                keyword: "bad keyword".into(),
                name: "Bad".into(),
                url_template: "https://example.com/search?q={query}".into(),
            })
            .is_err());
        assert!(storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: None,
                keyword: "bad".into(),
                name: "Bad".into(),
                url_template: "https://example.com/search".into(),
            })
            .is_err());

        storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: None,
                keyword: "gh".into(),
                name: "GitHub".into(),
                url_template: "https://github.com/search?q={query}".into(),
            })
            .expect("create web search template");

        assert!(storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: None,
                keyword: "GH".into(),
                name: "GitHub Duplicate".into(),
                url_template: "https://github.com/search?q={query}".into(),
            })
            .is_err());
    }

    #[test]
    fn exclusion_rule_add_update_delete_roundtrip() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        let created = storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: None,
                match_type: "path_pattern".into(),
                pattern: r"C:\Temp\*".into(),
            })
            .expect("create exclusion rule");

        assert_eq!(created.match_type, "path_pattern");
        assert_eq!(
            storage
                .list_exclusion_rules()
                .expect("list exclusion rules")
                .len(),
            1
        );

        let updated = storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: Some(created.id.clone()),
                match_type: "result_id".into(),
                pattern: "app:notepad".into(),
            })
            .expect("update exclusion rule");

        assert_eq!(updated.pattern, "app:notepad");
        assert!(storage
            .delete_exclusion_rule(&created.id)
            .expect("delete exclusion rule"));
        assert!(storage
            .list_exclusion_rules()
            .expect("list exclusion rules")
            .is_empty());
    }

    #[test]
    fn exclusion_rule_rejects_invalid_fields_and_duplicates() {
        let connection = Connection::open_in_memory().expect("open in-memory sqlite");
        initialize_schema(&connection).expect("initialize schema");
        let storage = Storage {
            database_path: PathBuf::from("memory"),
            connection,
        };

        assert!(storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: None,
                match_type: "title".into(),
                pattern: "Notepad".into(),
            })
            .is_err());
        assert!(storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: None,
                match_type: "result_id".into(),
                pattern: "".into(),
            })
            .is_err());

        storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: None,
                match_type: "result_id".into(),
                pattern: "app:notepad".into(),
            })
            .expect("create exclusion rule");

        assert!(storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: None,
                match_type: "result_id".into(),
                pattern: "APP:NOTEPAD".into(),
            })
            .is_err());
    }
}
