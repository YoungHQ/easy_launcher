use crate::apps::scan_apps;
use crate::everything::{
    detect_everything_status, search_everything_http, try_search_everything_ipc, EverythingStatus,
};
use crate::file_metadata::{read_file_metadata, FileMetadata};
use crate::pinyin_search::{pinyin_match_score, pinyin_matches};
use crate::storage::{
    CustomCommand, ExclusionRule, Phrase, RecentItem, ResultAlias, WebSearchTemplate,
};
use crate::tools::{tool_results, PasswordOptions, ToolAction};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MIN_FILE_SEARCH_QUERY_CHARS: usize = 2;
const MAX_DIRECT_PATH_RESULTS: usize = 30;
const MAX_DIRECTORY_SEARCH_RESULTS: usize = 30;
const MAX_DIRECTORY_SEARCH_VISITED: usize = 5000;
const EVERYTHING_GENERAL_SEARCH_LIMIT: usize = 30;
const EVERYTHING_DIRECTORY_SEARCH_LIMIT: usize = 60;
const EVERYTHING_PATH_ONLY_FILE_LIMIT: usize = 5;
const EVERYTHING_PATH_SUPPLEMENT_LIMIT: usize = 30;
const MAX_QUICK_ENTRY_RECENT_RESULTS: usize = 8;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: ResultKind,
    pub action: ActionKind,
    pub source: String,
    pub score: f32,
    pub shortcut: Option<String>,
    pub file_metadata: Option<FileMetadata>,
    pub icon_path: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultKind {
    App,
    File,
    Command,
    Calculator,
    AiAction,
    WebSearch,
    Tool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind {
    LaunchApp,
    OpenFile,
    RunCommand,
    CopyText,
    AiTranslate,
    AiSummarize,
    OpenUrl,
}

pub trait SearchProvider {
    fn search(&self, query: &str, context: &SearchContext<'_>) -> Vec<SearchResult>;
    fn source(&self) -> Option<SearchSource>;
    fn provider_id(&self) -> &'static str {
        "unknown"
    }
    fn provider_tier(&self) -> ProviderTier {
        ProviderTier::Fast
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SearchSource {
    Apps,
    Files,
    Calculator,
    System,
    Ai,
    Phrase,
    WebSearch,
    Tools,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTier {
    Fast,
    Slow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionKeywordRoute {
    pub query: String,
    pub sources: Option<HashSet<SearchSource>>,
    pub keyword: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuickEntryCategory {
    Cmd,
    Phrase,
    Web,
    Tools,
    RecentApps,
    RecentFolders,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuickEntryRoute {
    Categories {
        filter: String,
    },
    Category {
        category: QuickEntryCategory,
        query: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverythingSearchOptions {
    pub full_path: bool,
    pub search_content: bool,
}

impl Default for EverythingSearchOptions {
    fn default() -> Self {
        Self {
            full_path: false,
            search_content: false,
        }
    }
}

pub struct SearchContext<'a> {
    pub recent_scores: Option<&'a HashMap<String, f32>>,
    pub query_selection_scores: Option<&'a HashMap<String, f32>>,
    pub pinned_scores: Option<&'a HashMap<String, f32>>,
    pub result_aliases: Option<&'a [ResultAlias]>,
    pub custom_commands: Option<&'a [CustomCommand]>,
    pub phrases: Option<&'a [Phrase]>,
    pub web_search_templates: Option<&'a [WebSearchTemplate]>,
    pub password_options: Option<&'a PasswordOptions>,
    pub exclusion_rules: Option<&'a [ExclusionRule]>,
    pub source_weights: Option<&'a HashMap<SearchSource, f32>>,
    pub enabled_sources: &'a HashSet<SearchSource>,
    pub everything_options: Option<&'a EverythingSearchOptions>,
}

pub struct SearchCore {
    providers: Vec<Box<dyn SearchProvider + Send + Sync>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectorySearchQuery {
    directory: PathBuf,
    search_term: String,
}

#[derive(Clone)]
pub struct CachedSearchResults {
    pub results: Vec<SearchResult>,
    pub complete: bool,
}

pub struct SearchResultCache {
    max_entries: usize,
    ttl: Duration,
    entries: VecDeque<SearchResultCacheEntry>,
}

struct SearchResultCacheEntry {
    key: String,
    cached_at: Instant,
    value: CachedSearchResults,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchExecution {
    pub results: Vec<SearchResult>,
    pub diagnostics: SearchDiagnostics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchDiagnostics {
    pub total_duration_ms: u128,
    pub provider_timings: Vec<ProviderTiming>,
    pub stage_timings: Vec<StageTiming>,
    pub result_count: usize,
    pub query_length: usize,
    pub cache_hit: bool,
    pub cancelled: bool,
    pub tier: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTiming {
    pub provider: String,
    pub source: Option<String>,
    pub tier: String,
    pub duration_ms: u128,
    pub result_count: usize,
    pub skipped: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageTiming {
    pub stage: String,
    pub duration_ms: u128,
    pub item_count: usize,
}

impl SearchCore {
    pub fn new(providers: Vec<Box<dyn SearchProvider + Send + Sync>>) -> Self {
        Self { providers }
    }

    #[allow(dead_code)]
    pub fn search(
        &self,
        query: &str,
        context: &SearchContext<'_>,
        limit: usize,
    ) -> Vec<SearchResult> {
        self.search_with_diagnostics(query, context, limit, false)
            .results
    }

    pub fn search_with_diagnostics(
        &self,
        query: &str,
        context: &SearchContext<'_>,
        limit: usize,
        cache_hit: bool,
    ) -> SearchExecution {
        self.search_with_cancellation(query, context, limit, cache_hit, || false)
    }

    pub fn search_with_cancellation(
        &self,
        query: &str,
        context: &SearchContext<'_>,
        limit: usize,
        cache_hit: bool,
        should_cancel: impl Fn() -> bool,
    ) -> SearchExecution {
        self.search_with_filter(
            query,
            context,
            limit,
            cache_hit,
            "all".into(),
            |_| true,
            should_cancel,
        )
    }

    pub fn search_tier_with_cancellation(
        &self,
        tier: ProviderTier,
        query: &str,
        context: &SearchContext<'_>,
        limit: usize,
        cache_hit: bool,
        should_cancel: impl Fn() -> bool,
    ) -> SearchExecution {
        self.search_with_filter(
            query,
            context,
            limit,
            cache_hit,
            provider_tier_label(tier),
            |provider| provider.provider_tier() == tier,
            should_cancel,
        )
    }

    fn search_with_filter(
        &self,
        query: &str,
        context: &SearchContext<'_>,
        limit: usize,
        cache_hit: bool,
        tier_label: String,
        provider_filter: impl Fn(&(dyn SearchProvider + Send + Sync)) -> bool,
        should_cancel: impl Fn() -> bool,
    ) -> SearchExecution {
        let started_at = Instant::now();
        let normalized_query = normalize_query(query);
        let mut results = Vec::new();
        let mut provider_timings = Vec::new();
        let mut cancelled = false;

        for provider in &self.providers {
            if should_cancel() {
                cancelled = true;
                break;
            }

            let provider_source = provider.source();
            let provider_name = provider.provider_id().to_string();
            let provider_tier = provider.provider_tier();
            if !provider_filter(provider.as_ref()) {
                continue;
            }
            let provider_source_label = provider_source.map(search_source_label);

            if let Some(source) = provider_source {
                if !context.enabled_sources.contains(&source) {
                    provider_timings.push(ProviderTiming {
                        provider: provider_name,
                        source: provider_source_label,
                        tier: provider_tier_label(provider_tier),
                        duration_ms: 0,
                        result_count: 0,
                        skipped: true,
                    });
                    continue;
                }
            } else if !provider.should_search(context) {
                provider_timings.push(ProviderTiming {
                    provider: provider_name,
                    source: provider_source_label,
                    tier: provider_tier_label(provider_tier),
                    duration_ms: 0,
                    result_count: 0,
                    skipped: true,
                });
                continue;
            }

            let provider_started_at = Instant::now();
            let provider_results = provider.search(&normalized_query, context);
            let provider_duration_ms = provider_started_at.elapsed().as_millis();
            let provider_result_count = provider_results.len();
            results.extend(provider_results);
            provider_timings.push(ProviderTiming {
                provider: provider_name,
                source: provider_source_label,
                tier: provider_tier_label(provider_tier),
                duration_ms: provider_duration_ms,
                result_count: provider_result_count,
                skipped: false,
            });

            if should_cancel() {
                cancelled = true;
                break;
            }
        }

        results.extend(alias_results(&normalized_query, context));

        let dedupe_started_at = Instant::now();
        let mut results = if cancelled {
            Vec::new()
        } else {
            dedupe_results(results)
        };
        let mut stage_timings = vec![StageTiming {
            stage: "dedupe".into(),
            duration_ms: dedupe_started_at.elapsed().as_millis(),
            item_count: results.len(),
        }];

        let exclude_started_at = Instant::now();
        filter_excluded_results(&mut results, context.exclusion_rules);
        stage_timings.push(StageTiming {
            stage: "exclude".into(),
            duration_ms: exclude_started_at.elapsed().as_millis(),
            item_count: results.len(),
        });

        let score_started_at = Instant::now();
        for result in &mut results {
            let match_score = match_score(&normalized_query, &result.title, &result.subtitle);
            result.score += match_score;
            result.score += alias_boost(&normalized_query, context, result);
            result.score += pinned_score(context, result);
            result.score += query_selection_score(context, result);
            result.score *= source_weight(context, result);
        }
        stage_timings.push(StageTiming {
            stage: "score".into(),
            duration_ms: score_started_at.elapsed().as_millis(),
            item_count: results.len(),
        });

        let sort_started_at = Instant::now();
        results.sort_by(|left, right| compare_results(left, right));
        let results = results.into_iter().take(limit).collect::<Vec<_>>();
        stage_timings.push(StageTiming {
            stage: "sort-limit".into(),
            duration_ms: sort_started_at.elapsed().as_millis(),
            item_count: results.len(),
        });
        let diagnostics = SearchDiagnostics {
            total_duration_ms: started_at.elapsed().as_millis(),
            provider_timings,
            stage_timings,
            result_count: results.len(),
            query_length: query.chars().count(),
            cache_hit,
            cancelled,
            tier: tier_label,
        };

        SearchExecution {
            results,
            diagnostics,
        }
    }
}

pub fn merge_ranked_results(results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    let mut results = dedupe_results(results);
    results.sort_by(|left, right| compare_results(left, right));
    results.into_iter().take(limit).collect()
}

impl SearchResultCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            max_entries,
            ttl,
            entries: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<CachedSearchResults> {
        self.remove_expired();
        let index = self.entries.iter().position(|entry| entry.key == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.value.clone();
        self.entries.push_front(entry);
        Some(value)
    }

    pub fn insert(&mut self, key: String, value: CachedSearchResults) {
        self.remove_expired();
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            self.entries.remove(index);
        }

        self.entries.push_front(SearchResultCacheEntry {
            key,
            cached_at: Instant::now(),
            value,
        });

        while self.entries.len() > self.max_entries {
            self.entries.pop_back();
        }
    }

    pub fn clear_icon_paths(&mut self) {
        for entry in &mut self.entries {
            for result in &mut entry.value.results {
                result.icon_path = None;
            }
        }
    }

    fn remove_expired(&mut self) {
        let ttl = self.ttl;
        self.entries
            .retain(|entry| entry.cached_at.elapsed() <= ttl);
    }
}

pub fn cached_search_diagnostics(
    query: &str,
    result_count: usize,
    tier: impl Into<String>,
) -> SearchDiagnostics {
    SearchDiagnostics {
        total_duration_ms: 0,
        provider_timings: Vec::new(),
        stage_timings: Vec::new(),
        result_count,
        query_length: query.chars().count(),
        cache_hit: true,
        cancelled: false,
        tier: tier.into(),
    }
}

pub fn parse_action_keyword_query(query: &str) -> ActionKeywordRoute {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return ActionKeywordRoute {
            query: String::new(),
            sources: None,
            keyword: None,
        };
    }

    if let Some(rest) = trimmed.strip_prefix('=') {
        return ActionKeywordRoute {
            query: rest.trim().to_string(),
            sources: Some(HashSet::from([SearchSource::Calculator])),
            keyword: Some("=".into()),
        };
    }

    let Some((keyword, rest)) = split_action_keyword(trimmed) else {
        return ActionKeywordRoute {
            query: trimmed.to_string(),
            sources: None,
            keyword: None,
        };
    };

    let normalized_keyword = keyword.to_lowercase();
    let Some(sources) = action_keyword_sources(&normalized_keyword) else {
        return ActionKeywordRoute {
            query: trimmed.to_string(),
            sources: None,
            keyword: None,
        };
    };

    ActionKeywordRoute {
        query: rest.trim().to_string(),
        sources: Some(sources),
        keyword: Some(normalized_keyword),
    }
}

pub fn parse_web_search_template_query(
    query: &str,
    templates: &[WebSearchTemplate],
) -> Option<(WebSearchTemplate, String)> {
    let web_query = web_search_input_query(query);
    if web_query.is_empty() {
        return None;
    }

    let (keyword, rest) = split_action_keyword(web_query)?;
    let template = templates
        .iter()
        .find(|template| template.keyword.eq_ignore_ascii_case(keyword))?
        .clone();
    let search_query = rest.trim();
    if search_query.is_empty() {
        return None;
    }

    Some((template, search_query.to_string()))
}

fn parse_web_search_template_direct_query(
    query: &str,
    templates: &[WebSearchTemplate],
) -> Option<(WebSearchTemplate, String)> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }

    let (keyword, rest) = split_action_keyword(query)?;
    let template = templates
        .iter()
        .find(|template| template.keyword.eq_ignore_ascii_case(keyword))?
        .clone();
    let search_query = rest.trim();
    if search_query.is_empty() {
        return None;
    }

    Some((template, search_query.to_string()))
}

fn web_search_input_query(query: &str) -> &str {
    web_search_input_query_with_route(query).0
}

fn web_search_input_query_with_route(query: &str) -> (&str, bool) {
    let trimmed = query.trim();
    let Some((keyword, rest)) = split_action_keyword(trimmed) else {
        return (trimmed, false);
    };

    if matches!(keyword.to_lowercase().as_str(), "web" | "www") {
        (rest.trim(), true)
    } else {
        (trimmed, false)
    }
}

pub fn apply_action_keyword_route(
    enabled_sources: &HashSet<SearchSource>,
    route: &ActionKeywordRoute,
) -> HashSet<SearchSource> {
    let Some(route_sources) = route.sources.as_ref() else {
        return enabled_sources.clone();
    };

    enabled_sources
        .intersection(route_sources)
        .copied()
        .collect::<HashSet<_>>()
}

pub fn parse_quick_entry_query(query: &str, alias: &str) -> Option<QuickEntryRoute> {
    let alias = alias.trim();
    if alias.is_empty() {
        return None;
    }

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }

    let rest = trimmed.strip_prefix(alias)?.trim();
    if rest.is_empty() {
        return Some(QuickEntryRoute::Categories {
            filter: String::new(),
        });
    }

    let mut parts = rest.splitn(2, char::is_whitespace);
    let key = parts.next().unwrap_or_default().to_ascii_lowercase();
    let query = parts.next().unwrap_or_default().trim().to_string();

    if let Some(category) = QuickEntryCategory::from_key(&key) {
        return Some(QuickEntryRoute::Category { category, query });
    }

    Some(QuickEntryRoute::Categories {
        filter: rest.to_string(),
    })
}

pub fn quick_entry_results(
    route: &QuickEntryRoute,
    context: &SearchContext<'_>,
) -> Vec<SearchResult> {
    quick_entry_results_with_recents(route, context, &[])
}

pub fn quick_entry_results_with_recents(
    route: &QuickEntryRoute,
    context: &SearchContext<'_>,
    recent_items: &[RecentItem],
) -> Vec<SearchResult> {
    let mut results = match route {
        QuickEntryRoute::Categories { filter } => quick_entry_category_results(filter),
        QuickEntryRoute::Category { category, query } => match category {
            QuickEntryCategory::Cmd => quick_entry_custom_command_results(query, context),
            QuickEntryCategory::Phrase => quick_entry_phrase_results(query, context),
            QuickEntryCategory::Web => quick_entry_web_results(query, context),
            QuickEntryCategory::Tools => quick_entry_tool_results(query, context),
            QuickEntryCategory::RecentApps => {
                quick_entry_recent_app_results(query, context, recent_items)
            }
            QuickEntryCategory::RecentFolders => {
                quick_entry_recent_folder_results(query, context, recent_items)
            }
        },
    };

    filter_excluded_results(&mut results, context.exclusion_rules);
    results.sort_by(|left, right| compare_results(left, right));
    results
}

impl QuickEntryCategory {
    fn all() -> [Self; 6] {
        [
            Self::Cmd,
            Self::Phrase,
            Self::Web,
            Self::Tools,
            Self::RecentApps,
            Self::RecentFolders,
        ]
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "cmd" => Some(Self::Cmd),
            "phrase" => Some(Self::Phrase),
            "web" => Some(Self::Web),
            "tools" => Some(Self::Tools),
            "recent-apps" => Some(Self::RecentApps),
            "recent-folders" => Some(Self::RecentFolders),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Cmd => "cmd",
            Self::Phrase => "phrase",
            Self::Web => "web",
            Self::Tools => "tools",
            Self::RecentApps => "recent-apps",
            Self::RecentFolders => "recent-folders",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Cmd => "Commands · 自定义命令",
            Self::Phrase => "Phrases · 短语",
            Self::Web => "Web Search · 网页搜索",
            Self::Tools => "Tools · 工具",
            Self::RecentApps => "Recent Apps · 最近应用",
            Self::RecentFolders => "Recent Folders · 最近文件夹",
        }
    }

    fn score(self) -> f32 {
        match self {
            Self::Cmd => 1.00,
            Self::Phrase => 0.99,
            Self::Web => 0.98,
            Self::Tools => 0.97,
            Self::RecentApps => 0.96,
            Self::RecentFolders => 0.95,
        }
    }

    fn matches_filter(self, filter: &str) -> bool {
        let filter = normalize_query(filter);
        filter.is_empty()
            || self.key().contains(&filter)
            || self.subtitle().to_lowercase().contains(&filter)
    }
}

fn quick_entry_category_results(filter: &str) -> Vec<SearchResult> {
    QuickEntryCategory::all()
        .into_iter()
        .filter(|category| category.matches_filter(filter))
        .map(|category| SearchResult {
            id: format!("quick-entry-category:{}", category.key()),
            title: category.key().into(),
            subtitle: category.subtitle().into(),
            kind: ResultKind::Command,
            action: ActionKind::RunCommand,
            source: "快捷入口".into(),
            score: category.score(),
            shortcut: Some("Enter".into()),
            file_metadata: None,
            icon_path: None,
        })
        .collect()
}

fn quick_entry_custom_command_results(
    query: &str,
    context: &SearchContext<'_>,
) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::System) {
        return Vec::new();
    }

    let Some(commands) = context.custom_commands else {
        return Vec::new();
    };

    let query = normalize_query(query);
    commands
        .iter()
        .filter(|command| {
            query.is_empty()
                || matches_search_text(&query, &command.name, &command.target)
                || command.command_type.to_lowercase().contains(&query)
        })
        .map(|command| custom_command_result(command, context))
        .collect()
}

fn quick_entry_phrase_results(query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::Phrase) {
        return Vec::new();
    }

    let Some(phrases) = context.phrases else {
        return Vec::new();
    };

    let query = normalize_query(query);
    phrases
        .iter()
        .filter(|phrase| matches_search_text(&query, &phrase.title, &phrase.text))
        .map(phrase_result)
        .collect()
}

fn quick_entry_web_results(query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::WebSearch) {
        return Vec::new();
    }

    let Some(templates) = context.web_search_templates else {
        return Vec::new();
    };

    let query = query.trim();
    if let Some((template, search_query)) = parse_web_search_template_direct_query(query, templates)
    {
        return vec![web_search_result(&template, &search_query, 0.9)];
    }

    let normalized_query = normalize_query(query);
    templates
        .iter()
        .filter(|template| {
            normalized_query.is_empty()
                || matches_search_text(&normalized_query, &template.keyword, &template.name)
                || template
                    .url_template
                    .to_lowercase()
                    .contains(&normalized_query)
        })
        .map(web_search_template_entry_result)
        .collect()
}

fn quick_entry_tool_results(query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::Tools) {
        return Vec::new();
    }

    let password_options = context.password_options.cloned().unwrap_or_default();
    let tool_query = if query.trim().is_empty() {
        "/"
    } else {
        query.trim()
    };

    tool_results(tool_query, &password_options, "/")
        .into_iter()
        .map(tool_result_to_search_result)
        .collect()
}

fn quick_entry_recent_app_results(
    query: &str,
    context: &SearchContext<'_>,
    recent_items: &[RecentItem],
) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::Apps) {
        return Vec::new();
    }

    let query = normalize_query(query);
    recent_items
        .iter()
        .filter(|item| is_recent_app_item(item))
        .filter(|item| matches_search_text(&query, &item.title, &item.target))
        .take(MAX_QUICK_ENTRY_RECENT_RESULTS)
        .enumerate()
        .map(|(index, item)| recent_app_result(item, index))
        .collect()
}

