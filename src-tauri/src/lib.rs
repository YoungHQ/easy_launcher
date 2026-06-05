mod ai;
mod apps;
mod everything;
mod file_metadata;
mod pinyin_search;
mod process;
mod search;
mod selection;
mod selection_trigger;
mod storage;
mod tools;
mod updates;

use ai::{
    build_ai_request_messages, list_openai_compatible_models, merge_ai_params,
    run_ai_action as run_ai_action_inner, run_ai_action_stream as run_ai_action_stream_inner,
    send_openai_compatible_chat, test_openai_compatible_profile, AiChatCancelledEvent,
    AiChatDeltaEvent, AiChatDoneEvent, AiChatErrorEvent, AiChatSendRequest, AiChatStarted,
    AiConfig, AiRequest,
};
use apps::{launch_app, warm_app_scan_cache};
use clipboard_win::set_clipboard_string;
use everything::{detect_everything_status, EverythingStatus};
use process::hidden_command;
use search::{
    apply_action_keyword_route, cached_search_diagnostics, default_search_core,
    log_search_diagnostics, merge_ranked_results, parse_action_keyword_query, ActionKind,
    CachedSearchResults, EverythingSearchOptions, ProviderTier, SearchContext, SearchDiagnostics,
    SearchResult, SearchResultCache, SearchSource,
};
use selection::{capture_selected_text, SelectionCaptureResult};
use selection_trigger::{SelectionTriggerHandle, SELECTION_TRIGGER_MODE_CTRL_MOUSE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use storage::{
    selection_conversation_title, AiAssistant, AiAssistantInput, AiConversation, AiMessage,
    AiModelProfile, AiModelProfileInput, AiProvider, AiProviderInput, AiProviderModel,
    AiProviderModelInput, AiSelectionAction, AiSelectionActionInput, CustomCommand,
    CustomCommandInput, ExclusionRule, ExclusionRuleInput, Phrase, PhraseInput, WebSearchTemplate,
    WebSearchTemplateInput, TRANSLATION_AI_ASSISTANT_ID,
};
use storage::{Storage, StorageState, StorageStatus};
use tauri::menu::MenuBuilder;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri::{PhysicalPosition, PhysicalSize};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tools::{sanitize_password_options, PasswordOptions};
use updates::UpdateCheckResult;

const LAUNCHER_SHORTCUT_LABEL: &str = "Alt+1";
const AI_SHORTCUT_LABEL: &str = "Alt+3";
const SELECTION_CAPTURE_EVENT: &str = "selection-captured";
const LAUNCHER_OPENED_EVENT: &str = "launcher-opened";
const AI_OPENED_EVENT: &str = "ai-opened";
const AI_STREAM_EVENT: &str = "ai-stream";
const AI_CHAT_DELTA_EVENT: &str = "ai-chat-delta";
const AI_CHAT_DONE_EVENT: &str = "ai-chat-done";
const AI_CHAT_ERROR_EVENT: &str = "ai-chat-error";
const AI_CHAT_CANCELLED_EVENT: &str = "ai-chat-cancelled";
const SETTINGS_OPENED_EVENT: &str = "settings-opened";
const SEARCH_PROGRESS_EVENT: &str = "search-progress";
const TRAY_OPEN_SETTINGS_EVENT: &str = "tray-open-settings";
const TRAY_EVERYTHING_STATUS_EVENT: &str = "tray-everything-status";
const TRAY_MENU_OPEN: &str = "tray-open-main";
const TRAY_MENU_SETTINGS: &str = "tray-open-settings";
const TRAY_MENU_EVERYTHING: &str = "tray-check-everything";
const TRAY_MENU_EXIT: &str = "tray-exit";
const STARTUP_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_VALUE_NAME: &str = "EasyLauncher";
const SEARCH_RESULT_LIMIT: usize = 20;
const SEARCH_CACHE_MAX_ENTRIES: usize = 64;
const SEARCH_CACHE_TTL: Duration = Duration::from_secs(45);
const SEARCH_WINDOW_WIDTH: u32 = 728;
const SEARCH_WINDOW_HEIGHT: u32 = 286;
const SETTINGS_WINDOW_WIDTH: u32 = 960;
const SETTINGS_WINDOW_HEIGHT: u32 = 700;
const AI_WINDOW_WIDTH: u32 = 760;
const AI_WINDOW_HEIGHT: u32 = 640;
const SELECTION_PICKER_WINDOW_WIDTH: u32 = 520;
const SELECTION_PICKER_WINDOW_HEIGHT: u32 = 48;
const SELECTION_RESULT_WINDOW_WIDTH: u32 = 520;
const SELECTION_RESULT_WINDOW_HEIGHT: u32 = 420;
const DEFAULT_TOOL_MENU_ALIAS: &str = "/";
const EXPORTABLE_SETTING_KEYS: &[&str] = &[
    "launcher.shortcut",
    "launcher.double_alt.enabled",
    "ai.shortcut",
    "selection.enabled",
    "selection.trigger.mode",
    "file.editor.path",
    "folder.editor.path",
    "ui.language",
    "startup.enabled",
    "search.source.apps",
    "search.source.files",
    "search.source.calculator",
    "search.source.system",
    "search.source.ai",
    "search.source.phrase",
    "search.source.web_search",
    "search.source.tools",
    "search.weight.apps",
    "search.weight.files",
    "search.weight.calculator",
    "search.weight.system",
    "search.weight.ai",
    "search.weight.phrase",
    "search.weight.web_search",
    "search.weight.tools",
    "everything.search.full_path",
    "everything.search.content",
    "tools.menu.alias",
    "tools.password.length",
    "tools.password.uppercase",
    "tools.password.lowercase",
    "tools.password.digits",
    "tools.password.hyphen",
    "tools.password.underscore",
    "tools.password.special",
    "tools.password.brackets",
    "updates.check.enabled",
    "updates.check.interval_hours",
    "updates.check.include_prerelease",
];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShortcutStatus {
    shortcut: String,
    registered: bool,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCaptureEvent {
    result: SelectionCaptureResult,
    x: Option<i32>,
    y: Option<i32>,
}

type ShortcutStatusStore = Mutex<ShortcutStatus>;

struct AiShortcutStatusStore(Mutex<ShortcutStatus>);

type AiCancelMap = Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>;
type SearchRequestTracker = Arc<AtomicU64>;
type SearchResultCacheStore = Arc<Mutex<SearchResultCache>>;
type LauncherWindowPositionStore = Mutex<Option<PhysicalPosition<i32>>>;
type SettingsPanelOpenStore = Arc<AtomicBool>;
type SelectionCaptureStore = Arc<Mutex<Option<SelectionCaptureEvent>>>;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchSourceSettings {
    apps: bool,
    files: bool,
    calculator: bool,
    system: bool,
    ai: bool,
    phrase: bool,
    web_search: bool,
    tools: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchWeightSettings {
    apps: f32,
    files: f32,
    calculator: f32,
    system: f32,
    ai: f32,
    phrase: f32,
    web_search: f32,
    tools: f32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigExport {
    version: u32,
    product: String,
    exported_at: String,
    settings: HashMap<String, String>,
    #[serde(default)]
    custom_commands: Vec<CustomCommand>,
    #[serde(default)]
    phrases: Vec<Phrase>,
    #[serde(default)]
    web_search_templates: Vec<WebSearchTemplate>,
    #[serde(default)]
    exclusion_rules: Vec<ExclusionRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigExportResult {
    path: String,
    setting_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigImportResult {
    imported_count: usize,
    ignored_count: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsOpenedEvent {
    section: String,
    ai_tab: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiSelectionChatRequest {
    request_id: String,
    assistant_id: String,
    provider_id: String,
    model_name: String,
    selection_text: String,
    conversation_id: Option<String>,
    message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchProgressEvent {
    request_id: u64,
    results: Vec<SearchResult>,
    diagnostics: SearchDiagnostics,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}. Tauri backend is connected.")
}

#[tauri::command]
fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
async fn check_for_updates(app: AppHandle, include_prerelease: bool) -> UpdateCheckResult {
    updates::check_for_updates(&app.package_info().version.to_string(), include_prerelease).await
}

#[tauri::command]
fn open_update_release_page(url: String) -> Result<(), String> {
    if !updates::is_allowed_release_url(&url) {
        return Err("Release 链接无效".into());
    }

    open_url(&url)
}

#[tauri::command]
fn launcher_shortcut_status(
    shortcut_status: tauri::State<'_, ShortcutStatusStore>,
) -> ShortcutStatus {
    shortcut_status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| ShortcutStatus {
            shortcut: LAUNCHER_SHORTCUT_LABEL.into(),
            registered: false,
            message: "无法读取快捷键状态".into(),
        })
}

#[tauri::command]
fn ai_shortcut_status(shortcut_status: tauri::State<'_, AiShortcutStatusStore>) -> ShortcutStatus {
    shortcut_status
        .0
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| ShortcutStatus {
            shortcut: AI_SHORTCUT_LABEL.into(),
            registered: false,
            message: "无法读取 AI 快捷键状态".into(),
        })
}

#[tauri::command]
fn capture_selection() -> SelectionCaptureResult {
    capture_selected_text()
}

#[tauri::command]
fn get_pending_selection_capture(
    selection_capture: tauri::State<'_, SelectionCaptureStore>,
) -> Option<SelectionCaptureEvent> {
    selection_capture
        .lock()
        .ok()
        .and_then(|capture| capture.clone())
}

#[tauri::command]
fn show_selection_assistant(
    app: AppHandle,
    selection_capture: tauri::State<'_, SelectionCaptureStore>,
) {
    let result = capture_selected_text();
    publish_selection_capture(&app, selection_capture.inner(), None, result);
}

#[tauri::command]
fn show_search_window(app: AppHandle) {
    set_settings_panel_open(&app, false);
    show_main_window(&app);
}

#[tauri::command]
fn show_settings_window(
    app: AppHandle,
    launcher_position: tauri::State<'_, LauncherWindowPositionStore>,
) {
    set_settings_panel_open(&app, true);
    if let Some(window) = app.get_webview_window("main") {
        store_launcher_position(&window, launcher_position.inner());
        restore_settings_window(&window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn show_ai_settings_window(
    app: AppHandle,
    launcher_position: tauri::State<'_, LauncherWindowPositionStore>,
) {
    show_settings_window(app.clone(), launcher_position);
    let _ = app.emit(
        SETTINGS_OPENED_EVENT,
        SettingsOpenedEvent {
            section: "ai".into(),
            ai_tab: Some("providers".into()),
        },
    );
}

#[tauri::command]
fn hide_main_window(app: AppHandle) {
    if settings_panel_is_open(&app) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        hide_window_to_tray(&window);
    }
}

#[tauri::command]
fn hide_selection_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("selection") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn show_ai_window(app: AppHandle) {
    set_settings_panel_open(&app, false);
    show_ai_panel(&app);
}

#[tauri::command]
fn storage_status(storage: tauri::State<'_, StorageState>) -> Result<StorageStatus, String> {
    storage
        .lock()
        .map_err(|_| "无法读取存储状态".to_string())
        .map(|storage| storage.status())
}

#[tauri::command]
fn get_setting(
    key: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<Option<String>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取设置".to_string())?
        .get_setting(&key)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_setting(
    key: String,
    value: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    validate_setting_value(&key, &value)?;
    storage
        .lock()
        .map_err(|_| "无法写入设置".to_string())?
        .set_setting(&key, &value)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_launcher_shortcut(
    shortcut: String,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    shortcut_status: tauri::State<'_, ShortcutStatusStore>,
) -> Result<ShortcutStatus, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法写入快捷键设置".to_string())?;

    apply_launcher_shortcut(&app, &storage, shortcut_status.inner(), &shortcut)
}

#[tauri::command]
fn set_ai_shortcut(
    shortcut: String,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    shortcut_status: tauri::State<'_, AiShortcutStatusStore>,
) -> Result<ShortcutStatus, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法写入 AI 快捷键设置".to_string())?;

    apply_ai_shortcut(&app, &storage, &shortcut_status.0, &shortcut)
}

#[tauri::command]
fn get_search_source_settings(
    storage: tauri::State<'_, StorageState>,
) -> Result<SearchSourceSettings, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法读取搜索源设置".to_string())?;

    read_search_source_settings(&storage)
}

#[tauri::command]
fn set_search_source_settings(
    settings: SearchSourceSettings,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法写入搜索源设置".to_string())?;

    write_bool_setting(&storage, "search.source.apps", settings.apps)?;
    write_bool_setting(&storage, "search.source.files", settings.files)?;
    write_bool_setting(&storage, "search.source.calculator", settings.calculator)?;
    write_bool_setting(&storage, "search.source.system", settings.system)?;
    write_bool_setting(&storage, "search.source.ai", settings.ai)?;
    write_bool_setting(&storage, "search.source.phrase", settings.phrase)?;
    write_bool_setting(&storage, "search.source.web_search", settings.web_search)?;
    write_bool_setting(&storage, "search.source.tools", settings.tools)?;

    Ok(())
}

#[tauri::command]
fn get_search_weight_settings(
    storage: tauri::State<'_, StorageState>,
) -> Result<SearchWeightSettings, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法读取搜索权重设置".to_string())?;

    read_search_weight_settings(&storage)
}

#[tauri::command]
fn set_search_weight_settings(
    settings: SearchWeightSettings,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法写入搜索权重设置".to_string())?;

    write_weight_setting(&storage, "search.weight.apps", settings.apps)?;
    write_weight_setting(&storage, "search.weight.files", settings.files)?;
    write_weight_setting(&storage, "search.weight.calculator", settings.calculator)?;
    write_weight_setting(&storage, "search.weight.system", settings.system)?;
    write_weight_setting(&storage, "search.weight.ai", settings.ai)?;
    write_weight_setting(&storage, "search.weight.phrase", settings.phrase)?;
    write_weight_setting(&storage, "search.weight.web_search", settings.web_search)?;
    write_weight_setting(&storage, "search.weight.tools", settings.tools)?;

    Ok(())
}

#[tauri::command]
fn get_password_options(
    storage: tauri::State<'_, StorageState>,
) -> Result<PasswordOptions, String> {
    let storage = storage.lock().map_err(|_| "无法读取密码设置".to_string())?;

    read_password_options(&storage)
}

#[tauri::command]
fn set_password_options(
    options: PasswordOptions,
    storage: tauri::State<'_, StorageState>,
) -> Result<PasswordOptions, String> {
    let options = sanitize_password_options(options);
    let storage = storage.lock().map_err(|_| "无法写入密码设置".to_string())?;

    storage
        .set_setting("tools.password.length", &options.length.to_string())
        .map_err(|error| error.to_string())?;
    write_bool_setting(&storage, "tools.password.uppercase", options.uppercase)?;
    write_bool_setting(&storage, "tools.password.lowercase", options.lowercase)?;
    write_bool_setting(&storage, "tools.password.digits", options.digits)?;
    write_bool_setting(&storage, "tools.password.hyphen", options.hyphen)?;
    write_bool_setting(&storage, "tools.password.underscore", options.underscore)?;
    write_bool_setting(&storage, "tools.password.special", options.special)?;
    write_bool_setting(&storage, "tools.password.brackets", options.brackets)?;

    Ok(options)
}

#[tauri::command]
fn list_custom_commands(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<CustomCommand>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取自定义命令".to_string())?
        .list_custom_commands()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_custom_command(
    input: CustomCommandInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<CustomCommand, String> {
    storage
        .lock()
        .map_err(|_| "无法保存自定义命令".to_string())?
        .upsert_custom_command(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_custom_command(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除自定义命令".to_string())?
        .delete_custom_command(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_phrases(storage: tauri::State<'_, StorageState>) -> Result<Vec<Phrase>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取快捷短语".to_string())?
        .list_phrases()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_phrase(
    input: PhraseInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<Phrase, String> {
    storage
        .lock()
        .map_err(|_| "无法保存快捷短语".to_string())?
        .upsert_phrase(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_phrase(id: String, storage: tauri::State<'_, StorageState>) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除快捷短语".to_string())?
        .delete_phrase(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_web_search_templates(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<WebSearchTemplate>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取网页搜索模板".to_string())?
        .list_web_search_templates()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_web_search_template(
    input: WebSearchTemplateInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<WebSearchTemplate, String> {
    storage
        .lock()
        .map_err(|_| "无法保存网页搜索模板".to_string())?
        .upsert_web_search_template(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_web_search_template(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除网页搜索模板".to_string())?
        .delete_web_search_template(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_exclusion_rules(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<ExclusionRule>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取排除规则".to_string())?
        .list_exclusion_rules()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_exclusion_rule(
    input: ExclusionRuleInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<ExclusionRule, String> {
    storage
        .lock()
        .map_err(|_| "无法保存排除规则".to_string())?
        .upsert_exclusion_rule(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_exclusion_rule(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除排除规则".to_string())?
        .delete_exclusion_rule(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_startup_enabled(
    enabled: bool,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法写入开机自启动设置".to_string())?;

    apply_startup_enabled(&app, &storage, enabled)
}

#[tauri::command]
fn export_config(storage: tauri::State<'_, StorageState>) -> Result<ConfigExportResult, String> {
    let storage = storage.lock().map_err(|_| "无法导出配置".to_string())?;
    let settings = storage
        .export_settings(EXPORTABLE_SETTING_KEYS)
        .map_err(|error| error.to_string())?;
    let custom_commands = storage
        .list_custom_commands()
        .map_err(|error| error.to_string())?;
    let phrases = storage.list_phrases().map_err(|error| error.to_string())?;
    let web_search_templates = storage
        .list_web_search_templates()
        .map_err(|error| error.to_string())?;
    let exclusion_rules = storage
        .list_exclusion_rules()
        .map_err(|error| error.to_string())?;
    let export = ConfigExport {
        version: 1,
        product: "Easy Launcher".into(),
        exported_at: current_timestamp(),
        settings,
        custom_commands,
        phrases,
        web_search_templates,
        exclusion_rules,
    };
    let export_dir = storage_export_dir(&storage)?;
    fs::create_dir_all(&export_dir).map_err(|error| format!("创建导出目录失败：{error}"))?;
    let path = export_dir.join(format!(
        "easy-launcher-config-{}.json",
        export.exported_at.replace([':', '.'], "-")
    ));
    let content = serde_json::to_string_pretty(&export)
        .map_err(|error| format!("序列化配置失败：{error}"))?;
    fs::write(&path, content).map_err(|error| format!("写入配置失败：{error}"))?;

    Ok(ConfigExportResult {
        path: path.display().to_string(),
        setting_count: export.settings.len(),
    })
}

#[tauri::command]
fn import_config(
    path: String,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    shortcut_status: tauri::State<'_, ShortcutStatusStore>,
) -> Result<ConfigImportResult, String> {
    let content =
        fs::read_to_string(path.trim()).map_err(|error| format!("读取配置文件失败：{error}"))?;
    let export: ConfigExport =
        serde_json::from_str(&content).map_err(|error| format!("配置 JSON 无效：{error}"))?;

    if export.version != 1 {
        return Err(format!("不支持的配置版本：{}", export.version));
    }

    let allowed_keys = EXPORTABLE_SETTING_KEYS
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let imported_shortcut = export.settings.get("launcher.shortcut").cloned();
    let storage = storage.lock().map_err(|_| "无法导入配置".to_string())?;
    let mut imported_count = 0;
    let mut ignored_count = 0;

    if let Some(shortcut) = imported_shortcut.as_deref() {
        apply_launcher_shortcut(&app, &storage, shortcut_status.inner(), shortcut)?;
        imported_count += 1;
    }

    for (key, value) in export.settings {
        if allowed_keys.contains(key.as_str()) {
            if key == "launcher.shortcut" {
                continue;
            }
            validate_import_setting(&key, &value)?;
            if key == "startup.enabled" {
                apply_startup_enabled(&app, &storage, value.trim() == "true")?;
                imported_count += 1;
                continue;
            }
            storage
                .set_setting(&key, value.trim())
                .map_err(|error| error.to_string())?;
            imported_count += 1;
        } else {
            ignored_count += 1;
        }
    }

    for command in export.custom_commands {
        storage
            .upsert_custom_command(CustomCommandInput {
                id: Some(command.id),
                name: command.name,
                command_type: command.command_type,
                target: command.target,
            })
            .map_err(|error| error.to_string())?;
        imported_count += 1;
    }

    for phrase in export.phrases {
        storage
            .upsert_phrase(PhraseInput {
                id: Some(phrase.id),
                title: phrase.title,
                text: phrase.text,
            })
            .map_err(|error| error.to_string())?;
        imported_count += 1;
    }

    for template in export.web_search_templates {
        storage
            .upsert_web_search_template(WebSearchTemplateInput {
                id: Some(template.id),
                keyword: template.keyword,
                name: template.name,
                url_template: template.url_template,
            })
            .map_err(|error| error.to_string())?;
        imported_count += 1;
    }

    for rule in export.exclusion_rules {
        storage
            .upsert_exclusion_rule(ExclusionRuleInput {
                id: Some(rule.id),
                match_type: rule.match_type,
                pattern: rule.pattern,
            })
            .map_err(|error| error.to_string())?;
        imported_count += 1;
    }

    Ok(ConfigImportResult {
        imported_count,
        ignored_count,
    })
}

#[tauri::command]
fn get_ai_config(storage: tauri::State<'_, StorageState>) -> Result<AiConfig, String> {
    let storage = storage.lock().map_err(|_| "无法读取 AI 配置".to_string())?;

    ai_config_from_translation_assistant(&storage)
}

#[tauri::command]
fn set_ai_config(config: AiConfig, storage: tauri::State<'_, StorageState>) -> Result<(), String> {
    let storage = storage.lock().map_err(|_| "无法写入 AI 配置".to_string())?;

    let profiles = storage
        .list_ai_model_profiles()
        .map_err(|error| error.to_string())?;
    let profile_index = profiles
        .iter()
        .position(|profile| profile.id == storage::DEFAULT_AI_MODEL_PROFILE_ID)
        .or(if profiles.is_empty() { None } else { Some(0) })
        .ok_or_else(|| "没有可写入的 AI 模型配置".to_string())?;
    let profile = &profiles[profile_index];
    storage
        .upsert_ai_model_profile(AiModelProfileInput {
            id: Some(profile.id.clone()),
            provider_type: profile.provider_type.clone(),
            name: profile.name.clone(),
            base_url: config.base_url.trim().into(),
            api_key: config.api_key.trim().into(),
            model_name: config.model.trim().into(),
            temperature: profile.temperature,
            top_p: profile.top_p,
            max_tokens: profile.max_tokens,
            presence_penalty: profile.presence_penalty,
            frequency_penalty: profile.frequency_penalty,
            stream: profile.stream,
            enabled: profile.enabled,
            sort_order: profile.sort_order,
        })
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
fn list_ai_model_profiles(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiModelProfile>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取 AI 模型配置".to_string())?
        .list_ai_model_profiles()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_model_profile(
    input: AiModelProfileInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiModelProfile, String> {
    storage
        .lock()
        .map_err(|_| "无法保存 AI 模型配置".to_string())?
        .upsert_ai_model_profile(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_ai_model_profile(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除 AI 模型配置".to_string())?
        .delete_ai_model_profile(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn test_ai_model_profile(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<String, String> {
    let profile = {
        let storage = storage
            .lock()
            .map_err(|_| "无法读取 AI 模型配置".to_string())?;
        storage
            .get_ai_model_profile(&id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "模型配置不存在".to_string())?
    };
    test_openai_compatible_profile(profile).await
}

#[tauri::command]
async fn list_ai_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    list_openai_compatible_models(&base_url, &api_key).await
}

#[tauri::command]
fn list_ai_providers(storage: tauri::State<'_, StorageState>) -> Result<Vec<AiProvider>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取 AI 供应商".to_string())?
        .list_ai_providers()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_provider(
    input: AiProviderInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiProvider, String> {
    storage
        .lock()
        .map_err(|_| "无法保存 AI 供应商".to_string())?
        .upsert_ai_provider(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_ai_provider(id: String, storage: tauri::State<'_, StorageState>) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除 AI 供应商".to_string())?
        .delete_ai_provider(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ai_provider_models(
    provider_id: Option<String>,
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiProviderModel>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取 AI 模型".to_string())?
        .list_ai_provider_models(provider_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_enabled_ai_provider_models(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiProviderModel>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取已启用 AI 模型".to_string())?
        .list_enabled_ai_provider_models()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn fetch_ai_provider_models(
    provider_id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiProviderModel>, String> {
    let provider = {
        let storage = storage
            .lock()
            .map_err(|_| "无法读取 AI 供应商".to_string())?;
        storage
            .get_ai_provider(&provider_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "供应商不存在".to_string())?
    };
    let fetched = list_openai_compatible_models(&provider.base_url, &provider.api_key).await?;
    let storage = storage
        .lock()
        .map_err(|_| "无法写入 AI 模型列表".to_string())?;
    let existing = storage
        .list_ai_provider_models(Some(&provider.id))
        .map_err(|error| error.to_string())?;
    for (index, model_name) in fetched.iter().enumerate() {
        let existing_model = existing
            .iter()
            .find(|model| model.model_name == *model_name);
        storage
            .upsert_ai_provider_model(AiProviderModelInput {
                id: existing_model.map(|model| model.id.clone()),
                provider_id: provider.id.clone(),
                model_name: model_name.clone(),
                enabled: existing_model.map(|model| model.enabled).unwrap_or(false),
                sort_order: existing_model
                    .map(|model| model.sort_order)
                    .unwrap_or(index as i64),
            })
            .map_err(|error| error.to_string())?;
    }
    storage
        .list_ai_provider_models(Some(&provider.id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_provider_model(
    input: AiProviderModelInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiProviderModel, String> {
    storage
        .lock()
        .map_err(|_| "无法保存 AI 模型".to_string())?
        .upsert_ai_provider_model(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_ai_provider_model_enabled(
    provider_id: String,
    model_name: String,
    enabled: bool,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiProviderModel, String> {
    let storage = storage.lock().map_err(|_| "无法更新 AI 模型".to_string())?;
    let existing = storage
        .get_ai_provider_model(&provider_id, &model_name)
        .map_err(|error| error.to_string())?;
    storage
        .upsert_ai_provider_model(AiProviderModelInput {
            id: existing.as_ref().map(|model| model.id.clone()),
            provider_id,
            model_name,
            enabled,
            sort_order: existing.map(|model| model.sort_order).unwrap_or(0),
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ai_assistants(storage: tauri::State<'_, StorageState>) -> Result<Vec<AiAssistant>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取 AI 助手".to_string())?
        .list_ai_assistants()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_assistant(
    input: AiAssistantInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiAssistant, String> {
    storage
        .lock()
        .map_err(|_| "无法保存 AI 助手".to_string())?
        .upsert_ai_assistant(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_ai_assistant_model(
    assistant_id: String,
    provider_id: String,
    model_name: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiAssistant, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法更新 AI 助手模型".to_string())?;
    let assistant = storage
        .get_ai_assistant(&assistant_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "助手不存在".to_string())?;
    let provider = storage
        .get_ai_provider(&provider_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "供应商不存在".to_string())?;
    let model = storage
        .get_ai_provider_model(&provider.id, &model_name)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "模型不存在".to_string())?;
    if !provider.enabled || !model.enabled {
        return Err("请先在 AI / 供应商模型 中启用这个模型".into());
    }
    let existing_profile = storage
        .list_ai_model_profiles()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|profile| {
            profile.base_url.trim() == provider.base_url.trim()
                && profile.model_name == model.model_name
        });
    let profile = storage
        .upsert_ai_model_profile(AiModelProfileInput {
            id: Some(
                existing_profile
                    .as_ref()
                    .map(|profile| profile.id.clone())
                    .unwrap_or_else(|| {
                        format!(
                            "ai-model-profile:{}:{}",
                            provider.id,
                            urlencoding::encode(&model.model_name)
                        )
                    }),
            ),
            provider_type: provider.provider_type.clone(),
            name: format!("{} / {}", provider.name, model.model_name),
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
            model_name: model.model_name.clone(),
            temperature: existing_profile
                .as_ref()
                .and_then(|profile| profile.temperature),
            top_p: existing_profile.as_ref().and_then(|profile| profile.top_p),
            max_tokens: existing_profile
                .as_ref()
                .and_then(|profile| profile.max_tokens),
            presence_penalty: existing_profile
                .as_ref()
                .and_then(|profile| profile.presence_penalty),
            frequency_penalty: existing_profile
                .as_ref()
                .and_then(|profile| profile.frequency_penalty),
            stream: existing_profile
                .as_ref()
                .map(|profile| profile.stream)
                .unwrap_or(true),
            enabled: true,
            sort_order: model.sort_order,
        })
        .map_err(|error| error.to_string())?;

    storage
        .upsert_ai_assistant(AiAssistantInput {
            id: Some(assistant.id.clone()),
            name: assistant.name,
            icon: assistant.icon,
            description: assistant.description,
            model_profile_id: profile.id,
            system_prompt: assistant.system_prompt,
            enabled: assistant.enabled,
            sort_order: assistant.sort_order,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_ai_assistant(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除 AI 助手".to_string())?
        .delete_ai_assistant(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ai_selection_actions(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiSelectionAction>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取划词助手".to_string())?
        .list_ai_selection_actions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_visible_ai_selection_actions(
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiSelectionAction>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取划词助手".to_string())?
        .list_visible_ai_selection_actions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_ai_selection_action(
    input: AiSelectionActionInput,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiSelectionAction, String> {
    storage
        .lock()
        .map_err(|_| "无法保存划词助手设置".to_string())?
        .upsert_ai_selection_action(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_ai_selection_action_model(
    assistant_id: String,
    provider_id: String,
    model_name: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiSelectionAction, String> {
    storage
        .lock()
        .map_err(|_| "无法保存划词模型".to_string())?
        .set_ai_selection_action_model(&assistant_id, &provider_id, &model_name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn mark_ai_assistant_used(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    storage
        .lock()
        .map_err(|_| "无法更新 AI 助手".to_string())?
        .mark_ai_assistant_used(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ai_conversations(
    assistant_id: Option<String>,
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiConversation>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取 AI 会话".to_string())?
        .list_ai_conversations(assistant_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_ai_conversation(
    assistant_id: String,
    title: Option<String>,
    storage: tauri::State<'_, StorageState>,
) -> Result<AiConversation, String> {
    storage
        .lock()
        .map_err(|_| "无法创建 AI 会话".to_string())?
        .create_ai_conversation(&assistant_id, title.as_deref().unwrap_or(""))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_ai_conversation(
    id: String,
    title: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    storage
        .lock()
        .map_err(|_| "无法重命名 AI 会话".to_string())?
        .rename_ai_conversation(&id, &title)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_ai_conversation(
    id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除 AI 会话".to_string())?
        .delete_ai_conversation(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_ai_messages(
    conversation_id: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<Vec<AiMessage>, String> {
    storage
        .lock()
        .map_err(|_| "无法读取 AI 消息".to_string())?
        .list_ai_messages(&conversation_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_ai_message(id: String, storage: tauri::State<'_, StorageState>) -> Result<bool, String> {
    storage
        .lock()
        .map_err(|_| "无法删除 AI 消息".to_string())?
        .delete_ai_message(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn send_ai_chat_message(
    request: AiChatSendRequest,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    ai_cancels: tauri::State<'_, AiCancelMap>,
) -> Result<AiChatStarted, String> {
    let text = request.message.trim();
    if text.is_empty() {
        return Err("消息不能为空".into());
    }

    let (assistant, profile, conversation, previous_messages, user_message, assistant_message) = {
        let storage = storage.lock().map_err(|_| "无法准备 AI 聊天".to_string())?;
        let assistant = storage
            .get_ai_assistant(&request.assistant_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "助手不存在".to_string())?;
        if !assistant.enabled {
            return Err("助手已禁用".into());
        }
        let profile = storage
            .get_ai_model_profile(&assistant.model_profile_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "助手绑定的模型配置不存在".to_string())?;
        let conversation = if let Some(conversation_id) = request.conversation_id.as_deref() {
            storage
                .get_ai_conversation(conversation_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "会话不存在".to_string())?
        } else {
            storage
                .create_ai_conversation(&assistant.id, "")
                .map_err(|error| error.to_string())?
        };
        let previous_messages = storage
            .list_ai_messages(&conversation.id)
            .map_err(|error| error.to_string())?;
        let user_message = storage
            .insert_ai_message(&conversation.id, "user", text, "complete", None)
            .map_err(|error| error.to_string())?;
        let assistant_message = storage
            .insert_ai_message(&conversation.id, "assistant", "", "streaming", None)
            .map_err(|error| error.to_string())?;
        storage
            .mark_ai_assistant_used(&assistant.id)
            .map_err(|error| error.to_string())?;
        (
            assistant,
            profile,
            conversation,
            previous_messages,
            user_message,
            assistant_message,
        )
    };

    let params = merge_ai_params(&assistant, &profile);
    let messages = build_ai_request_messages(&assistant, &previous_messages, text);
    let cancelled = Arc::new(AtomicBool::new(false));
    ai_cancels
        .lock()
        .map_err(|_| "无法记录 AI 请求状态".to_string())?
        .insert(request.request_id.clone(), cancelled.clone());

    let storage = storage.inner().clone();
    let ai_cancels = ai_cancels.inner().clone();
    let request_id = request.request_id.clone();
    let conversation_id = conversation.id.clone();
    let message_id = assistant_message.id.clone();
    tauri::async_runtime::spawn(async move {
        let delta_app = app.clone();
        let delta_request_id = request_id.clone();
        let delta_conversation_id = conversation_id.clone();
        let delta_message_id = message_id.clone();
        let result = send_openai_compatible_chat(
            &profile,
            &params,
            &messages,
            cancelled.clone(),
            move |delta| {
                let _ = delta_app.emit(
                    AI_CHAT_DELTA_EVENT,
                    AiChatDeltaEvent {
                        request_id: delta_request_id.clone(),
                        conversation_id: delta_conversation_id.clone(),
                        message_id: delta_message_id.clone(),
                        delta,
                    },
                );
            },
        )
        .await;

        if let Ok(mut cancels) = ai_cancels.lock() {
            cancels.remove(&request_id);
        }

        match result {
            Ok(result) if result.cancelled => {
                if let Ok(storage) = storage.lock() {
                    let _ = storage.update_ai_message_status(
                        &message_id,
                        &result.content,
                        "error",
                        Some("AI 请求已取消"),
                    );
                }
                let _ = app.emit(
                    AI_CHAT_CANCELLED_EVENT,
                    AiChatCancelledEvent {
                        request_id,
                        conversation_id,
                        message_id,
                    },
                );
            }
            Ok(result) => {
                if let Ok(storage) = storage.lock() {
                    let _ = storage.update_ai_message_status(
                        &message_id,
                        &result.content,
                        "complete",
                        None,
                    );
                }
                let _ = app.emit(
                    AI_CHAT_DONE_EVENT,
                    AiChatDoneEvent {
                        request_id,
                        conversation_id,
                        message_id,
                        content: result.content,
                    },
                );
            }
            Err(error) => {
                if let Ok(storage) = storage.lock() {
                    let _ =
                        storage.update_ai_message_status(&message_id, "", "error", Some(&error));
                }
                let _ = app.emit(
                    AI_CHAT_ERROR_EVENT,
                    AiChatErrorEvent {
                        request_id,
                        conversation_id,
                        message_id,
                        error,
                    },
                );
            }
        }
    });

    Ok(AiChatStarted {
        request_id: request.request_id,
        conversation_id: conversation.id,
        user_message,
        assistant_message,
    })
}

#[tauri::command]
fn send_ai_selection_message(
    request: AiSelectionChatRequest,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    ai_cancels: tauri::State<'_, AiCancelMap>,
) -> Result<AiChatStarted, String> {
    let is_initial = request.conversation_id.is_none();
    let text = if is_initial {
        request.selection_text.trim()
    } else {
        request.message.as_deref().unwrap_or_default().trim()
    };
    if text.is_empty() {
        return Err("消息不能为空".into());
    }

    let (assistant, profile, conversation, previous_messages, user_message, assistant_message) = {
        let storage = storage
            .lock()
            .map_err(|_| "无法准备划词 AI 请求".to_string())?;
        let assistant = storage
            .get_ai_assistant(&request.assistant_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "划词助手不存在".to_string())?;
        if !assistant.enabled {
            return Err("划词助手已禁用".into());
        }
        let action = storage
            .get_ai_selection_action(&assistant.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "划词助手设置不存在".to_string())?;
        let provider = storage
            .get_ai_provider(&request.provider_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "供应商不存在".to_string())?;
        let model = storage
            .get_ai_provider_model(&provider.id, &request.model_name)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "模型不存在".to_string())?;
        if !provider.enabled || !model.enabled {
            return Err("请先在 AI / 供应商模型 中启用至少一个模型".into());
        }

        let conversation = if let Some(conversation_id) = request.conversation_id.as_deref() {
            let conversation = storage
                .get_ai_conversation(conversation_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "会话不存在".to_string())?;
            if conversation.assistant_id != assistant.id {
                return Err("会话不属于当前划词助手".into());
            }
            conversation
        } else {
            let title =
                selection_conversation_title(&action.selection_label, &request.selection_text);
            storage
                .create_ai_conversation(&assistant.id, &title)
                .map_err(|error| error.to_string())?
        };
        let previous_messages = storage
            .list_ai_messages(&conversation.id)
            .map_err(|error| error.to_string())?;
        let user_message = storage
            .insert_ai_message(&conversation.id, "user", text, "complete", None)
            .map_err(|error| error.to_string())?;
        let assistant_message = storage
            .insert_ai_message(&conversation.id, "assistant", "", "streaming", None)
            .map_err(|error| error.to_string())?;
        storage
            .mark_ai_provider_model_used(&provider.id, &model.model_name)
            .map_err(|error| error.to_string())?;
        storage
            .mark_ai_assistant_used(&assistant.id)
            .map_err(|error| error.to_string())?;

        (
            assistant,
            provider_model_profile(&provider, &model),
            conversation,
            previous_messages,
            user_message,
            assistant_message,
        )
    };

    let params = merge_ai_params(&assistant, &profile);
    let messages = build_ai_request_messages(&assistant, &previous_messages, text);
    let cancelled = Arc::new(AtomicBool::new(false));
    ai_cancels
        .lock()
        .map_err(|_| "无法记录 AI 请求状态".to_string())?
        .insert(request.request_id.clone(), cancelled.clone());

    let storage = storage.inner().clone();
    let ai_cancels = ai_cancels.inner().clone();
    let request_id = request.request_id.clone();
    let conversation_id = conversation.id.clone();
    let message_id = assistant_message.id.clone();
    tauri::async_runtime::spawn(async move {
        let delta_app = app.clone();
        let delta_request_id = request_id.clone();
        let delta_conversation_id = conversation_id.clone();
        let delta_message_id = message_id.clone();
        let result = send_openai_compatible_chat(
            &profile,
            &params,
            &messages,
            cancelled.clone(),
            move |delta| {
                let _ = delta_app.emit(
                    AI_CHAT_DELTA_EVENT,
                    AiChatDeltaEvent {
                        request_id: delta_request_id.clone(),
                        conversation_id: delta_conversation_id.clone(),
                        message_id: delta_message_id.clone(),
                        delta,
                    },
                );
            },
        )
        .await;

        if let Ok(mut cancels) = ai_cancels.lock() {
            cancels.remove(&request_id);
        }

        match result {
            Ok(result) if result.cancelled => {
                if let Ok(storage) = storage.lock() {
                    let _ = storage.update_ai_message_status(
                        &message_id,
                        &result.content,
                        "error",
                        Some("AI 请求已取消"),
                    );
                }
                let _ = app.emit(
                    AI_CHAT_CANCELLED_EVENT,
                    AiChatCancelledEvent {
                        request_id,
                        conversation_id,
                        message_id,
                    },
                );
            }
            Ok(result) => {
                if let Ok(storage) = storage.lock() {
                    let _ = storage.update_ai_message_status(
                        &message_id,
                        &result.content,
                        "complete",
                        None,
                    );
                }
                let _ = app.emit(
                    AI_CHAT_DONE_EVENT,
                    AiChatDoneEvent {
                        request_id,
                        conversation_id,
                        message_id,
                        content: result.content,
                    },
                );
            }
            Err(error) => {
                if let Ok(storage) = storage.lock() {
                    let _ =
                        storage.update_ai_message_status(&message_id, "", "error", Some(&error));
                }
                let _ = app.emit(
                    AI_CHAT_ERROR_EVENT,
                    AiChatErrorEvent {
                        request_id,
                        conversation_id,
                        message_id,
                        error,
                    },
                );
            }
        }
    });

    Ok(AiChatStarted {
        request_id: request.request_id,
        conversation_id: conversation.id,
        user_message,
        assistant_message,
    })
}

#[tauri::command]
fn cancel_ai_chat_message(
    request_id: String,
    ai_cancels: tauri::State<'_, AiCancelMap>,
) -> Result<(), String> {
    cancel_ai_request(request_id, ai_cancels)
}

#[tauri::command]
async fn run_ai_action(
    request: AiRequest,
    storage: tauri::State<'_, StorageState>,
) -> Result<String, String> {
    let config = get_ai_config(storage)?;
    run_ai_action_inner(config, request).await
}

#[tauri::command]
fn run_ai_action_stream(
    request_id: String,
    request: AiRequest,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    ai_cancels: tauri::State<'_, AiCancelMap>,
) -> Result<(), String> {
    let config = get_ai_config(storage)?;
    let cancelled = Arc::new(AtomicBool::new(false));

    ai_cancels
        .lock()
        .map_err(|_| "无法记录 AI 请求状态".to_string())?
        .insert(request_id.clone(), cancelled.clone());

    let ai_cancels = ai_cancels.inner().clone();
    tauri::async_runtime::spawn(async move {
        let request_id_for_cleanup = request_id.clone();
        run_ai_action_stream_inner(request_id, config, request, cancelled, move |event| {
            let _ = app.emit(AI_STREAM_EVENT, event);
        })
        .await;

        if let Ok(mut cancels) = ai_cancels.lock() {
            cancels.remove(&request_id_for_cleanup);
        }
    });

    Ok(())
}

#[tauri::command]
fn cancel_ai_action(
    request_id: String,
    ai_cancels: tauri::State<'_, AiCancelMap>,
) -> Result<(), String> {
    cancel_ai_request(request_id, ai_cancels)
}

fn cancel_ai_request(
    request_id: String,
    ai_cancels: tauri::State<'_, AiCancelMap>,
) -> Result<(), String> {
    let cancels = ai_cancels
        .lock()
        .map_err(|_| "无法读取 AI 请求状态".to_string())?;

    if let Some(cancelled) = cancels.get(&request_id) {
        cancelled.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err("没有正在运行的 AI 请求".into())
    }
}

fn provider_model_profile(provider: &AiProvider, model: &AiProviderModel) -> AiModelProfile {
    AiModelProfile {
        id: format!("{}:{}", provider.id, model.model_name),
        provider_type: provider.provider_type.clone(),
        name: provider.name.clone(),
        base_url: provider.base_url.clone(),
        api_key: provider.api_key.clone(),
        model_name: model.model_name.clone(),
        temperature: None,
        top_p: None,
        max_tokens: None,
        presence_penalty: None,
        frequency_penalty: None,
        stream: true,
        enabled: provider.enabled && model.enabled,
        sort_order: model.sort_order,
        last_used_at: model.last_used_at.clone(),
        created_at: model.created_at.clone(),
        updated_at: model.updated_at.clone(),
    }
}

fn ai_config_from_translation_assistant(storage: &Storage) -> Result<AiConfig, String> {
    let assistant = storage
        .get_ai_assistant(TRANSLATION_AI_ASSISTANT_ID)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "翻译助手不存在".to_string())?;
    let profile = storage
        .get_ai_model_profile(&assistant.model_profile_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "翻译助手绑定的模型配置不存在".to_string())?;

    Ok(AiConfig {
        base_url: profile.base_url,
        api_key: profile.api_key,
        model: profile.model_name,
        system_prompt: assistant.system_prompt,
    })
}

#[tauri::command]
fn everything_status(storage: tauri::State<'_, StorageState>) -> Result<EverythingStatus, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法读取 Everything 设置".to_string())?;
    Ok(detect_everything_status(
        storage
            .get_setting("everything.exe.path")
            .map_err(|error| error.to_string())?
            .as_deref(),
    ))
}

#[tauri::command]
fn set_everything_exe_path(
    path: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<EverythingStatus, String> {
    let path = path.trim().to_string();
    validate_everything_exe_path(&path)?;
    let storage = storage
        .lock()
        .map_err(|_| "无法保存 Everything 路径".to_string())?;
    storage
        .set_setting("everything.exe.path", &path)
        .map_err(|error| error.to_string())?;
    Ok(detect_everything_status(Some(&path)))
}

#[tauri::command]
fn get_everything_search_options(
    storage: tauri::State<'_, StorageState>,
) -> Result<EverythingSearchOptions, String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法读取 Everything 设置".to_string())?;
    read_everything_search_options(&storage)
}

#[tauri::command]
fn set_everything_search_options(
    options: EverythingSearchOptions,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    let storage = storage
        .lock()
        .map_err(|_| "无法保存 Everything 设置".to_string())?;
    write_bool_setting(&storage, "everything.search.full_path", options.full_path)?;
    write_bool_setting(
        &storage,
        "everything.search.content",
        options.search_content,
    )?;
    Ok(())
}

#[tauri::command]
fn search(query: String) -> Vec<SearchResult> {
    let enabled_sources = default_enabled_sources();
    let route = parse_action_keyword_query(&query);
    let enabled_sources = apply_action_keyword_route(&enabled_sources, &route);
    let password_options = PasswordOptions::default();
    let execution = default_search_core().search_with_diagnostics(
        &route.query,
        &SearchContext {
            recent_scores: None,
            query_selection_scores: None,
            custom_commands: None,
            phrases: None,
            web_search_templates: None,
            password_options: Some(&password_options),
            exclusion_rules: None,
            source_weights: None,
            enabled_sources: &enabled_sources,
            everything_options: None,
        },
        SEARCH_RESULT_LIMIT,
        false,
    );
    log_search_diagnostics(&execution.diagnostics);
    execution.results
}

#[tauri::command]
async fn search_with_recents(
    query: String,
    request_id: Option<u64>,
    app: AppHandle,
    storage: tauri::State<'_, StorageState>,
    search_requests: tauri::State<'_, SearchRequestTracker>,
    search_cache: tauri::State<'_, SearchResultCacheStore>,
) -> Result<Vec<SearchResult>, String> {
    let storage = storage.inner().clone();
    let search_requests = search_requests.inner().clone();
    let search_cache = search_cache.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        search_with_recents_blocking(
            query,
            request_id,
            app,
            storage,
            search_requests,
            search_cache,
        )
    })
    .await
    .map_err(|error| format!("搜索任务失败：{error}"))?
}

fn search_with_recents_blocking(
    query: String,
    request_id: Option<u64>,
    app: AppHandle,
    storage: StorageState,
    search_requests: SearchRequestTracker,
    search_cache: SearchResultCacheStore,
) -> Result<Vec<SearchResult>, String> {
    let request_id = register_search_request(&search_requests, request_id);
    if !is_current_search_request(&search_requests, request_id) {
        return Ok(Vec::new());
    }

    let route = parse_action_keyword_query(&query);
    let mut routed_query = route.query.clone();
    let (
        recent_scores,
        query_selection_scores,
        search_source_settings,
        search_weight_settings,
        everything_options,
        password_options,
        tool_menu_alias,
        custom_commands,
        phrases,
        web_search_templates,
        exclusion_rules,
    ) = {
        let storage = storage.lock().map_err(|_| "无法读取搜索数据".to_string())?;

        let recent_scores = storage.recent_scores().map_err(|error| error.to_string())?;
        let normalized_selection_query = normalize_selection_query(&routed_query);
        let query_selection_scores = storage
            .query_selection_scores(&normalized_selection_query)
            .map_err(|error| error.to_string())?;
        let search_source_settings = read_search_source_settings(&storage)?;
        let search_weight_settings = read_search_weight_settings(&storage)?;
        let everything_options = read_everything_search_options(&storage)?;
        let password_options = read_password_options(&storage)?;
        let tool_menu_alias = read_tool_menu_alias(&storage)?;
        let custom_commands = if search_source_settings.system {
            storage
                .list_custom_commands()
                .map_err(|error| error.to_string())?
        } else {
            Vec::new()
        };
        let phrases = if search_source_settings.phrase {
            storage.list_phrases().map_err(|error| error.to_string())?
        } else {
            Vec::new()
        };
        let web_search_templates = if search_source_settings.web_search {
            storage
                .list_web_search_templates()
                .map_err(|error| error.to_string())?
        } else {
            Vec::new()
        };
        let exclusion_rules = storage
            .list_exclusion_rules()
            .map_err(|error| error.to_string())?;

        (
            recent_scores,
            query_selection_scores,
            search_source_settings,
            search_weight_settings,
            everything_options,
            password_options,
            tool_menu_alias,
            custom_commands,
            phrases,
            web_search_templates,
            exclusion_rules,
        )
    };

    if !is_current_search_request(&search_requests, request_id) {
        return Ok(Vec::new());
    }

    let enabled_sources = apply_action_keyword_route(
        &enabled_sources_from_settings(&search_source_settings),
        &route,
    );
    let source_weights = source_weights_from_settings(&search_weight_settings);
    routed_query = normalize_tool_menu_query(&routed_query, &tool_menu_alias);
    let bypass_cache = is_tool_query(&routed_query);
    let cache_key = search_cache_key(
        &routed_query,
        &enabled_sources,
        &search_weight_settings,
        &everything_options,
        &recent_scores,
        &query_selection_scores,
        &custom_commands,
        &phrases,
        &web_search_templates,
        &exclusion_rules,
    );

    if request_id == 0 {
        let execution = default_search_core().search_with_cancellation(
            &routed_query,
            &SearchContext {
                recent_scores: Some(&recent_scores),
                query_selection_scores: Some(&query_selection_scores),
                custom_commands: Some(&custom_commands),
                phrases: Some(&phrases),
                web_search_templates: Some(&web_search_templates),
                password_options: Some(&password_options),
                exclusion_rules: Some(&exclusion_rules),
                source_weights: Some(&source_weights),
                enabled_sources: &enabled_sources,
                everything_options: Some(&everything_options),
            },
            SEARCH_RESULT_LIMIT,
            false,
            || false,
        );
        log_search_diagnostics(&execution.diagnostics);

        return Ok(execution.results);
    }

    if bypass_cache {
        let execution = default_search_core().search_with_cancellation(
            &routed_query,
            &SearchContext {
                recent_scores: Some(&recent_scores),
                query_selection_scores: Some(&query_selection_scores),
                custom_commands: Some(&custom_commands),
                phrases: Some(&phrases),
                web_search_templates: Some(&web_search_templates),
                password_options: Some(&password_options),
                exclusion_rules: Some(&exclusion_rules),
                source_weights: Some(&source_weights),
                enabled_sources: &enabled_sources,
                everything_options: Some(&everything_options),
            },
            SEARCH_RESULT_LIMIT,
            false,
            || !is_current_search_request(&search_requests, request_id),
        );
        log_search_diagnostics(&execution.diagnostics);
        return Ok(execution.results);
    }

    let prioritize_files = should_prioritize_file_search(&routed_query, &route, &enabled_sources);

    if let Some(cached) = read_cached_search_results(&search_cache, &cache_key) {
        if prioritize_files && !cached.complete {
            return run_complete_search_with_cache(
                &routed_query,
                request_id,
                &search_requests,
                &search_cache,
                cache_key,
                recent_scores,
                query_selection_scores,
                custom_commands,
                phrases,
                web_search_templates,
                password_options,
                exclusion_rules,
                everything_options,
                source_weights,
                enabled_sources,
            );
        }

        let tier = if cached.complete {
            "cache-complete"
        } else {
            "cache-fast"
        };
        let diagnostics = cached_search_diagnostics(&routed_query, cached.results.len(), tier);
        log_search_diagnostics(&diagnostics);

        if !cached.complete && is_current_search_request(&search_requests, request_id) {
            spawn_slow_search_progress(
                app,
                routed_query,
                request_id,
                search_requests,
                search_cache,
                cache_key,
                recent_scores,
                query_selection_scores,
                custom_commands,
                phrases,
                web_search_templates,
                password_options,
                exclusion_rules,
                everything_options,
                source_weights,
                enabled_sources,
                cached.results.clone(),
            );
        }

        return Ok(cached.results);
    }

    if prioritize_files {
        return run_complete_search_with_cache(
            &routed_query,
            request_id,
            &search_requests,
            &search_cache,
            cache_key,
            recent_scores,
            query_selection_scores,
            custom_commands,
            phrases,
            web_search_templates,
            password_options,
            exclusion_rules,
            everything_options,
            source_weights,
            enabled_sources,
        );
    }

    let execution = default_search_core().search_tier_with_cancellation(
        ProviderTier::Fast,
        &routed_query,
        &SearchContext {
            recent_scores: Some(&recent_scores),
            query_selection_scores: Some(&query_selection_scores),
            custom_commands: Some(&custom_commands),
            phrases: Some(&phrases),
            web_search_templates: Some(&web_search_templates),
            password_options: Some(&password_options),
            exclusion_rules: Some(&exclusion_rules),
            source_weights: Some(&source_weights),
            enabled_sources: &enabled_sources,
            everything_options: Some(&everything_options),
        },
        SEARCH_RESULT_LIMIT,
        false,
        || !is_current_search_request(&search_requests, request_id),
    );
    log_search_diagnostics(&execution.diagnostics);
    let fast_results = execution.results.clone();

    if !execution.diagnostics.cancelled && is_current_search_request(&search_requests, request_id) {
        write_cached_search_results(
            &search_cache,
            cache_key.clone(),
            fast_results.clone(),
            false,
        );
        spawn_slow_search_progress(
            app,
            routed_query,
            request_id,
            search_requests,
            search_cache,
            cache_key,
            recent_scores,
            query_selection_scores,
            custom_commands,
            phrases,
            web_search_templates,
            password_options,
            exclusion_rules,
            everything_options,
            source_weights,
            enabled_sources,
            fast_results.clone(),
        );
    }

    Ok(execution.results)
}

fn run_complete_search_with_cache(
    query: &str,
    request_id: u64,
    search_requests: &SearchRequestTracker,
    search_cache: &SearchResultCacheStore,
    cache_key: String,
    recent_scores: HashMap<String, f32>,
    query_selection_scores: HashMap<String, f32>,
    custom_commands: Vec<CustomCommand>,
    phrases: Vec<Phrase>,
    web_search_templates: Vec<WebSearchTemplate>,
    password_options: PasswordOptions,
    exclusion_rules: Vec<ExclusionRule>,
    everything_options: EverythingSearchOptions,
    source_weights: HashMap<SearchSource, f32>,
    enabled_sources: std::collections::HashSet<SearchSource>,
) -> Result<Vec<SearchResult>, String> {
    let execution = default_search_core().search_with_cancellation(
        query,
        &SearchContext {
            recent_scores: Some(&recent_scores),
            query_selection_scores: Some(&query_selection_scores),
            custom_commands: Some(&custom_commands),
            phrases: Some(&phrases),
            web_search_templates: Some(&web_search_templates),
            password_options: Some(&password_options),
            exclusion_rules: Some(&exclusion_rules),
            source_weights: Some(&source_weights),
            enabled_sources: &enabled_sources,
            everything_options: Some(&everything_options),
        },
        SEARCH_RESULT_LIMIT,
        false,
        || !is_current_search_request(search_requests, request_id),
    );
    log_search_diagnostics(&execution.diagnostics);

    if !execution.diagnostics.cancelled && is_current_search_request(search_requests, request_id) {
        write_cached_search_results(search_cache, cache_key, execution.results.clone(), true);
    }

    Ok(execution.results)
}

fn spawn_slow_search_progress(
    app: AppHandle,
    query: String,
    request_id: u64,
    search_requests: SearchRequestTracker,
    search_cache: SearchResultCacheStore,
    cache_key: String,
    recent_scores: HashMap<String, f32>,
    query_selection_scores: HashMap<String, f32>,
    custom_commands: Vec<CustomCommand>,
    phrases: Vec<Phrase>,
    web_search_templates: Vec<WebSearchTemplate>,
    password_options: PasswordOptions,
    exclusion_rules: Vec<ExclusionRule>,
    everything_options: EverythingSearchOptions,
    source_weights: HashMap<SearchSource, f32>,
    enabled_sources: std::collections::HashSet<SearchSource>,
    fast_results: Vec<SearchResult>,
) {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_current_search_request(&search_requests, request_id) {
            return;
        }

        let execution = default_search_core().search_tier_with_cancellation(
            ProviderTier::Slow,
            &query,
            &SearchContext {
                recent_scores: Some(&recent_scores),
                query_selection_scores: Some(&query_selection_scores),
                custom_commands: Some(&custom_commands),
                phrases: Some(&phrases),
                web_search_templates: Some(&web_search_templates),
                password_options: Some(&password_options),
                exclusion_rules: Some(&exclusion_rules),
                source_weights: Some(&source_weights),
                enabled_sources: &enabled_sources,
                everything_options: Some(&everything_options),
            },
            SEARCH_RESULT_LIMIT,
            false,
            || !is_current_search_request(&search_requests, request_id),
        );
        log_search_diagnostics(&execution.diagnostics);

        if execution.diagnostics.cancelled
            || execution.results.is_empty()
            || !is_current_search_request(&search_requests, request_id)
        {
            return;
        }

        let mut results = fast_results;
        results.extend(execution.results.clone());
        let results = merge_ranked_results(results, SEARCH_RESULT_LIMIT);
        write_cached_search_results(&search_cache, cache_key, results.clone(), true);

        let _ = app.emit(
            SEARCH_PROGRESS_EVENT,
            SearchProgressEvent {
                request_id,
                results,
                diagnostics: execution.diagnostics,
            },
        );
    });
}

fn read_cached_search_results(
    cache: &SearchResultCacheStore,
    key: &str,
) -> Option<CachedSearchResults> {
    cache.lock().ok()?.get(key)
}

fn write_cached_search_results(
    cache: &SearchResultCacheStore,
    key: String,
    results: Vec<SearchResult>,
    complete: bool,
) {
    if let Ok(mut cache) = cache.lock() {
        cache.insert(key, CachedSearchResults { results, complete });
    }
}

fn search_cache_key(
    query: &str,
    sources: &std::collections::HashSet<SearchSource>,
    weights: &SearchWeightSettings,
    everything_options: &EverythingSearchOptions,
    recent_scores: &HashMap<String, f32>,
    query_selection_scores: &HashMap<String, f32>,
    custom_commands: &[CustomCommand],
    phrases: &[Phrase],
    web_search_templates: &[WebSearchTemplate],
    exclusion_rules: &[ExclusionRule],
) -> String {
    format!(
        "q={}|src={}|w={}|everything={}|recent={}|learned={}|cmd={}|phrase={}|web={}|exclude={}",
        query.trim().to_lowercase(),
        search_source_signature(sources),
        search_weight_signature(weights),
        everything_options_signature(everything_options),
        recent_scores_signature(recent_scores),
        query_selection_scores_signature(query_selection_scores),
        custom_commands_signature(custom_commands),
        phrases_signature(phrases),
        web_search_templates_signature(web_search_templates),
        exclusion_rules_signature(exclusion_rules)
    )
}

fn search_source_signature(sources: &std::collections::HashSet<SearchSource>) -> String {
    format!(
        "{}{}{}{}{}{}{}{}",
        bool_digit(sources.contains(&SearchSource::Apps)),
        bool_digit(sources.contains(&SearchSource::Files)),
        bool_digit(sources.contains(&SearchSource::Calculator)),
        bool_digit(sources.contains(&SearchSource::System)),
        bool_digit(sources.contains(&SearchSource::Ai)),
        bool_digit(sources.contains(&SearchSource::Phrase)),
        bool_digit(sources.contains(&SearchSource::WebSearch)),
        bool_digit(sources.contains(&SearchSource::Tools))
    )
}

fn search_weight_signature(settings: &SearchWeightSettings) -> String {
    format!(
        "{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
        settings.apps,
        settings.files,
        settings.calculator,
        settings.system,
        settings.ai,
        settings.phrase,
        settings.web_search,
        settings.tools
    )
}

fn everything_options_signature(options: &EverythingSearchOptions) -> String {
    format!(
        "path={},content={}",
        bool_digit(options.full_path),
        bool_digit(options.search_content)
    )
}

fn recent_scores_signature(scores: &HashMap<String, f32>) -> String {
    let mut entries = scores
        .iter()
        .map(|(id, score)| format!("{id}:{score:.3}"))
        .collect::<Vec<_>>();
    entries.sort();
    entries.join("|")
}

fn query_selection_scores_signature(scores: &HashMap<String, f32>) -> String {
    recent_scores_signature(scores)
}

fn custom_commands_signature(commands: &[CustomCommand]) -> String {
    commands
        .iter()
        .map(|command| format!("{}:{}", command.id, command.updated_at))
        .collect::<Vec<_>>()
        .join("|")
}

fn phrases_signature(phrases: &[Phrase]) -> String {
    phrases
        .iter()
        .map(|phrase| format!("{}:{}:{}", phrase.id, phrase.updated_at, phrase.use_count))
        .collect::<Vec<_>>()
        .join("|")
}

fn web_search_templates_signature(templates: &[WebSearchTemplate]) -> String {
    templates
        .iter()
        .map(|template| {
            format!(
                "{}:{}:{}:{}",
                template.id, template.keyword, template.updated_at, template.url_template
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn exclusion_rules_signature(rules: &[ExclusionRule]) -> String {
    rules
        .iter()
        .map(|rule| format!("{}:{}:{}", rule.id, rule.match_type, rule.updated_at))
        .collect::<Vec<_>>()
        .join("|")
}

fn bool_digit(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

fn normalize_selection_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn register_search_request(search_requests: &SearchRequestTracker, request_id: Option<u64>) -> u64 {
    let request_id = request_id.unwrap_or_default();
    if request_id > 0 {
        search_requests.fetch_max(request_id, Ordering::Relaxed);
    }

    request_id
}

fn is_current_search_request(search_requests: &SearchRequestTracker, request_id: u64) -> bool {
    request_id == 0 || search_requests.load(Ordering::Relaxed) == request_id
}

fn normalize_tool_menu_query(query: &str, menu_alias: &str) -> String {
    if query.trim() == menu_alias.trim() {
        DEFAULT_TOOL_MENU_ALIAS.into()
    } else {
        query.to_string()
    }
}

fn is_tool_query(query: &str) -> bool {
    if query.trim() == DEFAULT_TOOL_MENU_ALIAS {
        return true;
    }

    matches!(
        query
            .split_whitespace()
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("enc" | "dec" | "pwd" | "time")
    )
}

fn should_prioritize_file_search(
    query: &str,
    route: &search::ActionKeywordRoute,
    enabled_sources: &std::collections::HashSet<SearchSource>,
) -> bool {
    if !enabled_sources.contains(&SearchSource::Files) {
        return false;
    }

    if route
        .sources
        .as_ref()
        .is_some_and(|sources| sources.len() == 1 && sources.contains(&SearchSource::Files))
    {
        return true;
    }

    if enabled_sources.len() == 1 {
        return true;
    }

    looks_like_file_query(query)
}

fn looks_like_file_query(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.chars().count() < 2 {
        return false;
    }

    let path_like = trimmed.contains('\\')
        || trimmed.contains('/')
        || trimmed.starts_with('.')
        || trimmed.starts_with('~')
        || Path::new(trimmed).is_absolute();
    if path_like {
        return true;
    }

    let Some(last_token) = trimmed.split_whitespace().last() else {
        return false;
    };

    let Some((stem, extension)) = last_token.rsplit_once('.') else {
        return false;
    };

    !stem.is_empty()
        && extension.len() >= 2
        && extension.len() <= 8
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

#[tauri::command]
fn execute_result(
    result: SearchResult,
    query: Option<String>,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    let record_selection = || -> Result<(), String> {
        let Some(query) = query.as_deref() else {
            return Ok(());
        };
        let route = parse_action_keyword_query(query);
        let normalized_query = normalize_selection_query(&route.query);
        storage
            .lock()
            .map_err(|_| "无法记录查询选择".to_string())?
            .record_query_selection(&normalized_query, &result.id)
            .map_err(|error| error.to_string())?;
        Ok(())
    };

    match result.action {
        ActionKind::LaunchApp => {
            launch_app(&result.subtitle)?;
            record_selection()?;
            storage
                .lock()
                .map_err(|_| "无法记录最近使用".to_string())?
                .record_recent_item(&result.id, "app", &result.title, &result.subtitle)
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        ActionKind::OpenFile => {
            launch_app(&result.subtitle)?;
            record_selection()?;
            Ok(())
        }
        ActionKind::CopyText => {
            write_text_clipboard(&result.subtitle, "复制文本失败")?;
            record_selection()?;
            if result.id.starts_with("phrase:") {
                storage
                    .lock()
                    .map_err(|_| "无法记录快捷短语使用".to_string())?
                    .mark_phrase_used(&result.id)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }
        ActionKind::RunCommand => {
            if result.id.starts_with("custom-command:") {
                run_custom_command(&result.subtitle)?;
                record_selection()?;
                storage
                    .lock()
                    .map_err(|_| "无法记录自定义命令使用".to_string())?
                    .record_recent_item(
                        &result.id,
                        "custom-command",
                        &result.title,
                        &result.subtitle,
                    )
                    .map_err(|error| error.to_string())?;
            } else {
                run_system_command(&result.subtitle)?;
                record_selection()?;
            }
            Ok(())
        }
        ActionKind::OpenUrl => {
            open_url(&result.subtitle)?;
            record_selection()?;
            storage
                .lock()
                .map_err(|_| "无法记录网页搜索使用".to_string())?
                .record_recent_item(&result.id, "web-search", &result.title, &result.subtitle)
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        _ => Err("当前结果类型还不支持执行".into()),
    }
}

fn run_system_command(command: &str) -> Result<(), String> {
    match command {
        "ms-settings:" => launch_app(command),
        "terminal" => Command::new("wt")
            .spawn()
            .or_else(|_| Command::new("cmd").spawn())
            .map(|_| ())
            .map_err(|error| format!("打开终端失败：{error}")),
        "explorer" => Command::new("explorer")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开文件管理器失败：{error}")),
        "task-manager" => Command::new("taskmgr")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开任务管理器失败：{error}")),
        "control-panel" => Command::new("control")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开控制面板失败：{error}")),
        "index-options" => Command::new("control")
            .arg("srchadmin.dll")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开索引选项失败：{error}")),
        "recycle-bin" => Command::new("explorer")
            .arg("shell:RecycleBinFolder")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开回收站失败：{error}")),
        "lock" => Command::new("rundll32.exe")
            .args(["user32.dll,LockWorkStation"])
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("锁屏失败：{error}")),
        "logoff" => Command::new("shutdown")
            .arg("/l")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("注销失败：{error}")),
        "sleep" => Command::new("rundll32.exe")
            .args(["powrprof.dll,SetSuspendState", "0,1,0"])
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("睡眠失败：{error}")),
        "hibernate" => Command::new("shutdown")
            .arg("/h")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("休眠失败：{error}")),
        "shutdown" => Command::new("shutdown")
            .args(["/s", "/t", "0"])
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("关机失败：{error}")),
        "restart" => Command::new("shutdown")
            .args(["/r", "/t", "0"])
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("重启失败：{error}")),
        "restart-advanced" => Command::new("shutdown")
            .args(["/r", "/o", "/t", "0"])
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("高级重启失败：{error}")),
        _ => Err("未知系统命令".into()),
    }
}

fn run_custom_command(target: &str) -> Result<(), String> {
    hidden_command("cmd")
        .args(custom_command_launch_args(target))
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("执行自定义命令失败：{error}"))
}

fn open_url(url: &str) -> Result<(), String> {
    hidden_command("cmd")
        .args(open_url_launch_args(url))
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("打开网页失败：{error}"))
}

fn custom_command_launch_args(target: &str) -> Vec<String> {
    vec!["/C".into(), "start".into(), "".into(), target.into()]
}

fn open_url_launch_args(url: &str) -> Vec<String> {
    vec!["/C".into(), "start".into(), "".into(), url.into()]
}

#[tauri::command]
fn open_parent_dir(path: String) -> Result<(), String> {
    let parent = parent_dir_target(Path::new(&path))?;

    Command::new("explorer")
        .arg(parent)
        .spawn()
        .map_err(|error| format!("打开所在目录失败：{error}"))?;

    Ok(())
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法在资源管理器中选中".into());
    }

    Command::new("explorer")
        .arg(format!("/select,{}", target.display()))
        .spawn()
        .map_err(|error| format!("在资源管理器中选中失败：{error}"))?;

    Ok(())
}

#[tauri::command]
fn delete_path(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法删除".into());
    }

    if target.is_dir() {
        fs::remove_dir_all(target).map_err(|error| format!("删除目录失败：{error}"))?;
    } else {
        fs::remove_file(target).map_err(|error| format!("删除文件失败：{error}"))?;
    }

    Ok(())
}

#[tauri::command]
fn copy_file_to_clipboard(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法复制文件本体".into());
    }

    hidden_command("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Set-Clipboard -LiteralPath $args[0]",
            &path,
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("复制文件本体失败：{error}"))
}

#[tauri::command]
fn open_with_dialog(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法打开“打开方式”".into());
    }

    Command::new("rundll32.exe")
        .arg("shell32.dll,OpenAs_RunDLL")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("打开方式失败：{error}"))
}

#[tauri::command]
fn open_terminal_at_path(path: String) -> Result<(), String> {
    let directory = parent_dir_target(Path::new(&path))?;
    Command::new("wt")
        .arg("-d")
        .arg(&directory)
        .spawn()
        .or_else(|_| {
            Command::new("cmd")
                .arg("/K")
                .arg(format!("cd /d {}", directory.display()))
                .spawn()
        })
        .map(|_| ())
        .map_err(|error| format!("打开终端失败：{error}"))
}

#[tauri::command]
fn open_configured_editor(
    path: String,
    storage: tauri::State<'_, StorageState>,
) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法用配置编辑器打开".into());
    }

    let setting_key = if target.is_dir() {
        "folder.editor.path"
    } else {
        "file.editor.path"
    };
    let editor_path = storage
        .lock()
        .map_err(|_| "无法读取编辑器设置".to_string())?
        .get_setting(setting_key)
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let editor_path = editor_path.trim();
    if editor_path.is_empty() {
        return Err("尚未配置对应编辑器路径".into());
    }
    if !Path::new(editor_path).exists() {
        return Err("配置的编辑器路径不存在".into());
    }

    Command::new(editor_path)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("配置编辑器打开失败：{error}"))
}

#[tauri::command]
fn show_native_context_menu(path: String, window: tauri::Window) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法显示 Windows 原生菜单".into());
    }

    show_native_context_menu_inner(target, &window)
}

#[cfg(windows)]
fn show_native_context_menu_inner(target: &Path, window: &tauri::Window) -> Result<(), String> {
    use std::ptr;
    use windows::core::{PCSTR, PCWSTR};
    use windows::Win32::Foundation::{HWND, POINT, RPC_E_CHANGED_MODE, S_FALSE, S_OK};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        IContextMenu, ILFree, IShellFolder, SHBindToParent, SHParseDisplayName, CMF_EXTENDEDVERBS,
        CMF_NORMAL, CMINVOKECOMMANDINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, GetCursorPos, TrackPopupMenu, HMENU, SW_SHOWNORMAL,
        TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN,
    };

    struct ComApartment {
        should_uninitialize: bool,
    }

    impl ComApartment {
        fn initialize() -> Result<Self, String> {
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if result == S_OK || result == S_FALSE {
                return Ok(Self {
                    should_uninitialize: true,
                });
            }
            if result == RPC_E_CHANGED_MODE {
                return Ok(Self {
                    should_uninitialize: false,
                });
            }
            Err(format!("初始化 Shell COM 失败：{result:?}"))
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.should_uninitialize {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }

    struct Pidl(*mut ITEMIDLIST);

    impl Drop for Pidl {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    ILFree(Some(self.0));
                }
            }
        }
    }

    struct PopupMenu(HMENU);

    impl Drop for PopupMenu {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = DestroyMenu(self.0);
                }
            }
        }
    }

    let _com = ComApartment::initialize()?;
    let hwnd = window
        .hwnd()
        .map_err(|error| format!("获取窗口句柄失败：{error}"))?;
    let hwnd = HWND(hwnd.0 as _);
    let target_wide = path_to_wide(target);

    let mut full_pidl = ptr::null_mut();
    unsafe {
        SHParseDisplayName(PCWSTR(target_wide.as_ptr()), None, &mut full_pidl, 0, None)
            .map_err(|error| format!("解析 Shell 路径失败：{error}"))?;
    }
    let full_pidl = Pidl(full_pidl);

    let mut item_pidl = ptr::null_mut();
    let parent_folder: IShellFolder = unsafe {
        SHBindToParent(full_pidl.0, Some(&mut item_pidl))
            .map_err(|error| format!("绑定 Shell 父目录失败：{error}"))?
    };
    if item_pidl.is_null() {
        return Err("无法取得 Shell 菜单目标".into());
    }

    let context_menu: IContextMenu = unsafe {
        parent_folder
            .GetUIObjectOf(hwnd, &[item_pidl], None)
            .map_err(|error| format!("读取 Windows 原生菜单失败：{error}"))?
    };
    let menu =
        PopupMenu(unsafe { CreatePopupMenu() }.map_err(|error| format!("创建菜单失败：{error}"))?);
    const FIRST_COMMAND_ID: u32 = 1;
    const LAST_COMMAND_ID: u32 = 0x7fff;
    let query_result = unsafe {
        context_menu.QueryContextMenu(
            menu.0,
            0,
            FIRST_COMMAND_ID,
            LAST_COMMAND_ID,
            CMF_NORMAL | CMF_EXTENDEDVERBS,
        )
    };
    if query_result.is_err() {
        return Err(format!("查询 Windows 原生菜单失败：{query_result:?}"));
    }

    let mut cursor = POINT::default();
    unsafe {
        GetCursorPos(&mut cursor).map_err(|error| format!("获取鼠标位置失败：{error}"))?;
    }
    let selected_command = unsafe {
        TrackPopupMenu(
            menu.0,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_TOPALIGN,
            cursor.x,
            cursor.y,
            None,
            hwnd,
            None,
        )
    };
    if selected_command.0 == 0 {
        return Ok(());
    }
    if selected_command.0 < FIRST_COMMAND_ID as i32 {
        return Err("Windows 原生菜单返回了无效命令".into());
    }

    let command_offset = (selected_command.0 as u32 - FIRST_COMMAND_ID) as usize;
    let command_verb = PCSTR(command_offset as *const u8);
    let invoke_info = CMINVOKECOMMANDINFO {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
        hwnd,
        lpVerb: command_verb,
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    unsafe {
        context_menu
            .InvokeCommand(&invoke_info)
            .map_err(|error| format!("执行 Windows 原生命令失败：{error}"))?;
    }

    Ok(())
}

#[cfg(not(windows))]
fn show_native_context_menu_inner(_target: &Path, _window: &tauri::Window) -> Result<(), String> {
    Err("Windows 原生菜单仅支持 Windows".into())
}

#[tauri::command]
fn set_quick_access(path: String, pinned: bool) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Err("路径不存在，无法更新快速访问".into());
    }
    if !target.is_dir() {
        return Err("仅目录支持添加到或移除出快速访问".into());
    }

    let verb = if pinned { "pintohome" } else { "unpinfromhome" };
    hidden_command("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$shell = New-Object -ComObject Shell.Application; $folder = $shell.Namespace($args[0]); if ($null -eq $folder) { throw 'folder unavailable' }; $item = $folder.Self; if ($null -eq $item) { throw 'folder item unavailable' }; $item.InvokeVerb($args[1])",
            &path,
            verb,
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("快速访问更新失败：{error}"))
}

#[tauri::command]
fn open_shortcut_target_parent(path: String) -> Result<(), String> {
    if !path.to_lowercase().ends_with(".lnk") {
        return Err("仅 .lnk 快捷方式支持打开目标所在目录".into());
    }

    open_shortcut_target_parent_inner(&path)
}

#[tauri::command]
fn run_app_as_different_user(path: String, username: String) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("用户名不能为空".into());
    }

    Command::new("runas")
        .arg(format!("/user:{username}"))
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("以其他用户运行失败：{error}"))
}

#[tauri::command]
fn copy_path(path: String) -> Result<(), String> {
    write_text_clipboard(&path, "复制路径失败")
}

fn write_text_clipboard(text: &str, error_context: &str) -> Result<(), String> {
    set_clipboard_string(text).map_err(|error| format!("{error_context}：{error}"))
}

#[tauri::command]
fn run_app_as_admin(path: String) -> Result<(), String> {
    run_path_as_admin(&path)
}

fn parent_dir_target(target: &Path) -> Result<PathBuf, String> {
    if target.is_dir() {
        return Ok(target.to_path_buf());
    }

    target
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法找到所在目录".to_string())
}

#[cfg(windows)]
fn open_shortcut_target_parent_inner(path: &str) -> Result<(), String> {
    hidden_command("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "$shortcut=(New-Object -ComObject WScript.Shell).CreateShortcut($args[0]); if (-not $shortcut.TargetPath) { exit 1 }; explorer.exe /select, $shortcut.TargetPath",
            path,
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("打开快捷方式目标所在目录失败：{error}"))
}

#[cfg(not(windows))]
fn open_shortcut_target_parent_inner(_path: &str) -> Result<(), String> {
    Err("打开快捷方式目标所在目录仅支持 Windows".into())
}

#[cfg(windows)]
fn run_path_as_admin(path: &str) -> Result<(), String> {
    use std::ptr;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    let operation = wide_null("runas");
    let file = wide_null(path);
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
        )
    };

    if result as isize <= 32 {
        return Err(format!("以管理员运行失败，ShellExecute 返回 {result:?}"));
    }

    Ok(())
}

#[cfg(not(windows))]
fn run_path_as_admin(_path: &str) -> Result<(), String> {
    Err("以管理员运行仅支持 Windows".into())
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn path_to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn apply_launcher_shortcut(
    app: &AppHandle,
    storage: &Storage,
    shortcut_status: &ShortcutStatusStore,
    shortcut: &str,
) -> Result<ShortcutStatus, String> {
    apply_named_shortcut(
        app,
        storage,
        shortcut_status,
        shortcut,
        "launcher.shortcut",
        "呼出/隐藏启动器",
    )
}

fn apply_ai_shortcut(
    app: &AppHandle,
    storage: &Storage,
    shortcut_status: &ShortcutStatusStore,
    shortcut: &str,
) -> Result<ShortcutStatus, String> {
    apply_named_shortcut(
        app,
        storage,
        shortcut_status,
        shortcut,
        "ai.shortcut",
        "打开 AI 面板",
    )
}

fn apply_named_shortcut(
    app: &AppHandle,
    storage: &Storage,
    shortcut_status: &ShortcutStatusStore,
    shortcut: &str,
    setting_key: &str,
    usage: &str,
) -> Result<ShortcutStatus, String> {
    let new_shortcut = parse_shortcut_label(shortcut)?;
    let normalized_label = normalize_shortcut_label(shortcut)?;

    let previous_status = shortcut_status
        .lock()
        .map_err(|_| "无法读取快捷键状态".to_string())?
        .clone();

    if previous_status.registered {
        if let Ok(previous_shortcut) = parse_shortcut_label(&previous_status.shortcut) {
            let _ = app.global_shortcut().unregister(previous_shortcut);
        }
    }

    match app.global_shortcut().register(new_shortcut) {
        Ok(_) => {
            storage
                .set_setting(setting_key, &normalized_label)
                .map_err(|error| error.to_string())?;

            let status = ShortcutStatus {
                shortcut: normalized_label,
                registered: true,
                message: format!("{} 用于{usage}", shortcut_display(shortcut)),
            };
            *shortcut_status
                .lock()
                .map_err(|_| "无法更新快捷键状态".to_string())? = status.clone();
            Ok(status)
        }
        Err(error) => {
            if previous_status.registered {
                if let Ok(previous_shortcut) = parse_shortcut_label(&previous_status.shortcut) {
                    let _ = app.global_shortcut().register(previous_shortcut);
                }
            }

            let status = ShortcutStatus {
                shortcut: previous_status.shortcut,
                registered: previous_status.registered,
                message: format!(
                    "快捷键 {} 注册失败，可能已被占用：{error}",
                    shortcut_display(shortcut)
                ),
            };
            *shortcut_status
                .lock()
                .map_err(|_| "无法更新快捷键状态".to_string())? = status.clone();
            Err(status.message)
        }
    }
}

fn validate_import_setting(key: &str, value: &str) -> Result<(), String> {
    validate_setting_value(key, value)
}

fn validate_setting_value(key: &str, value: &str) -> Result<(), String> {
    match key {
        "launcher.shortcut" | "ai.shortcut" => {
            parse_shortcut_label(value)?;
            Ok(())
        }
        "selection.trigger.mode" => {
            if matches!(value.trim(), SELECTION_TRIGGER_MODE_CTRL_MOUSE) {
                Ok(())
            } else {
                Err("selection.trigger.mode 只能是 ctrl_mouse".into())
            }
        }
        "ui.language" => {
            if matches!(value.trim(), "system" | "zh-CN" | "en-US") {
                Ok(())
            } else {
                Err("ui.language 只能是 system、zh-CN 或 en-US".into())
            }
        }
        "launcher.double_alt.enabled"
        | "selection.enabled"
        | "startup.enabled"
        | "search.source.apps"
        | "search.source.files"
        | "search.source.calculator"
        | "search.source.system"
        | "search.source.ai"
        | "search.source.phrase"
        | "search.source.web_search"
        | "search.source.tools"
        | "everything.search.full_path"
        | "everything.search.content"
        | "tools.password.uppercase"
        | "tools.password.lowercase"
        | "tools.password.digits"
        | "tools.password.hyphen"
        | "tools.password.underscore"
        | "tools.password.special"
        | "tools.password.brackets"
        | "updates.check.enabled"
        | "updates.check.include_prerelease" => {
            if matches!(value.trim(), "true" | "false") {
                Ok(())
            } else {
                Err(format!("{key} 只能是 true 或 false"))
            }
        }
        "search.weight.apps"
        | "search.weight.files"
        | "search.weight.calculator"
        | "search.weight.system"
        | "search.weight.ai"
        | "search.weight.phrase"
        | "search.weight.web_search"
        | "search.weight.tools" => parse_weight(value).map(|_| ()),
        "tools.menu.alias" => validate_tool_menu_alias(value),
        "tools.password.length" => parse_password_length(value).map(|_| ()),
        "updates.check.interval_hours" => parse_update_interval_hours(value).map(|_| ()),
        "updates.check.last_checked_at"
        | "updates.check.last_seen_tag"
        | "updates.check.dismissed_tag" => Ok(()),
        "file.editor.path" | "folder.editor.path" => validate_optional_existing_path(value),
        "everything.exe.path" => validate_everything_exe_path(value),
        _ => Ok(()),
    }
}

fn apply_startup_enabled(app: &AppHandle, storage: &Storage, enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let exe_path =
            std::env::current_exe().map_err(|error| format!("无法获取当前程序路径：{error}"))?;
        if enabled {
            run_startup_registry_command(&startup_registry_args(
                true,
                exe_path.to_string_lossy().as_ref(),
            ))?;
        } else {
            let _ = run_startup_registry_command(&startup_registry_args(false, ""));
        }

        storage
            .set_setting("startup.enabled", if enabled { "true" } else { "false" })
            .map_err(|error| error.to_string())?;
        let _ = app;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = app;
        let _ = storage;
        Err("当前平台暂不支持开机自启动设置".into())
    }
}

fn startup_registry_args(enabled: bool, exe_path: &str) -> Vec<String> {
    if enabled {
        vec![
            "add".into(),
            STARTUP_RUN_KEY.into(),
            "/v".into(),
            STARTUP_VALUE_NAME.into(),
            "/t".into(),
            "REG_SZ".into(),
            "/d".into(),
            format!("\"{exe_path}\""),
            "/f".into(),
        ]
    } else {
        vec![
            "delete".into(),
            STARTUP_RUN_KEY.into(),
            "/v".into(),
            STARTUP_VALUE_NAME.into(),
            "/f".into(),
        ]
    }
}

#[cfg(windows)]
fn run_startup_registry_command(args: &[String]) -> Result<(), String> {
    let output = hidden_command("reg")
        .args(args)
        .output()
        .map_err(|error| format!("执行注册表命令失败：{error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        Err(format!("更新开机自启动失败：{message}"))
    }
}

fn storage_export_dir(storage: &Storage) -> Result<std::path::PathBuf, String> {
    let database_path = storage.status().database_path;
    let database_path = std::path::PathBuf::from(database_path);
    database_path
        .parent()
        .map(|path| path.join("exports"))
        .ok_or_else(|| "无法定位导出目录".to_string())
}

fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("{seconds}")
}

#[tauri::command]
fn open_everything_download() -> Result<(), String> {
    hidden_command("cmd")
        .args(["/C", "start", "", "https://www.voidtools.com/downloads/"])
        .spawn()
        .map_err(|error| format!("打开 Everything 下载页失败：{error}"))?;

    Ok(())
}

fn parse_shortcut_label(shortcut: &str) -> Result<Shortcut, String> {
    normalize_shortcut_label(shortcut)?
        .parse::<Shortcut>()
        .map_err(|error| format!("快捷键格式无效：{error}"))
}

fn normalize_shortcut_label(shortcut: &str) -> Result<String, String> {
    let parts = shortcut
        .split('+')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.len() < 2 {
        return Err("快捷键至少需要一个修饰键和一个按键，例如 Alt+1".into());
    }

    let mut normalized = Vec::new();
    let mut has_modifier = false;
    for part in parts {
        let token = match part.to_lowercase().as_str() {
            "ctrl" | "control" => {
                has_modifier = true;
                "Ctrl".to_string()
            }
            "alt" | "option" => {
                has_modifier = true;
                "Alt".to_string()
            }
            "shift" => {
                has_modifier = true;
                "Shift".to_string()
            }
            "super" | "win" | "cmd" | "command" => {
                has_modifier = true;
                "Super".to_string()
            }
            "space" => "Space".to_string(),
            key if key.len() == 1 => key.to_uppercase(),
            key if key.starts_with('f') && key[1..].parse::<u8>().is_ok() => key.to_uppercase(),
            key => {
                let mut chars = key.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        };
        normalized.push(token);
    }

    if !has_modifier {
        return Err("快捷键至少需要 Ctrl、Alt、Shift 或 Win 中的一个修饰键".into());
    }

    Ok(normalized.join("+"))
}

fn shortcut_display(shortcut: &str) -> String {
    normalize_shortcut_label(shortcut).unwrap_or_else(|_| shortcut.trim().to_string())
}

fn shortcut_matches_label(shortcut: &Shortcut, label: &str) -> bool {
    parse_shortcut_label(label)
        .map(|configured| configured == *shortcut)
        .unwrap_or(false)
}

fn bool_setting(storage: &Storage, key: &str, default: bool) -> Result<bool, String> {
    storage
        .get_setting(key)
        .map_err(|error| error.to_string())
        .map(|value| value.map(|value| value == "true").unwrap_or(default))
}

fn write_bool_setting(storage: &Storage, key: &str, value: bool) -> Result<(), String> {
    storage
        .set_setting(key, if value { "true" } else { "false" })
        .map_err(|error| error.to_string())
}

fn write_weight_setting(storage: &Storage, key: &str, value: f32) -> Result<(), String> {
    let value = sanitize_weight(value);
    storage
        .set_setting(key, &format!("{value:.2}"))
        .map_err(|error| error.to_string())
}

fn read_search_source_settings(storage: &Storage) -> Result<SearchSourceSettings, String> {
    Ok(SearchSourceSettings {
        apps: bool_setting(storage, "search.source.apps", true)?,
        files: bool_setting(storage, "search.source.files", true)?,
        calculator: bool_setting(storage, "search.source.calculator", true)?,
        system: bool_setting(storage, "search.source.system", true)?,
        ai: bool_setting(storage, "search.source.ai", true)?,
        phrase: bool_setting(storage, "search.source.phrase", true)?,
        web_search: bool_setting(storage, "search.source.web_search", true)?,
        tools: bool_setting(storage, "search.source.tools", true)?,
    })
}

fn read_search_weight_settings(storage: &Storage) -> Result<SearchWeightSettings, String> {
    Ok(SearchWeightSettings {
        apps: weight_setting(storage, "search.weight.apps", 1.0)?,
        files: weight_setting(storage, "search.weight.files", 1.0)?,
        calculator: weight_setting(storage, "search.weight.calculator", 1.0)?,
        system: weight_setting(storage, "search.weight.system", 1.0)?,
        ai: weight_setting(storage, "search.weight.ai", 1.0)?,
        phrase: weight_setting(storage, "search.weight.phrase", 1.0)?,
        web_search: weight_setting(storage, "search.weight.web_search", 1.0)?,
        tools: weight_setting(storage, "search.weight.tools", 1.0)?,
    })
}

fn read_password_options(storage: &Storage) -> Result<PasswordOptions, String> {
    let defaults = PasswordOptions::default();
    let length = storage
        .get_setting("tools.password.length")
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(defaults.length);

    Ok(sanitize_password_options(PasswordOptions {
        length,
        uppercase: bool_setting(storage, "tools.password.uppercase", defaults.uppercase)?,
        lowercase: bool_setting(storage, "tools.password.lowercase", defaults.lowercase)?,
        digits: bool_setting(storage, "tools.password.digits", defaults.digits)?,
        hyphen: bool_setting(storage, "tools.password.hyphen", defaults.hyphen)?,
        underscore: bool_setting(storage, "tools.password.underscore", defaults.underscore)?,
        special: bool_setting(storage, "tools.password.special", defaults.special)?,
        brackets: bool_setting(storage, "tools.password.brackets", defaults.brackets)?,
    }))
}

fn read_tool_menu_alias(storage: &Storage) -> Result<String, String> {
    let alias = storage
        .get_setting("tools.menu.alias")
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| DEFAULT_TOOL_MENU_ALIAS.into());
    validate_tool_menu_alias(&alias)?;
    Ok(alias.trim().to_string())
}

fn read_everything_search_options(storage: &Storage) -> Result<EverythingSearchOptions, String> {
    Ok(EverythingSearchOptions {
        full_path: bool_setting(storage, "everything.search.full_path", false)?,
        search_content: bool_setting(storage, "everything.search.content", false)?,
    })
}

fn weight_setting(storage: &Storage, key: &str, default: f32) -> Result<f32, String> {
    storage
        .get_setting(key)
        .map_err(|error| error.to_string())
        .map(|value| {
            value
                .as_deref()
                .and_then(|value| parse_weight(value).ok())
                .unwrap_or(default)
        })
}

fn parse_weight(value: &str) -> Result<f32, String> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .map(sanitize_weight)
        .filter(|value| value.is_finite())
        .ok_or_else(|| "搜索权重必须是有效数字".to_string())
}

fn validate_optional_existing_path(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || Path::new(trimmed).exists() {
        Ok(())
    } else {
        Err("配置路径不存在".into())
    }
}

fn validate_everything_exe_path(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let path = Path::new(trimmed);
    if !path.is_file() {
        return Err("Everything 路径必须是一个已存在的文件".into());
    }

    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err("Everything 路径无效".into());
    };
    if !file_name.eq_ignore_ascii_case("Everything.exe") {
        return Err("请选择 Everything.exe".into());
    }

    Ok(())
}

fn sanitize_weight(value: f32) -> f32 {
    if !value.is_finite() {
        return 1.0;
    }

    value.clamp(0.1, 3.0)
}

fn validate_tool_menu_alias(value: &str) -> Result<(), String> {
    let alias = value.trim();
    if alias.is_empty() {
        return Err("工具总入口不能为空".into());
    }
    if alias.chars().any(char::is_whitespace) {
        return Err("工具总入口不能包含空格".into());
    }
    if matches!(
        alias.to_ascii_lowercase().as_str(),
        "tools" | "enc" | "dec" | "pwd" | "time" | "="
    ) {
        return Err("工具总入口不能使用已有快捷指令".into());
    }
    Ok(())
}

fn parse_password_length(value: &str) -> Result<usize, String> {
    let length = value
        .trim()
        .parse::<usize>()
        .map_err(|_| "密码长度必须是数字".to_string())?;
    if (4..=128).contains(&length) {
        Ok(length)
    } else {
        Err("密码长度必须在 4 到 128 之间".into())
    }
}

fn parse_update_interval_hours(value: &str) -> Result<u32, String> {
    let hours = value
        .trim()
        .parse::<u32>()
        .map_err(|_| "更新检查间隔必须是数字".to_string())?;
    if (1..=720).contains(&hours) {
        Ok(hours)
    } else {
        Err("更新检查间隔必须在 1 到 720 小时之间".into())
    }
}

fn enabled_sources_from_settings(
    settings: &SearchSourceSettings,
) -> std::collections::HashSet<SearchSource> {
    let mut sources = std::collections::HashSet::new();
    if settings.apps {
        sources.insert(SearchSource::Apps);
    }
    if settings.files {
        sources.insert(SearchSource::Files);
    }
    if settings.calculator {
        sources.insert(SearchSource::Calculator);
    }
    if settings.system {
        sources.insert(SearchSource::System);
    }
    if settings.ai {
        sources.insert(SearchSource::Ai);
    }
    if settings.phrase {
        sources.insert(SearchSource::Phrase);
    }
    if settings.web_search {
        sources.insert(SearchSource::WebSearch);
    }
    if settings.tools {
        sources.insert(SearchSource::Tools);
    }
    sources
}

fn default_enabled_sources() -> std::collections::HashSet<SearchSource> {
    enabled_sources_from_settings(&SearchSourceSettings {
        apps: true,
        files: true,
        calculator: true,
        system: true,
        ai: true,
        phrase: true,
        web_search: true,
        tools: true,
    })
}

fn source_weights_from_settings(
    settings: &SearchWeightSettings,
) -> std::collections::HashMap<SearchSource, f32> {
    [
        (SearchSource::Apps, settings.apps),
        (SearchSource::Files, settings.files),
        (SearchSource::Calculator, settings.calculator),
        (SearchSource::System, settings.system),
        (SearchSource::Ai, settings.ai),
        (SearchSource::Phrase, settings.phrase),
        (SearchSource::WebSearch, settings.web_search),
        (SearchSource::Tools, settings.tools),
    ]
    .into_iter()
    .collect()
}

fn toggle_launcher_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(true);

        if is_visible {
            if settings_panel_is_open(app) {
                let _ = window.set_focus();
                return;
            }
            hide_window_to_tray(&window);
        } else {
            set_settings_panel_open(app, false);
            if let Some(launcher_position) = app.try_state::<LauncherWindowPositionStore>() {
                restore_search_window(&window, launcher_position.inner());
            }
            let _ = app.emit(LAUNCHER_OPENED_EVENT, ());
            disable_native_window_frame(&window);
            let _ = window.show();
            disable_native_window_frame(&window);
            let _ = window.set_focus();
        }
    }
}

fn settings_panel_is_open(app: &AppHandle) -> bool {
    app.try_state::<SettingsPanelOpenStore>()
        .map(|state| state.load(Ordering::Relaxed))
        .unwrap_or(false)
}

fn set_settings_panel_open(app: &AppHandle, open: bool) {
    if let Some(state) = app.try_state::<SettingsPanelOpenStore>() {
        state.store(open, Ordering::Relaxed);
    }
}

fn hide_window_to_tray(window: &tauri::WebviewWindow) {
    let _ = window.eval(
        "if (document.activeElement instanceof HTMLElement) { document.activeElement.blur(); }",
    );
    std::thread::sleep(Duration::from_millis(80));
    let _ = window.hide();
}

fn show_main_window(app: &AppHandle) {
    set_settings_panel_open(app, false);
    if let Some(window) = app.get_webview_window("main") {
        if let Some(launcher_position) = app.try_state::<LauncherWindowPositionStore>() {
            restore_search_window(&window, launcher_position.inner());
        }
        let _ = app.emit(LAUNCHER_OPENED_EVENT, ());
        disable_native_window_frame(&window);
        let _ = window.show();
        disable_native_window_frame(&window);
        let _ = window.set_focus();
    }
}

fn store_launcher_position(
    window: &tauri::WebviewWindow,
    launcher_position: &LauncherWindowPositionStore,
) {
    if let Ok(position) = window.outer_position() {
        if let Ok(mut stored_position) = launcher_position.lock() {
            *stored_position = Some(position);
        }
    }
}

fn restore_search_window(
    window: &tauri::WebviewWindow,
    launcher_position: &LauncherWindowPositionStore,
) {
    let position = launcher_position
        .lock()
        .ok()
        .and_then(|stored_position| *stored_position);
    disable_native_window_frame(window);
    let _ = window.set_size(PhysicalSize::new(SEARCH_WINDOW_WIDTH, SEARCH_WINDOW_HEIGHT));
    if let Some(position) = position {
        let _ = window.set_position(position);
    }
    disable_native_window_frame(window);
}

fn restore_settings_window(window: &tauri::WebviewWindow) {
    disable_native_window_frame(window);
    let _ = window.set_size(PhysicalSize::new(
        SETTINGS_WINDOW_WIDTH,
        SETTINGS_WINDOW_HEIGHT,
    ));
    let _ = window.center();
    disable_native_window_frame(window);
}

fn show_selection_window(app: &AppHandle, point: Option<(i32, i32)>) {
    set_settings_panel_open(app, false);
    let window = if let Some(window) = app.get_webview_window("selection") {
        window
    } else {
        match WebviewWindowBuilder::new(app, "selection", WebviewUrl::App("index.html".into()))
            .title("Easy Launcher")
            .inner_size(
                SELECTION_PICKER_WINDOW_WIDTH as f64,
                SELECTION_PICKER_WINDOW_HEIGHT as f64,
            )
            .resizable(true)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .build()
        {
            Ok(window) => window,
            Err(_) => return,
        }
    };
    disable_native_window_frame(&window);
    let _ = window.set_size(PhysicalSize::new(
        SELECTION_PICKER_WINDOW_WIDTH,
        SELECTION_PICKER_WINDOW_HEIGHT,
    ));
    let _ = window.set_position(selection_window_position(app, point));
    let _ = window.set_always_on_top(true);
    disable_native_window_frame(&window);
    let _ = window.show();
    disable_native_window_frame(&window);
    let _ = window.set_focus();
}

fn selection_window_position(app: &AppHandle, point: Option<(i32, i32)>) -> PhysicalPosition<i32> {
    let (target_x, target_y) = point.unwrap_or((120, 120));
    let mut x = target_x + 12;
    let mut y = target_y + 12;
    if let Ok(monitors) = app.available_monitors() {
        let monitor = monitors
            .iter()
            .find(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                target_x >= position.x
                    && target_y >= position.y
                    && target_x <= position.x + size.width as i32
                    && target_y <= position.y + size.height as i32
            })
            .or_else(|| monitors.first());
        if let Some(monitor) = monitor {
            let position = monitor.position();
            let size = monitor.size();
            let max_x = position.x + size.width as i32 - SELECTION_RESULT_WINDOW_WIDTH as i32;
            let max_y = position.y + size.height as i32 - SELECTION_RESULT_WINDOW_HEIGHT as i32;
            x = x.clamp(position.x, max_x.max(position.x));
            y = y.clamp(position.y, max_y.max(position.y));
        }
    }
    PhysicalPosition::new(x, y)
}

fn show_ai_panel(app: &AppHandle) {
    set_settings_panel_open(app, false);
    if let Some(window) = app.get_webview_window("main") {
        disable_native_window_frame(&window);
        let _ = window.set_size(PhysicalSize::new(AI_WINDOW_WIDTH, AI_WINDOW_HEIGHT));
        let _ = window.center();
        let _ = app.emit(AI_OPENED_EVENT, ());
        disable_native_window_frame(&window);
        let _ = window.show();
        disable_native_window_frame(&window);
        let _ = window.set_focus();
    }
}

fn open_settings_from_tray(app: &AppHandle) {
    set_settings_panel_open(app, true);
    if let Some(window) = app.get_webview_window("main") {
        if let Some(launcher_position) = app.try_state::<LauncherWindowPositionStore>() {
            store_launcher_position(&window, launcher_position.inner());
        }
        restore_settings_window(&window);
        disable_native_window_frame(&window);
        let _ = window.show();
        disable_native_window_frame(&window);
        let _ = window.set_focus();
    }
    let _ = app.emit(TRAY_OPEN_SETTINGS_EVENT, ());
}

fn disable_native_window_frame(window: &tauri::WebviewWindow) {
    let _ = window.set_shadow(false);
    // The frameless launcher chrome stays out of the taskbar, but the settings
    // panel should behave like an ordinary window: keep it in the taskbar so
    // that if it loses focus and gets covered, it can be clicked back from
    // there instead of looking like it vanished into the tray.
    let _ = window.set_skip_taskbar(!settings_panel_is_open(window.app_handle()));
    disable_windows_corner_frame(window);
}

#[cfg(windows)]
fn disable_windows_corner_frame(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };

    if let Ok(hwnd) = window.hwnd() {
        let preference = DWMWCP_DONOTROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd.0 as _,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &preference as *const _ as _,
                std::mem::size_of_val(&preference) as u32,
            );
        }
    }
}

#[cfg(not(windows))]
fn disable_windows_corner_frame(_window: &tauri::WebviewWindow) {}

fn check_everything_from_tray(app: &AppHandle) {
    let status = app
        .state::<StorageState>()
        .lock()
        .ok()
        .and_then(|storage| storage.get_setting("everything.exe.path").ok())
        .flatten()
        .map(|path| detect_everything_status(Some(&path)))
        .unwrap_or_else(|| detect_everything_status(None));
    show_main_window(app);
    let _ = app.emit(TRAY_EVERYTHING_STATUS_EVENT, status);
}

fn create_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_MENU_OPEN, "打开主窗口")
        .text(TRAY_MENU_SETTINGS, "打开设置")
        .text(TRAY_MENU_EVERYTHING, "检查 Everything 状态")
        .separator()
        .text(TRAY_MENU_EXIT, "退出")
        .build()?;

    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("Easy Launcher")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_OPEN => show_main_window(app),
            TRAY_MENU_SETTINGS => open_settings_from_tray(app),
            TRAY_MENU_EVERYTHING => check_everything_from_tray(app),
            TRAY_MENU_EXIT => app.exit(0),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::DoubleClick { .. } = event {
            show_main_window(tray.app_handle());
        }
    })
    .build(app)?;

    Ok(())
}

fn publish_selection_capture(
    app: &AppHandle,
    selection_capture: &SelectionCaptureStore,
    point: Option<(i32, i32)>,
    result: SelectionCaptureResult,
) {
    let event = SelectionCaptureEvent {
        result,
        x: point.map(|point| point.0),
        y: point.map(|point| point.1),
    };
    if let Ok(mut pending) = selection_capture.lock() {
        *pending = Some(event.clone());
    }
    show_selection_window(app, point);
    if let Some(window) = app.get_webview_window("selection") {
        let _ = window.emit(SELECTION_CAPTURE_EVENT, event);
    }
}

pub fn emit_selection_capture(app: &AppHandle, point: Option<(i32, i32)>) {
    let result = capture_selected_text();
    if let Some(selection_capture) = app.try_state::<SelectionCaptureStore>() {
        publish_selection_capture(app, selection_capture.inner(), point, result);
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    let launcher_shortcut_matches = app
                        .try_state::<ShortcutStatusStore>()
                        .and_then(|status| status.lock().ok().map(|status| status.shortcut.clone()))
                        .map(|label| shortcut_matches_label(shortcut, &label))
                        .unwrap_or_else(|| {
                            shortcut_matches_label(shortcut, LAUNCHER_SHORTCUT_LABEL)
                        });
                    let ai_shortcut_matches = app
                        .try_state::<AiShortcutStatusStore>()
                        .and_then(|status| {
                            status.0.lock().ok().map(|status| status.shortcut.clone())
                        })
                        .map(|label| shortcut_matches_label(shortcut, &label))
                        .unwrap_or_else(|| shortcut_matches_label(shortcut, AI_SHORTCUT_LABEL));

                    if launcher_shortcut_matches && event.state() == ShortcutState::Pressed {
                        toggle_launcher_window(app);
                    }

                    if ai_shortcut_matches && event.state() == ShortcutState::Pressed {
                        show_ai_panel(app);
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            if window.label() == "main" || window.label() == "selection" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let storage = Storage::initialize().map_err(|error| error.to_string())?;
            let launcher_shortcut_label = storage
                .get_setting("launcher.shortcut")
                .map_err(|error| error.to_string())?
                .unwrap_or_else(|| LAUNCHER_SHORTCUT_LABEL.into());
            let ai_shortcut_label = storage
                .get_setting("ai.shortcut")
                .map_err(|error| error.to_string())?
                .unwrap_or_else(|| AI_SHORTCUT_LABEL.into());
            let storage = Arc::new(Mutex::new(storage));
            app.manage(storage);
            let selection_trigger = SelectionTriggerHandle::start(
                app.handle().clone(),
                app.state::<StorageState>().inner().clone(),
            );
            app.manage(selection_trigger);

            let launcher_shortcut = parse_shortcut_label(&launcher_shortcut_label)
                .unwrap_or_else(|_| Shortcut::new(Some(Modifiers::ALT), Code::Digit1));
            let ai_shortcut = parse_shortcut_label(&ai_shortcut_label)
                .unwrap_or_else(|_| Shortcut::new(Some(Modifiers::ALT), Code::Digit3));
            let shortcut_status = match app.global_shortcut().register(launcher_shortcut) {
                Ok(_) => ShortcutStatus {
                    shortcut: launcher_shortcut_label.clone(),
                    registered: true,
                    message: format!("{} 用于呼出/隐藏启动器", launcher_shortcut_label),
                },
                Err(error) => ShortcutStatus {
                    shortcut: launcher_shortcut_label.clone(),
                    registered: false,
                    message: format!(
                        "{} 注册失败，可能已被占用：{error}",
                        launcher_shortcut_label
                    ),
                },
            };

            let ai_shortcut_status = match app.global_shortcut().register(ai_shortcut) {
                Ok(_) => ShortcutStatus {
                    shortcut: ai_shortcut_label.clone(),
                    registered: true,
                    message: format!("{ai_shortcut_label} 用于打开 AI 面板"),
                },
                Err(error) => ShortcutStatus {
                    shortcut: ai_shortcut_label.clone(),
                    registered: false,
                    message: format!("{ai_shortcut_label} 注册失败，可能已被占用：{error}"),
                },
            };

            app.manage(Mutex::new(shortcut_status));
            app.manage(AiShortcutStatusStore(Mutex::new(ai_shortcut_status)));
            app.manage(Arc::new(Mutex::new(
                HashMap::<String, Arc<AtomicBool>>::new(),
            )));
            app.manage(Arc::new(Mutex::new(None::<SelectionCaptureEvent>)));
            app.manage(Arc::new(AtomicU64::new(0)));
            app.manage(Arc::new(Mutex::new(SearchResultCache::new(
                SEARCH_CACHE_MAX_ENTRIES,
                SEARCH_CACHE_TTL,
            ))));
            app.manage(Mutex::new(None::<PhysicalPosition<i32>>));
            app.manage(Arc::new(AtomicBool::new(false)));
            warm_app_scan_cache();
            create_tray(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ai_shortcut_status,
            cancel_ai_action,
            cancel_ai_chat_message,
            capture_selection,
            check_for_updates,
            copy_file_to_clipboard,
            copy_path,
            delete_path,
            delete_ai_assistant,
            delete_ai_conversation,
            delete_ai_message,
            delete_ai_model_profile,
            delete_ai_provider,
            everything_status,
            execute_result,
            export_config,
            fetch_ai_provider_models,
            get_setting,
            get_pending_selection_capture,
            get_app_version,
            get_password_options,
            get_search_source_settings,
            get_search_weight_settings,
            get_everything_search_options,
            get_ai_config,
            list_ai_assistants,
            list_ai_conversations,
            list_ai_messages,
            list_ai_model_profiles,
            list_ai_models,
            list_ai_provider_models,
            list_ai_providers,
            list_ai_selection_actions,
            list_enabled_ai_provider_models,
            list_visible_ai_selection_actions,
            greet,
            launcher_shortcut_status,
            show_native_context_menu,
            open_with_dialog,
            open_configured_editor,
            open_parent_dir,
            open_shortcut_target_parent,
            open_terminal_at_path,
            open_update_release_page,
            import_config,
            open_everything_download,
            reveal_path,
            rename_ai_conversation,
            run_ai_action,
            run_ai_action_stream,
            run_app_as_admin,
            run_app_as_different_user,
            search,
            search_with_recents,
            send_ai_chat_message,
            send_ai_selection_message,
            set_ai_shortcut,
            set_ai_provider_model_enabled,
            set_quick_access,
            set_ai_config,
            set_setting,
            set_launcher_shortcut,
            set_search_source_settings,
            set_search_weight_settings,
            set_password_options,
            set_everything_exe_path,
            set_everything_search_options,
            set_startup_enabled,
            hide_main_window,
            hide_selection_window,
            show_ai_settings_window,
            show_ai_window,
            show_search_window,
            show_selection_assistant,
            show_settings_window,
            storage_status,
            create_ai_conversation,
            list_custom_commands,
            mark_ai_assistant_used,
            save_ai_assistant,
            save_ai_model_profile,
            save_ai_provider,
            save_ai_provider_model,
            save_ai_selection_action,
            set_ai_assistant_model,
            set_ai_selection_action_model,
            save_custom_command,
            delete_custom_command,
            list_phrases,
            save_phrase,
            delete_phrase,
            list_web_search_templates,
            save_web_search_template,
            delete_web_search_template,
            list_exclusion_rules,
            save_exclusion_rule,
            delete_exclusion_rule,
            test_ai_model_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_command_launch_args_open_target_through_start() {
        let args = custom_command_launch_args("https://example.com/docs");

        assert_eq!(args, vec!["/C", "start", "", "https://example.com/docs"]);
    }

    #[test]
    fn open_url_launch_args_open_web_search_through_start() {
        let args = open_url_launch_args("https://example.com/search?q=rust%20tauri");

        assert_eq!(
            args,
            vec![
                "/C",
                "start",
                "",
                "https://example.com/search?q=rust%20tauri"
            ]
        );
    }

    #[test]
    fn parent_dir_target_returns_parent_for_files_and_self_for_directories() {
        let temp_root =
            std::env::temp_dir().join(format!("easy-launcher-parent-test-{}", std::process::id()));
        let nested_dir = temp_root.join("nested");
        let nested_file = nested_dir.join("app.exe");
        fs::create_dir_all(&nested_dir).expect("create temp dir");
        fs::write(&nested_file, b"test").expect("create temp file");

        assert_eq!(
            parent_dir_target(&nested_file).expect("file parent"),
            nested_dir
        );
        assert_eq!(
            parent_dir_target(&temp_root).expect("directory target"),
            temp_root.clone()
        );

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn file_intent_queries_prioritize_complete_search() {
        let all_sources = std::collections::HashSet::from([
            SearchSource::Apps,
            SearchSource::Files,
            SearchSource::System,
        ]);
        let file_route = parse_action_keyword_query("file notes");
        let app_route = parse_action_keyword_query("app notes");
        let plain_route = parse_action_keyword_query("notes");

        assert!(should_prioritize_file_search(
            &file_route.query,
            &file_route,
            &apply_action_keyword_route(&all_sources, &file_route)
        ));
        assert!(!should_prioritize_file_search(
            &app_route.query,
            &app_route,
            &apply_action_keyword_route(&all_sources, &app_route)
        ));
        assert!(!should_prioritize_file_search(
            &plain_route.query,
            &plain_route,
            &all_sources
        ));
        assert!(should_prioritize_file_search(
            r"C:\work\notes",
            &plain_route,
            &all_sources
        ));
        assert!(should_prioritize_file_search(
            "report.pdf",
            &plain_route,
            &all_sources
        ));
    }

    #[test]
    fn file_intent_requires_file_source_enabled() {
        let app_sources = std::collections::HashSet::from([SearchSource::Apps]);
        let file_route = parse_action_keyword_query("file notes");

        assert!(!should_prioritize_file_search(
            &file_route.query,
            &file_route,
            &app_sources
        ));
    }

    #[cfg(windows)]
    #[test]
    fn wide_null_encodes_runas_operation_for_shell_execute() {
        assert_eq!(wide_null("runas"), vec![114, 117, 110, 97, 115, 0]);
    }

    #[test]
    fn config_export_includes_custom_commands_without_api_key() {
        let mut settings = HashMap::new();
        settings.insert("launcher.shortcut".into(), "Alt+1".into());

        let export = ConfigExport {
            version: 1,
            product: "Easy Launcher".into(),
            exported_at: "2026-06-01".into(),
            settings,
            custom_commands: vec![CustomCommand {
                id: "custom-command:docs".into(),
                name: "Docs".into(),
                command_type: "url".into(),
                target: "https://example.com".into(),
                created_at: "2026-06-01T00:00:00.000Z".into(),
                updated_at: "2026-06-01T00:00:00.000Z".into(),
            }],
            phrases: Vec::new(),
            web_search_templates: Vec::new(),
            exclusion_rules: Vec::new(),
        };

        let json = serde_json::to_string(&export).expect("serialize config export");

        assert!(json.contains("customCommands"));
        assert!(json.contains("https://example.com"));
        assert!(!json.contains("ai.api_key"));
    }

    #[test]
    fn config_import_accepts_older_exports_without_custom_commands() {
        let json = r#"{
            "version": 1,
            "product": "Easy Launcher",
            "exportedAt": "2026-06-01",
            "settings": {
                "launcher.shortcut": "Alt+1"
            }
        }"#;

        let export: ConfigExport = serde_json::from_str(json).expect("deserialize config export");

        assert!(export.custom_commands.is_empty());
    }

    #[test]
    fn search_weight_defaults_and_invalid_values_fall_back() {
        let connection = rusqlite::Connection::open_in_memory().expect("open in-memory sqlite");
        crate::storage::initialize_schema_for_tests(&connection);
        let storage = Storage::from_connection_for_tests(connection);

        storage
            .set_setting("search.weight.apps", "not-a-number")
            .expect("write invalid weight");

        let settings = read_search_weight_settings(&storage).expect("read weights");

        assert_eq!(settings.apps, 1.0);
        assert_eq!(settings.files, 1.0);
    }

    #[test]
    fn search_weight_import_validation_accepts_numbers() {
        assert!(validate_import_setting("search.weight.apps", "2.5").is_ok());
        assert!(validate_import_setting("search.weight.apps", "invalid").is_err());
    }

    #[test]
    fn search_weight_export_keys_include_all_sources() {
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.apps"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.files"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.calculator"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.system"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.ai"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.phrase"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"search.weight.web_search"));
    }

    #[test]
    fn everything_options_export_and_import_validation_are_supported() {
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"everything.search.full_path"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"everything.search.content"));
        assert!(validate_import_setting("everything.search.full_path", "true").is_ok());
        assert!(validate_import_setting("everything.search.content", "false").is_ok());
        assert!(validate_import_setting("everything.search.content", "yes").is_err());
    }

    #[test]
    fn update_check_settings_export_and_import_validation_are_supported() {
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"updates.check.enabled"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"updates.check.interval_hours"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"updates.check.include_prerelease"));
        assert!(validate_import_setting("updates.check.enabled", "true").is_ok());
        assert!(validate_import_setting("updates.check.include_prerelease", "false").is_ok());
        assert!(validate_import_setting("updates.check.interval_hours", "24").is_ok());
        assert!(validate_import_setting("updates.check.interval_hours", "0").is_err());
    }

    #[test]
    fn configured_editor_settings_are_exportable_and_validate_paths() {
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"file.editor.path"));
        assert!(EXPORTABLE_SETTING_KEYS.contains(&"folder.editor.path"));
        assert!(validate_import_setting("file.editor.path", "").is_ok());
        assert!(validate_import_setting("folder.editor.path", ".").is_ok());
        assert!(validate_import_setting("file.editor.path", "Z:\\missing\\editor.exe").is_err());
    }

    #[test]
    fn everything_options_change_search_cache_key() {
        let sources = default_enabled_sources();
        let weights = SearchWeightSettings {
            apps: 1.0,
            files: 1.0,
            calculator: 1.0,
            system: 1.0,
            ai: 1.0,
            phrase: 1.0,
            web_search: 1.0,
            tools: 1.0,
        };
        let default_key = search_cache_key(
            "easy",
            &sources,
            &weights,
            &EverythingSearchOptions::default(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
        );
        let full_path_key = search_cache_key(
            "easy",
            &sources,
            &weights,
            &EverythingSearchOptions {
                full_path: true,
                search_content: false,
            },
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
        );

        assert_ne!(default_key, full_path_key);
    }

    #[test]
    fn search_request_tracker_keeps_newest_request_current() {
        let tracker = Arc::new(AtomicU64::new(0));

        let first = register_search_request(&tracker, Some(1));
        assert_eq!(first, 1);
        assert!(is_current_search_request(&tracker, first));

        let second = register_search_request(&tracker, Some(2));
        assert_eq!(second, 2);
        assert!(!is_current_search_request(&tracker, first));
        assert!(is_current_search_request(&tracker, second));

        let late_first = register_search_request(&tracker, Some(1));
        assert_eq!(late_first, 1);
        assert_eq!(tracker.load(Ordering::Relaxed), 2);
        assert!(!is_current_search_request(&tracker, late_first));
    }

    #[test]
    fn selection_query_normalization_collapses_case_and_spacing() {
        assert_eq!(normalize_selection_query("  Code   Editor "), "code editor");
    }

    #[test]
    fn search_cache_key_changes_when_local_search_inputs_change() {
        let source_settings = SearchSourceSettings {
            apps: true,
            files: true,
            calculator: true,
            system: true,
            ai: true,
            phrase: true,
            web_search: true,
            tools: true,
        };
        let sources = enabled_sources_from_settings(&source_settings);
        let weights = SearchWeightSettings {
            apps: 1.0,
            files: 1.0,
            calculator: 1.0,
            system: 1.0,
            ai: 1.0,
            phrase: 1.0,
            web_search: 1.0,
            tools: 1.0,
        };
        let recent_scores = HashMap::from([("app:one".into(), 0.2)]);
        let changed_recent_scores = HashMap::from([("app:one".into(), 0.5)]);
        let query_selection_scores = HashMap::from([("app:two".into(), 0.35)]);

        let first = search_cache_key(
            "Code",
            &sources,
            &weights,
            &EverythingSearchOptions::default(),
            &recent_scores,
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
        );
        let second = search_cache_key(
            "Code",
            &sources,
            &weights,
            &EverythingSearchOptions::default(),
            &changed_recent_scores,
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
        );

        assert_ne!(first, second);
        assert_eq!(
            first,
            search_cache_key(
                " code ",
                &sources,
                &weights,
                &EverythingSearchOptions::default(),
                &recent_scores,
                &HashMap::new(),
                &[],
                &[],
                &[],
                &[]
            )
        );
        assert_ne!(
            first,
            search_cache_key(
                "Code",
                &sources,
                &weights,
                &EverythingSearchOptions::default(),
                &recent_scores,
                &query_selection_scores,
                &[],
                &[],
                &[],
                &[]
            )
        );
    }

    #[test]
    fn search_cache_key_changes_when_action_keyword_filters_sources() {
        let source_settings = SearchSourceSettings {
            apps: true,
            files: true,
            calculator: true,
            system: true,
            ai: true,
            phrase: true,
            web_search: true,
            tools: true,
        };
        let all_sources = enabled_sources_from_settings(&source_settings);
        let app_sources = std::collections::HashSet::from([SearchSource::Apps]);
        let weights = SearchWeightSettings {
            apps: 1.0,
            files: 1.0,
            calculator: 1.0,
            system: 1.0,
            ai: 1.0,
            phrase: 1.0,
            web_search: 1.0,
            tools: 1.0,
        };

        let all_key = search_cache_key(
            "code",
            &all_sources,
            &weights,
            &EverythingSearchOptions::default(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
        );
        let app_key = search_cache_key(
            "code",
            &app_sources,
            &weights,
            &EverythingSearchOptions::default(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
        );

        assert_ne!(all_key, app_key);
    }

    #[test]
    fn v2_regression_config_export_shape_includes_v2_collections_and_weights() {
        let mut settings = HashMap::new();
        settings.insert("search.weight.apps".into(), "1.50".into());
        settings.insert("search.source.phrase".into(), "true".into());

        let export = ConfigExport {
            version: 1,
            product: "Easy Launcher".into(),
            exported_at: "2026-06-01".into(),
            settings,
            custom_commands: vec![CustomCommand {
                id: "custom-command:docs".into(),
                name: "Docs".into(),
                command_type: "url".into(),
                target: "https://example.com".into(),
                created_at: "2026-06-01T00:00:00.000Z".into(),
                updated_at: "2026-06-01T00:00:00.000Z".into(),
            }],
            phrases: vec![Phrase {
                id: "phrase:greeting".into(),
                title: "Greeting".into(),
                text: "Hello there".into(),
                created_at: "2026-06-01T00:00:00.000Z".into(),
                updated_at: "2026-06-01T00:00:00.000Z".into(),
                use_count: 0,
            }],
            web_search_templates: Vec::new(),
            exclusion_rules: vec![ExclusionRule {
                id: "exclusion-rule:result_id:appnotepad".into(),
                match_type: "result_id".into(),
                pattern: "app:notepad".into(),
                created_at: "2026-06-01T00:00:00.000Z".into(),
                updated_at: "2026-06-01T00:00:00.000Z".into(),
            }],
        };

        let json = serde_json::to_string(&export).expect("serialize export");

        assert!(json.contains("customCommands"));
        assert!(json.contains("phrases"));
        assert!(json.contains("exclusionRules"));
        assert!(json.contains("search.weight.apps"));
        assert!(!json.contains("ai.api_key"));
    }

    #[test]
    fn startup_registry_add_args_quote_executable_path() {
        let args = startup_registry_args(true, r"C:\Program Files\Easy Launcher\easy.exe");

        assert_eq!(args[0], "add");
        assert!(args.contains(&STARTUP_RUN_KEY.to_string()));
        assert!(args.contains(&STARTUP_VALUE_NAME.to_string()));
        assert!(args.contains(&r#""C:\Program Files\Easy Launcher\easy.exe""#.to_string()));
        assert!(args.contains(&"/f".to_string()));
    }

    #[test]
    fn startup_registry_delete_args_remove_run_value() {
        let args = startup_registry_args(false, "");

        assert_eq!(args[0], "delete");
        assert!(args.contains(&STARTUP_RUN_KEY.to_string()));
        assert!(args.contains(&STARTUP_VALUE_NAME.to_string()));
        assert!(args.contains(&"/f".to_string()));
    }
}