fn quick_entry_recent_folder_results(
    query: &str,
    context: &SearchContext<'_>,
    recent_items: &[RecentItem],
) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::Files) {
        return Vec::new();
    }

    let query = normalize_query(query);
    recent_items
        .iter()
        .filter(|item| is_recent_folder_item(item))
        .filter(|item| matches_search_text(&query, &item.title, &item.target))
        .take(MAX_QUICK_ENTRY_RECENT_RESULTS)
        .enumerate()
        .map(|(index, item)| recent_folder_result(item, index))
        .collect()
}

fn is_recent_app_item(item: &RecentItem) -> bool {
    item.kind == "app"
        && Path::new(&item.target)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("lnk")
            })
}

fn is_recent_folder_item(item: &RecentItem) -> bool {
    item.kind == "file" && Path::new(&item.target).is_dir()
}

fn recent_app_result(item: &RecentItem, index: usize) -> SearchResult {
    SearchResult {
        id: item.id.clone(),
        title: item.title.clone(),
        subtitle: item.target.clone(),
        kind: ResultKind::App,
        action: ActionKind::LaunchApp,
        source: "最近应用".into(),
        score: recent_entry_score(index, item.use_count),
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

fn recent_folder_result(item: &RecentItem, index: usize) -> SearchResult {
    SearchResult {
        id: item.id.clone(),
        title: item.title.clone(),
        subtitle: item.target.clone(),
        kind: ResultKind::File,
        action: ActionKind::OpenFile,
        source: "最近目录".into(),
        score: recent_entry_score(index, item.use_count),
        shortcut: Some("Enter".into()),
        file_metadata: Some(read_file_metadata(&item.target)),
        icon_path: None,
    }
}

fn recent_entry_score(index: usize, use_count: i64) -> f32 {
    0.86 - (index as f32 * 0.01) + (use_count.min(5) as f32 * 0.001)
}

fn split_action_keyword(query: &str) -> Option<(&str, &str)> {
    if let Some((keyword, rest)) = query.split_once(':') {
        if !keyword.is_empty() && !keyword.contains(['\\', '/']) {
            return Some((keyword, rest));
        }
    }

    let mut parts = query.splitn(2, char::is_whitespace);
    let keyword = parts.next()?;
    let rest = parts.next()?;
    Some((keyword, rest))
}

fn action_keyword_sources(keyword: &str) -> Option<HashSet<SearchSource>> {
    let sources = match keyword {
        "app" | "apps" | "a" => [SearchSource::Apps].into_iter().collect(),
        "file" | "files" | "f" => [SearchSource::Files].into_iter().collect(),
        "cmd" | "command" | "commands" | "sys" | "system" => {
            [SearchSource::System].into_iter().collect()
        }
        "calc" | "calculator" => [SearchSource::Calculator].into_iter().collect(),
        "phrase" | "phrases" | "snippet" | "snippets" => {
            [SearchSource::Phrase].into_iter().collect()
        }
        "ai" | "gpt" => [SearchSource::Ai].into_iter().collect(),
        "web" | "www" => [SearchSource::WebSearch].into_iter().collect(),
        _ => return None,
    };

    Some(sources)
}

pub fn log_search_diagnostics(diagnostics: &SearchDiagnostics) {
    #[cfg(debug_assertions)]
    {
        eprintln!("{}", format_search_diagnostics(diagnostics));
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = diagnostics;
    }
}

#[cfg(any(debug_assertions, test))]
pub fn format_search_diagnostics(diagnostics: &SearchDiagnostics) -> String {
    let providers = diagnostics
        .provider_timings
        .iter()
        .map(|timing| {
            let source = timing.source.as_deref().unwrap_or("shared");
            let skipped = if timing.skipped { " skipped" } else { "" };
            format!(
                "{}({source}):{}ms/{}{}",
                timing.provider, timing.duration_ms, timing.result_count, skipped
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let stages = diagnostics
        .stage_timings
        .iter()
        .map(|timing| {
            format!(
                "{}:{}ms/{}",
                timing.stage, timing.duration_ms, timing.item_count
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "[search perf] tier={} query_len={} cache_hit={} cancelled={} total={}ms results={} providers=[{}] stages=[{}]",
        diagnostics.tier,
        diagnostics.query_length,
        diagnostics.cache_hit,
        diagnostics.cancelled,
        diagnostics.total_duration_ms,
        diagnostics.result_count,
        providers,
        stages
    )
}

fn provider_tier_label(tier: ProviderTier) -> String {
    match tier {
        ProviderTier::Fast => "fast",
        ProviderTier::Slow => "slow",
    }
    .into()
}

fn search_source_label(source: SearchSource) -> String {
    match source {
        SearchSource::Apps => "apps",
        SearchSource::Files => "files",
        SearchSource::Calculator => "calculator",
        SearchSource::System => "system",
        SearchSource::Ai => "ai",
        SearchSource::Phrase => "phrase",
        SearchSource::WebSearch => "web-search",
        SearchSource::Tools => "tools",
    }
    .into()
}

impl dyn SearchProvider + Send + Sync {
    fn should_search(&self, context: &SearchContext<'_>) -> bool {
        [
            SearchSource::Calculator,
            SearchSource::Ai,
            SearchSource::Tools,
        ]
        .iter()
        .any(|source| context.enabled_sources.contains(source))
    }
}

pub struct AppProvider;

impl SearchProvider for AppProvider {
    fn provider_id(&self) -> &'static str {
        "apps"
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::Apps)
    }

    fn search(&self, query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
        scan_apps()
            .into_iter()
            .filter(|app| {
                matches_search_text(query, &app.name, &app.path)
                    || app.aliases.iter().any(|alias| matches_alias(query, alias))
            })
            .map(|app| {
                let recent_score = context
                    .recent_scores
                    .and_then(|scores| scores.get(&app.id))
                    .copied()
                    .unwrap_or(0.0);
                let alias_score = app
                    .aliases
                    .iter()
                    .map(|alias| alias_match_score(query, alias))
                    .fold(0.0, f32::max);

                SearchResult {
                    score: 0.5 + app_source_boost(&app.source) + alias_score + recent_score,
                    id: app.id,
                    title: app.name,
                    subtitle: app.path,
                    kind: ResultKind::App,
                    action: ActionKind::LaunchApp,
                    source: app.source,
                    shortcut: Some("Enter".into()),
                    file_metadata: None,
                    icon_path: None,
                }
            })
            .collect()
    }
}

pub struct BuiltinProvider;

impl SearchProvider for BuiltinProvider {
    fn provider_id(&self) -> &'static str {
        "builtin"
    }

    fn source(&self) -> Option<SearchSource> {
        None
    }

    fn search(&self, query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
        let mut results = builtin_results(_context);
        if _context.enabled_sources.contains(&SearchSource::Calculator) {
            if let Some(result) = calculator_result(query) {
                results.push(result);
            }
        }

        results
            .into_iter()
            .filter(|result| {
                matches_search_text(query, &result.title, &result.subtitle)
                    || result.source.to_lowercase().contains(query)
                    || pinyin_matches(query, &result.source)
            })
            .collect()
    }
}

pub fn default_search_core() -> SearchCore {
    SearchCore::new(vec![
        Box::new(AppProvider),
        Box::new(DirectPathProvider),
        Box::new(EverythingIpcProvider),
        Box::new(SystemCommandProvider),
        Box::new(PhraseProvider),
        Box::new(WebSearchProvider),
        Box::new(ToolProvider),
        Box::new(BuiltinProvider),
    ])
}

pub struct ToolProvider;

impl SearchProvider for ToolProvider {
    fn provider_id(&self) -> &'static str {
        "tools"
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::Tools)
    }

    fn search(&self, query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
        let password_options = context.password_options.cloned().unwrap_or_default();
        tool_results(query, &password_options, "")
            .into_iter()
            .map(tool_result_to_search_result)
            .collect()
    }
}

pub struct PhraseProvider;

impl SearchProvider for PhraseProvider {
    fn provider_id(&self) -> &'static str {
        "phrases"
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::Phrase)
    }

    fn search(&self, query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
        let Some(phrases) = context.phrases else {
            return Vec::new();
        };

        phrases
            .iter()
            .filter(|phrase| matches_search_text(query, &phrase.title, &phrase.text))
            .map(phrase_result)
            .collect()
    }
}

pub struct WebSearchProvider;

impl SearchProvider for WebSearchProvider {
    fn provider_id(&self) -> &'static str {
        "web-search"
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::WebSearch)
    }

    fn search(&self, query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
        let Some(templates) = context.web_search_templates else {
            return Vec::new();
        };
        let (web_query, explicit_web_route) = web_search_input_query_with_route(query);
        if web_query.is_empty() {
            return Vec::new();
        }

        if let Some((template, search_query)) =
            parse_web_search_template_query(web_query, templates)
        {
            return vec![web_search_result(&template, &search_query, 0.9)];
        }

        if explicit_web_route || is_web_search_only(context.enabled_sources) {
            if let Some(template) = default_web_search_template(templates) {
                return vec![web_search_result(template, web_query, 0.86)];
            }
        }

        templates
            .iter()
            .filter(|template| {
                matches_search_text(web_query, &template.keyword, &template.name)
                    || matches_search_text(web_query, &template.name, &template.keyword)
                    || fuzzy_match(web_query, &template.keyword)
                    || pinyin_matches(web_query, &template.name)
            })
            .map(|template| web_search_result(template, web_query, 0.58))
            .collect()
    }
}

fn is_web_search_only(enabled_sources: &HashSet<SearchSource>) -> bool {
    enabled_sources.len() == 1 && enabled_sources.contains(&SearchSource::WebSearch)
}

fn default_web_search_template(templates: &[WebSearchTemplate]) -> Option<&WebSearchTemplate> {
    templates
        .iter()
        .find(|template| template.keyword.eq_ignore_ascii_case("web"))
        .or_else(|| templates.first())
}

fn web_search_result(template: &WebSearchTemplate, query: &str, score: f32) -> SearchResult {
    let url = expand_web_search_url(&template.url_template, query);

    SearchResult {
        id: format!("{}:{}", template.id, query),
        title: format!("{} 搜索 {}", template.name, query),
        subtitle: url,
        kind: ResultKind::WebSearch,
        action: ActionKind::OpenUrl,
        source: "网页搜索".into(),
        score,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

fn web_search_template_entry_result(template: &WebSearchTemplate) -> SearchResult {
    SearchResult {
        id: format!("quick-entry-web-template:{}", template.id),
        title: template.keyword.clone(),
        subtitle: format!("{} · {}", template.name, template.url_template),
        kind: ResultKind::WebSearch,
        action: ActionKind::RunCommand,
        source: "网页搜索模板".into(),
        score: 0.82,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

pub fn expand_web_search_url(url_template: &str, query: &str) -> String {
    url_template.replace("{query}", &urlencoding::encode(query))
}

pub struct SystemCommandProvider;

impl SearchProvider for SystemCommandProvider {
    fn provider_id(&self) -> &'static str {
        "system"
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::System)
    }

    fn search(&self, query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = system_command_results();
        if let Some(commands) = _context.custom_commands {
            results.extend(custom_command_results(commands, _context));
        }

        results
            .into_iter()
            .filter(|result| {
                matches_search_text(query, &result.title, &result.subtitle)
                    || result.source.to_lowercase().contains(query)
                    || pinyin_matches(query, &result.source)
            })
            .collect()
    }
}

pub struct EverythingIpcProvider;

pub struct DirectPathProvider;

impl SearchProvider for DirectPathProvider {
    fn provider_id(&self) -> &'static str {
        "direct-path"
    }

    fn provider_tier(&self) -> ProviderTier {
        ProviderTier::Slow
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::Files)
    }

    fn search(&self, query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
        direct_path_results(query)
    }
}

impl SearchProvider for EverythingIpcProvider {
    fn provider_id(&self) -> &'static str {
        "everything-ipc"
    }

    fn provider_tier(&self) -> ProviderTier {
        ProviderTier::Slow
    }

    fn source(&self) -> Option<SearchSource> {
        Some(SearchSource::Files)
    }

    fn search(&self, query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
        if let Some(directory_query) = parse_directory_search_query(query) {
            if is_file_query_too_short(&directory_query.search_term) {
                return Vec::new();
            }

            if let Some(result) = everything_unavailable_result() {
                return vec![result];
            }

            let everything_query =
                everything_directory_search_query(&directory_query, everything_options(context));
            let (files, source_prefix) = search_everything_with_http_fallback(
                &everything_query,
                EVERYTHING_DIRECTORY_SEARCH_LIMIT,
                true,
            );
            return everything_results(
                filter_everything_directory_results(files, &directory_query.directory),
                source_prefix,
                &directory_query.search_term,
            );
        }

        if is_file_query_too_short(query) {
            return Vec::new();
        }

        if let Some(result) = everything_unavailable_result() {
            return vec![result];
        }

        let everything_options = everything_options(context);
        let everything_query = everything_search_query(query, everything_options);
        let match_path = everything_options.full_path;
        let (mut files, source_prefix) = search_everything_with_http_fallback(
            &everything_query,
            EVERYTHING_GENERAL_SEARCH_LIMIT,
            match_path,
        );
        let primary_result_count = files.len();
        let mut combined_source_prefix = source_prefix;
        if should_search_everything_path_supplement(primary_result_count, everything_options) {
            let (path_files, path_source_prefix) = search_everything_with_http_fallback(
                &everything_query,
                EVERYTHING_PATH_SUPPLEMENT_LIMIT,
                true,
            );
            files.extend(path_files);
            combined_source_prefix =
                combined_everything_source_prefix(combined_source_prefix, path_source_prefix);
        }
        if should_search_everything_folder_supplement(primary_result_count, everything_options) {
            let (folder_files, folder_source_prefix) = search_everything_with_http_fallback(
                &everything_folder_search_query(query),
                EVERYTHING_GENERAL_SEARCH_LIMIT,
                false,
            );
            files.extend(folder_files);
            combined_source_prefix =
                combined_everything_source_prefix(combined_source_prefix, folder_source_prefix);
            return everything_results(files, combined_source_prefix, query);
        }
        everything_results(files, combined_source_prefix, query)
    }
}

fn search_everything_with_http_fallback(
    query: &str,
    limit: usize,
    match_path: bool,
) -> (Vec<crate::everything::EverythingFileResult>, &'static str) {
    match try_search_everything_ipc(query, limit, match_path) {
        Ok(results) => (results, "Everything IPC"),
        Err(_) => (
            search_everything_http(&everything_http_search_query(query, match_path), limit),
            "Everything HTTP",
        ),
    }
}

fn should_search_everything_folder_supplement(
    primary_result_count: usize,
    options: EverythingSearchOptions,
) -> bool {
    !options.full_path && primary_result_count < EVERYTHING_GENERAL_SEARCH_LIMIT
}

fn should_search_everything_path_supplement(
    primary_result_count: usize,
    options: EverythingSearchOptions,
) -> bool {
    !options.full_path
        && !options.search_content
        && primary_result_count < EVERYTHING_GENERAL_SEARCH_LIMIT
}

fn combined_everything_source_prefix(
    primary_source_prefix: &'static str,
    supplement_source_prefix: &'static str,
) -> &'static str {
    if primary_source_prefix == supplement_source_prefix {
        primary_source_prefix
    } else {
        "Everything IPC/HTTP"
    }
}

fn is_file_query_too_short(query: &str) -> bool {
    query.chars().count() < MIN_FILE_SEARCH_QUERY_CHARS
}

fn parse_directory_search_query(query: &str) -> Option<DirectorySearchQuery> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut candidate = None;
    for (index, character) in trimmed.char_indices() {
        if !character.is_whitespace() {
            continue;
        }

        let directory_text = trimmed[..index].trim_end();
        let search_term = trimmed[index..].trim();
        if directory_text.is_empty()
            || search_term.is_empty()
            || !is_directory_search_path_candidate(directory_text)
        {
            continue;
        }

        let directory = expand_path_query(directory_text);
        if directory.is_dir() {
            candidate = Some(DirectorySearchQuery {
                directory,
                search_term: normalize_query(search_term),
            });
        }
    }

    candidate
}

fn is_directory_search_path_candidate(path: &str) -> bool {
    is_path_like(path) || path == "." || path == ".." || Path::new(path).is_absolute()
}

fn directory_search_results(query: &DirectorySearchQuery) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut stack = vec![query.directory.clone()];
    let mut visited = 0usize;

    while let Some(directory) = stack.pop() {
        if results.len() >= MAX_DIRECTORY_SEARCH_RESULTS || visited >= MAX_DIRECTORY_SEARCH_VISITED
        {
            break;
        }

        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.file_name().to_string_lossy().to_lowercase())
        });

        for entry in entries {
            if results.len() >= MAX_DIRECTORY_SEARCH_RESULTS
                || visited >= MAX_DIRECTORY_SEARCH_VISITED
            {
                break;
            }
            visited += 1;

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let relative_path_text = path
                .strip_prefix(&query.directory)
                .unwrap_or(&path)
                .display()
                .to_string();
            if matches_search_text(&query.search_term, &name, &relative_path_text) {
                results.push(path_result_with_title(
                    name.clone(),
                    path.clone(),
                    "目录内搜索",
                    0.86,
                ));
            }

            if entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
            {
                stack.push(path);
            }
        }
    }

    results
}

fn everything_options(context: &SearchContext<'_>) -> EverythingSearchOptions {
    context.everything_options.copied().unwrap_or_default()
}

fn everything_search_query(query: &str, options: EverythingSearchOptions) -> String {
    let query = normalize_query(query);
    if query.is_empty() {
        return String::new();
    }

    let base = if options.full_path {
        format!("path:{}", quote_everything_search_value(&query))
    } else {
        quote_everything_search_value(&query)
    };

    if options.search_content {
        format!("{base} | content:{}", quote_everything_search_value(&query))
    } else {
        base
    }
}

fn everything_folder_search_query(query: &str) -> String {
    format!(
        "folder:{}",
        quote_everything_search_value(&normalize_query(query))
    )
}

fn everything_directory_search_query(
    query: &DirectorySearchQuery,
    options: EverythingSearchOptions,
) -> String {
    let directory = quote_everything_search_value(&query.directory.display().to_string());
    let search = everything_search_query(&query.search_term, options);
    format!("path:{directory} {search}")
}

fn everything_http_search_query(query: &str, match_path: bool) -> String {
    if match_path && !query.trim_start().starts_with("path:") {
        format!("path:{}", quote_everything_search_value(query))
    } else {
        query.to_string()
    }
}

fn everything_unavailable_result() -> Option<SearchResult> {
    let status = detect_everything_status(None);
    everything_unavailable_result_from_status(status)
}

fn everything_unavailable_result_from_status(status: EverythingStatus) -> Option<SearchResult> {
    if status.running {
        return None;
    }

    let (title, subtitle, action) = if !status.installed {
        (
            "安装 Everything 以启用文件搜索".to_string(),
            "https://www.voidtools.com/downloads/".to_string(),
            ActionKind::OpenUrl,
        )
    } else if let Some(path) = status.install_path {
        (
            "启动 Everything 以恢复文件搜索".to_string(),
            path,
            ActionKind::OpenFile,
        )
    } else {
        (
            "启动 Everything 以恢复文件搜索".to_string(),
            "Everything 已安装但未运行".to_string(),
            ActionKind::RunCommand,
        )
    };

    Some(SearchResult {
        id: "everything:diagnostic:not-ready".into(),
        title,
        subtitle,
        kind: ResultKind::Command,
        action,
        source: "Everything 诊断".into(),
        score: 0.62,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    })
}

fn quote_everything_search_value(value: &str) -> String {
    if value.chars().any(char::is_whitespace) || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn filter_everything_directory_results(
    files: Vec<crate::everything::EverythingFileResult>,
    directory: &Path,
) -> Vec<crate::everything::EverythingFileResult> {
    files
        .into_iter()
        .filter(|file| path_is_inside_directory(&file.path, directory))
        .take(MAX_DIRECTORY_SEARCH_RESULTS)
        .collect()
}

fn path_is_inside_directory(path: &str, directory: &Path) -> bool {
    let mut directory_text = normalize_path_match_text(&directory.display().to_string());
    if directory_text.ends_with('\\') {
        path_is_inside_normalized_directory(path, &directory_text)
    } else {
        directory_text.push('\\');
        path_is_inside_normalized_directory(path, &directory_text)
    }
}

fn path_is_inside_normalized_directory(path: &str, directory_prefix: &str) -> bool {
    let path = normalize_path_match_text(path);
    path.starts_with(directory_prefix) && path.len() > directory_prefix.len()
}

fn direct_path_results(query: &str) -> Vec<SearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    if !query.chars().any(char::is_whitespace) && is_environment_variable_lookup(query) {
        return environment_path_results(query);
    }

    let expanded = expand_path_query(query);
    if expanded.exists() {
        return vec![path_result(expanded, "路径直达", 0.9)];
    }

    if let Some(directory_query) = parse_directory_search_query(query) {
        return directory_search_results(&directory_query);
    }

    if is_environment_variable_lookup(query) {
        return environment_path_results(query);
    }

    if is_path_like(query) {
        return child_path_results(&expanded);
    }

    Vec::new()
}

fn environment_path_results(query: &str) -> Vec<SearchResult> {
    let search = query.trim_matches('%').to_lowercase();
    let exact = query.starts_with('%') && query.ends_with('%') && query.len() > 2;

    env_path_entries()
        .into_iter()
        .filter(|(name, _)| {
            if exact {
                name.eq_ignore_ascii_case(&search)
            } else {
                name.to_lowercase().starts_with(&search)
            }
        })
        .take(MAX_DIRECT_PATH_RESULTS)
        .map(|(name, path)| {
            path_result_with_title(
                format!("%{}%", name.to_uppercase()),
                path,
                "环境变量路径",
                0.86,
            )
        })
        .collect()
}

fn child_path_results(path: &Path) -> Vec<SearchResult> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let Some(search_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    if search_name.is_empty() || !parent.is_dir() {
        return Vec::new();
    }

    let search_name = search_name.to_lowercase();
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let name_lower = name.to_lowercase();
            if name_lower.starts_with(&search_name)
                || ordered_char_score(&search_name, &name_lower).is_some()
            {
                Some(path_result(entry.path(), "路径补全", 0.84))
            } else {
                None
            }
        })
        .take(MAX_DIRECT_PATH_RESULTS)
        .collect()
}

fn path_result(path: PathBuf, source: &str, score: f32) -> SearchResult {
    let title = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_str().unwrap_or("路径"))
        .to_string();

    path_result_with_title(title, path, source, score)
}

fn path_result_with_title(title: String, path: PathBuf, source: &str, score: f32) -> SearchResult {
    let path_text = path.display().to_string();
    let metadata = read_file_metadata(&path_text);

    SearchResult {
        id: format!("file:{}", path_text.to_lowercase()),
        title,
        subtitle: path_text,
        kind: ResultKind::File,
        action: ActionKind::OpenFile,
        source: if metadata.is_dir {
            format!("{source} 目录")
        } else {
            format!("{source} 文件")
        },
        score,
        shortcut: Some("Enter".into()),
        file_metadata: Some(metadata),
        icon_path: None,
    }
}

fn is_environment_variable_lookup(query: &str) -> bool {
    query.starts_with('%') && query != "%%" && !query.contains(['\\', '/'])
}

fn env_path_entries() -> Vec<(String, PathBuf)> {
    let home_drive = env::var("HOMEDRIVE").unwrap_or_else(|_| "C:\\".into());
    let mut entries = env::vars()
        .filter_map(|(name, value)| {
            let path = normalize_env_path(&value, &home_drive)?;
            if path.is_dir() {
                Some((name, path))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| left.0.to_lowercase().cmp(&right.0.to_lowercase()));
    entries
}

fn normalize_env_path(value: &str, home_drive: &str) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() || value.contains(';') {
        return None;
    }

    let path = PathBuf::from(value);
    if path.is_absolute() {
        Some(path)
    } else if value.starts_with(['\\', '/']) {
        Some(PathBuf::from(home_drive).join(value.trim_start_matches(['\\', '/'])))
    } else {
        None
    }
}

fn expand_path_query(query: &str) -> PathBuf {
    let expanded = expand_environment_variables(query);
    if let Some(stripped) = expanded
        .strip_prefix("~/")
        .or_else(|| expanded.strip_prefix("~\\"))
    {
        if let Ok(user_profile) = env::var("USERPROFILE") {
            return PathBuf::from(user_profile).join(stripped);
        }
    }

    PathBuf::from(expanded)
}

fn expand_environment_variables(query: &str) -> String {
    let mut expanded = String::new();
    let mut rest = query;

    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            expanded.push_str(&rest[start..]);
            return expanded;
        };

        let name = &after_start[..end];
        if name.is_empty() {
            expanded.push_str("%%");
        } else if let Ok(value) = env::var(name) {
            expanded.push_str(&value);
        } else {
            expanded.push('%');
            expanded.push_str(name);
            expanded.push('%');
        }
        rest = &after_start[end + 1..];
    }

    expanded.push_str(rest);
    expanded
}

fn is_path_like(query: &str) -> bool {
    query.contains(['\\', '/'])
        || query.starts_with("~/")
        || query.starts_with("~\\")
        || query.chars().nth(1) == Some(':')
        || query.contains('%')
}

fn everything_results(
    mut files: Vec<crate::everything::EverythingFileResult>,
    source_prefix: &str,
    query: &str,
) -> Vec<SearchResult> {
    let query = normalize_query(query);
    let mut inferred_folders = infer_exact_parent_folder_results(&files, &query);
    if !inferred_folders.is_empty() {
        inferred_folders.append(&mut files);
        files = inferred_folders;
    }
    files = prune_everything_path_only_files(files, &query);

    files
        .into_iter()
        .map(|file| {
            let is_dir = file.is_folder;
            let score = everything_result_score(&file.name, &file.path, is_dir, &query);
            SearchResult {
                id: format!("file:{}", file.path.to_lowercase()),
                title: file.name,
                subtitle: file.path,
                kind: ResultKind::File,
                action: ActionKind::OpenFile,
                source: if is_dir {
                    format!("{source_prefix} 目录")
                } else {
                    format!("{source_prefix} 文件")
                },
                score,
                shortcut: Some("Enter".into()),
                file_metadata: None,
                icon_path: None,
            }
        })
        .collect()
}

fn prune_everything_path_only_files(
    files: Vec<crate::everything::EverythingFileResult>,
    query: &str,
) -> Vec<crate::everything::EverythingFileResult> {
    if query.is_empty() {
        return files;
    }

    let displayed_folders = files
        .iter()
        .filter(|file| file.is_folder)
        .map(|file| normalize_directory_prefix(&file.path))
        .collect::<HashSet<_>>();
    let mut path_only_files = 0usize;
    let mut pruned = Vec::with_capacity(files.len());

    for file in files {
        if file.is_folder || everything_name_matches_query(&file.name, query) {
            pruned.push(file);
            continue;
        }

        if displayed_folders
            .iter()
            .any(|folder| path_is_inside_normalized_directory(&file.path, folder))
        {
            continue;
        }

        if path_only_files >= EVERYTHING_PATH_ONLY_FILE_LIMIT {
            continue;
        }

        path_only_files += 1;
        pruned.push(file);
    }

    pruned
}

fn everything_name_matches_query(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    name.contains(query)
        || multi_word_match_score(query, &name).is_some()
        || compact_matches(query, &name)
        || fuzzy_match(query, &name)
        || pinyin_matches(query, &name)
}

fn normalize_directory_prefix(path: &str) -> String {
    let mut normalized = normalize_path_match_text(path);
    if !normalized.ends_with('\\') {
        normalized.push('\\');
    }
    normalized
}

fn infer_exact_parent_folder_results(
    files: &[crate::everything::EverythingFileResult],
    query: &str,
) -> Vec<crate::everything::EverythingFileResult> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for file in files {
        for ancestor in Path::new(&file.path).ancestors() {
            let Some(name) = ancestor.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.eq_ignore_ascii_case(query) {
                continue;
            }

            let path = ancestor.display().to_string();
            if seen.insert(normalize_path_match_text(&path)) {
                results.push(crate::everything::EverythingFileResult {
                    name: name.to_string(),
                    path,
                    is_folder: true,
                });
            }
        }
    }

    results
}

fn everything_result_score(name: &str, path: &str, is_dir: bool, query: &str) -> f32 {
    let name = name.to_lowercase();
    let path_text = normalize_path_match_text(path);
    let name_matches = everything_name_matches_query(&name, query);
    let mut score = if is_dir && name == query {
        0.88
    } else if is_dir && name.starts_with(query) {
        0.78
    } else if name == query {
        0.76
    } else if name.starts_with(query) {
        0.68
    } else if is_dir && path_segment_starts_with(&path_text, query) {
        0.62
    } else if path_segment_starts_with(&path_text, query) {
        if name_matches {
            0.56
        } else {
            0.34
        }
    } else {
        0.28
    };

    if path_text.contains("\\node_modules\\") || path_text.ends_with("\\node_modules") {
        score -= 0.2;
    }
    if path_text.contains("\\.git\\") || path_text.contains("\\target\\") {
        score -= 0.12;
    }

    let depth = path_text.matches('\\').count() as f32;
    score -= (depth * 0.006).min(0.12);

    score.max(0.1)
}

pub fn dedupe_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for result in results {
        let key = dedupe_key(&result);
        if seen.insert(key) {
            deduped.push(result);
        }
    }

    deduped
}

pub fn match_score(query: &str, title: &str, subtitle: &str) -> f32 {
    let query = normalize_query(query);
    if query.is_empty() {
        return 0.0;
    }

    let title = title.to_lowercase();
    let subtitle = subtitle.to_lowercase();

    title_match_score(&query, &title).max(subtitle_match_score(&query, &subtitle))
}

fn title_match_score(query: &str, title: &str) -> f32 {
    if title == query {
        0.50
    } else if title.starts_with(query) {
        0.42
    } else if let Some(score) = multi_word_match_score(query, title) {
        score
    } else if word_starts_with(title, query) {
        0.36
    } else if title.contains(query) {
        0.31
    } else if abbreviation_matches(query, title) {
        0.28
    } else if let Some(score) = ordered_char_score(query, title) {
        score
    } else if pinyin_matches(query, title) {
        pinyin_match_score(query, title)
    } else {
        0.0
    }
}

fn subtitle_match_score(query: &str, subtitle: &str) -> f32 {
    let file_name = subtitle.rsplit(['\\', '/']).next().unwrap_or(subtitle);

    if file_name == query {
        0.24
    } else if file_name.starts_with(query) {
        0.20
    } else if let Some(score) = multi_word_match_score(query, file_name) {
        score * 0.55
    } else if word_starts_with(file_name, query) {
        0.16
    } else if file_name.contains(query) {
        0.13
    } else if path_segment_starts_with(subtitle, query) {
        0.11
    } else if subtitle.contains(query) {
        0.08
    } else if pinyin_matches(query, subtitle) {
        pinyin_match_score(query, subtitle) * 0.5
    } else {
        0.0
    }
}

pub fn matches_search_text(query: &str, title: &str, subtitle: &str) -> bool {
    query.is_empty()
        || title.to_lowercase().contains(query)
        || subtitle.to_lowercase().contains(query)
        || multi_word_match_score(query, &title.to_lowercase()).is_some()
        || compact_matches(query, title)
        || fuzzy_match(query, title)
        || pinyin_matches(query, title)
        || pinyin_matches(query, subtitle)
}

fn compare_results(left: &SearchResult, right: &SearchResult) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        .then_with(|| left.id.cmp(&right.id))
}

fn source_weight(context: &SearchContext<'_>, result: &SearchResult) -> f32 {
    context
        .source_weights
        .and_then(|weights| weights.get(&result_source(result)))
        .copied()
        .unwrap_or(1.0)
}

fn query_selection_score(context: &SearchContext<'_>, result: &SearchResult) -> f32 {
    context
        .query_selection_scores
        .and_then(|scores| scores.get(&result.id))
        .copied()
        .unwrap_or(0.0)
}

fn pinned_score(context: &SearchContext<'_>, result: &SearchResult) -> f32 {
    context
        .pinned_scores
        .and_then(|scores| scores.get(&result.id))
        .copied()
        .unwrap_or(0.0)
}

fn alias_boost(query: &str, context: &SearchContext<'_>, result: &SearchResult) -> f32 {
    let Some(aliases) = context.result_aliases else {
        return 0.0;
    };
    let query = normalize_query(query);
    if query.is_empty() {
        return 0.0;
    }

    aliases
        .iter()
        .filter(|alias| alias.result_id == result.id)
        .map(|alias| alias_match_boost(&query, &alias.normalized_alias))
        .fold(0.0, f32::max)
}

fn alias_match_boost(query: &str, alias: &str) -> f32 {
    if alias == query {
        0.95
    } else if alias.starts_with(query) {
        0.45
    } else {
        0.0
    }
}

fn result_source(result: &SearchResult) -> SearchSource {
    if result.id.starts_with("phrase:") {
        return SearchSource::Phrase;
    }

    match result.kind {
        ResultKind::App => SearchSource::Apps,
        ResultKind::File => SearchSource::Files,
        ResultKind::Command => SearchSource::System,
        ResultKind::Calculator => SearchSource::Calculator,
        ResultKind::AiAction => SearchSource::Ai,
        ResultKind::WebSearch => SearchSource::WebSearch,
        ResultKind::Tool => SearchSource::Tools,
    }
}

fn alias_results(query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
    let Some(aliases) = context.result_aliases else {
        return Vec::new();
    };
    let query = normalize_query(query);
    if query.is_empty() {
        return Vec::new();
    }

    aliases
        .iter()
        .filter(|alias| alias_match_boost(&query, &alias.normalized_alias) > 0.0)
        .filter_map(alias_result)
        .filter(|result| context.enabled_sources.contains(&result_source(result)))
        .collect()
}

fn alias_result(alias: &ResultAlias) -> Option<SearchResult> {
    let kind = result_kind_from_storage_kind(&alias.kind)?;
    let action = default_action_for_alias(alias, &kind);
    Some(SearchResult {
        id: alias.result_id.clone(),
        title: alias.title.clone(),
        subtitle: alias.target.clone(),
        kind,
        action,
        source: format!("Alias · {}", alias.alias),
        score: 0.62,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    })
}

fn result_kind_from_storage_kind(kind: &str) -> Option<ResultKind> {
    match kind {
        "app" => Some(ResultKind::App),
        "file" => Some(ResultKind::File),
        "command" => Some(ResultKind::Command),
        "calculator" => Some(ResultKind::Calculator),
        "aiAction" | "ai-action" => Some(ResultKind::AiAction),
        "webSearch" | "web-search" => Some(ResultKind::WebSearch),
        "tool" => Some(ResultKind::Tool),
        _ => None,
    }
}

fn default_action_for_kind(kind: ResultKind) -> ActionKind {
    match kind {
        ResultKind::App => ActionKind::LaunchApp,
        ResultKind::File => ActionKind::OpenFile,
        ResultKind::Command => ActionKind::RunCommand,
        ResultKind::Calculator | ResultKind::Tool => ActionKind::CopyText,
        ResultKind::AiAction => ActionKind::AiTranslate,
        ResultKind::WebSearch => ActionKind::OpenUrl,
    }
}

fn default_action_for_alias(alias: &ResultAlias, kind: &ResultKind) -> ActionKind {
    if alias.result_id.starts_with("phrase:") {
        return ActionKind::CopyText;
    }

    default_action_for_kind(kind.clone())
}

fn filter_excluded_results(results: &mut Vec<SearchResult>, rules: Option<&[ExclusionRule]>) {
    let Some(rules) = rules else {
        return;
    };

    results.retain(|result| !is_result_excluded(result, rules));
}

fn is_result_excluded(result: &SearchResult, rules: &[ExclusionRule]) -> bool {
    if !matches!(result.kind, ResultKind::App | ResultKind::File) {
        return false;
    }

    rules.iter().any(|rule| match rule.match_type.as_str() {
        "result_id" => rule.pattern.eq_ignore_ascii_case(&result.id),
        "path_pattern" => {
            let pattern = normalize_path_match_text(&rule.pattern);
            let subtitle = normalize_path_match_text(&result.subtitle);
            let metadata_path = result
                .file_metadata
                .as_ref()
                .map(|metadata| normalize_path_match_text(&metadata.full_path));

            path_pattern_matches(&pattern, &subtitle)
                || metadata_path
                    .as_deref()
                    .is_some_and(|path| path_pattern_matches(&pattern, path))
        }
        _ => false,
    })
}

fn normalize_path_match_text(value: &str) -> String {
    value.trim().replace('/', "\\").to_lowercase()
}

fn path_pattern_matches(pattern: &str, path: &str) -> bool {
    if pattern.contains(['*', '?']) {
        wildcard_match(pattern, path)
    } else {
        path.contains(pattern)
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let (mut pattern_index, mut text_index) = (0, 0);
    let mut star_index = None;
    let mut star_text_index = 0;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_text_index = text_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

pub fn fuzzy_match(query: &str, text: &str) -> bool {
    ordered_char_score(query, &text.to_lowercase()).is_some()
}

fn ordered_char_score(query: &str, text: &str) -> Option<f32> {
    if query.is_empty() {
        return Some(0.0);
    }

    let query_chars = query.chars().collect::<Vec<_>>();
    let mut query_index = 0;
    let mut last_match_index: Option<usize> = None;
    let mut contiguous_matches = 0;
    let mut best_contiguous_run = 0;

    for (text_index, character) in text.chars().enumerate() {
        if query_chars.get(query_index).copied() == Some(character) {
            if last_match_index.is_some_and(|last| last + 1 == text_index) {
                contiguous_matches += 1;
            } else {
                contiguous_matches = 1;
            }
            best_contiguous_run = best_contiguous_run.max(contiguous_matches);
            last_match_index = Some(text_index);
            query_index += 1;

            if query_index == query_chars.len() {
                return Some(if best_contiguous_run == query_chars.len() {
                    0.22
                } else {
                    0.14
                });
            }
        }
    }

    None
}

fn abbreviation_matches(query: &str, text: &str) -> bool {
    let abbreviation = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.chars().next())
        .collect::<String>();

    !abbreviation.is_empty() && abbreviation.starts_with(query)
}

fn matches_alias(query: &str, alias: &str) -> bool {
    query.is_empty()
        || alias.contains(query)
        || compact_query(query).is_some_and(|compact| alias.contains(&compact))
        || ordered_char_score(query, alias).is_some()
}

fn alias_match_score(query: &str, alias: &str) -> f32 {
    if query.is_empty() || alias.is_empty() {
        return 0.0;
    }

    if alias == query {
        0.18
    } else if alias.starts_with(query) {
        0.14
    } else if compact_query(query).is_some_and(|compact| alias == compact) {
        0.18
    } else if compact_query(query).is_some_and(|compact| alias.starts_with(&compact)) {
        0.14
    } else if alias.contains(query) {
        0.10
    } else {
        ordered_char_score(query, alias)
            .map(|score| score * 0.5)
            .unwrap_or(0.0)
    }
}

fn app_source_boost(source: &str) -> f32 {
    match source {
        "开始菜单" | "公共开始菜单" => 0.04,
        "桌面" | "公共桌面" => 0.03,
        "WindowsApps" => 0.02,
        _ => 0.0,
    }
}

fn multi_word_match_score(query: &str, text: &str) -> Option<f32> {
    let tokens = query_tokens(query);
    if tokens.len() < 2 {
        return None;
    }

    let mut score = 0.0_f32;
    for token in tokens {
        if text.split_whitespace().any(|part| part.starts_with(token)) {
            score += 0.08;
        } else if text.contains(token) {
            score += 0.05;
        } else if ordered_char_score(token, text).is_some() {
            score += 0.03;
        } else {
            return None;
        }
    }

    Some((0.24 + score).min(0.40))
}

fn compact_matches(query: &str, text: &str) -> bool {
    let Some(query) = compact_query(query) else {
        return false;
    };
    let Some(text) = compact_query(text) else {
        return false;
    };

    text.contains(&query)
}

fn compact_query(text: &str) -> Option<String> {
    let compact = text
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();

    if compact.is_empty() {
        None
    } else {
        Some(compact)
    }
}

fn query_tokens(query: &str) -> Vec<&str> {
    query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect()
}

fn word_starts_with(text: &str, query: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|part| part.starts_with(query))
}

fn path_segment_starts_with(path: &str, query: &str) -> bool {
    path.split(['\\', '/'])
        .any(|segment| segment.starts_with(query))
}

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

fn dedupe_key(result: &SearchResult) -> String {
    if result.subtitle.is_empty() {
        result.id.to_lowercase()
    } else {
        format!(
            "{:?}:{}",
            result.kind_name(),
            result.subtitle.to_lowercase()
        )
    }
}

impl SearchResult {
    fn kind_name(&self) -> &'static str {
        match self.kind {
            ResultKind::App => "app",
            ResultKind::File => "file",
            ResultKind::Command => "command",
            ResultKind::Calculator => "calculator",
            ResultKind::AiAction => "ai-action",
            ResultKind::WebSearch => "web-search",
            ResultKind::Tool => "tool",
        }
    }
}

fn builtin_results(context: &SearchContext<'_>) -> Vec<SearchResult> {
    if !context.enabled_sources.contains(&SearchSource::Ai) {
        return Vec::new();
    }

    Vec::new()
}

#[allow(dead_code)]
fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(text)
        .trim();

    if line.chars().count() <= 48 {
        line.to_string()
    } else {
        let mut shortened: String = line.chars().take(48).collect();
        shortened.push_str("...");
        shortened
    }
}

fn system_command_results() -> Vec<SearchResult> {
    vec![
        system_command("command-settings", "打开系统设置", "ms-settings:", 0.55),
        system_command("command-terminal", "打开终端", "terminal", 0.54),
        system_command("command-explorer", "打开文件管理器", "explorer", 0.53),
        system_command(
            "command-task-manager",
            "打开任务管理器",
            "task-manager",
            0.52,
        ),
        system_command(
            "command-control-panel",
            "打开控制面板",
            "control-panel",
            0.52,
        ),
        system_command(
            "command-index-options",
            "打开索引选项",
            "index-options",
            0.51,
        ),
        system_command("command-recycle-bin", "打开回收站", "recycle-bin", 0.51),
        system_command("command-lock", "锁屏", "lock", 0.52),
        system_command("command-logoff", "注销", "logoff", 0.46),
        system_command("command-sleep", "睡眠", "sleep", 0.46),
        system_command("command-hibernate", "休眠", "hibernate", 0.46),
        system_command("command-shutdown", "关机", "shutdown", 0.45),
        system_command("command-restart", "重启", "restart", 0.45),
        system_command(
            "command-restart-advanced",
            "重启到高级启动选项",
            "restart-advanced",
            0.44,
        ),
    ]
}

fn system_command(id: &str, title: &str, command: &str, score: f32) -> SearchResult {
    SearchResult {
        id: id.into(),
        title: title.into(),
        subtitle: command.into(),
        kind: ResultKind::Command,
        action: ActionKind::RunCommand,
        source: "系统命令".into(),
        score,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

fn custom_command_results(
    commands: &[CustomCommand],
    context: &SearchContext<'_>,
) -> Vec<SearchResult> {
    commands
        .iter()
        .map(|command| custom_command_result(command, context))
        .collect()
}

fn custom_command_result(command: &CustomCommand, context: &SearchContext<'_>) -> SearchResult {
    let recent_score = context
        .recent_scores
        .and_then(|scores| scores.get(&command.id))
        .copied()
        .unwrap_or(0.0);

    SearchResult {
        id: command.id.clone(),
        title: command.name.clone(),
        subtitle: command.target.clone(),
        kind: ResultKind::Command,
        action: ActionKind::RunCommand,
        source: format!("自定义命令 · {}", command.command_type),
        score: 0.56 + recent_score,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

fn phrase_result(phrase: &Phrase) -> SearchResult {
    SearchResult {
        id: phrase.id.clone(),
        title: phrase.title.clone(),
        subtitle: phrase.text.clone(),
        kind: ResultKind::Command,
        action: ActionKind::CopyText,
        source: "快捷短语".into(),
        score: 0.57 + (phrase.use_count.min(10) as f32 * 0.01),
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

fn tool_result_to_search_result(result: crate::tools::ToolResult) -> SearchResult {
    let action = match result.action {
        ToolAction::Enter { .. } => ActionKind::RunCommand,
        ToolAction::Copy { .. } => ActionKind::CopyText,
    };
    let score = tool_result_score(&result.id);

    SearchResult {
        id: result.id,
        title: result.title,
        subtitle: result.subtitle,
        kind: ResultKind::Tool,
        action,
        source: "工具".into(),
        score,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    }
}

fn tool_result_score(id: &str) -> f32 {
    match id {
        "tool-entry:enc" => 0.96,
        "tool-entry:dec" => 0.95,
        "tool-entry:pwd" => 0.94,
        "tool-entry:time" => 0.93,
        _ => 0.92,
    }
}

fn calculator_result(query: &str) -> Option<SearchResult> {
    let expression = normalize_calculator_query(query)?;
    let value = parse_expression(&expression)?;
    let formatted = format_number(value);

    Some(SearchResult {
        id: format!("calculator:{expression}"),
        title: format!("计算 {expression}"),
        subtitle: formatted,
        kind: ResultKind::Calculator,
        action: ActionKind::CopyText,
        source: "计算器".into(),
        score: 0.92,
        shortcut: Some("Enter".into()),
        file_metadata: None,
        icon_path: None,
    })
}

fn normalize_calculator_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    let expression = trimmed
        .strip_prefix('=')
        .or_else(|| trimmed.strip_prefix("calc "))
        .or_else(|| trimmed.strip_prefix("计算 "))
        .unwrap_or(trimmed)
        .trim();

    if expression.is_empty()
        || !expression
            .chars()
            .any(|character| character.is_ascii_digit())
        || !expression
            .chars()
            .any(|character| matches!(character, '+' | '-' | '*' | '/' | '(' | ')'))
        || !expression.chars().all(|character| {
            character.is_ascii_digit()
                || character.is_ascii_whitespace()
                || matches!(character, '.' | '+' | '-' | '*' | '/' | '(' | ')')
        })
    {
        return None;
    }

    Some(expression.split_whitespace().collect())
}

fn parse_expression(expression: &str) -> Option<f64> {
    let mut parser = CalculatorParser::new(expression);
    let value = parser.parse_sum()?;
    if parser.is_finished() && value.is_finite() {
        Some(value)
    } else {
        None
    }
}

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        return format!("{}", value as i64);
    }

    let formatted = format!("{value:.10}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

struct CalculatorParser<'a> {
    chars: Vec<char>,
    position: usize,
    _source: &'a str,
}

impl<'a> CalculatorParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            position: 0,
            _source: source,
        }
    }

    fn parse_sum(&mut self) -> Option<f64> {
        let mut value = self.parse_product()?;

        loop {
            match self.peek() {
                Some('+') => {
                    self.position += 1;
                    value += self.parse_product()?;
                }
                Some('-') => {
                    self.position += 1;
                    value -= self.parse_product()?;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_product(&mut self) -> Option<f64> {
        let mut value = self.parse_factor()?;

        loop {
            match self.peek() {
                Some('*') => {
                    self.position += 1;
                    value *= self.parse_factor()?;
                }
                Some('/') => {
                    self.position += 1;
                    let divisor = self.parse_factor()?;
                    if divisor.abs() < f64::EPSILON {
                        return None;
                    }
                    value /= divisor;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_factor(&mut self) -> Option<f64> {
        match self.peek()? {
            '+' => {
                self.position += 1;
                self.parse_factor()
            }
            '-' => {
                self.position += 1;
                self.parse_factor().map(|value| -value)
            }
            '(' => {
                self.position += 1;
                let value = self.parse_sum()?;
                if self.peek()? != ')' {
                    return None;
                }
                self.position += 1;
                Some(value)
            }
            _ => self.parse_number(),
        }
    }

    fn parse_number(&mut self) -> Option<f64> {
        let start = self.position;
        let mut dot_count = 0;

        while let Some(character) = self.peek() {
            if character == '.' {
                dot_count += 1;
                if dot_count > 1 {
                    return None;
                }
                self.position += 1;
            } else if character.is_ascii_digit() {
                self.position += 1;
            } else {
                break;
            }
        }

        if start == self.position {
            return None;
        }

        self.chars[start..self.position]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .ok()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn is_finished(&self) -> bool {
        self.position == self.chars.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_results_by_kind_and_subtitle() {
        let result = SearchResult {
            id: "one".into(),
            title: "App One".into(),
            subtitle: "C:\\App\\app.exe".into(),
            kind: ResultKind::App,
            action: ActionKind::LaunchApp,
            source: "test".into(),
            score: 0.1,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };

        let deduped = dedupe_results(vec![
            result.clone(),
            SearchResult {
                id: "two".into(),
                ..result
            },
        ]);

        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn exclusion_rules_hide_app_and_file_results() {
        struct StaticProvider;

        impl SearchProvider for StaticProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![
                    SearchResult {
                        id: "app:notepad".into(),
                        title: "Notepad".into(),
                        subtitle: r"C:\Windows\System32\notepad.exe".into(),
                        kind: ResultKind::App,
                        action: ActionKind::LaunchApp,
                        source: "测试应用".into(),
                        score: 0.5,
                        shortcut: None,
                        file_metadata: None,
                        icon_path: None,
                    },
                    SearchResult {
                        id: "file:notes".into(),
                        title: "notes.txt".into(),
                        subtitle: r"C:\Temp\notes.txt".into(),
                        kind: ResultKind::File,
                        action: ActionKind::OpenFile,
                        source: "测试文件".into(),
                        score: 0.4,
                        shortcut: None,
                        file_metadata: None,
                        icon_path: None,
                    },
                    SearchResult {
                        id: "command-settings".into(),
                        title: "Settings".into(),
                        subtitle: "ms-settings:".into(),
                        kind: ResultKind::Command,
                        action: ActionKind::RunCommand,
                        source: "系统命令".into(),
                        score: 0.3,
                        shortcut: None,
                        file_metadata: None,
                        icon_path: None,
                    },
                ]
            }
        }

        let core = SearchCore::new(vec![Box::new(StaticProvider)]);
        let rules = vec![
            ExclusionRule {
                id: "rule:app".into(),
                match_type: "result_id".into(),
                pattern: "APP:NOTEPAD".into(),
                created_at: "2026-06-01T00:00:00.000Z".into(),
                updated_at: "2026-06-01T00:00:00.000Z".into(),
            },
            ExclusionRule {
                id: "rule:path".into(),
                match_type: "path_pattern".into(),
                pattern: r"C:\Temp\*".into(),
                created_at: "2026-06-01T00:00:00.000Z".into(),
                updated_at: "2026-06-01T00:00:00.000Z".into(),
            },
        ];
        let enabled_sources = HashSet::from([SearchSource::Apps]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: Some(&rules),
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let results = core.search("note", &context, 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "command-settings");
    }

    #[test]
    fn path_pattern_without_wildcards_matches_substring() {
        let result = SearchResult {
            id: "file:notes".into(),
            title: "notes.txt".into(),
            subtitle: r"C:\Users\Alice\Downloads\notes.txt".into(),
            kind: ResultKind::File,
            action: ActionKind::OpenFile,
            source: "测试文件".into(),
            score: 0.4,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };
        let rules = vec![ExclusionRule {
            id: "rule:path".into(),
            match_type: "path_pattern".into(),
            pattern: "/downloads/".into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }];

        assert!(is_result_excluded(&result, &rules));
    }

    #[test]
    fn scores_match_quality() {
        assert!(match_score("code", "code", "") > match_score("code", "my code", ""));
        assert!(match_score("code", "my code", "") > match_score("code", "c o d e", ""));
        assert!(match_score("code", "c o d e", "") > 0.0);
    }

    #[test]
    fn search_scores_exact_match_above_prefix_and_contains() {
        assert!(match_score("code", "code", "") > match_score("code", "code helper", ""));
        assert!(match_score("code", "code helper", "") > match_score("code", "my code helper", ""));
    }

    #[test]
    fn search_scores_prefix_above_regular_contains() {
        assert!(match_score("term", "terminal", "") > match_score("term", "open terminal", ""));
    }

    #[test]
    fn search_scores_contiguous_characters_above_sparse_matches() {
        assert!(match_score("code", "my-code", "") > match_score("code", "c o d e", ""));
    }

    #[test]
    fn search_scores_abbreviation_and_path_segments() {
        assert!(match_score("vsc", "Visual Studio Code", "") > 0.0);
        assert!(match_score("bin", "tool", r"C:\Program Files\App\bin\tool.exe") > 0.0);
        assert!(
            match_score("tool", "tool", r"C:\Program Files\App\bin\tool.exe")
                > match_score("bin", "tool", r"C:\Program Files\App\bin\tool.exe")
        );
    }

    #[test]
    fn search_scores_multi_word_queries() {
        assert!(match_score("visual code", "Visual Studio Code", "") > 0.0);
        assert!(
            match_score("visual code", "Visual Studio Code", "")
                > match_score("code", "c o d e", "")
        );
    }

    #[test]
    fn fuzzy_matches_ordered_characters() {
        assert!(fuzzy_match("vsc", "Visual Studio Code"));
        assert!(!fuzzy_match("vsz", "Visual Studio Code"));
    }

    #[test]
    fn app_alias_matching_supports_compact_program_names() {
        assert!(matches_alias("vscode", "vscode"));
        assert!(matches_alias("vs code", "vscode"));
        assert!(alias_match_score("vscode", "vscode") > alias_match_score("vsc", "vscode"));
    }

    #[test]
    fn app_source_boost_prefers_shortcuts_over_raw_exes() {
        assert!(app_source_boost("开始菜单") > app_source_boost("Program Files"));
    }

    #[test]
    fn direct_path_provider_returns_existing_paths() {
        let temp_root =
            std::env::temp_dir().join(format!("easy-launcher-search-test-{}", std::process::id()));
        let nested = temp_root.join("SampleFolder");
        std::fs::create_dir_all(&nested).expect("create temp folder");

        let results = direct_path_results(nested.to_str().expect("utf-8 temp path"));

        assert_eq!(results[0].subtitle, nested.display().to_string());

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn direct_path_provider_completes_child_paths() {
        let temp_root =
            std::env::temp_dir().join(format!("easy-launcher-child-test-{}", std::process::id()));
        let nested = temp_root.join("NeedleApp.exe");
        std::fs::create_dir_all(&temp_root).expect("create temp folder");
        std::fs::write(&nested, "").expect("create temp file");

        let query = temp_root.join("Needle");
        let results = direct_path_results(query.to_str().expect("utf-8 temp path"));

        assert!(results
            .iter()
            .any(|result| result.subtitle == nested.display().to_string()));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn directory_search_query_parses_existing_folder_and_term() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-launcher-dir-query-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_root).expect("create temp folder");

        let query = format!("{} easy", temp_root.display());
        let parsed = parse_directory_search_query(&query).expect("directory query");

        assert_eq!(parsed.directory, temp_root);
        assert_eq!(parsed.search_term, "easy");

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn directory_search_query_supports_paths_with_spaces() {
        let temp_root = std::env::temp_dir()
            .join(format!(
                "easy-launcher-dir-space-test-{}",
                std::process::id()
            ))
            .join("Folder With Spaces");
        std::fs::create_dir_all(&temp_root).expect("create temp folder");

        let query = format!("{} easy", temp_root.display());
        let parsed = parse_directory_search_query(&query).expect("directory query");

        assert_eq!(parsed.directory, temp_root);
        assert_eq!(parsed.search_term, "easy");

        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!(
            "easy-launcher-dir-space-test-{}",
            std::process::id()
        )));
    }

    #[test]
    fn direct_path_provider_searches_inside_directory() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-launcher-dir-search-test-{}",
            std::process::id()
        ));
        let nested_dir = temp_root.join("nested");
        let matching_file = nested_dir.join("easy-note.txt");
        let non_matching_file = nested_dir.join("plain.txt");
        let outside_file = std::env::temp_dir().join(format!(
            "easy-launcher-outside-easy-{}.txt",
            std::process::id()
        ));
        std::fs::create_dir_all(&nested_dir).expect("create temp folder");
        std::fs::write(&matching_file, "").expect("create matching file");
        std::fs::write(&non_matching_file, "").expect("create non matching file");
        std::fs::write(&outside_file, "").expect("create outside file");

        let query = format!("{} easy", temp_root.display());
        let results = direct_path_results(&query);

        assert!(results
            .iter()
            .any(|result| result.subtitle == matching_file.display().to_string()));
        assert!(!results
            .iter()
            .any(|result| result.subtitle == non_matching_file.display().to_string()));
        assert!(!results
            .iter()
            .any(|result| result.subtitle == outside_file.display().to_string()));

        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(outside_file);
    }

    #[test]
    fn direct_path_provider_prefers_existing_file_paths_with_spaces() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-launcher-exact-space-path-test-{}",
            std::process::id()
        ));
        let nested_dir = temp_root.join("easy");
        let exact_file = temp_root.join("easy note.txt");
        std::fs::create_dir_all(&nested_dir).expect("create temp folder");
        std::fs::write(&exact_file, "").expect("create exact file");

        let results = direct_path_results(exact_file.to_str().expect("utf-8 temp path"));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subtitle, exact_file.display().to_string());
        assert_eq!(results[0].source, "路径直达 文件");

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn directory_search_does_not_match_everything_from_root_path() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-launcher-root-easy-test-{}",
            std::process::id()
        ));
        let non_matching_file = temp_root.join("plain.txt");
        std::fs::create_dir_all(&temp_root).expect("create temp folder");
        std::fs::write(&non_matching_file, "").expect("create non matching file");

        let query = format!("{} easy", temp_root.display());
        let results = direct_path_results(&query);

        assert!(!results
            .iter()
            .any(|result| result.subtitle == non_matching_file.display().to_string()));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn everything_directory_search_filters_results_to_directory() {
        let directory = PathBuf::from(r"C:\work");
        let files = vec![
            crate::everything::EverythingFileResult {
                name: "easy.txt".into(),
                path: r"C:\work\easy.txt".into(),
                is_folder: false,
            },
            crate::everything::EverythingFileResult {
                name: "easy.txt".into(),
                path: r"C:\other\easy.txt".into(),
                is_folder: false,
            },
        ];

        let filtered = filter_everything_directory_results(files, &directory);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, r"C:\work\easy.txt");
    }

    #[test]
    fn everything_search_query_respects_advanced_options() {
        assert_eq!(
            everything_search_query("easy note", EverythingSearchOptions::default()),
            r#""easy note""#
        );
        assert_eq!(
            everything_search_query(
                "easy",
                EverythingSearchOptions {
                    full_path: true,
                    search_content: false,
                },
            ),
            "path:easy"
        );
        assert_eq!(
            everything_search_query(
                "easy note",
                EverythingSearchOptions {
                    full_path: true,
                    search_content: true,
                },
            ),
            r#"path:"easy note" | content:"easy note""#
        );
    }

    #[test]
    fn everything_http_search_query_wraps_match_path_fallback() {
        assert_eq!(everything_http_search_query("easy", true), "path:easy");
        assert_eq!(
            everything_http_search_query("easy note", true),
            r#"path:"easy note""#
        );
        assert_eq!(everything_http_search_query("path:easy", true), "path:easy");
        assert_eq!(everything_http_search_query("easy", false), "easy");
    }

    #[test]
    fn everything_folder_supplement_runs_only_when_primary_results_are_short() {
        assert!(should_search_everything_folder_supplement(
            EVERYTHING_GENERAL_SEARCH_LIMIT - 1,
            EverythingSearchOptions::default(),
        ));
        assert!(!should_search_everything_folder_supplement(
            EVERYTHING_GENERAL_SEARCH_LIMIT,
            EverythingSearchOptions::default(),
        ));
        assert!(!should_search_everything_folder_supplement(
            0,
            EverythingSearchOptions {
                full_path: true,
                search_content: false,
            },
        ));
    }

    #[test]
    fn everything_path_supplement_skips_full_path_and_content_search() {
        assert!(should_search_everything_path_supplement(
            EVERYTHING_GENERAL_SEARCH_LIMIT - 1,
            EverythingSearchOptions::default(),
        ));
        assert!(!should_search_everything_path_supplement(
            EVERYTHING_GENERAL_SEARCH_LIMIT,
            EverythingSearchOptions::default(),
        ));
        assert!(!should_search_everything_path_supplement(
            0,
            EverythingSearchOptions {
                full_path: true,
                search_content: false,
            },
        ));
        assert!(!should_search_everything_path_supplement(
            0,
            EverythingSearchOptions {
                full_path: false,
                search_content: true,
            },
        ));
    }

    #[test]
    fn everything_unavailable_result_guides_install_or_startup() {
        let install_result = everything_unavailable_result_from_status(EverythingStatus {
            installed: false,
            running: false,
            ipc_available: false,
            http_available: false,
            install_path: None,
            message: "missing".into(),
        })
        .expect("install diagnostic");
        assert_eq!(install_result.action, ActionKind::OpenUrl);
        assert!(install_result.title.contains("安装 Everything"));

        let startup_result = everything_unavailable_result_from_status(EverythingStatus {
            installed: true,
            running: false,
            ipc_available: false,
            http_available: false,
            install_path: Some(r"C:\Tools\Everything.exe".into()),
            message: "stopped".into(),
        })
        .expect("startup diagnostic");
        assert_eq!(startup_result.action, ActionKind::OpenFile);
        assert_eq!(startup_result.subtitle, r"C:\Tools\Everything.exe");

        assert!(everything_unavailable_result_from_status(EverythingStatus {
            installed: true,
            running: true,
            ipc_available: true,
            http_available: false,
            install_path: Some(r"C:\Tools\Everything.exe".into()),
            message: "ready".into(),
        })
        .is_none());
    }

    #[test]
    fn everything_directory_search_query_quotes_paths_with_spaces() {
        let query = DirectorySearchQuery {
            directory: PathBuf::from(r"C:\Program Files"),
            search_term: "easy".into(),
        };

        assert_eq!(
            everything_directory_search_query(&query, EverythingSearchOptions::default()),
            r#"path:"C:\Program Files" easy"#
        );
    }

    #[test]
    fn environment_path_helpers_normalize_home_relative_paths() {
        let path = normalize_env_path(r"\Users", "C:\\").expect("normalized path");

        assert_eq!(path, PathBuf::from(r"C:\Users"));
    }

    #[test]
    fn system_commands_include_flow_style_utilities() {
        let ids = system_command_results()
            .into_iter()
            .map(|result| result.id)
            .collect::<HashSet<_>>();

        assert!(ids.contains("command-task-manager"));
        assert!(ids.contains("command-recycle-bin"));
        assert!(ids.contains("command-restart-advanced"));
    }

    #[test]
    fn action_keyword_parser_routes_known_prefixes_and_preserves_unknown_queries() {
        let app_route = parse_action_keyword_query("app vscode");
        assert_eq!(app_route.query, "vscode");
        assert_eq!(
            app_route.sources.expect("app route sources"),
            HashSet::from([SearchSource::Apps])
        );

        let command_route = parse_action_keyword_query("cmd:settings");
        assert_eq!(command_route.query, "settings");
        assert_eq!(
            command_route.sources.expect("command route sources"),
            HashSet::from([SearchSource::System])
        );

        let web_route = parse_action_keyword_query("web rust tauri");
        assert_eq!(web_route.query, "rust tauri");
        assert_eq!(
            web_route.sources.expect("web route sources"),
            HashSet::from([SearchSource::WebSearch])
        );

        let calculator_route = parse_action_keyword_query("=1+2*3");
        assert_eq!(calculator_route.query, "1+2*3");
        assert_eq!(
            calculator_route.sources.expect("calculator route sources"),
            HashSet::from([SearchSource::Calculator])
        );

        let unknown_route = parse_action_keyword_query("unknown value");
        assert_eq!(unknown_route.query, "unknown value");
        assert!(unknown_route.sources.is_none());
    }

    #[test]
    fn action_keyword_route_filters_enabled_sources_and_falls_back_for_unknown_prefixes() {
        let enabled_sources = HashSet::from([SearchSource::Apps, SearchSource::System]);
        let app_route = parse_action_keyword_query("app code");
        let filtered = apply_action_keyword_route(&enabled_sources, &app_route);

        assert_eq!(filtered, HashSet::from([SearchSource::Apps]));

        let file_route = parse_action_keyword_query("file notes");
        let filtered = apply_action_keyword_route(&enabled_sources, &file_route);

        assert!(filtered.is_empty());

        let unknown_route = parse_action_keyword_query("maybe code");
        let filtered = apply_action_keyword_route(&enabled_sources, &unknown_route);

        assert_eq!(filtered, enabled_sources);
    }

    #[test]
    fn action_keyword_filtered_sources_limit_provider_results() {
        struct AppHitProvider;
        struct CommandHitProvider;

        impl SearchProvider for AppHitProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![SearchResult {
                    id: "app:code".into(),
                    title: "Code".into(),
                    subtitle: "code.exe".into(),
                    kind: ResultKind::App,
                    action: ActionKind::LaunchApp,
                    source: "测试应用".into(),
                    score: 0.5,
                    shortcut: None,
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        impl SearchProvider for CommandHitProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::System)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![SearchResult {
                    id: "command:code".into(),
                    title: "Code Command".into(),
                    subtitle: "code command".into(),
                    kind: ResultKind::Command,
                    action: ActionKind::RunCommand,
                    source: "测试命令".into(),
                    score: 0.5,
                    shortcut: None,
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        let core = SearchCore::new(vec![Box::new(AppHitProvider), Box::new(CommandHitProvider)]);
        let route = parse_action_keyword_query("cmd code");
        let enabled_sources = apply_action_keyword_route(
            &HashSet::from([SearchSource::Apps, SearchSource::System]),
            &route,
        );
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let results = core.search_with_diagnostics(&route.query, &context, 10, false);

        assert_eq!(results.results.len(), 1);
        assert_eq!(results.results[0].id, "command:code");
    }

    fn quick_entry_test_context<'a>(
        commands: &'a [CustomCommand],
        phrases: &'a [Phrase],
        templates: &'a [WebSearchTemplate],
        password_options: &'a PasswordOptions,
        enabled_sources: &'a HashSet<SearchSource>,
    ) -> SearchContext<'a> {
        SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: Some(commands),
            phrases: Some(phrases),
            web_search_templates: Some(templates),
            password_options: Some(password_options),
            exclusion_rules: None,
            source_weights: None,
            enabled_sources,
            everything_options: None,
        }
    }

    fn test_custom_command(
        id: &str,
        name: &str,
        command_type: &str,
        target: &str,
    ) -> CustomCommand {
        CustomCommand {
            id: id.into(),
            name: name.into(),
            command_type: command_type.into(),
            target: target.into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }
    }

    fn test_phrase(id: &str, title: &str, text: &str) -> Phrase {
        Phrase {
            id: id.into(),
            title: title.into(),
            text: text.into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
            use_count: 0,
        }
    }

    fn test_web_template(id: &str, keyword: &str, name: &str) -> WebSearchTemplate {
        WebSearchTemplate {
            id: id.into(),
            keyword: keyword.into(),
            name: name.into(),
            url_template: format!("https://example.com/{keyword}?q={{query}}"),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }
    }

    fn test_recent_item(
        id: &str,
        kind: &str,
        title: &str,
        target: &str,
        use_count: i64,
    ) -> RecentItem {
        RecentItem {
            id: id.into(),
            kind: kind.into(),
            title: title.into(),
            target: target.into(),
            use_count,
            last_used_at: "2026-06-01T00:00:00.000Z".into(),
        }
    }

    #[test]
    fn quick_entry_alias_returns_category_entries() {
        let enabled_sources = HashSet::from([
            SearchSource::System,
            SearchSource::Phrase,
            SearchSource::WebSearch,
            SearchSource::Tools,
            SearchSource::Apps,
            SearchSource::Files,
        ]);
        let password_options = PasswordOptions::default();
        let context = quick_entry_test_context(&[], &[], &[], &password_options, &enabled_sources);
        let route = parse_quick_entry_query("/", "/").expect("quick entry route");
        let results = quick_entry_results(&route, &context);

        assert_eq!(
            results
                .iter()
                .map(|result| result.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "quick-entry-category:cmd",
                "quick-entry-category:phrase",
                "quick-entry-category:web",
                "quick-entry-category:tools",
                "quick-entry-category:recent-apps",
                "quick-entry-category:recent-folders",
            ]
        );
    }

    #[test]
    fn quick_entry_alias_can_change_without_keeping_slash_active() {
        assert!(parse_quick_entry_query("/", ">").is_none());
        assert_eq!(
            parse_quick_entry_query(">", ">"),
            Some(QuickEntryRoute::Categories {
                filter: String::new()
            })
        );
        assert_eq!(
            parse_quick_entry_query(">cmd", ">"),
            Some(QuickEntryRoute::Category {
                category: QuickEntryCategory::Cmd,
                query: String::new()
            })
        );
    }

    #[test]
    fn quick_entry_unknown_child_filters_categories_without_plain_search_fallback() {
        let enabled_sources = HashSet::from([SearchSource::Tools]);
        let password_options = PasswordOptions::default();
        let context = quick_entry_test_context(&[], &[], &[], &password_options, &enabled_sources);
        let route = parse_quick_entry_query("/to", "/").expect("quick entry filter route");
        let results = quick_entry_results(&route, &context);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "quick-entry-category:tools");
        assert!(parse_quick_entry_query("cmd", "/").is_none());
        assert!(parse_quick_entry_query("web", "/").is_none());
        assert!(parse_quick_entry_query("tools", "/").is_none());
    }

    #[test]
    fn quick_entry_cmd_lists_and_filters_only_custom_commands() {
        let commands = vec![
            test_custom_command(
                "custom-command:docs",
                "Docs",
                "url",
                "https://example.com/docs",
            ),
            test_custom_command(
                "custom-command:build",
                "Build",
                "program",
                r"C:\Tools\build.exe",
            ),
        ];
        let enabled_sources = HashSet::from([SearchSource::System]);
        let password_options = PasswordOptions::default();
        let context =
            quick_entry_test_context(&commands, &[], &[], &password_options, &enabled_sources);

        let all_route = parse_quick_entry_query("/cmd", "/").expect("cmd route");
        let all_results = quick_entry_results(&all_route, &context);
        assert_eq!(all_results.len(), 2);
        assert!(all_results
            .iter()
            .all(|result| result.id.starts_with("custom-command:")));
        assert!(all_results
            .iter()
            .all(|result| !result.id.starts_with("command-")));

        let filtered_route = parse_quick_entry_query("/cmd program", "/").expect("filtered cmd");
        let filtered_results = quick_entry_results(&filtered_route, &context);
        assert_eq!(filtered_results.len(), 1);
        assert_eq!(filtered_results[0].id, "custom-command:build");
    }

    #[test]
    fn quick_entry_phrase_empty_query_lists_phrases() {
        let phrases = vec![
            test_phrase("phrase:greeting", "Greeting", "Hello"),
            test_phrase("phrase:bye", "Bye", "Goodbye"),
        ];
        let enabled_sources = HashSet::from([SearchSource::Phrase]);
        let password_options = PasswordOptions::default();
        let context =
            quick_entry_test_context(&[], &phrases, &[], &password_options, &enabled_sources);
        let route = parse_quick_entry_query("/phrase", "/").expect("phrase route");
        let results = quick_entry_results(&route, &context);

        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|result| result.id.starts_with("phrase:")));
    }

    #[test]
    fn quick_entry_web_empty_query_lists_templates_without_searching_default() {
        let templates = vec![
            test_web_template("web-template:gh", "gh", "GitHub"),
            test_web_template("web-template:bing", "bing", "Bing"),
        ];
        let enabled_sources = HashSet::from([SearchSource::WebSearch]);
        let password_options = PasswordOptions::default();
        let context =
            quick_entry_test_context(&[], &[], &templates, &password_options, &enabled_sources);
        let route = parse_quick_entry_query("/web", "/").expect("web route");
        let results = quick_entry_results(&route, &context);

        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|result| result.id.starts_with("quick-entry-web-template:")));
        assert!(results
            .iter()
            .all(|result| matches!(result.action, ActionKind::RunCommand)));
    }

    #[test]
    fn quick_entry_web_keyword_query_uses_matching_template() {
        let templates = vec![test_web_template("web-template:gh", "gh", "GitHub")];
        let enabled_sources = HashSet::from([SearchSource::WebSearch]);
        let password_options = PasswordOptions::default();
        let context =
            quick_entry_test_context(&[], &[], &templates, &password_options, &enabled_sources);
        let route = parse_quick_entry_query("/web gh rust tauri", "/").expect("web search route");
        let results = quick_entry_results(&route, &context);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, ActionKind::OpenUrl);
        assert_eq!(results[0].subtitle, "https://example.com/gh?q=rust%20tauri");
    }

    #[test]
    fn quick_entry_web_keyword_can_be_named_web() {
        let templates = vec![test_web_template("web-template:web", "web", "Default Web")];
        let enabled_sources = HashSet::from([SearchSource::WebSearch]);
        let password_options = PasswordOptions::default();
        let context =
            quick_entry_test_context(&[], &[], &templates, &password_options, &enabled_sources);
        let route = parse_quick_entry_query("/web web asd", "/").expect("web search route");
        let results = quick_entry_results(&route, &context);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, ActionKind::OpenUrl);
        assert_eq!(results[0].subtitle, "https://example.com/web?q=asd");
    }

    #[test]
    fn quick_entry_tools_lists_tools_and_enters_individual_tool() {
        let enabled_sources = HashSet::from([SearchSource::Tools]);
        let password_options = PasswordOptions::default();
        let context = quick_entry_test_context(&[], &[], &[], &password_options, &enabled_sources);
        assert!(ToolProvider.search("/", &context).is_empty());

        let menu_route = parse_quick_entry_query("/tools", "/").expect("tools route");
        let menu_results = quick_entry_results(&menu_route, &context);
        assert_eq!(
            menu_results
                .iter()
                .map(|result| result.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "tool-entry:enc",
                "tool-entry:dec",
                "tool-entry:pwd",
                "tool-entry:time"
            ]
        );

        let enc_route = parse_quick_entry_query("/tools enc", "/").expect("tool hint route");
        let enc_results = quick_entry_results(&enc_route, &context);
        assert_eq!(enc_results.len(), 1);
        assert_eq!(enc_results[0].id, "tool-hint:enc");
    }

    #[test]
    fn quick_entry_recent_apps_lists_limited_exe_and_shortcut_results() {
        let recent_items = (0..10)
            .map(|index| {
                test_recent_item(
                    &format!("app:app-{index}"),
                    "app",
                    &format!("App {index}"),
                    &format!(r"C:\Apps\App{index}.exe"),
                    1,
                )
            })
            .chain([
                test_recent_item(
                    "app:shortcut",
                    "app",
                    "Shortcut",
                    r"C:\Apps\Shortcut.lnk",
                    1,
                ),
                test_recent_item("app:url", "app", "Web Shortcut", r"C:\Apps\Web.url", 1),
                test_recent_item("file:folder", "file", "Folder", r"C:\Apps", 1),
            ])
            .collect::<Vec<_>>();
        let enabled_sources = HashSet::from([SearchSource::Apps]);
        let password_options = PasswordOptions::default();
        let context = quick_entry_test_context(&[], &[], &[], &password_options, &enabled_sources);

        let route = parse_quick_entry_query("/recent-apps", "/").expect("recent apps route");
        let results = quick_entry_results_with_recents(&route, &context, &recent_items);
        assert_eq!(results.len(), MAX_QUICK_ENTRY_RECENT_RESULTS);
        assert!(results
            .iter()
            .all(|result| matches!(&result.kind, ResultKind::App)));
        assert!(results
            .iter()
            .all(|result| matches!(result.action, ActionKind::LaunchApp)));
        assert!(results.iter().all(|result| !result.id.eq("app:url")));

        let filtered_route =
            parse_quick_entry_query("/recent-apps app 2", "/").expect("recent apps filter");
        let filtered_results =
            quick_entry_results_with_recents(&filtered_route, &context, &recent_items);
        assert_eq!(filtered_results.len(), 1);
        assert_eq!(filtered_results[0].id, "app:app-2");
    }

    #[test]
    fn quick_entry_recent_folders_lists_only_existing_directories() {
        let root = env::temp_dir().join(format!(
            "easy-launcher-recent-folders-{}",
            std::process::id()
        ));
        let folder = root.join("Project Folder");
        let file = root.join("notes.txt");
        fs::create_dir_all(&folder).expect("create temp folder");
        fs::write(&file, "notes").expect("write temp file");

        let recent_items = vec![
            test_recent_item(
                "file:folder",
                "file",
                "Project Folder",
                folder.to_str().expect("folder utf-8"),
                3,
            ),
            test_recent_item(
                "file:notes",
                "file",
                "notes.txt",
                file.to_str().expect("file utf-8"),
                2,
            ),
            test_recent_item("app:code", "app", "Code", r"C:\Apps\Code.exe", 1),
        ];
        let enabled_sources = HashSet::from([SearchSource::Files]);
        let password_options = PasswordOptions::default();
        let context = quick_entry_test_context(&[], &[], &[], &password_options, &enabled_sources);

        let route = parse_quick_entry_query("/recent-folders", "/").expect("recent folders route");
        let results = quick_entry_results_with_recents(&route, &context, &recent_items);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "file:folder");
        assert!(matches!(&results[0].kind, ResultKind::File));
        assert_eq!(results[0].action, ActionKind::OpenFile);
        assert!(results[0]
            .file_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.is_dir));

        let filtered_route =
            parse_quick_entry_query("/recent-folders project", "/").expect("recent folders filter");
        let filtered_results =
            quick_entry_results_with_recents(&filtered_route, &context, &recent_items);
        assert_eq!(filtered_results.len(), 1);
        assert_eq!(filtered_results[0].id, "file:folder");

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn search_diagnostics_record_provider_timings_without_query_text() {
        struct DiagnosticProvider;
        struct DisabledDiagnosticProvider;

        impl SearchProvider for DiagnosticProvider {
            fn provider_id(&self) -> &'static str {
                "diagnostic-provider"
            }

            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![SearchResult {
                    id: "app:diagnostic".into(),
                    title: "Diagnostic App".into(),
                    subtitle: "diagnostic.exe".into(),
                    kind: ResultKind::App,
                    action: ActionKind::LaunchApp,
                    source: "测试应用".into(),
                    score: 0.5,
                    shortcut: Some("Enter".into()),
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        impl SearchProvider for DisabledDiagnosticProvider {
            fn provider_id(&self) -> &'static str {
                "disabled-diagnostic-provider"
            }

            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Files)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                panic!("disabled providers must not run");
            }
        }

        let core = SearchCore::new(vec![
            Box::new(DiagnosticProvider),
            Box::new(DisabledDiagnosticProvider),
        ]);
        let enabled_sources = HashSet::from([SearchSource::Apps]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let execution = core.search_with_diagnostics("SecretTerm", &context, 10, true);

        assert_eq!(execution.results.len(), 1);
        assert_eq!(execution.diagnostics.result_count, 1);
        assert_eq!(
            execution.diagnostics.query_length,
            "SecretTerm".chars().count()
        );
        assert!(execution.diagnostics.cache_hit);
        assert!(!execution.diagnostics.cancelled);
        assert_eq!(execution.diagnostics.provider_timings.len(), 2);
        assert_eq!(
            execution
                .diagnostics
                .stage_timings
                .iter()
                .map(|timing| timing.stage.as_str())
                .collect::<Vec<_>>(),
            vec!["dedupe", "exclude", "score", "sort-limit"]
        );

        let active_timing = &execution.diagnostics.provider_timings[0];
        assert_eq!(active_timing.provider, "diagnostic-provider");
        assert_eq!(active_timing.source.as_deref(), Some("apps"));
        assert_eq!(active_timing.result_count, 1);
        assert!(!active_timing.skipped);

        let skipped_timing = &execution.diagnostics.provider_timings[1];
        assert_eq!(skipped_timing.provider, "disabled-diagnostic-provider");
        assert_eq!(skipped_timing.source.as_deref(), Some("files"));
        assert_eq!(skipped_timing.result_count, 0);
        assert!(skipped_timing.skipped);

        let diagnostics_json =
            serde_json::to_string(&execution.diagnostics).expect("serialize diagnostics");
        assert!(!diagnostics_json.contains("SecretTerm"));

        let log_line = format_search_diagnostics(&execution.diagnostics);
        assert!(log_line.contains("query_len=10"));
        assert!(log_line.contains("cancelled=false"));
        assert!(log_line.contains("diagnostic-provider(apps):"));
        assert!(log_line.contains("disabled-diagnostic-provider(files):0ms/0 skipped"));
        assert!(log_line.contains("stages=[dedupe:"));
        assert!(log_line.contains("exclude:"));
        assert!(log_line.contains("score:"));
        assert!(log_line.contains("sort-limit:"));
        assert!(!log_line.contains("SecretTerm"));
    }

    #[test]
    fn default_search_core_uses_everything_http_only_as_fallback() {
        let core = default_search_core();
        let provider_ids = core
            .providers
            .iter()
            .map(|provider| provider.provider_id())
            .collect::<Vec<_>>();

        assert!(provider_ids.contains(&"everything-ipc"));
        assert!(!provider_ids.contains(&"everything-http"));
    }

    #[test]
    fn search_cancellation_discards_stale_results_between_providers() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };

        struct StaleAfterProvider {
            stale: Arc<AtomicBool>,
        }
        struct MustNotRunProvider;

        impl SearchProvider for StaleAfterProvider {
            fn provider_id(&self) -> &'static str {
                "stale-after-provider"
            }

            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                self.stale.store(true, Ordering::Relaxed);
                vec![SearchResult {
                    id: "app:stale".into(),
                    title: "Stale App".into(),
                    subtitle: "stale.exe".into(),
                    kind: ResultKind::App,
                    action: ActionKind::LaunchApp,
                    source: "测试应用".into(),
                    score: 0.5,
                    shortcut: Some("Enter".into()),
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        impl SearchProvider for MustNotRunProvider {
            fn provider_id(&self) -> &'static str {
                "must-not-run-provider"
            }

            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::System)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                panic!("stale searches must stop before running later providers");
            }
        }

        let stale = Arc::new(AtomicBool::new(false));
        let core = SearchCore::new(vec![
            Box::new(StaleAfterProvider {
                stale: stale.clone(),
            }),
            Box::new(MustNotRunProvider),
        ]);
        let enabled_sources = HashSet::from([SearchSource::Apps, SearchSource::System]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let execution = core.search_with_cancellation("stale", &context, 10, false, || {
            stale.load(Ordering::Relaxed)
        });

        assert!(execution.diagnostics.cancelled);
        assert!(execution.results.is_empty());
        assert_eq!(execution.diagnostics.result_count, 0);
        assert_eq!(execution.diagnostics.provider_timings.len(), 1);
        assert_eq!(
            execution.diagnostics.provider_timings[0].provider,
            "stale-after-provider"
        );
    }

    #[test]
    fn fast_tier_search_returns_without_running_slow_providers() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };

        struct FastProvider;
        struct SlowProvider {
            ran: Arc<AtomicBool>,
        }

        impl SearchProvider for FastProvider {
            fn provider_id(&self) -> &'static str {
                "fast-provider"
            }

            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![SearchResult {
                    id: "app:fast".into(),
                    title: "Fast App".into(),
                    subtitle: "fast.exe".into(),
                    kind: ResultKind::App,
                    action: ActionKind::LaunchApp,
                    source: "测试应用".into(),
                    score: 0.5,
                    shortcut: Some("Enter".into()),
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        impl SearchProvider for SlowProvider {
            fn provider_id(&self) -> &'static str {
                "slow-provider"
            }

            fn provider_tier(&self) -> ProviderTier {
                ProviderTier::Slow
            }

            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Files)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                self.ran.store(true, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(200));
                Vec::new()
            }
        }

        let slow_ran = Arc::new(AtomicBool::new(false));
        let core = SearchCore::new(vec![
            Box::new(FastProvider),
            Box::new(SlowProvider {
                ran: slow_ran.clone(),
            }),
        ]);
        let enabled_sources = HashSet::from([SearchSource::Apps, SearchSource::Files]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let started_at = Instant::now();
        let execution = core.search_tier_with_cancellation(
            ProviderTier::Fast,
            "fast",
            &context,
            10,
            false,
            || false,
        );

        assert!(started_at.elapsed() < std::time::Duration::from_millis(100));
        assert!(!slow_ran.load(Ordering::Relaxed));
        assert_eq!(execution.results[0].id, "app:fast");
        assert_eq!(execution.diagnostics.tier, "fast");
        assert!(execution
            .diagnostics
            .provider_timings
            .iter()
            .all(|timing| timing.tier == "fast"));
    }

    #[test]
    fn search_result_cache_returns_recent_entries_and_replaces_fast_snapshot() {
        let mut cache = SearchResultCache::new(2, Duration::from_secs(60));
        let fast_result = SearchResult {
            id: "app:fast".into(),
            title: "Fast App".into(),
            subtitle: "fast.exe".into(),
            kind: ResultKind::App,
            action: ActionKind::LaunchApp,
            source: "测试应用".into(),
            score: 0.5,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };
        let slow_result = SearchResult {
            id: "file:slow".into(),
            title: "slow.txt".into(),
            subtitle: r"C:\slow.txt".into(),
            kind: ResultKind::File,
            action: ActionKind::OpenFile,
            source: "测试文件".into(),
            score: 0.4,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };

        cache.insert(
            "query".into(),
            CachedSearchResults {
                results: vec![fast_result.clone()],
                complete: false,
            },
        );
        let cached = cache.get("query").expect("fast cache hit");
        assert!(!cached.complete);
        assert_eq!(cached.results[0].id, "app:fast");

        cache.insert(
            "query".into(),
            CachedSearchResults {
                results: vec![fast_result, slow_result],
                complete: true,
            },
        );
        let cached = cache.get("query").expect("complete cache hit");
        assert!(cached.complete);
        assert_eq!(cached.results.len(), 2);
    }

    #[test]
    fn search_result_cache_evicts_oldest_entries() {
        let mut cache = SearchResultCache::new(1, Duration::from_secs(60));
        let result = SearchResult {
            id: "app:one".into(),
            title: "One".into(),
            subtitle: "one.exe".into(),
            kind: ResultKind::App,
            action: ActionKind::LaunchApp,
            source: "测试应用".into(),
            score: 0.5,
            shortcut: None,
            file_metadata: None,
            icon_path: None,
        };

        cache.insert(
            "one".into(),
            CachedSearchResults {
                results: vec![result.clone()],
                complete: true,
            },
        );
        cache.insert(
            "two".into(),
            CachedSearchResults {
                results: vec![result],
                complete: true,
            },
        );

        assert!(cache.get("one").is_none());
        assert!(cache.get("two").is_some());
    }

    #[test]
    fn everything_results_defer_file_metadata_reads() {
        let files = (0..5)
            .map(|index| crate::everything::EverythingFileResult {
                name: format!("file-{index}.txt"),
                path: format!(r"C:\missing\file-{index}.txt"),
                is_folder: false,
            })
            .collect::<Vec<_>>();

        let results = everything_results(files, "Everything Test", "Everything Test");

        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|result| result.file_metadata.is_none()));
    }

    #[test]
    fn everything_results_promote_exact_parent_folder_from_deep_match() {
        let files = vec![crate::everything::EverythingFileResult {
            name: ".bin".into(),
            path: r"D:\workspace_html\huhu_html\h5\xiaoxiaole\node_modules\.bin".into(),
            is_folder: true,
        }];

        let results = everything_results(files, "Everything Test", "huhu_html");

        assert_eq!(results[0].title, "huhu_html");
        assert_eq!(results[0].subtitle, r"D:\workspace_html\huhu_html");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn everything_results_fold_path_only_files_under_displayed_folder() {
        let files = vec![
            crate::everything::EverythingFileResult {
                name: "project".into(),
                path: r"D:\workspace\project".into(),
                is_folder: true,
            },
            crate::everything::EverythingFileResult {
                name: "notes.txt".into(),
                path: r"D:\workspace\project\notes.txt".into(),
                is_folder: false,
            },
            crate::everything::EverythingFileResult {
                name: "project-plan.txt".into(),
                path: r"D:\workspace\project\project-plan.txt".into(),
                is_folder: false,
            },
        ];

        let results = everything_results(files, "Everything Test", "project");
        let titles = results
            .iter()
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>();

        assert!(titles.contains(&"project"));
        assert!(titles.contains(&"project-plan.txt"));
        assert!(!titles.contains(&"notes.txt"));
    }

    #[test]
    fn everything_results_limit_path_only_files() {
        let files = (0..10)
            .map(|index| crate::everything::EverythingFileResult {
                name: format!("item-{index}.txt"),
                path: format!(r"D:\workspace\project_data\item-{index}.txt"),
                is_folder: false,
            })
            .collect::<Vec<_>>();

        let results = everything_results(files, "Everything Test", "project");

        assert_eq!(results.len(), EVERYTHING_PATH_ONLY_FILE_LIMIT);
        assert!(results.iter().all(|result| result.score < 0.5));
    }

    #[test]
    fn search_sorting_uses_recent_weight_then_stable_tiebreakers() {
        struct StaticProvider;

        impl SearchProvider for StaticProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, context: &SearchContext<'_>) -> Vec<SearchResult> {
                ["app:beta", "app:alpha"]
                    .into_iter()
                    .map(|id| {
                        let title = id.trim_start_matches("app:").to_string();
                        let recent_score = context
                            .recent_scores
                            .and_then(|scores| scores.get(id))
                            .copied()
                            .unwrap_or(0.0);

                        SearchResult {
                            id: id.into(),
                            title,
                            subtitle: format!("{id}.exe"),
                            kind: ResultKind::App,
                            action: ActionKind::LaunchApp,
                            source: "测试应用".into(),
                            score: 0.5 + recent_score,
                            shortcut: Some("Enter".into()),
                            file_metadata: None,
                            icon_path: None,
                        }
                    })
                    .collect()
            }
        }

        let core = SearchCore::new(vec![Box::new(StaticProvider)]);
        let enabled_sources = all_search_sources();
        let recent_scores = HashMap::from([("app:beta".to_string(), 0.3)]);
        let context = SearchContext {
            recent_scores: Some(&recent_scores),
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let results = core.search("a", &context, 10);

        assert_eq!(results[0].id, "app:beta");

        let context_without_recents = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };
        let results = core.search("a", &context_without_recents, 10);

        assert_eq!(results[0].id, "app:alpha");
    }

    #[test]
    fn query_selection_scores_only_boost_matching_query_context() {
        struct StaticProvider;

        impl SearchProvider for StaticProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                ["app:alpha", "app:beta"]
                    .into_iter()
                    .map(|id| SearchResult {
                        id: id.into(),
                        title: id.trim_start_matches("app:").to_string(),
                        subtitle: format!("{id}.exe"),
                        kind: ResultKind::App,
                        action: ActionKind::LaunchApp,
                        source: "测试应用".into(),
                        score: 0.5,
                        shortcut: None,
                        file_metadata: None,
                        icon_path: None,
                    })
                    .collect()
            }
        }

        let core = SearchCore::new(vec![Box::new(StaticProvider)]);
        let enabled_sources = all_search_sources();
        let learned_scores = HashMap::from([("app:beta".to_string(), 0.35)]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: Some(&learned_scores),
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let results = core.search("a", &context, 10);
        assert_eq!(results[0].id, "app:beta");

        let context_without_learned_query = SearchContext {
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            ..context
        };
        let results = core.search("a", &context_without_learned_query, 10);
        assert_eq!(results[0].id, "app:alpha");
    }

    #[test]
    fn pinned_scores_rank_above_query_selection_scores() {
        struct StaticProvider;

        impl SearchProvider for StaticProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                ["app:alpha", "app:beta"]
                    .into_iter()
                    .map(|id| SearchResult {
                        id: id.into(),
                        title: id.trim_start_matches("app:").to_string(),
                        subtitle: format!("{id}.exe"),
                        kind: ResultKind::App,
                        action: ActionKind::LaunchApp,
                        source: "测试应用".into(),
                        score: 0.5,
                        shortcut: None,
                        file_metadata: None,
                        icon_path: None,
                    })
                    .collect()
            }
        }

        let core = SearchCore::new(vec![Box::new(StaticProvider)]);
        let enabled_sources = all_search_sources();
        let learned_scores = HashMap::from([("app:alpha".to_string(), 0.35)]);
        let pinned_scores = HashMap::from([("app:beta".to_string(), 0.65)]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: Some(&learned_scores),
            pinned_scores: Some(&pinned_scores),
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let results = core.search("a", &context, 10);

        assert_eq!(results[0].id, "app:beta");
    }

    #[test]
    fn result_aliases_inject_matching_results_and_respect_source_filter() {
        let core = SearchCore::new(Vec::new());
        let aliases = vec![ResultAlias {
            alias: "ide".into(),
            normalized_alias: "ide".into(),
            result_id: "app:vscode".into(),
            kind: "app".into(),
            title: "Visual Studio Code".into(),
            target: r"C:\Apps\Code.exe".into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }];
        let app_sources = HashSet::from([SearchSource::Apps]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: Some(&aliases),
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &app_sources,
            everything_options: None,
        };

        let results = core.search("ide", &context, 10);

        assert_eq!(results[0].id, "app:vscode");

        let file_sources = HashSet::from([SearchSource::Files]);
        let context_without_apps = SearchContext {
            enabled_sources: &file_sources,
            ..context
        };
        assert!(core.search("ide", &context_without_apps, 10).is_empty());
    }

    #[test]
    fn calculator_respects_operator_precedence() {
        let result = calculator_result("1+2*3").expect("calculator result");
        assert_eq!(result.subtitle, "7");
    }

    #[test]
    fn calculator_supports_parentheses_and_decimal_values() {
        let result = calculator_result("(1.5+2.5)*2").expect("calculator result");
        assert_eq!(result.subtitle, "8");
    }

    #[test]
    fn calculator_rejects_invalid_expressions() {
        assert!(calculator_result("1/0").is_none());
        assert!(calculator_result("1+").is_none());
        assert!(calculator_result("code").is_none());
    }

    #[test]
    fn system_commands_are_hidden_until_query_matches() {
        let provider = SystemCommandProvider;
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &all_search_sources(),
            everything_options: None,
        };

        assert!(provider.search("", &context).is_empty());
        assert_eq!(provider.search("设置", &context)[0].id, "command-settings");
    }

    #[test]
    fn pinyin_matches_system_commands() {
        let provider = SystemCommandProvider;
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &all_search_sources(),
            everything_options: None,
        };

        assert_eq!(
            provider.search("dkxtsz", &context)[0].id,
            "command-settings"
        );
        assert_eq!(
            provider.search("dakaixitongshezhi", &context)[0].id,
            "command-settings"
        );
    }

    #[test]
    fn custom_command_provider_returns_search_hits() {
        let commands = vec![CustomCommand {
            id: "custom-command:docs".into(),
            name: "项目文档".into(),
            command_type: "url".into(),
            target: "https://example.com/docs".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        }];
        let provider = SystemCommandProvider;
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: Some(&commands),
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &all_search_sources(),
            everything_options: None,
        };

        let results = provider.search("xmw", &context);

        assert_eq!(results[0].id, "custom-command:docs");
        assert!(matches!(results[0].action, ActionKind::RunCommand));
    }

    #[test]
    fn phrase_provider_returns_copy_result() {
        let phrases = vec![Phrase {
            id: "phrase:greeting".into(),
            title: "问候语".into(),
            text: "你好，感谢你的更新。".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
            use_count: 2,
        }];
        let provider = PhraseProvider;
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: Some(&phrases),
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &all_search_sources(),
            everything_options: None,
        };

        let results = provider.search("wh", &context);

        assert_eq!(results[0].id, "phrase:greeting");
        assert!(matches!(results[0].action, ActionKind::CopyText));
        assert_eq!(results[0].source, "快捷短语");
    }

    #[test]
    fn web_search_provider_returns_keyword_template_result() {
        let templates = vec![WebSearchTemplate {
            id: "web-search:gh".into(),
            keyword: "gh".into(),
            name: "GitHub".into(),
            url_template: "https://github.com/search?q={query}".into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }];
        let provider = WebSearchProvider;
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: Some(&templates),
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &all_search_sources(),
            everything_options: None,
        };

        let results = provider.search("gh rust tauri", &context);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "GitHub 搜索 rust tauri");
        assert_eq!(
            results[0].subtitle,
            "https://github.com/search?q=rust%20tauri"
        );
        assert!(matches!(results[0].kind, ResultKind::WebSearch));
        assert!(matches!(results[0].action, ActionKind::OpenUrl));
    }

    #[test]
    fn web_search_route_uses_default_template_for_plain_query() {
        let templates = vec![WebSearchTemplate {
            id: "web-search:bing".into(),
            keyword: "web".into(),
            name: "Bing".into(),
            url_template: "https://www.bing.com/search?q={query}".into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }];
        let provider = WebSearchProvider;
        let web_only_sources = HashSet::from([SearchSource::WebSearch]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: Some(&templates),
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &web_only_sources,
            everything_options: None,
        };

        let explicit_results = provider.search("web gg", &context);
        let routed_results = provider.search("gg", &context);

        assert_eq!(explicit_results.len(), 1);
        assert_eq!(explicit_results[0].title, "Bing 搜索 gg");
        assert_eq!(
            explicit_results[0].subtitle,
            "https://www.bing.com/search?q=gg"
        );
        assert_eq!(routed_results.len(), 1);
        assert_eq!(routed_results[0].title, "Bing 搜索 gg");
    }

    #[test]
    fn web_search_provider_keeps_template_keywords_separate_from_action_keywords() {
        let templates = vec![WebSearchTemplate {
            id: "web-search:app".into(),
            keyword: "app".into(),
            name: "App Docs".into(),
            url_template: "https://example.com/search?q={query}".into(),
            created_at: "2026-06-01T00:00:00.000Z".into(),
            updated_at: "2026-06-01T00:00:00.000Z".into(),
        }];
        let provider = WebSearchProvider;
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: Some(&templates),
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &all_search_sources(),
            everything_options: None,
        };

        let direct_results = provider.search("app rust", &context);
        let routed_results = provider.search("web app rust", &context);

        assert_eq!(direct_results[0].title, "App Docs 搜索 rust");
        assert_eq!(routed_results[0].title, "App Docs 搜索 rust");
    }

    #[test]
    fn expands_web_search_url_template_with_encoded_query() {
        let url = expand_web_search_url("https://www.bing.com/search?q={query}", "rust tauri 中文");

        assert_eq!(
            url,
            "https://www.bing.com/search?q=rust%20tauri%20%E4%B8%AD%E6%96%87"
        );
    }

    #[test]
    fn search_weight_changes_sort_order_without_enabling_disabled_sources() {
        struct WeightedAppProvider;
        struct WeightedPhraseProvider;

        impl SearchProvider for WeightedAppProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![SearchResult {
                    id: "app:test".into(),
                    title: "Alpha".into(),
                    subtitle: "alpha.exe".into(),
                    kind: ResultKind::App,
                    action: ActionKind::LaunchApp,
                    source: "测试应用".into(),
                    score: 0.5,
                    shortcut: None,
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        impl SearchProvider for WeightedPhraseProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Phrase)
            }

            fn search(&self, _query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                vec![SearchResult {
                    id: "phrase:test".into(),
                    title: "Alpha phrase".into(),
                    subtitle: "alpha phrase".into(),
                    kind: ResultKind::Command,
                    action: ActionKind::CopyText,
                    source: "快捷短语".into(),
                    score: 0.5,
                    shortcut: None,
                    file_metadata: None,
                    icon_path: None,
                }]
            }
        }

        let core = SearchCore::new(vec![
            Box::new(WeightedAppProvider),
            Box::new(WeightedPhraseProvider),
        ]);
        let enabled_sources = HashSet::from([SearchSource::Apps, SearchSource::Phrase]);
        let weights = HashMap::from([(SearchSource::Phrase, 2.0), (SearchSource::Apps, 0.5)]);
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: Some(&[]),
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: Some(&weights),
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let results = core.search("alpha", &context, 10);

        assert_eq!(results[0].id, "phrase:test");

        let disabled_phrases = HashSet::from([SearchSource::Apps]);
        let context = SearchContext {
            enabled_sources: &disabled_phrases,
            everything_options: None,
            ..context
        };

        assert!(core
            .search("real phrase", &context, 10)
            .iter()
            .all(|result| result.id != "phrase:test"));
    }

    #[test]
    fn v2_regression_disabled_system_source_hides_custom_commands() {
        let commands = vec![CustomCommand {
            id: "custom-command:docs".into(),
            name: "项目文档".into(),
            command_type: "url".into(),
            target: "https://example.com/docs".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        }];
        let core = SearchCore::new(vec![Box::new(SystemCommandProvider)]);
        let enabled_sources = HashSet::new();
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: Some(&commands),
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        assert!(core.search("xmw", &context, 10).is_empty());
    }

    #[test]
    fn v2_regression_disabled_phrase_source_hides_phrases() {
        let phrases = vec![Phrase {
            id: "phrase:greeting".into(),
            title: "问候语".into(),
            text: "你好，感谢你的更新。".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
            use_count: 0,
        }];
        let core = SearchCore::new(vec![Box::new(PhraseProvider)]);
        let enabled_sources = HashSet::new();
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: Some(&phrases),
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        assert!(core.search("wh", &context, 10).is_empty());
    }

    #[test]
    fn v2_regression_custom_command_and_phrase_actions_are_stable() {
        let commands = vec![CustomCommand {
            id: "custom-command:docs".into(),
            name: "项目文档".into(),
            command_type: "url".into(),
            target: "https://example.com/docs".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        }];
        let phrases = vec![Phrase {
            id: "phrase:greeting".into(),
            title: "问候语".into(),
            text: "你好，感谢你的更新。".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
            use_count: 0,
        }];
        let enabled_sources = all_search_sources();
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: Some(&commands),
            phrases: Some(&phrases),
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        let command = SystemCommandProvider.search("xmw", &context);
        let phrase = PhraseProvider.search("wh", &context);

        assert!(matches!(command[0].action, ActionKind::RunCommand));
        assert!(matches!(phrase[0].action, ActionKind::CopyText));
    }

    #[test]
    fn disabled_sources_are_skipped() {
        let core = SearchCore::new(vec![Box::new(SystemCommandProvider)]);
        let enabled_sources = HashSet::new();
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        assert!(core.search("设置", &context, 10).is_empty());
    }

    #[test]
    fn disabled_command_source_skips_pinyin_matches() {
        let core = SearchCore::new(vec![Box::new(SystemCommandProvider)]);
        let enabled_sources = HashSet::new();
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        assert!(core.search("dakaixitongshezhi", &context, 10).is_empty());
    }

    #[test]
    fn disabled_app_source_skips_pinyin_matches() {
        struct TestAppProvider;

        impl SearchProvider for TestAppProvider {
            fn source(&self) -> Option<SearchSource> {
                Some(SearchSource::Apps)
            }

            fn search(&self, query: &str, _context: &SearchContext<'_>) -> Vec<SearchResult> {
                if matches_search_text(query, "记事本", r"C:\Windows\notepad.exe") {
                    vec![SearchResult {
                        id: "app:notepad".into(),
                        title: "记事本".into(),
                        subtitle: r"C:\Windows\notepad.exe".into(),
                        kind: ResultKind::App,
                        action: ActionKind::LaunchApp,
                        source: "测试应用".into(),
                        score: 0.5,
                        shortcut: Some("Enter".into()),
                        file_metadata: None,
                        icon_path: None,
                    }]
                } else {
                    Vec::new()
                }
            }
        }

        let core = SearchCore::new(vec![Box::new(TestAppProvider)]);
        let enabled_sources = HashSet::new();
        let context = SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            pinned_scores: None,
            result_aliases: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: None,
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        };

        assert!(matches_search_text(
            "jsb",
            "记事本",
            r"C:\Windows\notepad.exe"
        ));
        assert!(core.search("jsb", &context, 10).is_empty());
    }

    fn all_search_sources() -> HashSet<SearchSource> {
        [
            SearchSource::Apps,
            SearchSource::Files,
            SearchSource::Calculator,
            SearchSource::System,
            SearchSource::Ai,
            SearchSource::Phrase,
            SearchSource::WebSearch,
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn file_search_requires_at_least_two_query_chars() {
        assert!(is_file_query_too_short(""));
        assert!(is_file_query_too_short("a"));
        assert!(!is_file_query_too_short("ab"));
        assert!(!is_file_query_too_short("文档"));
    }
}
