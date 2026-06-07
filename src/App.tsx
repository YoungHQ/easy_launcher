import { useEffect, useMemo, useRef, useState } from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import {
  normalizeLanguagePreference,
  resolveDisplayLanguage,
  useDisplayTranslations,
} from "./i18n";
import type { LanguagePreference } from "./i18n";

type ResultKind =
  | "app"
  | "file"
  | "command"
  | "calculator"
  | "aiAction"
  | "webSearch"
  | "tool";
type ActionKind =
  | "launchApp"
  | "openFile"
  | "runCommand"
  | "copyText"
  | "aiTranslate"
  | "aiSummarize"
  | "openUrl";

type SearchResult = {
  id: string;
  title: string;
  subtitle: string;
  kind: ResultKind;
  action: ActionKind;
  source: string;
  score: number;
  shortcut?: string;
  fileMetadata?: FileMetadata | null;
  iconPath?: string | null;
};

type FileMetadata = {
  isDir: boolean;
  sizeBytes?: number | null;
  modifiedUnixSeconds?: number | null;
  extension?: string | null;
  fullPath: string;
};

type ShortcutStatus = {
  shortcut: string;
  registered: boolean;
  message: string;
};

type StorageStatus = {
  dataDir: string;
  databasePath: string;
  initialized: boolean;
};

type EverythingStatus = {
  installed: boolean;
  running: boolean;
  ipcAvailable: boolean;
  httpAvailable: boolean;
  installPath?: string;
  message: string;
};

type SelectionCaptureResult = {
  ok: boolean;
  text: string;
  message: string;
};

type EverythingStatusGuide = {
  tone: "ok" | "warning" | "neutral";
  title: string;
  detail: string;
};

type SelectionCaptureEvent = {
  result: SelectionCaptureResult;
  x?: number | null;
  y?: number | null;
};

type SelectionTriggerMode = "ctrl_mouse";
type ViewMode = "launcher" | "ai";
type AiSettingsTab = "providers" | "assistants";
type LanguageOption = LanguagePreference;

type AiModelProfile = {
  id: string;
  providerType: "openai" | "anthropic" | "google" | "openai_compatible";
  name: string;
  baseUrl: string;
  apiKey: string;
  modelName: string;
  temperature?: number | null;
  topP?: number | null;
  maxTokens?: number | null;
  presencePenalty?: number | null;
  frequencyPenalty?: number | null;
  stream: boolean;
  enabled: boolean;
  sortOrder: number;
  lastUsedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

type AiProvider = {
  id: string;
  name: string;
  providerType: "openai_compatible";
  baseUrl: string;
  apiKey: string;
  enabled: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
};

type AiProviderDraft = {
  id?: string;
  name: string;
  providerType: "openai_compatible";
  baseUrl: string;
  apiKey: string;
  enabled: boolean;
  sortOrder: number;
};

type AiProviderModel = {
  id: string;
  providerId: string;
  modelName: string;
  enabled: boolean;
  sortOrder: number;
  lastUsedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

type AiSelectionAction = {
  assistantId: string;
  assistantName: string;
  assistantIcon: string;
  assistantDescription: string;
  assistantModelProfileId: string;
  systemPrompt: string;
  assistantEnabled: boolean;
  showInSelection: boolean;
  selectionLabel: string;
  sortOrder: number;
  lastProviderId?: string | null;
  lastModelName?: string | null;
};

type AiAssistant = {
  id: string;
  name: string;
  icon: string;
  description: string;
  modelProfileId: string;
  systemPrompt: string;
  enabled: boolean;
  sortOrder: number;
  lastUsedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

type AiAssistantDraft = {
  id?: string;
  name: string;
  icon: string;
  description: string;
  modelProfileId: string;
  systemPrompt: string;
  enabled: boolean;
  sortOrder: number;
};

type AiConversation = {
  id: string;
  assistantId: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  lastMessageAt?: string | null;
};

type AiMessage = {
  id: string;
  conversationId: string;
  role: "user" | "assistant" | "system";
  content: string;
  status: "streaming" | "complete" | "error";
  error?: string | null;
  createdAt: string;
};

type AiChatStarted = {
  requestId: string;
  conversationId: string;
  userMessage: AiMessage;
  assistantMessage: AiMessage;
};

type AiChatDeltaEvent = {
  requestId: string;
  conversationId: string;
  messageId: string;
  delta: string;
};

type AiChatDoneEvent = {
  requestId: string;
  conversationId: string;
  messageId: string;
  content: string;
};

type AiChatErrorEvent = {
  requestId: string;
  conversationId: string;
  messageId: string;
  error: string;
};

type SearchSourceSettings = {
  apps: boolean;
  files: boolean;
  calculator: boolean;
  system: boolean;
  ai: boolean;
  phrase: boolean;
  webSearch: boolean;
  tools: boolean;
};

type SearchWeightSettings = {
  apps: number;
  files: number;
  calculator: number;
  system: number;
  ai: number;
  phrase: number;
  webSearch: number;
  tools: number;
};

type PasswordOptions = {
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  hyphen: boolean;
  underscore: boolean;
  special: boolean;
  brackets: boolean;
};

type EverythingSearchOptions = {
  fullPath: boolean;
  searchContent: boolean;
};

type UpdateCheckResult = {
  currentVersion: string;
  latestVersion?: string | null;
  latestTag?: string | null;
  releaseName?: string | null;
  releaseUrl?: string | null;
  publishedAt?: string | null;
  isNewer?: boolean | null;
  isPrerelease: boolean;
  assetName?: string | null;
  assetDownloadUrl?: string | null;
  error?: string | null;
};

type UpdateStatusGuide = {
  tone: "ok" | "warning" | "neutral";
  title: string;
  detail: string;
};

type ConfigExportResult = {
  path: string;
  settingCount: number;
};

type ConfigImportResult = {
  importedCount: number;
  ignoredCount: number;
};

type CustomCommand = {
  id: string;
  name: string;
  commandType: CustomCommandType;
  target: string;
  createdAt: string;
  updatedAt: string;
};

type CustomCommandType = "url" | "file" | "program";

type CustomCommandDraft = {
  id?: string;
  name: string;
  commandType: CustomCommandType;
  target: string;
};

type Phrase = {
  id: string;
  title: string;
  text: string;
  createdAt: string;
  updatedAt: string;
  useCount: number;
};

type PhraseDraft = {
  id?: string;
  title: string;
  text: string;
};

type WebSearchTemplate = {
  id: string;
  keyword: string;
  name: string;
  urlTemplate: string;
  createdAt: string;
  updatedAt: string;
};

type WebSearchTemplateDraft = {
  id?: string;
  keyword: string;
  name: string;
  urlTemplate: string;
};

type ExclusionMatchType = "result_id" | "path_pattern";

type ExclusionRule = {
  id: string;
  matchType: ExclusionMatchType;
  pattern: string;
  createdAt: string;
  updatedAt: string;
};

type ExclusionRuleDraft = {
  id?: string;
  matchType: ExclusionMatchType;
  pattern: string;
};

type PinnedResult = {
  resultId: string;
  kind: string;
  title: string;
  target: string;
  createdAt: string;
  updatedAt: string;
};

type ResultAlias = {
  alias: string;
  normalizedAlias: string;
  resultId: string;
  kind: string;
  title: string;
  target: string;
  createdAt: string;
  updatedAt: string;
};

type ResultContextActionId =
  | "execute"
  | "pinResult"
  | "unpinResult"
  | "addAlias"
  | "deleteAlias"
  | "openParent"
  | "revealPath"
  | "copyPath"
  | "copyName"
  | "copyFile"
  | "showNativeContextMenu"
  | "openConfiguredEditor"
  | "openWith"
  | "openTerminal"
  | "addQuickAccess"
  | "removeQuickAccess"
  | "runAsAdmin"
  | "runAsUser"
  | "openShortcutTargetParent"
  | "deletePath"
  | "hideResult";

type ResultContextAction = {
  id: ResultContextActionId;
  title: string;
  subtitle: string;
  danger?: boolean;
};

type ResultContextSession = {
  result: SearchResult;
  actions: ResultContextAction[];
  filter: string;
};

type SearchProgressEvent = {
  requestId: number;
  results: SearchResult[];
};

type SearchIconUpdate = {
  resultId: string;
  iconPath: string;
};

type SearchIconsUpdatedEvent = {
  requestId: number;
  icons: SearchIconUpdate[];
};

type IconCacheStatus = {
  directory: string;
  fileCount: number;
  sizeBytes: number;
};

type IconCacheClearResult = {
  clearedCount: number;
  status: IconCacheStatus;
};

type ErrorScope =
  | "快捷键"
  | "Everything"
  | "AI"
  | "配置"
  | "搜索"
  | "系统"
  | "划词"
  | "更新";

type SettingsSection =
  | "general"
  | "search"
  | "tools"
  | "ai"
  | "selection"
  | "commands"
  | "phrases"
  | "webSearch"
  | "exclusions"
  | "updates"
  | "backup";

type AppError = {
  scope: ErrorScope;
  title: string;
  message: string;
  detail?: string;
};

const kindMarks: Record<ResultKind, string> = {
  app: "A",
  file: "F",
  command: ">",
  calculator: "=",
  aiAction: "AI",
  webSearch: "WEB",
  tool: "T",
};

const actionLabels: Record<ActionKind, string> = {
  launchApp: "启动",
  openFile: "打开",
  runCommand: "运行",
  copyText: "复制",
  aiTranslate: "翻译",
  aiSummarize: "总结",
  openUrl: "打开",
};

const SEARCH_DEBOUNCE_MS = 160;
const LAUNCHER_WINDOW_WIDTH = 728;
const SETTINGS_WINDOW_WIDTH = 960;
const SETTINGS_WINDOW_HEIGHT = 700;
const AI_WINDOW_WIDTH = 1040;
const AI_WINDOW_HEIGHT = 680;
const SELECTION_PICKER_WINDOW_WIDTH = 520;
const SELECTION_PICKER_WINDOW_HEIGHT = 48;
const SELECTION_RESULT_WINDOW_WIDTH = 520;
const SELECTION_RESULT_WINDOW_HEIGHT = 420;
const SELECTION_PRIMARY_ACTION_LIMIT = 5;
const MODEL_BINDING_SEPARATOR = "::";
const SEARCH_WINDOW_MIN_HEIGHT = 64;
const SEARCH_WINDOW_MAX_HEIGHT = 286;
const SEARCH_WINDOW_BOTTOM_GUTTER = 10;
const BUILTIN_SELECTION_ASSISTANT_IDS = new Set([
  "translation-assistant",
  "summary-assistant",
  "professional-explanation-assistant",
  "polish-assistant",
  "key-points-assistant",
]);

const settingsSections: { id: SettingsSection; label: string; meta: string }[] = [
  { id: "general", label: "通用", meta: "快捷键、系统" },
  { id: "search", label: "搜索", meta: "来源、权重" },
  { id: "tools", label: "工具", meta: "密码、转换" },
  { id: "ai", label: "AI", meta: "模型、助手" },
  { id: "selection", label: "划词", meta: "触发、浮窗" },
  { id: "commands", label: "命令", meta: "固定入口" },
  { id: "phrases", label: "短语", meta: "常用文本" },
  { id: "webSearch", label: "网页", meta: "搜索模板" },
  { id: "exclusions", label: "隐藏", meta: "排除规则" },
  { id: "updates", label: "更新", meta: "版本、Release" },
  { id: "backup", label: "配置", meta: "导入导出" },
];

function App() {
  return getCurrentWindow().label === "selection" ? <SelectionAssistantApp /> : <MainApp />;
}

function MainApp() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [resultsRevision, setResultsRevision] = useState(0);
  const resultIconPathsRef = useRef<Map<string, string>>(new Map());
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [backendMessage, setBackendMessage] = useState("后端未验证");
  const [shortcutStatus, setShortcutStatus] = useState<ShortcutStatus>({
    shortcut: "Alt+1",
    registered: false,
    message: "Alt+1 用于呼出启动器",
  });
  const [aiShortcutStatus, setAiShortcutStatus] = useState<ShortcutStatus>({
    shortcut: "Alt+3",
    registered: false,
    message: "Alt+3 用于打开 AI 面板",
  });
  const [storageStatus, setStorageStatus] = useState<StorageStatus | null>(null);
  const [everythingStatus, setEverythingStatus] = useState<EverythingStatus | null>(null);
  const [iconCacheStatus, setIconCacheStatus] = useState<IconCacheStatus | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("launcher");
  const [storedShortcut, setStoredShortcut] = useState("Alt+1");
  const [shortcutInput, setShortcutInput] = useState("Alt+1");
  const [fileEditorPath, setFileEditorPath] = useState("");
  const [folderEditorPath, setFolderEditorPath] = useState("");
  const [everythingExePath, setEverythingExePath] = useState("");
  const [selectionTriggerMode, setSelectionTriggerMode] =
    useState<SelectionTriggerMode>("ctrl_mouse");
  const [selectionEnabled, setSelectionEnabled] = useState(true);
  const [doubleAltEnabled, setDoubleAltEnabled] = useState(true);
  const [startupEnabled, setStartupEnabled] = useState(false);
  const [languageOption, setLanguageOption] = useState<LanguageOption>("system");
  const [showSettings, setShowSettings] = useState(false);
  const [activeSettingsSection, setActiveSettingsSection] =
    useState<SettingsSection>("general");
  const [activeAiSettingsTab, setActiveAiSettingsTab] =
    useState<AiSettingsTab>("providers");
  const [aiShortcutInput, setAiShortcutInput] = useState("Alt+3");
  const [aiModelProfiles, setAiModelProfiles] = useState<AiModelProfile[]>([]);
  const [aiProviders, setAiProviders] = useState<AiProvider[]>([]);
  const [aiProviderModels, setAiProviderModels] = useState<AiProviderModel[]>([]);
  const [selectedAiProviderId, setSelectedAiProviderId] = useState<string | null>(null);
  const [aiProviderDraft, setAiProviderDraft] = useState<AiProviderDraft>({
    name: "本地 OpenAI 兼容接口",
    providerType: "openai_compatible",
    baseUrl: "",
    apiKey: "",
    enabled: true,
    sortOrder: 0,
  });
  const [aiModelSearch, setAiModelSearch] = useState("");
  const [manualAiModelName, setManualAiModelName] = useState("");
  const [showAiApiKey, setShowAiApiKey] = useState(false);
  const [aiSelectionActions, setAiSelectionActions] = useState<AiSelectionAction[]>([]);
  const [aiAssistants, setAiAssistants] = useState<AiAssistant[]>([]);
  const [aiConversations, setAiConversations] = useState<AiConversation[]>([]);
  const [aiMessages, setAiMessages] = useState<AiMessage[]>([]);
  const [selectedAiAssistantId, setSelectedAiAssistantId] = useState<string | null>(null);
  const [selectedAiConversationId, setSelectedAiConversationId] = useState<string | null>(null);
  const [renamingAiConversationId, setRenamingAiConversationId] = useState<string | null>(null);
  const [renamingAiConversationTitle, setRenamingAiConversationTitle] = useState("");
  const [aiInput, setAiInput] = useState("");
  const [activeAiRequestId, setActiveAiRequestId] = useState<string | null>(null);
  const [isFetchingAiModels, setIsFetchingAiModels] = useState(false);
  const [aiAssistantDraft, setAiAssistantDraft] = useState<AiAssistantDraft>({
    name: "默认助手",
    icon: "AI",
    description: "",
    modelProfileId: "",
    systemPrompt: "",
    enabled: true,
    sortOrder: 0,
  });
  const [searchSources, setSearchSources] = useState<SearchSourceSettings>({
    apps: true,
    files: true,
    calculator: true,
    system: true,
    ai: true,
    phrase: true,
    webSearch: true,
    tools: true,
  });
  const [searchWeights, setSearchWeights] = useState<SearchWeightSettings>({
    apps: 1,
    files: 1,
    calculator: 1,
    system: 1,
    ai: 1,
    phrase: 1,
    webSearch: 1,
    tools: 1,
  });
  const [passwordOptions, setPasswordOptions] = useState<PasswordOptions>({
    length: 16,
    uppercase: true,
    lowercase: true,
    digits: true,
    hyphen: false,
    underscore: false,
    special: true,
    brackets: false,
  });
  const [toolMenuAlias, setToolMenuAlias] = useState("/");
  const [everythingSearchOptions, setEverythingSearchOptions] =
    useState<EverythingSearchOptions>({
      fullPath: false,
      searchContent: false,
    });
  const [appVersion, setAppVersion] = useState("0.1.0");
  const [includePrereleaseUpdates, setIncludePrereleaseUpdates] = useState(false);
  const [lastUpdateCheckAt, setLastUpdateCheckAt] = useState<string | null>(null);
  const [dismissedUpdateTag, setDismissedUpdateTag] = useState<string | null>(null);
  const [updateCheckResult, setUpdateCheckResult] = useState<UpdateCheckResult | null>(null);
  const [isCheckingUpdates, setIsCheckingUpdates] = useState(false);
  const [importPath, setImportPath] = useState("");
  const [customCommands, setCustomCommands] = useState<CustomCommand[]>([]);
  const [customCommandDraft, setCustomCommandDraft] = useState<CustomCommandDraft>({
    name: "",
    commandType: "url",
    target: "",
  });
  const [phrases, setPhrases] = useState<Phrase[]>([]);
  const [phraseDraft, setPhraseDraft] = useState<PhraseDraft>({
    title: "",
    text: "",
  });
  const [webSearchTemplates, setWebSearchTemplates] = useState<WebSearchTemplate[]>([]);
  const [webSearchTemplateDraft, setWebSearchTemplateDraft] = useState<WebSearchTemplateDraft>({
    keyword: "",
    name: "",
    urlTemplate: "https://www.bing.com/search?q={query}",
  });
  const [exclusionRules, setExclusionRules] = useState<ExclusionRule[]>([]);
  const [exclusionRuleDraft, setExclusionRuleDraft] = useState<ExclusionRuleDraft>({
    matchType: "result_id",
    pattern: "",
  });
  const [pinnedResults, setPinnedResults] = useState<PinnedResult[]>([]);
  const [resultAliases, setResultAliases] = useState<ResultAlias[]>([]);
  const [smartRankingEnabled, setSmartRankingEnabled] = useState(true);
  const [actionMessage, setActionMessage] = useState("输入关键词搜索");
  const [appError, setAppError] = useState<AppError | null>(null);
  const [contextSession, setContextSession] = useState<ResultContextSession | null>(null);
  const launcherRef = useRef<HTMLElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const resultItemRefs = useRef<Array<HTMLDivElement | null>>([]);
  const aiInputRef = useRef<HTMLTextAreaElement | null>(null);
  const latestSearchRequestId = useRef(0);
  const aiMessagesRef = useRef<AiMessage[]>([]);
  const displayLanguage = resolveDisplayLanguage(languageOption);
  useDisplayTranslations(displayLanguage);
  const searchSourceLabels = {
    apps: displayLanguage === "en-US" ? "Apps" : "应用",
    files: displayLanguage === "en-US" ? "Files" : "文件",
    calculator: displayLanguage === "en-US" ? "Calculator" : "计算",
    system: displayLanguage === "en-US" ? "Commands" : "命令",
    ai: "AI",
    phrase: displayLanguage === "en-US" ? "Phrases" : "短语",
    webSearch: displayLanguage === "en-US" ? "Web" : "网页",
    tools: displayLanguage === "en-US" ? "Tools" : "工具",
  };

  const visibleContextActions = useMemo(() => {
    if (!contextSession) {
      return [];
    }

    const normalizedFilter = contextSession.filter.trim().toLowerCase();
    if (!normalizedFilter) {
      return contextSession.actions;
    }

    return contextSession.actions.filter(
      (action) =>
        action.title.toLowerCase().includes(normalizedFilter) ||
        action.subtitle.toLowerCase().includes(normalizedFilter),
    );
  }, [contextSession]);

  const displayResults = useMemo(() => {
    const trimmed = query.trim().toLowerCase();
    const optionMatches =
      trimmed.length > 0 &&
      ("option".includes(trimmed) ||
        "options".includes(trimmed) ||
        "设置".includes(trimmed) ||
        "配置".includes(trimmed));
    const optionResult: SearchResult = {
      id: "internal:settings",
      title: "设置",
      subtitle: "打开 Easy Launcher 设置",
      kind: "command",
      action: "runCommand",
      source: "设置",
      score: 1,
      shortcut: "Enter",
    };

    const baseResults = optionMatches
      ? [optionResult, ...results.filter((result) => result.id !== optionResult.id)]
      : results;

    if (!contextSession) {
      return baseResults;
    }

    return visibleContextActions.map((action) =>
      contextActionResult(contextSession.result, action),
    );
  }, [contextSession, query, results, visibleContextActions]);

  const selectedDisplayResult = useMemo(
    () => displayResults[Math.min(selectedIndex, Math.max(displayResults.length - 1, 0))],
    [displayResults, selectedIndex],
  );
  const selectedAiAssistant =
    aiAssistants.find((assistant) => assistant.id === selectedAiAssistantId) ?? aiAssistants[0];
  const selectedAiConversation =
    aiConversations.find((conversation) => conversation.id === selectedAiConversationId) ?? null;
  const selectedAiProvider = selectedAiProviderId
    ? aiProviders.find((provider) => provider.id === selectedAiProviderId) ?? null
    : null;
  const selectedAiProviderModels = selectedAiProvider
    ? aiProviderModels.filter((model) => model.providerId === selectedAiProvider.id)
    : [];
  const filteredAiProviderModels = selectedAiProviderModels.filter((model) =>
    model.modelName.toLowerCase().includes(aiModelSearch.trim().toLowerCase()),
  );
  const enabledAiProviderModels = aiProviderModels.filter((model) => {
    const provider = aiProviders.find((item) => item.id === model.providerId);
    return model.enabled && provider?.enabled;
  });
  const enabledAiModelBindings = enabledAiProviderModels.map((model) => {
    const provider = aiProviders.find((item) => item.id === model.providerId);
    return {
      value: modelBindingValue(model.providerId, model.modelName),
      label: `${provider?.name ?? "未知供应商"} / ${model.modelName}`,
      provider,
      model,
    };
  });
  const selectedAssistantModelBinding = profileIdToModelBindingValue(
    aiAssistantDraft.modelProfileId,
    aiModelProfiles,
    aiProviders,
    enabledAiProviderModels,
  );
  const visibleSelectionActions = aiSelectionActions.filter(
    (action) => action.showInSelection && action.assistantEnabled,
  );
  const selectionActionRows = aiAssistants.map((assistant) => {
    const action = aiSelectionActions.find((item) => item.assistantId === assistant.id);
    return (
      action ?? {
        assistantId: assistant.id,
        assistantName: assistant.name,
        assistantIcon: assistant.icon,
        assistantDescription: assistant.description,
        assistantModelProfileId: assistant.modelProfileId,
        systemPrompt: assistant.systemPrompt,
        assistantEnabled: assistant.enabled,
        showInSelection: false,
        selectionLabel: assistant.name,
        sortOrder: assistant.sortOrder,
      }
    );
  });
  const enabledModelCountByProvider = aiProviderModels.reduce<Record<string, number>>(
    (counts, model) => {
      if (model.enabled) {
        counts[model.providerId] = (counts[model.providerId] ?? 0) + 1;
      }
      return counts;
    },
    {},
  );
  const aiProfileMissing =
    aiModelProfiles.length === 0 ||
    aiModelProfiles.every(
      (profile) => !profile.enabled || !profile.baseUrl.trim() || !profile.modelName.trim(),
    );
  const everythingStatusGuide = everythingStatusGuideFromStatus(everythingStatus);
  const updateStatusGuide = updateStatusGuideFromResult(
    updateCheckResult,
    isCheckingUpdates,
    dismissedUpdateTag,
    lastUpdateCheckAt,
  );

  useEffect(() => {
    aiMessagesRef.current = aiMessages;
  }, [aiMessages]);

  useEffect(() => {
    if (!launcherRef.current) {
      return;
    }

    const contentHeight =
      Math.ceil(launcherRef.current.getBoundingClientRect().height) +
      (showSettings || viewMode === "ai" ? 0 : SEARCH_WINDOW_BOTTOM_GUTTER);
    const width =
      showSettings
        ? SETTINGS_WINDOW_WIDTH
        : viewMode === "ai"
          ? AI_WINDOW_WIDTH
          : LAUNCHER_WINDOW_WIDTH;
    const height = showSettings
      ? SETTINGS_WINDOW_HEIGHT
      : viewMode === "ai"
        ? AI_WINDOW_HEIGHT
        : Math.max(SEARCH_WINDOW_MIN_HEIGHT, Math.min(contentHeight, SEARCH_WINDOW_MAX_HEIGHT));

    getCurrentWindow()
      .setSize(new LogicalSize(width, height))
      .catch(() => {
        // Browser preview has no desktop window to resize.
      });
  }, [
    appError,
    displayResults.length,
    query,
    selectedIndex,
    showSettings,
    viewMode,
  ]);

  useEffect(() => {
    if (viewMode !== "launcher" || showSettings) {
      return;
    }

    resultItemRefs.current[selectedIndex]?.scrollIntoView({
      block: "nearest",
      inline: "nearest",
    });
  }, [selectedIndex, displayResults.length, showSettings, viewMode]);

  function showError(scope: ErrorScope, title: string, error?: unknown) {
    const message = errorMessage(error, title);
    setAppError({
      scope,
      title,
      message,
      detail: typeof error === "string" && error !== message ? error : undefined,
    });
    setActionMessage(message);
  }

  function clearError() {
    setAppError(null);
  }

  function focusSearchInput() {
    window.setTimeout(() => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    }, 0);
    window.setTimeout(() => {
      searchInputRef.current?.focus();
    }, 80);
  }

  async function openSettingsPanel(
    section: SettingsSection = "general",
    options: { syncWindow?: boolean; aiTab?: AiSettingsTab } = {},
  ) {
    setViewMode("launcher");
    setShowSettings(true);
    setActiveSettingsSection(section);
    if (options.aiTab) {
      setActiveAiSettingsTab(options.aiTab);
    }
    setActionMessage("已打开设置");
    clearError();
    if (options.syncWindow === false) {
      return;
    }
    try {
      await invoke("show_settings_window");
    } catch {
      // Browser preview has no desktop window to resize.
    }
  }

  async function openSearchPanel() {
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
    setViewMode("launcher");
    setShowSettings(false);
    setContextSession(null);
    focusSearchInput();
    try {
      await invoke("show_search_window");
      focusSearchInput();
    } catch {
      // Browser preview has no desktop window to resize.
    }
  }

  async function closeSettingsPanel() {
    await openSearchPanel();
  }

  async function hideLauncherWindow() {
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
    try {
      await invoke("hide_main_window");
    } catch {
      // Browser preview has no desktop window to hide.
    }
  }

  function reserveSearchRequestId() {
    latestSearchRequestId.current += 1;
    return latestSearchRequestId.current;
  }

  function isLatestSearchRequest(requestId: number) {
    return latestSearchRequestId.current === requestId;
  }

  async function invokeSearchWithRecents(searchQuery: string, requestId = reserveSearchRequestId()) {
    const response = await invoke<SearchResult[]>("search_with_recents", {
      query: searchQuery,
      requestId,
    });

    return isLatestSearchRequest(requestId) ? response : null;
  }

  function setSearchResults(nextResults: SearchResult[]) {
    setResults((current) => {
      const resultIconPaths = resultIconPathsRef.current;
      cacheResultIcons(current, resultIconPaths);
      const mergedResults = mergeResultsPreservingIcons(
        nextResults,
        current,
        resultIconPaths,
      );
      cacheResultIcons(mergedResults, resultIconPaths);
      return mergedResults;
    });
    setResultsRevision((current) => current + 1);
  }

  useEffect(() => {
    async function loadRuntimeStatus() {
      try {
        const status = await invoke<ShortcutStatus>("launcher_shortcut_status");
        setShortcutStatus(status);
        const aiStatus = await invoke<ShortcutStatus>("ai_shortcut_status");
        setAiShortcutStatus(aiStatus);
        setAiShortcutInput(aiStatus.shortcut);
      } catch (error) {
        setShortcutStatus({
          shortcut: "Alt+1",
          registered: false,
          message: "浏览器预览模式，快捷键仅在桌面端生效",
        });
        setAiShortcutStatus({
          shortcut: "Alt+3",
          registered: false,
          message: "浏览器预览模式，AI 快捷键仅在桌面端生效",
        });
        showError("快捷键", "快捷键状态读取失败", error);
      }

      try {
        const status = await invoke<StorageStatus>("storage_status");
        const loadedIconCacheStatus = await invoke<IconCacheStatus>("icon_cache_status");
        const shortcut = await invoke<string | null>("get_setting", {
          key: "launcher.shortcut",
        });

        setStorageStatus(status);
        setIconCacheStatus(loadedIconCacheStatus);
        setStoredShortcut(shortcut ?? "Alt+1");
        setShortcutInput(shortcut ?? "Alt+1");
        const triggerMode = await invoke<string | null>("get_setting", {
          key: "selection.trigger.mode",
        });
        setSelectionTriggerMode("ctrl_mouse");
        const loadedSelectionEnabled = await invoke<string | null>("get_setting", {
          key: "selection.enabled",
        });
        setSelectionEnabled(loadedSelectionEnabled !== "false");
        const doubleAlt = await invoke<string | null>("get_setting", {
          key: "launcher.double_alt.enabled",
        });
        setDoubleAltEnabled(doubleAlt !== "false");
        const startup = await invoke<string | null>("get_setting", {
          key: "startup.enabled",
        });
        setStartupEnabled(startup === "true");
        const loadedLanguage = await invoke<string | null>("get_setting", {
          key: "ui.language",
        });
        setLanguageOption(normalizeLanguagePreference(loadedLanguage));
        await loadUpdateSettings();
        await loadAiData();
        const loadedSources = await invoke<SearchSourceSettings>("get_search_source_settings");
        setSearchSources(loadedSources);
        const loadedWeights = await invoke<SearchWeightSettings>("get_search_weight_settings");
        setSearchWeights(loadedWeights);
        const loadedSmartRanking = await invoke<string | null>("get_setting", {
          key: "search.smart_ranking.enabled",
        });
        setSmartRankingEnabled(loadedSmartRanking !== "false");
        const loadedPasswordOptions = await invoke<PasswordOptions>("get_password_options");
        setPasswordOptions(loadedPasswordOptions);
        const loadedToolMenuAlias = await invoke<string | null>("get_setting", {
          key: "tools.menu.alias",
        });
        setToolMenuAlias(loadedToolMenuAlias ?? "/");
        const loadedEverythingOptions = await invoke<EverythingSearchOptions>(
          "get_everything_search_options",
        );
        setEverythingSearchOptions(loadedEverythingOptions);
        const loadedEverythingExePath = await invoke<string | null>("get_setting", {
          key: "everything.exe.path",
        });
        setEverythingExePath(loadedEverythingExePath ?? "");
        const loadedCommands = await invoke<CustomCommand[]>("list_custom_commands");
        setCustomCommands(loadedCommands);
        const loadedPhrases = await invoke<Phrase[]>("list_phrases");
        setPhrases(loadedPhrases);
        const loadedWebSearchTemplates =
          await invoke<WebSearchTemplate[]>("list_web_search_templates");
        setWebSearchTemplates(loadedWebSearchTemplates);
        const loadedExclusionRules = await invoke<ExclusionRule[]>("list_exclusion_rules");
        setExclusionRules(loadedExclusionRules);
        const loadedPinnedResults = await invoke<PinnedResult[]>("list_pinned_results");
        setPinnedResults(loadedPinnedResults);
        const loadedResultAliases = await invoke<ResultAlias[]>("list_result_aliases");
        setResultAliases(loadedResultAliases);
      } catch (error) {
        setStorageStatus(null);
        showError("配置", "本地设置读取失败", error);
      }

      try {
        const status = await invoke<EverythingStatus>("everything_status");
        setEverythingStatus(status);
      } catch (error) {
        setEverythingStatus(null);
        showError("Everything", "Everything 状态读取失败", error);
      }
    }

    loadRuntimeStatus();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen("launcher-opened", () => {
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
      setViewMode("launcher");
      setShowSettings(false);
      setContextSession(null);
      focusSearchInput();
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);


  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen("ai-opened", () => {
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
      setViewMode("ai");
      setShowSettings(false);
      loadAiData().catch((error) => {
        showError("AI", "AI 数据读取失败", error);
      });
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<AiChatDeltaEvent>("ai-chat-delta", (event) => {
      const payload = event.payload;
      setAiMessages((current) =>
        current.map((message) =>
          message.id === payload.messageId
            ? { ...message, content: `${message.content}${payload.delta}`, status: "streaming" }
            : message,
        ),
      );
    }).then((handler) => unlisteners.push(handler));

    listen<AiChatDoneEvent>("ai-chat-done", (event) => {
      const payload = event.payload;
      setActiveAiRequestId((current) => (current === payload.requestId ? null : current));
      setAiMessages((current) =>
        current.map((message) =>
          message.id === payload.messageId
            ? { ...message, content: payload.content, status: "complete", error: null }
            : message,
        ),
      );
      setActionMessage("AI 回复完成");
    }).then((handler) => unlisteners.push(handler));

    listen<AiChatErrorEvent>("ai-chat-error", (event) => {
      const payload = event.payload;
      setActiveAiRequestId((current) => (current === payload.requestId ? null : current));
      setAiMessages((current) =>
        current.map((message) =>
          message.id === payload.messageId
            ? { ...message, status: "error", error: payload.error }
            : message,
        ),
      );
      showError("AI", "AI 回复失败", payload.error);
    }).then((handler) => unlisteners.push(handler));

    listen<AiChatErrorEvent>("ai-chat-cancelled", (event) => {
      const payload = event.payload;
      setActiveAiRequestId((current) => (current === payload.requestId ? null : current));
      setAiMessages((current) =>
        current.map((message) =>
          message.id === payload.messageId
            ? { ...message, status: "error", error: "AI 请求已取消" }
            : message,
        ),
      );
      setActionMessage("AI 回复已取消");
    }).then((handler) => unlisteners.push(handler));

    return () => {
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen("tray-open-settings", () => {
      openSettingsPanel("general", { syncWindow: false });
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    function handleGlobalKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") {
        return;
      }

      if (contextSession && viewMode === "launcher" && !showSettings) {
        event.preventDefault();
        event.stopPropagation();
        setContextSession(null);
        setSelectedIndex(0);
        setActionMessage("已返回搜索结果");
        return;
      }

      if (showSettings) {
        event.preventDefault();
        event.stopPropagation();
        openSearchPanel();
        return;
      }


      if (viewMode === "launcher" && query.trim().length === 0) {
        event.preventDefault();
        event.stopPropagation();
        hideLauncherWindow();
      }
    }

    window.addEventListener("keydown", handleGlobalKeyDown, true);
    return () => window.removeEventListener("keydown", handleGlobalKeyDown, true);
  }, [contextSession, query, showSettings, viewMode]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<{ section: SettingsSection; aiTab?: AiSettingsTab | null }>("settings-opened", (event) => {
      const section = event.payload.section;
      openSettingsPanel(section, {
        syncWindow: false,
        aiTab: event.payload.aiTab ?? undefined,
      });
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<EverythingStatus>("tray-everything-status", (event) => {
      setEverythingStatus(event.payload);
      setActionMessage(event.payload.message);
      clearError();
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<SearchProgressEvent>("search-progress", (event) => {
      if (!isLatestSearchRequest(event.payload.requestId)) {
        return;
      }

      setSearchResults(event.payload.results);
      setSelectedIndex((current) =>
        Math.min(current, Math.max(event.payload.results.length - 1, 0)),
      );
      setActionMessage(event.payload.results.length > 0 ? "已补充文件结果" : "没有匹配结果");
      clearError();
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<SearchIconsUpdatedEvent>("search-icons-updated", (event) => {
      if (!isLatestSearchRequest(event.payload.requestId)) {
        return;
      }

      setResults((current) => {
        const mergedResults = mergeIconUpdates(current, event.payload.icons);
        cacheResultIcons(mergedResults, resultIconPathsRef.current);
        return mergedResults;
      });
      setResultsRevision((current) => current + 1);
    }).then((handler) => {
      unlisten = handler;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (contextSession) {
      return;
    }

    const requestId = reserveSearchRequestId();
    const searchQuery = query;

    async function runSearch() {
      try {
        const response = await invokeSearchWithRecents(searchQuery, requestId);
        if (response !== null) {
          setSearchResults(response);
          setSelectedIndex(0);
          setActionMessage(response.length > 0 ? "Enter 执行，Esc 清空" : "没有匹配结果");
        }
      } catch (error) {
        if (isLatestSearchRequest(requestId)) {
          const fallback = fallbackResults(searchQuery);
          setSearchResults(fallback);
          setSelectedIndex(0);
          setActionMessage("浏览器预览模式，使用 mock 结果");
          showError("搜索", "搜索失败", error);
        }
      }
    }

    const timeoutId = window.setTimeout(runSearch, SEARCH_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timeoutId);
      if (latestSearchRequestId.current === requestId) {
        latestSearchRequestId.current += 1;
      }
    };
  }, [contextSession, query]);

  async function pingBackend() {
    try {
      const response = await invoke<string>("greet", { name: "Easy Launcher" });
      setBackendMessage(response);
      clearError();
    } catch (error) {
      setBackendMessage("浏览器预览模式");
      showError("系统", "后端连接失败", error);
    }
  }

  async function loadUpdateSettings() {
    try {
      const version = await invoke<string>("get_app_version");
      setAppVersion(version || "0.1.0");
    } catch {
      setAppVersion("0.1.0");
    }

    const includePrerelease = await invoke<string | null>("get_setting", {
      key: "updates.check.include_prerelease",
    });
    const lastCheckedAt = await invoke<string | null>("get_setting", {
      key: "updates.check.last_checked_at",
    });
    const dismissedTag = await invoke<string | null>("get_setting", {
      key: "updates.check.dismissed_tag",
    });
    setIncludePrereleaseUpdates(includePrerelease === "true");
    setLastUpdateCheckAt(lastCheckedAt?.trim() || null);
    setDismissedUpdateTag(dismissedTag?.trim() || null);
  }

  async function loadAiConversations(assistantId: string | null): Promise<AiConversation[]> {
    return invoke<AiConversation[]>("list_ai_conversations", {
      assistantId,
    });
  }

  async function loadAiData(
    nextAssistantId = selectedAiAssistantId,
    nextProviderId = selectedAiProviderId,
  ) {
    const [profiles, providers, providerModels, assistants, selectionActions] = await Promise.all([
      invoke<AiModelProfile[]>("list_ai_model_profiles"),
      invoke<AiProvider[]>("list_ai_providers"),
      invoke<AiProviderModel[]>("list_ai_provider_models"),
      invoke<AiAssistant[]>("list_ai_assistants"),
      invoke<AiSelectionAction[]>("list_ai_selection_actions"),
    ]);
    setAiModelProfiles(profiles);
    setAiProviders(providers);
    setAiProviderModels(providerModels);
    setAiAssistants(assistants);
    setAiSelectionActions(selectionActions);
    const activeProvider = nextProviderId
      ? providers.find((provider) => provider.id === nextProviderId)
      : providers[0];
    if (activeProvider) {
      setSelectedAiProviderId(activeProvider.id);
      setAiProviderDraft(providerToDraft(activeProvider));
    }
    const activeAssistantId = nextAssistantId ?? assistants[0]?.id ?? null;
    setSelectedAiAssistantId(activeAssistantId);
    const activeAssistant = assistants.find((assistant) => assistant.id === activeAssistantId);
    if (activeAssistant) {
      setAiAssistantDraft(assistantToDraft(activeAssistant));
    } else if (profiles[0]) {
      setAiAssistantDraft((current) => ({ ...current, modelProfileId: profiles[0].id }));
    }
    if (activeAssistantId) {
      const conversations = await loadAiConversations(activeAssistantId);
      setAiConversations(conversations);
      const activeConversationId = conversations[0]?.id ?? null;
      setSelectedAiConversationId(activeConversationId);
      const activeConversation = conversations[0] ?? null;
      if (activeConversationId) {
        setAiMessages(
          await invoke<AiMessage[]>("list_ai_messages", { conversationId: activeConversationId }),
        );
      } else {
        setAiMessages([]);
      }
    } else {
      setAiConversations([]);
      setAiMessages([]);
    }
  }

  async function saveAiAssistant() {
    try {
      const binding = parseModelBindingValue(selectedAssistantModelBinding);
      if (!binding) {
        showError("AI", "请先在 AI / 供应商模型 中启用至少一个模型");
        return;
      }
      const profileId = await ensureAiModelProfileForBinding(binding.providerId, binding.modelName);
      const input = {
        ...aiAssistantDraft,
        modelProfileId: profileId,
      };
      const assistant = await invoke<AiAssistant>("save_ai_assistant", { input });
      setSelectedAiAssistantId(assistant.id);
      setActionMessage(`助手已保存：${assistant.name}`);
      await loadAiData(assistant.id);
      clearError();
    } catch (error) {
      showError("AI", "助手保存失败", error);
    }
  }

  async function ensureAiModelProfileForBinding(providerId: string, modelName: string) {
    const provider = aiProviders.find((item) => item.id === providerId);
    const model = aiProviderModels.find(
      (item) => item.providerId === providerId && item.modelName === modelName,
    );
    if (!provider || !model || !provider.enabled || !model.enabled) {
      throw new Error("请先在 AI / 供应商模型 中启用至少一个模型");
    }

    const existing = aiModelProfiles.find(
      (profile) =>
        profile.baseUrl.trim() === provider.baseUrl.trim() &&
        profile.modelName === model.modelName,
    );
    const profile = await invoke<AiModelProfile>("save_ai_model_profile", {
      input: {
        id: existing?.id ?? providerModelProfileId(provider.id, model.modelName),
        providerType: "openai_compatible",
        name: `${provider.name} / ${model.modelName}`,
        baseUrl: provider.baseUrl,
        apiKey: provider.apiKey,
        modelName: model.modelName,
        temperature: existing?.temperature ?? null,
        topP: existing?.topP ?? null,
        maxTokens: existing?.maxTokens ?? null,
        presencePenalty: existing?.presencePenalty ?? null,
        frequencyPenalty: existing?.frequencyPenalty ?? null,
        stream: existing?.stream ?? true,
        enabled: true,
        sortOrder: model.sortOrder,
      },
    });
    setAiModelProfiles((current) => [
      ...current.filter((item) => item.id !== profile.id),
      profile,
    ]);
    return profile.id;
  }

  function editAiAssistant(assistant: AiAssistant) {
    setAiAssistantDraft(assistantToDraft(assistant));
    setSelectedAiAssistantId(assistant.id);
    setActiveAiSettingsTab("assistants");
    setActionMessage(`正在编辑助手：${assistant.name}`);
  }

  function newAiAssistantDraft() {
    const firstBinding = enabledAiModelBindings[0]?.value ?? "";
    setAiAssistantDraft({
      name: "新助手",
      icon: "AI",
      description: "",
      modelProfileId: firstBinding,
      systemPrompt: "",
      enabled: true,
      sortOrder: aiAssistants.length,
    });
    setActiveAiSettingsTab("assistants");
  }

  async function deleteAiAssistant(id: string) {
    if (!window.confirm("删除这个助手？")) {
      return;
    }
    try {
      await invoke("delete_ai_assistant", { id });
      await loadAiData(null);
      setActionMessage("助手已删除");
      clearError();
    } catch (error) {
      showError("AI", "助手删除失败", error);
    }
  }

  async function saveAiProvider() {
    try {
      const provider = await invoke<AiProvider>("save_ai_provider", {
        input: {
          ...aiProviderDraft,
          providerType: "openai_compatible" as const,
          name:
            aiProviderDraft.name.trim() ||
            inferProviderName(aiProviderDraft.baseUrl) ||
            "OpenAI 兼容接口",
          baseUrl: aiProviderDraft.baseUrl.trim(),
          apiKey: aiProviderDraft.apiKey.trim(),
        },
      });
      setSelectedAiProviderId(provider.id);
      setAiProviderDraft(providerToDraft(provider));
      await loadAiData(selectedAiAssistantId, provider.id);
      setActionMessage(`供应商已保存：${provider.name}`);
      clearError();
      return provider;
    } catch (error) {
      showError("AI", "供应商保存失败", error);
      return null;
    }
  }

  function editAiProvider(provider: AiProvider) {
    setSelectedAiProviderId(provider.id);
    setAiProviderDraft(providerToDraft(provider));
    setAiModelSearch("");
    setManualAiModelName("");
    setActionMessage(`正在编辑供应商：${provider.name}`);
  }

  function newAiProviderDraft() {
    setSelectedAiProviderId(null);
    setAiProviderDraft({
      name: "",
      providerType: "openai_compatible",
      baseUrl: "",
      apiKey: "",
      enabled: true,
      sortOrder: aiProviders.length,
    });
    setAiModelSearch("");
    setManualAiModelName("");
  }

  async function deleteAiProvider(id: string) {
    if (!window.confirm("删除这个供应商及其模型？")) {
      return;
    }
    try {
      await invoke("delete_ai_provider", { id });
      await loadAiData(selectedAiAssistantId);
      setActionMessage("供应商已删除");
      clearError();
    } catch (error) {
      showError("AI", "供应商删除失败", error);
    }
  }

  async function fetchAiProviderModels() {
    let targetProvider = selectedAiProvider;
    if (!targetProvider) {
      targetProvider = await saveAiProvider();
    }
    if (!targetProvider) {
      return;
    }
    setIsFetchingAiModels(true);
    try {
      const models = await invoke<AiProviderModel[]>("fetch_ai_provider_models", {
        providerId: targetProvider.id,
      });
      setAiProviderModels((current) => [
        ...current.filter((model) => model.providerId !== targetProvider.id),
        ...models,
      ]);
      setActionMessage(`已获取 ${models.length} 个模型，请勾选需要启用的模型`);
      clearError();
    } catch (error) {
      showError("AI", "无法从该服务获取模型。你可以手动添加模型名称。", error);
    } finally {
      setIsFetchingAiModels(false);
    }
  }

  async function addManualAiProviderModel() {
    const modelName = manualAiModelName.trim();
    if (!modelName) {
      showError("AI", "请输入模型名称");
      return;
    }
    let targetProvider = selectedAiProvider;
    if (!targetProvider) {
      targetProvider = await saveAiProvider();
    }
    if (!targetProvider) {
      showError("AI", "请先保存供应商，再添加模型");
      return;
    }
    try {
      const model = await invoke<AiProviderModel>("save_ai_provider_model", {
        input: {
          providerId: targetProvider.id,
          modelName,
          enabled: true,
          sortOrder: aiProviderModels.filter((item) => item.providerId === targetProvider.id)
            .length,
        },
      });
      setAiProviderModels((current) => [
        ...current.filter(
          (item) =>
            !(item.providerId === model.providerId && item.modelName === model.modelName),
        ),
        model,
      ]);
      setManualAiModelName("");
      setActionMessage(`模型已启用：${model.modelName}`);
      clearError();
    } catch (error) {
      showError("AI", "手动添加模型失败", error);
    }
  }

  async function toggleAiProviderModel(model: AiProviderModel, enabled: boolean) {
    try {
      const updated = await invoke<AiProviderModel>("set_ai_provider_model_enabled", {
        providerId: model.providerId,
        modelName: model.modelName,
        enabled,
      });
      setAiProviderModels((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      setActionMessage(enabled ? `模型已启用：${model.modelName}` : `模型已停用：${model.modelName}`);
      clearError();
    } catch (error) {
      showError("AI", "模型启用状态更新失败", error);
    }
  }

  async function toggleSelectionAction(action: AiSelectionAction, showInSelection: boolean) {
    await saveSelectionAction({
      ...action,
      showInSelection,
    });
  }

  async function updateSelectionActionLabel(action: AiSelectionAction, selectionLabel: string) {
    await saveSelectionAction({
      ...action,
      selectionLabel,
    });
  }

  async function updateSelectionActionSortOrder(action: AiSelectionAction, sortOrder: number) {
    await saveSelectionAction({
      ...action,
      sortOrder,
    });
  }

  async function saveSelectionAction(action: AiSelectionAction) {
    try {
      const updated = await invoke<AiSelectionAction>("save_ai_selection_action", {
        input: {
          assistantId: action.assistantId,
          showInSelection: action.showInSelection,
          selectionLabel: action.selectionLabel,
          sortOrder: action.sortOrder,
        },
      });
      setAiSelectionActions((current) =>
        current.some((item) => item.assistantId === updated.assistantId)
          ? current.map((item) => (item.assistantId === updated.assistantId ? updated : item))
          : [...current, updated],
      );
      setActionMessage(
        updated.showInSelection
          ? `已显示划词动作：${action.selectionLabel}`
          : `已隐藏划词动作：${action.selectionLabel}`,
      );
      clearError();
    } catch (error) {
      showError("划词", "划词设置保存失败", error);
    }
  }

  async function saveAiShortcut() {
    try {
      const status = await invoke<ShortcutStatus>("set_ai_shortcut", {
        shortcut: aiShortcutInput,
      });
      setAiShortcutStatus(status);
      setAiShortcutInput(status.shortcut);
      setActionMessage("AI 快捷键已更新");
      clearError();
    } catch (error) {
      showError("快捷键", "AI 快捷键更新失败", error);
    }
  }

  async function selectAiAssistant(id: string) {
    setSelectedAiAssistantId(id);
    const assistant = aiAssistants.find((item) => item.id === id);
    if (assistant) {
      setAiAssistantDraft(assistantToDraft(assistant));
    }
    try {
      const conversations = await loadAiConversations(id);
      setAiConversations(conversations);
      const conversationId = conversations[0]?.id ?? null;
      setSelectedAiConversationId(conversationId);
      setAiMessages(
        conversationId
          ? await invoke<AiMessage[]>("list_ai_messages", { conversationId })
          : [],
      );
      clearError();
    } catch (error) {
      showError("AI", "会话读取失败", error);
    }
  }

  async function selectAiConversation(id: string) {
    setSelectedAiConversationId(id);
    const conversation = aiConversations.find((item) => item.id === id);
    if (conversation && conversation.assistantId !== selectedAiAssistantId) {
      setSelectedAiAssistantId(conversation.assistantId);
      const assistant = aiAssistants.find((item) => item.id === conversation.assistantId);
      if (assistant) {
        setAiAssistantDraft(assistantToDraft(assistant));
      }
    }
    try {
      setAiMessages(await invoke<AiMessage[]>("list_ai_messages", { conversationId: id }));
      clearError();
    } catch (error) {
      showError("AI", "消息读取失败", error);
    }
  }

  function startNewAiConversation() {
    setSelectedAiConversationId(null);
    setAiMessages([]);
    setRenamingAiConversationId(null);
    setActionMessage("已准备新 AI 会话");
    window.setTimeout(() => aiInputRef.current?.focus(), 0);
  }

  async function sendAiMessage() {
    if (!selectedAiAssistant || !aiInput.trim() || activeAiRequestId) {
      return;
    }
    const requestId = `ai-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const message = aiInput.trim();
    const conversationId =
      selectedAiConversation?.assistantId === selectedAiAssistant.id ? selectedAiConversation.id : null;
    const isNewConversation = conversationId === null;
    setAiInput("");
    setActiveAiRequestId(requestId);
    try {
      const started = await invoke<AiChatStarted>("send_ai_chat_message", {
        request: {
          requestId,
          assistantId: selectedAiAssistant.id,
          conversationId,
          message,
        },
      });
      setSelectedAiConversationId(started.conversationId);
      setAiMessages((current) =>
        isNewConversation
          ? [started.userMessage, started.assistantMessage]
          : [...current, started.userMessage, started.assistantMessage],
      );
      const conversations = await loadAiConversations(selectedAiAssistant.id);
      setAiConversations(conversations);
      setSelectedAiConversationId(started.conversationId);
      setActionMessage("AI 正在回复");
      clearError();
    } catch (error) {
      setActiveAiRequestId(null);
      setAiInput(message);
      showError("AI", "发送消息失败", error);
    }
  }

  async function cancelAiMessage() {
    if (!activeAiRequestId) {
      return;
    }
    try {
      await invoke("cancel_ai_chat_message", { requestId: activeAiRequestId });
      setActionMessage("正在取消 AI 回复");
    } catch (error) {
      showError("AI", "取消 AI 回复失败", error);
    }
  }

  async function deleteAiMessage(id: string) {
    try {
      await invoke("delete_ai_message", { id });
      setAiMessages((current) => current.filter((message) => message.id !== id));
      setActionMessage("消息已删除");
      clearError();
    } catch (error) {
      showError("AI", "消息删除失败", error);
    }
  }

  async function deleteAiConversation(id: string) {
    if (!window.confirm("删除这个会话及其消息？")) {
      return;
    }
    try {
      await invoke("delete_ai_conversation", { id });
      await loadAiData(selectedAiAssistantId);
      setActionMessage("会话已删除");
      clearError();
    } catch (error) {
      showError("AI", "会话删除失败", error);
    }
  }

  function startRenameAiConversation(conversation: AiConversation) {
    setRenamingAiConversationId(conversation.id);
    setRenamingAiConversationTitle(conversation.title || "新会话");
  }

  async function saveAiConversationTitle() {
    if (!renamingAiConversationId) {
      return;
    }
    try {
      await invoke("rename_ai_conversation", {
        id: renamingAiConversationId,
        title: renamingAiConversationTitle,
      });
      setAiConversations((current) =>
        current.map((conversation) =>
          conversation.id === renamingAiConversationId
            ? { ...conversation, title: renamingAiConversationTitle }
            : conversation,
        ),
      );
      setRenamingAiConversationId(null);
      setActionMessage("会话已重命名");
      clearError();
    } catch (error) {
      showError("AI", "会话重命名失败", error);
    }
  }

  async function saveLauncherShortcut() {
    try {
      const status = await invoke<ShortcutStatus>("set_launcher_shortcut", {
        shortcut: shortcutInput,
      });
      setShortcutStatus(status);
      setStoredShortcut(status.shortcut);
      setShortcutInput(status.shortcut);
      setActionMessage("主快捷键已更新");
      clearError();
    } catch (error) {
      showError("快捷键", "主快捷键更新失败", error);
    }
  }

  async function saveEditorPaths(
    nextFileEditorPath = fileEditorPath,
    nextFolderEditorPath = folderEditorPath,
  ) {
    const normalizedFileEditorPath = nextFileEditorPath.trim();
    const normalizedFolderEditorPath = nextFolderEditorPath.trim();

    try {
      await invoke("set_setting", {
        key: "file.editor.path",
        value: normalizedFileEditorPath,
      });
      await invoke("set_setting", {
        key: "folder.editor.path",
        value: normalizedFolderEditorPath,
      });
      setFileEditorPath(normalizedFileEditorPath);
      setFolderEditorPath(normalizedFolderEditorPath);
      setActionMessage("编辑器路径已保存");
      clearError();
    } catch (error) {
      showError("配置", "编辑器路径保存失败", error);
    }
  }

  async function chooseEditorPath(kind: "file" | "folder") {
    try {
      const selected = await open({
        title: kind === "file" ? "选择文件编辑器" : "选择目录编辑器",
        multiple: false,
        directory: false,
        filters: [
          { name: "可执行文件", extensions: ["exe"] },
          { name: "所有文件", extensions: ["*"] },
        ],
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }

      if (kind === "file") {
        await saveEditorPaths(selected, folderEditorPath);
      } else {
        await saveEditorPaths(fileEditorPath, selected);
      }
    } catch (error) {
      showError("配置", kind === "file" ? "选择文件编辑器失败" : "选择目录编辑器失败", error);
    }
  }

  async function clearEditorPath(kind: "file" | "folder") {
    if (kind === "file") {
      await saveEditorPaths("", folderEditorPath);
    } else {
      await saveEditorPaths(fileEditorPath, "");
    }
  }

  async function updateSelectionTriggerMode(mode: SelectionTriggerMode) {
    try {
      await invoke("set_setting", {
        key: "selection.trigger.mode",
        value: mode,
      });
      setSelectionTriggerMode(mode);
      setActionMessage("Ctrl+鼠标划词已开启");
      clearError();
    } catch (error) {
      showError("划词", "划词触发模式更新失败", error);
    }
  }

  async function toggleSelectionEnabled() {
    const nextEnabled = !selectionEnabled;
    try {
      await invoke("set_setting", {
        key: "selection.enabled",
        value: String(nextEnabled),
      });
      setSelectionEnabled(nextEnabled);
      setActionMessage(nextEnabled ? "划词功能已开启" : "划词功能已关闭");
      clearError();
    } catch (error) {
      showError("划词", "划词功能开关保存失败", error);
    }
  }

  async function toggleDoubleAlt() {
    const nextEnabled = !doubleAltEnabled;

    try {
      await invoke("set_setting", {
        key: "launcher.double_alt.enabled",
        value: String(nextEnabled),
      });
      setDoubleAltEnabled(nextEnabled);
      setActionMessage(nextEnabled ? "双击 Alt 唤起已开启" : "双击 Alt 唤起已关闭");
      clearError();
    } catch (error) {
      showError("快捷键", "双击 Alt 设置失败", error);
    }
  }

  async function toggleIncludePrereleaseUpdates() {
    const nextEnabled = !includePrereleaseUpdates;
    try {
      await invoke("set_setting", {
        key: "updates.check.include_prerelease",
        value: String(nextEnabled),
      });
      setIncludePrereleaseUpdates(nextEnabled);
      setUpdateCheckResult(null);
      setActionMessage(nextEnabled ? "更新检查已包含预发布版本" : "更新检查已排除预发布版本");
      clearError();
    } catch (error) {
      showError("更新", "更新检查偏好保存失败", error);
    }
  }

  async function checkForUpdates() {
    setIsCheckingUpdates(true);
    try {
      const result = await invoke<UpdateCheckResult>("check_for_updates", {
        includePrerelease: includePrereleaseUpdates,
      });
      const checkedAt = new Date().toISOString();
      setUpdateCheckResult(result);
      setAppVersion(result.currentVersion || appVersion);
      setLastUpdateCheckAt(checkedAt);
      await invoke("set_setting", {
        key: "updates.check.last_checked_at",
        value: checkedAt,
      }).catch(() => undefined);

      if (result.error && !result.releaseUrl) {
        showError("更新", "更新检查失败", result.error);
        return;
      }

      if (result.isNewer) {
        setActionMessage(`发现新版本：${result.latestTag ?? result.latestVersion}`);
      } else if (result.error) {
        setActionMessage(result.error);
      } else {
        setActionMessage("已是最新版本");
      }
      clearError();
    } catch (error) {
      showError("更新", "更新检查失败", error);
    } finally {
      setIsCheckingUpdates(false);
    }
  }

  async function openLatestReleasePage() {
    const releaseUrl = updateCheckResult?.releaseUrl;
    if (!releaseUrl) {
      showError("更新", "没有可打开的 Release 页面");
      return;
    }

    try {
      await invoke("open_update_release_page", { url: releaseUrl });
      if (updateCheckResult?.latestTag) {
        await invoke("set_setting", {
          key: "updates.check.last_seen_tag",
          value: updateCheckResult.latestTag,
        }).catch(() => undefined);
      }
      setActionMessage("已打开 Release 页面");
      clearError();
    } catch (error) {
      showError("更新", "打开 Release 页面失败", error);
    }
  }

  async function copyUpdateDownloadUrl() {
    const downloadUrl = updateCheckResult?.assetDownloadUrl;
    if (!downloadUrl) {
      showError("更新", "当前 Release 没有可复制的 MSI 下载链接");
      return;
    }

    try {
      await invoke("copy_path", { path: downloadUrl });
      setActionMessage(`已复制下载链接：${updateCheckResult?.assetName ?? "MSI"}`);
      clearError();
    } catch (error) {
      try {
        await navigator.clipboard.writeText(downloadUrl);
        setActionMessage(`已复制下载链接：${updateCheckResult?.assetName ?? "MSI"}`);
        clearError();
      } catch {
        showError("更新", "复制下载链接失败", error);
      }
    }
  }

  async function dismissLatestUpdate() {
    const latestTag = updateCheckResult?.latestTag;
    if (!latestTag) {
      return;
    }

    try {
      await invoke("set_setting", {
        key: "updates.check.dismissed_tag",
        value: latestTag,
      });
      setDismissedUpdateTag(latestTag);
      setActionMessage(`已忽略版本：${latestTag}`);
      clearError();
    } catch (error) {
      showError("更新", "忽略版本失败", error);
    }
  }

  async function reloadSettings() {
    const shortcut = await invoke<string | null>("get_setting", {
      key: "launcher.shortcut",
    });
    setStoredShortcut(shortcut ?? "Alt+1");
    setShortcutInput(shortcut ?? "Alt+1");


    await invoke<string | null>("get_setting", {
      key: "selection.trigger.mode",
    });
    setSelectionTriggerMode("ctrl_mouse");
    const loadedSelectionEnabled = await invoke<string | null>("get_setting", {
      key: "selection.enabled",
    });
    setSelectionEnabled(loadedSelectionEnabled !== "false");

    const doubleAlt = await invoke<string | null>("get_setting", {
      key: "launcher.double_alt.enabled",
    });
    setDoubleAltEnabled(doubleAlt !== "false");

    const startup = await invoke<string | null>("get_setting", {
      key: "startup.enabled",
    });
    setStartupEnabled(startup === "true");

    const loadedLanguage = await invoke<string | null>("get_setting", {
      key: "ui.language",
    });
    setLanguageOption(normalizeLanguagePreference(loadedLanguage));

    await loadUpdateSettings();
    await loadAiData();

    const loadedSources = await invoke<SearchSourceSettings>("get_search_source_settings");
    setSearchSources(loadedSources);
    const loadedWeights = await invoke<SearchWeightSettings>("get_search_weight_settings");
    setSearchWeights(loadedWeights);
    const loadedSmartRanking = await invoke<string | null>("get_setting", {
      key: "search.smart_ranking.enabled",
    });
    setSmartRankingEnabled(loadedSmartRanking !== "false");
    const loadedIconCacheStatus = await invoke<IconCacheStatus>("icon_cache_status");
    setIconCacheStatus(loadedIconCacheStatus);
    const loadedPasswordOptions = await invoke<PasswordOptions>("get_password_options");
    setPasswordOptions(loadedPasswordOptions);
    const loadedToolMenuAlias = await invoke<string | null>("get_setting", {
      key: "tools.menu.alias",
    });
    setToolMenuAlias(loadedToolMenuAlias ?? "/");
    const loadedEverythingOptions = await invoke<EverythingSearchOptions>(
      "get_everything_search_options",
    );
    setEverythingSearchOptions(loadedEverythingOptions);
    const loadedFileEditorPath = await invoke<string | null>("get_setting", {
      key: "file.editor.path",
    });
    setFileEditorPath(loadedFileEditorPath ?? "");
    const loadedFolderEditorPath = await invoke<string | null>("get_setting", {
      key: "folder.editor.path",
    });
    setFolderEditorPath(loadedFolderEditorPath ?? "");

    const loadedCommands = await invoke<CustomCommand[]>("list_custom_commands");
    setCustomCommands(loadedCommands);
    const loadedPhrases = await invoke<Phrase[]>("list_phrases");
    setPhrases(loadedPhrases);
    const loadedWebSearchTemplates =
      await invoke<WebSearchTemplate[]>("list_web_search_templates");
    setWebSearchTemplates(loadedWebSearchTemplates);
    const loadedExclusionRules = await invoke<ExclusionRule[]>("list_exclusion_rules");
    setExclusionRules(loadedExclusionRules);
    const loadedPinnedResults = await invoke<PinnedResult[]>("list_pinned_results");
    setPinnedResults(loadedPinnedResults);
    const loadedResultAliases = await invoke<ResultAlias[]>("list_result_aliases");
    setResultAliases(loadedResultAliases);
  }

  async function exportConfig() {
    try {
      const result = await invoke<ConfigExportResult>("export_config");
      setImportPath(result.path);
      setActionMessage(`已导出 ${result.settingCount} 项配置`);
      clearError();
    } catch (error) {
      showError("配置", "配置导出失败", error);
    }
  }

  async function importConfig() {
    if (!importPath.trim()) {
      showError("配置", "请输入配置 JSON 路径");
      return;
    }

    try {
      const result = await invoke<ConfigImportResult>("import_config", {
        path: importPath,
      });
      await reloadSettings();
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage(`已导入 ${result.importedCount} 项配置`);
      clearError();
    } catch (error) {
      showError("配置", "配置导入失败", error);
    }
  }

  async function saveCustomCommand() {
    if (!customCommandDraft.name.trim() || !customCommandDraft.target.trim()) {
      showError("配置", "自定义命令名称和目标不能为空");
      return;
    }

    try {
      await invoke<CustomCommand>("save_custom_command", {
        input: customCommandDraft,
      });
      const commands = await invoke<CustomCommand[]>("list_custom_commands");
      const response = await invokeSearchWithRecents(query);
      setCustomCommands(commands);
      if (response !== null) {
        setSearchResults(response);
      }
      setCustomCommandDraft({ name: "", commandType: "url", target: "" });
      setActionMessage("自定义命令已保存");
      clearError();
    } catch (error) {
      showError("配置", "自定义命令保存失败", error);
    }
  }

  async function editCustomCommand(command: CustomCommand) {
    setCustomCommandDraft({
      id: command.id,
      name: command.name,
      commandType: command.commandType,
      target: command.target,
    });
    setActionMessage(`正在编辑：${command.name}`);
  }

  async function deleteCustomCommand(command: CustomCommand) {
    try {
      await invoke("delete_custom_command", { id: command.id });
      const commands = await invoke<CustomCommand[]>("list_custom_commands");
      const response = await invokeSearchWithRecents(query);
      setCustomCommands(commands);
      if (response !== null) {
        setSearchResults(response);
      }
      if (customCommandDraft.id === command.id) {
        setCustomCommandDraft({ name: "", commandType: "url", target: "" });
      }
      setActionMessage(`已删除：${command.name}`);
      clearError();
    } catch (error) {
      showError("配置", `自定义命令删除失败：${command.name}`, error);
    }
  }

  async function savePhrase() {
    if (!phraseDraft.title.trim() || !phraseDraft.text.trim()) {
      showError("配置", "快捷短语标题和内容不能为空");
      return;
    }

    try {
      await invoke<Phrase>("save_phrase", {
        input: phraseDraft,
      });
      const loadedPhrases = await invoke<Phrase[]>("list_phrases");
      const response = await invokeSearchWithRecents(query);
      setPhrases(loadedPhrases);
      if (response !== null) {
        setSearchResults(response);
      }
      setPhraseDraft({ title: "", text: "" });
      setActionMessage("快捷短语已保存");
      clearError();
    } catch (error) {
      showError("配置", "快捷短语保存失败", error);
    }
  }

  function editPhrase(phrase: Phrase) {
    setPhraseDraft({
      id: phrase.id,
      title: phrase.title,
      text: phrase.text,
    });
    setActionMessage(`正在编辑：${phrase.title}`);
  }

  async function deletePhrase(phrase: Phrase) {
    try {
      await invoke("delete_phrase", { id: phrase.id });
      const loadedPhrases = await invoke<Phrase[]>("list_phrases");
      const response = await invokeSearchWithRecents(query);
      setPhrases(loadedPhrases);
      if (response !== null) {
        setSearchResults(response);
      }
      if (phraseDraft.id === phrase.id) {
        setPhraseDraft({ title: "", text: "" });
      }
      setActionMessage(`已删除：${phrase.title}`);
      clearError();
    } catch (error) {
      showError("配置", `快捷短语删除失败：${phrase.title}`, error);
    }
  }

  async function saveWebSearchTemplate() {
    if (
      !webSearchTemplateDraft.keyword.trim() ||
      !webSearchTemplateDraft.name.trim() ||
      !webSearchTemplateDraft.urlTemplate.trim()
    ) {
      showError("配置", "网页搜索关键词、名称和模板不能为空");
      return;
    }

    try {
      await invoke<WebSearchTemplate>("save_web_search_template", {
        input: webSearchTemplateDraft,
      });
      const loadedWebSearchTemplates = await invoke<WebSearchTemplate[]>(
        "list_web_search_templates",
      );
      const response = await invokeSearchWithRecents(query);
      setWebSearchTemplates(loadedWebSearchTemplates);
      if (response !== null) {
        setSearchResults(response);
      }
      setWebSearchTemplateDraft({
        keyword: "",
        name: "",
        urlTemplate: "https://www.bing.com/search?q={query}",
      });
      setActionMessage("网页搜索模板已保存");
      clearError();
    } catch (error) {
      showError("配置", "网页搜索模板保存失败", error);
    }
  }

  function editWebSearchTemplate(template: WebSearchTemplate) {
    setWebSearchTemplateDraft({
      id: template.id,
      keyword: template.keyword,
      name: template.name,
      urlTemplate: template.urlTemplate,
    });
    setActionMessage(`正在编辑：${template.name}`);
  }

  async function deleteWebSearchTemplate(template: WebSearchTemplate) {
    try {
      await invoke("delete_web_search_template", { id: template.id });
      const loadedWebSearchTemplates = await invoke<WebSearchTemplate[]>(
        "list_web_search_templates",
      );
      const response = await invokeSearchWithRecents(query);
      setWebSearchTemplates(loadedWebSearchTemplates);
      if (response !== null) {
        setSearchResults(response);
      }
      if (webSearchTemplateDraft.id === template.id) {
        setWebSearchTemplateDraft({
          keyword: "",
          name: "",
          urlTemplate: "https://www.bing.com/search?q={query}",
        });
      }
      setActionMessage(`已删除：${template.name}`);
      clearError();
    } catch (error) {
      showError("配置", `网页搜索模板删除失败：${template.name}`, error);
    }
  }

  async function saveExclusionRule() {
    if (!exclusionRuleDraft.pattern.trim()) {
      showError("配置", "排除规则内容不能为空");
      return;
    }

    try {
      await invoke<ExclusionRule>("save_exclusion_rule", {
        input: exclusionRuleDraft,
      });
      const loadedExclusionRules = await invoke<ExclusionRule[]>("list_exclusion_rules");
      const response = await invokeSearchWithRecents(query);
      setExclusionRules(loadedExclusionRules);
      if (response !== null) {
        setSearchResults(response);
      }
      setExclusionRuleDraft({ matchType: "result_id", pattern: "" });
      setActionMessage("排除规则已保存");
      clearError();
    } catch (error) {
      showError("配置", "排除规则保存失败", error);
    }
  }

  function editExclusionRule(rule: ExclusionRule) {
    setExclusionRuleDraft({
      id: rule.id,
      matchType: rule.matchType,
      pattern: rule.pattern,
    });
    setActionMessage(`正在编辑排除规则：${rule.pattern}`);
  }

  async function deleteExclusionRule(rule: ExclusionRule) {
    if (!window.confirm(`删除隐藏规则？\n\n${rule.pattern}\n\n删除后匹配的结果会重新出现在搜索中。`)) {
      return;
    }

    try {
      await invoke("delete_exclusion_rule", { id: rule.id });
      const loadedExclusionRules = await invoke<ExclusionRule[]>("list_exclusion_rules");
      const response = await invokeSearchWithRecents(query);
      setExclusionRules(loadedExclusionRules);
      if (response !== null) {
        setSearchResults(response);
      }
      if (exclusionRuleDraft.id === rule.id) {
        setExclusionRuleDraft({ matchType: "result_id", pattern: "" });
      }
      setActionMessage(`已删除排除规则：${rule.pattern}`);
      clearError();
    } catch (error) {
      showError("配置", `排除规则删除失败：${rule.pattern}`, error);
    }
  }

  async function updateSearchSource(key: keyof SearchSourceSettings, value: boolean) {
    const nextSources = { ...searchSources, [key]: value };
    setSearchSources(nextSources);

    try {
      await invoke("set_search_source_settings", {
        settings: nextSources,
      });
      setActionMessage("搜索源设置已保存");
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      clearError();
    } catch (error) {
      setSearchSources(searchSources);
      showError("配置", "搜索源设置保存失败", error);
    }
  }

  async function updateSearchWeight(key: keyof SearchWeightSettings, value: number) {
    const nextWeights = { ...searchWeights, [key]: value };
    setSearchWeights(nextWeights);

    try {
      await invoke("set_search_weight_settings", {
        settings: nextWeights,
      });
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage("搜索权重已保存");
      clearError();
    } catch (error) {
      setSearchWeights(searchWeights);
      showError("配置", "搜索权重保存失败", error);
    }
  }

  async function updateSmartRankingEnabled(value: boolean) {
    const previousValue = smartRankingEnabled;
    setSmartRankingEnabled(value);

    try {
      await invoke("set_setting", {
        key: "search.smart_ranking.enabled",
        value: value ? "true" : "false",
      });
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage(value ? "智能排序已开启" : "智能排序已关闭");
      clearError();
    } catch (error) {
      setSmartRankingEnabled(previousValue);
      showError("配置", "智能排序设置保存失败", error);
    }
  }

  async function clearRankingData(kind: "recent" | "query" | "all") {
    const label =
      kind === "recent" ? "最近使用记录" : kind === "query" ? "查询词学习记录" : "全部学习数据";
    if (!window.confirm(`清空${label}？\n\n固定结果和 alias 会保留。`)) {
      return;
    }

    const command =
      kind === "recent"
        ? "clear_recent_items"
        : kind === "query"
          ? "clear_query_selection_stats"
          : "clear_ranking_learning";

    try {
      const cleared = await invoke<number>(command);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage(`已清空${label}：${cleared} 条`);
      clearError();
    } catch (error) {
      showError("配置", `${label}清空失败`, error);
    }
  }

  async function refreshIconCacheStatus() {
    try {
      const status = await invoke<IconCacheStatus>("icon_cache_status");
      setIconCacheStatus(status);
      setActionMessage(`图标缓存：${status.fileCount} 个文件`);
      clearError();
    } catch (error) {
      showError("配置", "图标缓存状态读取失败", error);
    }
  }

  async function clearIconCache() {
    if (!window.confirm("清空图标缓存？\n\n下次搜索会按需重新生成本地图标。")) {
      return;
    }

    try {
      const result = await invoke<IconCacheClearResult>("clear_icon_cache");
      setIconCacheStatus(result.status);
      resultIconPathsRef.current.clear();
      setResults((current) =>
        current.map((item) => (item.iconPath ? { ...item, iconPath: null } : item)),
      );
      setActionMessage(`已清空图标缓存：${result.clearedCount} 个文件`);
      clearError();
    } catch (error) {
      showError("配置", "图标缓存清空失败", error);
    }
  }

  async function updatePasswordOptions(nextOptions: PasswordOptions) {
    const previousOptions = passwordOptions;
    setPasswordOptions(nextOptions);

    try {
      const savedOptions = await invoke<PasswordOptions>("set_password_options", {
        options: nextOptions,
      });
      setPasswordOptions(savedOptions);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage("工具设置已保存");
      clearError();
    } catch (error) {
      setPasswordOptions(previousOptions);
      showError("配置", "工具设置保存失败", error);
    }
  }

  async function saveToolMenuAlias() {
    const previousAlias = toolMenuAlias;
    const nextAlias = toolMenuAlias.trim();
    setToolMenuAlias(nextAlias);

    try {
      await invoke("set_setting", {
        key: "tools.menu.alias",
        value: nextAlias,
      });
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage(`工具总入口已保存：${nextAlias}`);
      clearError();
    } catch (error) {
      setToolMenuAlias(previousAlias);
      showError("配置", "工具总入口保存失败", error);
    }
  }

  async function updateEverythingSearchOption(
    key: keyof EverythingSearchOptions,
    value: boolean,
  ) {
    const nextOptions = { ...everythingSearchOptions, [key]: value };
    setEverythingSearchOptions(nextOptions);

    try {
      await invoke("set_everything_search_options", {
        options: nextOptions,
      });
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      setActionMessage("Everything 搜索选项已保存");
      clearError();
    } catch (error) {
      setEverythingSearchOptions(everythingSearchOptions);
      showError("配置", "Everything 搜索选项保存失败", error);
    }
  }

  async function saveEverythingExePath(nextPath: string) {
    const previousPath = everythingExePath;
    const normalizedPath = nextPath.trim();
    setEverythingExePath(normalizedPath);

    try {
      const status = await invoke<EverythingStatus>("set_everything_exe_path", {
        path: normalizedPath,
      });
      setEverythingStatus(status);
      setActionMessage(normalizedPath ? "Everything 路径已保存" : "Everything 路径已恢复自动检测");
      clearError();
    } catch (error) {
      setEverythingExePath(previousPath);
      showError("Everything", "Everything 路径保存失败", error);
    }
  }

  async function chooseEverythingExePath() {
    try {
      const selected = await open({
        title: "选择 Everything.exe",
        multiple: false,
        directory: false,
        filters: [{ name: "Everything", extensions: ["exe"] }],
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      await saveEverythingExePath(selected);
    } catch (error) {
      showError("Everything", "选择 Everything 路径失败", error);
    }
  }

  async function clearEverythingExePath() {
    await saveEverythingExePath("");
  }

  async function toggleStartup() {
    const nextEnabled = !startupEnabled;

    try {
      await invoke("set_startup_enabled", {
        enabled: nextEnabled,
      });
      setStartupEnabled(nextEnabled);
      setActionMessage(nextEnabled ? "开机自启动已开启" : "开机自启动已关闭");
      clearError();
    } catch (error) {
      showError("系统", "开机自启动设置失败", error);
    }
  }

  async function updateLanguageOption(option: LanguageOption) {
    const previousOption = languageOption;
    setLanguageOption(option);

    try {
      await invoke("set_setting", {
        key: "ui.language",
        value: option,
      });
      setActionMessage(
        option === "system"
          ? "语言已设置为跟随系统"
          : option === "zh-CN"
            ? "语言已切换为中文"
            : "Language switched to English",
      );
      clearError();
    } catch (error) {
      setLanguageOption(previousOption);
      showError("配置", "语言设置保存失败", error);
    }
  }

  async function executeSelectedResult(result: SearchResult) {
    if (result.id === "internal:settings") {
      await openSettingsPanel();
      return;
    }

    if (result.id.startsWith("tool-entry:") || result.id.startsWith("tool-hint:")) {
      const nextQuery = toolCommandFromResult(result);
      if (!nextQuery) {
        showError("系统", `无法进入工具：${result.title}`);
        return;
      }
      setQuery(nextQuery);
      setSelectedIndex(0);
      setContextSession(null);
      focusSearchInput();
      setActionMessage(`已进入：${result.title}`);
      const response = await invokeSearchWithRecents(nextQuery);
      if (response !== null) {
        setSearchResults(response);
      }
      clearError();
      return;
    }

    if (
      result.action === "runCommand" &&
      (result.subtitle === "shutdown" || result.subtitle === "restart") &&
      !window.confirm(`确认执行${result.title}？`)
    ) {
      setActionMessage(`已取消：${result.title}`);
      return;
    }

    try {
      await invoke("execute_result", { result, query });
      setActionMessage(`${actionLabels[result.action]}：${result.title}`);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
      }
      clearError();
    } catch (error) {
      showError("系统", `执行失败：${result.title}`, error);
    }
  }

  async function openResultParent(result: SearchResult) {
    try {
      await invoke("open_parent_dir", { path: result.subtitle });
      setActionMessage(`打开目录：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `打开目录失败：${result.title}`, error);
    }
  }

  async function hideResultFromSearch(result: SearchResult) {
    if (!matchesExcludableResult(result)) {
      showError("配置", `无法隐藏此类结果：${result.title}`);
      return;
    }

    if (!window.confirm(`隐藏此结果？\n\n${result.title}\n${result.subtitle}\n\n可在设置的“隐藏”分组恢复。`)) {
      setActionMessage(`已取消隐藏：${result.title}`);
      return;
    }

    try {
      await invoke<ExclusionRule>("save_exclusion_rule", {
        input: {
          matchType: "result_id",
          pattern: result.id,
        },
      });
      const loadedExclusionRules = await invoke<ExclusionRule[]>("list_exclusion_rules");
      setExclusionRules(loadedExclusionRules);
      setContextSession(null);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
        setSelectedIndex(0);
      }
      setActionMessage(`已隐藏：${result.title}`);
      clearError();
    } catch (error) {
      showError("配置", `隐藏失败：${result.title}`, error);
    }
  }

  async function runResultAsAdmin(result: SearchResult) {
    try {
      await invoke("run_app_as_admin", { path: result.subtitle });
      setActionMessage(`以管理员运行：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `以管理员运行失败：${result.title}`, error);
    }
  }

  async function openEverythingDownload() {
    try {
      await invoke("open_everything_download");
      setActionMessage("已打开 Everything 下载页");
      clearError();
    } catch (error) {
      showError("Everything", "打开 Everything 下载页失败", error);
    }
  }

  async function runResultAsUser(result: SearchResult) {
    const username = window.prompt("输入要使用的 Windows 用户名，例如 .\\User 或 DOMAIN\\User");
    if (!username?.trim()) {
      setActionMessage("已取消以其他用户运行");
      return;
    }

    try {
      await invoke("run_app_as_different_user", {
        path: result.subtitle,
        username,
      });
      setActionMessage(`以其他用户运行：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `以其他用户运行失败：${result.title}`, error);
    }
  }

  async function revealResultPath(result: SearchResult) {
    try {
      await invoke("reveal_path", { path: result.subtitle });
      setActionMessage(`已在资源管理器中选中：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `资源管理器选中失败：${result.title}`, error);
    }
  }

  async function openShortcutTargetParent(result: SearchResult) {
    try {
      await invoke("open_shortcut_target_parent", { path: result.subtitle });
      setActionMessage(`已打开快捷方式目标位置：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `打开快捷方式目标位置失败：${result.title}`, error);
    }
  }

  async function deleteResultPath(result: SearchResult) {
    if (
      !window.confirm(
        `删除此${result.kind === "file" ? "文件或目录" : "应用路径"}？\n\n${result.title}\n${result.subtitle}\n\n此操作会直接删除磁盘文件。`,
      )
    ) {
      setActionMessage(`已取消删除：${result.title}`);
      return;
    }

    try {
      await invoke("delete_path", { path: result.subtitle });
      setContextSession(null);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
        setSelectedIndex(0);
      }
      setActionMessage(`已删除：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `删除失败：${result.title}`, error);
    }
  }

  async function copyFileResult(result: SearchResult) {
    try {
      await invoke("copy_file_to_clipboard", { path: result.subtitle });
      setActionMessage(`已复制文件本体：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `复制文件本体失败：${result.title}`, error);
    }
  }

  async function showResultNativeContextMenu(result: SearchResult) {
    try {
      await invoke("show_native_context_menu", { path: result.subtitle });
      setActionMessage(`已打开 Windows 原生菜单：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `Windows 原生菜单打开失败：${result.title}`, error);
    }
  }

  async function openResultWithDialog(result: SearchResult) {
    try {
      await invoke("open_with_dialog", { path: result.subtitle });
      setActionMessage(`打开方式：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `打开方式失败：${result.title}`, error);
    }
  }

  async function openTerminalAtResult(result: SearchResult) {
    try {
      await invoke("open_terminal_at_path", { path: result.subtitle });
      setActionMessage(`已在当前目录打开终端：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `打开终端失败：${result.title}`, error);
    }
  }

  async function openResultWithConfiguredEditor(result: SearchResult) {
    try {
      await invoke("open_configured_editor", { path: result.subtitle });
      setActionMessage(`已用配置编辑器打开：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `配置编辑器打开失败：${result.title}`, error);
    }
  }

  async function setResultQuickAccess(result: SearchResult, pinned: boolean) {
    try {
      await invoke("set_quick_access", { path: result.subtitle, pinned });
      setActionMessage(`${pinned ? "已添加到" : "已从"}快速访问${pinned ? "" : "移除"}：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `${pinned ? "添加到" : "移除出"}快速访问失败：${result.title}`, error);
    }
  }

  async function setResultPinned(result: SearchResult, pinned: boolean) {
    try {
      await invoke<PinnedResult | null>("set_result_pinned", {
        input: resultRankingInput(result),
        pinned,
      });
      const loadedPinnedResults = await invoke<PinnedResult[]>("list_pinned_results");
      setPinnedResults(loadedPinnedResults);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
        setSelectedIndex(0);
      }
      setActionMessage(`${pinned ? "已固定" : "已取消固定"}：${result.title}`);
      clearError();
    } catch (error) {
      showError("配置", `${pinned ? "固定" : "取消固定"}失败：${result.title}`, error);
    }
  }

  async function addResultAlias(result: SearchResult) {
    const alias = window.prompt(`为此结果添加 alias：\n\n${result.title}`);
    if (!alias?.trim()) {
      setActionMessage(`已取消添加 alias：${result.title}`);
      return;
    }

    try {
      await invoke<ResultAlias>("save_result_alias", {
        input: {
          ...resultRankingInput(result),
          alias: alias.trim(),
        },
      });
      const loadedResultAliases = await invoke<ResultAlias[]>("list_result_aliases");
      setResultAliases(loadedResultAliases);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
        setSelectedIndex(0);
      }
      setActionMessage(`Alias 已保存：${alias.trim()} -> ${result.title}`);
      clearError();
    } catch (error) {
      showError("配置", `Alias 保存失败：${result.title}`, error);
    }
  }

  async function deleteResultAliases(result: SearchResult) {
    const aliases = resultAliases.filter((alias) => alias.resultId === result.id);
    if (aliases.length === 0) {
      setActionMessage(`没有可删除的 alias：${result.title}`);
      return;
    }

    if (
      !window.confirm(
        `删除此结果的 alias？\n\n${aliases.map((alias) => alias.alias).join(", ")}\n\n${result.title}`,
      )
    ) {
      return;
    }

    try {
      for (const alias of aliases) {
        await invoke("delete_result_alias", { normalizedAlias: alias.normalizedAlias });
      }
      const loadedResultAliases = await invoke<ResultAlias[]>("list_result_aliases");
      setResultAliases(loadedResultAliases);
      const response = await invokeSearchWithRecents(query);
      if (response !== null) {
        setSearchResults(response);
        setSelectedIndex(0);
      }
      setActionMessage(`已删除 alias：${result.title}`);
      clearError();
    } catch (error) {
      showError("配置", `Alias 删除失败：${result.title}`, error);
    }
  }

  async function copyResultValue(result: SearchResult, value: string, label: string) {
    try {
      await invoke("copy_path", { path: value });
      setActionMessage(`已复制${label}：${result.title}`);
      clearError();
    } catch (error) {
      showError("系统", `复制${label}失败：${result.title}`, error);
    }
  }

  function openResultContextMenu(result: SearchResult) {
    const actions = contextActionsForResult(
      result,
      fileEditorPath,
      folderEditorPath,
      pinnedResults,
      resultAliases,
    );
    if (actions.length === 0) {
      setActionMessage(`没有可用操作：${result.title}`);
      return;
    }

    setContextSession({ result, actions, filter: "" });
    setSelectedIndex(0);
    setActionMessage(`上下文菜单：${result.title}`);
    clearError();
  }

  async function executeContextAction(action: ResultContextAction) {
    if (!contextSession) {
      return;
    }

    const result = contextSession.result;
    setContextSession(null);

    switch (action.id) {
      case "execute":
        await executeSelectedResult(result);
        return;
      case "pinResult":
        await setResultPinned(result, true);
        return;
      case "unpinResult":
        await setResultPinned(result, false);
        return;
      case "addAlias":
        await addResultAlias(result);
        return;
      case "deleteAlias":
        await deleteResultAliases(result);
        return;
      case "openParent":
        await openResultParent(result);
        return;
      case "revealPath":
        await revealResultPath(result);
        return;
      case "copyPath":
        await copyResultValue(result, result.subtitle, "路径");
        return;
      case "copyName":
        await copyResultValue(result, resultName(result), "名称");
        return;
      case "copyFile":
        await copyFileResult(result);
        return;
      case "showNativeContextMenu":
        await showResultNativeContextMenu(result);
        return;
      case "openConfiguredEditor":
        await openResultWithConfiguredEditor(result);
        return;
      case "openWith":
        await openResultWithDialog(result);
        return;
      case "openTerminal":
        await openTerminalAtResult(result);
        return;
      case "addQuickAccess":
        await setResultQuickAccess(result, true);
        return;
      case "removeQuickAccess":
        await setResultQuickAccess(result, false);
        return;
      case "runAsAdmin":
        await runResultAsAdmin(result);
        return;
      case "runAsUser":
        await runResultAsUser(result);
        return;
      case "openShortcutTargetParent":
        await openShortcutTargetParent(result);
        return;
      case "deletePath":
        await deleteResultPath(result);
        return;
      case "hideResult":
        await hideResultFromSearch(result);
        return;
      default:
        return;
    }
  }

  async function refreshEverythingStatus() {
    try {
      const status = await invoke<EverythingStatus>("everything_status");
      setEverythingStatus(status);
      setActionMessage(status.message);
      clearError();
    } catch (error) {
      showError("Everything", "Everything 状态读取失败", error);
    }
  }

  function showEverythingHttpGuide() {
    setViewMode("launcher");
    setActionMessage("Everything 设置路径：工具 > 选项 > HTTP 服务器");
  }

  async function captureSelectionManually() {
    try {
      await invoke("show_selection_assistant");
      setActionMessage("已打开划词小对话框");
      clearError();
    } catch (error) {
      showError("划词", "浏览器预览模式无法读取选中文本", error);
    }
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    handleSearchNavigationKeyDown(event);
  }

  function handleSearchNavigationKeyDown(
    event: React.KeyboardEvent<HTMLInputElement | HTMLDivElement>,
  ) {
    if (contextSession) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelectedIndex((current) =>
          Math.min(current + 1, Math.max(displayResults.length - 1, 0)),
        );
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((current) => Math.max(current - 1, 0));
        return;
      }

      if (event.key === "Enter") {
        event.preventDefault();
        const action = visibleContextActions[Math.min(selectedIndex, visibleContextActions.length - 1)];
        if (action) {
          executeContextAction(action);
        }
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        setContextSession(null);
        setSelectedIndex(0);
        setActionMessage("已返回搜索结果");
        return;
      }
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((current) =>
        Math.min(current + 1, Math.max(displayResults.length - 1, 0)),
      );
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((current) => Math.max(current - 1, 0));
      return;
    }

    if (event.key === "Enter" && selectedDisplayResult) {
      event.preventDefault();
      if (event.shiftKey) {
        openResultContextMenu(selectedDisplayResult);
        return;
      }
      executeSelectedResult(selectedDisplayResult);
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      if (query.trim().length === 0) {
        hideLauncherWindow();
        return;
      }
      setQuery("");
      setActionMessage("已清空");
    }
  }

  function startWindowDrag(event: React.MouseEvent<HTMLElement>) {
    if (event.button !== 0) {
      return;
    }

    if (isInteractiveDragTarget(event.target)) {
      return;
    }

    getCurrentWindow()
      .startDragging()
      .catch(() => {
        // Browser preview has no desktop window to drag.
      });
  }

  return (
    <main className="shell">
      <section
        key={showSettings ? "settings-page" : "launcher-page"}
        ref={launcherRef}
        className={
          showSettings
            ? "settingsPage"
            : viewMode === "ai"
              ? "launcherPage aiPage"
              : contextSession
                ? "launcherPage contextMode"
                : "launcherPage"
        }
        onMouseDown={startWindowDrag}
        aria-label="Easy Launcher"
      >
        {viewMode === "launcher" && !showSettings ? (
          <div className={contextSession ? "searchLine contextSearchLine" : "searchLine"}>
            <input
              autoFocus
              ref={searchInputRef}
              value={contextSession ? contextSession.filter : query}
              onChange={(event) => {
                if (contextSession) {
                  setContextSession({
                    ...contextSession,
                    filter: event.target.value,
                  });
                  setSelectedIndex(0);
                  return;
                }
                setQuery(event.target.value);
              }}
              onKeyDown={handleKeyDown}
              placeholder={
                contextSession
                  ? `筛选操作：${contextSession.result.title}`
                  : "搜索，输入 option 打开设置"
              }
            />
          </div>
        ) : null}


        {viewMode === "launcher" && !showSettings && appError ? (
          <ErrorNotice error={appError} onDismiss={clearError} />
        ) : null}


        {viewMode === "ai" && appError ? (
          <ErrorNotice error={appError} onDismiss={clearError} />
        ) : null}

        {!showSettings ? (
          <div className="actionStatus" aria-live="polite">
            {actionMessage}
          </div>
        ) : null}

        {showSettings ? (
          <>
            {appError ? <ErrorNotice error={appError} onDismiss={clearError} /> : null}
            <div className="settingsPane">
              <button
                className="settingsCloseButton"
                type="button"
                aria-label="关闭设置"
                onClick={closeSettingsPanel}
              >
                关闭
              </button>
              <nav className="settingsNav" aria-label="设置分组">
              {settingsSections.map((section) => (
                <button
                  className={
                    activeSettingsSection === section.id
                      ? "settingsNavItem active"
                      : "settingsNavItem"
                  }
                  type="button"
                  aria-current={activeSettingsSection === section.id ? "page" : undefined}
                  key={section.id}
                  onClick={() => setActiveSettingsSection(section.id)}
                >
                  <span>{section.label}</span>
                  <small>{section.meta}</small>
                </button>
              ))}
            </nav>

            <section className="settingsContent" aria-label="设置操作区">
              {activeSettingsSection === "general" ? (
                <>
                  <div className="settingsHeader">
                    <strong>通用设置</strong>
                    <small>搜索入口、启动方式和本机状态</small>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>状态</strong>
                      <small>{backendMessage}</small>
                    </div>
                    <div className="sourceToggles" aria-label="状态检查">
                      <button className="sourceToggle" type="button" onClick={pingBackend}>
                        检查后端
                      </button>
                    </div>
                    <small className="settingsHint">
                      {storageStatus?.initialized ? `SQLite 已初始化：${storedShortcut}` : "SQLite 未验证"}
                      {" · "}
                      {shortcutStatus.message}
                      {" · "}
                      {aiShortcutStatus.message}
                    </small>
                  </div>
                  <div className="settingRow shortcutRow">
                    <span className="settingLabel">主快捷键</span>
                    <input
                      aria-label="主快捷键"
                      value={shortcutInput}
                      onChange={(event) => setShortcutInput(event.target.value)}
                      placeholder="Alt+1"
                    />
                    <button type="button" onClick={saveLauncherShortcut}>
                      应用
                    </button>
                  </div>
                  <div className="settingRow settingButtonRow">
                    <span className="settingLabel">快速唤起</span>
                    <button
                      className={doubleAltEnabled ? "sourceToggle active" : "sourceToggle"}
                      type="button"
                      aria-pressed={doubleAltEnabled}
                      onClick={toggleDoubleAlt}
                    >
                      双击 Alt{doubleAltEnabled ? "开" : "关"}
                    </button>
                  </div>
                  <div className="settingRow systemButtonRow">
                    <span className="settingLabel">系统</span>
                    <div className="systemButtonGroup">
                      <button
                        className={startupEnabled ? "sourceToggle active" : "sourceToggle"}
                        type="button"
                        aria-pressed={startupEnabled}
                        onClick={toggleStartup}
                      >
                        开机自启动{startupEnabled ? "开" : "关"}
                      </button>
                    </div>
                  </div>
                  <div className="settingRow shortcutRow">
                    <span className="settingLabel">语言</span>
                    <SegmentedControl<LanguageOption>
                      ariaLabel="显示语言"
                      value={languageOption}
                      options={[
                        { value: "system", label: "跟随系统" },
                        { value: "zh-CN", label: "中文" },
                        { value: "en-US", label: "English" },
                      ]}
                      onChange={updateLanguageOption}
                    />
                  </div>
                  <small className="settingsHint">
                    默认跟随系统；系统语言不是中文时显示英文界面。
                  </small>
                  <small className="settingsHint">
                    应用设置后仅影响界面展示文案，不会修改快捷键、命令或已有常用英语。
                  </small>
                  <div className="settingRow shortcutRow">
                    <span className="settingLabel">AI 快捷键</span>
                    <input
                      aria-label="AI 快捷键"
                      value={aiShortcutInput}
                      onChange={(event) => setAiShortcutInput(event.target.value)}
                      placeholder="Alt+3"
                    />
                    <button type="button" onClick={saveAiShortcut}>
                      应用
                    </button>
                  </div>
                  <small className="settingsHint">{aiShortcutStatus.message}</small>
                </>
              ) : null}

              {activeSettingsSection === "search" ? (
                <>
                  <div className="settingsHeader">
                    <strong>搜索设置</strong>
                    <small>选择参与搜索的来源，并调整结果排序权重</small>
                  </div>
                  <div className="sourceToggles" aria-label="搜索源开关">
                    <ToggleButton
                      active={searchSources.apps}
                      label={searchSourceLabels.apps}
                      onClick={() => updateSearchSource("apps", !searchSources.apps)}
                    />
                    <ToggleButton
                      active={searchSources.files}
                      label={searchSourceLabels.files}
                      onClick={() => updateSearchSource("files", !searchSources.files)}
                    />
                    <ToggleButton
                      active={searchSources.calculator}
                      label={searchSourceLabels.calculator}
                      onClick={() => updateSearchSource("calculator", !searchSources.calculator)}
                    />
                    <ToggleButton
                      active={searchSources.system}
                      label={searchSourceLabels.system}
                      onClick={() => updateSearchSource("system", !searchSources.system)}
                    />
                    <ToggleButton
                      active={searchSources.ai}
                      label="AI"
                      onClick={() => updateSearchSource("ai", !searchSources.ai)}
                    />
                    <ToggleButton
                      active={searchSources.phrase}
                      label={searchSourceLabels.phrase}
                      onClick={() => updateSearchSource("phrase", !searchSources.phrase)}
                    />
                    <ToggleButton
                      active={searchSources.webSearch}
                      label={searchSourceLabels.webSearch}
                      onClick={() => updateSearchSource("webSearch", !searchSources.webSearch)}
                    />
                    <ToggleButton
                      active={searchSources.tools}
                      label={searchSourceLabels.tools}
                      onClick={() => updateSearchSource("tools", !searchSources.tools)}
                    />
                  </div>
                  <div className="weightToggles" aria-label="搜索权重">
                    <WeightInput
                      label={searchSourceLabels.apps}
                      value={searchWeights.apps}
                      onChange={(value) => updateSearchWeight("apps", value)}
                    />
                    <WeightInput
                      label={searchSourceLabels.files}
                      value={searchWeights.files}
                      onChange={(value) => updateSearchWeight("files", value)}
                    />
                    <WeightInput
                      label={searchSourceLabels.calculator}
                      value={searchWeights.calculator}
                      onChange={(value) => updateSearchWeight("calculator", value)}
                    />
                    <WeightInput
                      label={searchSourceLabels.system}
                      value={searchWeights.system}
                      onChange={(value) => updateSearchWeight("system", value)}
                    />
                    <WeightInput
                      label="AI"
                      value={searchWeights.ai}
                      onChange={(value) => updateSearchWeight("ai", value)}
                    />
                    <WeightInput
                      label={searchSourceLabels.phrase}
                      value={searchWeights.phrase}
                      onChange={(value) => updateSearchWeight("phrase", value)}
                    />
                    <WeightInput
                      label={searchSourceLabels.webSearch}
                      value={searchWeights.webSearch}
                      onChange={(value) => updateSearchWeight("webSearch", value)}
                    />
                    <WeightInput
                      label={searchSourceLabels.tools}
                      value={searchWeights.tools}
                      onChange={(value) => updateSearchWeight("tools", value)}
                    />
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>智能排序</strong>
                      <small>最近使用和查询词学习只保存在本机；固定结果和 alias 不受此开关影响</small>
                    </div>
                    <div className="sourceToggles" aria-label="智能排序操作">
                      <ToggleButton
                        active={smartRankingEnabled}
                        label={`习惯学习${smartRankingEnabled ? "开" : "关"}`}
                        onClick={() => updateSmartRankingEnabled(!smartRankingEnabled)}
                      />
                      <button
                        className="sourceToggle"
                        type="button"
                        onClick={() => clearRankingData("recent")}
                      >
                        清空最近
                      </button>
                      <button
                        className="sourceToggle"
                        type="button"
                        onClick={() => clearRankingData("query")}
                      >
                        清空学习
                      </button>
                      <button
                        className="sourceToggle dangerToggle"
                        type="button"
                        onClick={() => clearRankingData("all")}
                      >
                        清空全部
                      </button>
                    </div>
                    <small className="settingsHint">
                      固定结果和 alias 通过搜索结果右键菜单管理，不会被清空学习数据删除。
                    </small>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>结果图标缓存</strong>
                      <small>
                        {iconCacheStatus
                          ? `${iconCacheStatus.fileCount} 个文件 · ${formatFileSize(iconCacheStatus.sizeBytes) ?? "0 B"}`
                          : "尚未读取图标缓存"}
                      </small>
                    </div>
                    <div className="settingRow iconCacheRow">
                      <span className="settingLabel">目录</span>
                      <span
                        className={iconCacheStatus ? "pathPreview configured" : "pathPreview"}
                        title={iconCacheStatus?.directory || undefined}
                      >
                        {iconCacheStatus?.directory || "本地图标缓存目录"}
                      </span>
                      <button type="button" onClick={refreshIconCacheStatus}>
                        刷新
                      </button>
                      <button
                        type="button"
                        className="secondaryButton"
                        disabled={!iconCacheStatus || iconCacheStatus.fileCount === 0}
                        onClick={clearIconCache}
                      >
                        清空
                      </button>
                    </div>
                    <small className="settingsHint">
                      只保存本机 Shell 提取出的 32x32 PNG；搜索词、路径和文件信息不会上传。
                    </small>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>Everything 高级选项</strong>
                      <small>{everythingStatus?.message ?? "用于文件搜索的 Everything 查询参数"}</small>
                    </div>
                    <div className={`everythingStatusGuide ${everythingStatusGuide.tone}`}>
                      <strong>{everythingStatusGuide.title}</strong>
                      <span>{everythingStatusGuide.detail}</span>
                    </div>
                    <div className="configGuide">
                      <span>
                        <strong>版本支持</strong>
                        支持普通 Installer / Portable 版 Everything；不建议使用 Lite 版，因为 Lite
                        移除了 IPC 和 HTTP Server，文件搜索无法正常关联。
                      </span>
                      <span>
                        <strong>关联逻辑</strong>
                        Easy Launcher 会连接正在运行的 <code>Everything.exe</code>，优先使用 IPC
                        查询；IPC 失败时才尝试本机 <code>127.0.0.1:8080</code> 的 HTTP Server
                        备用接口。
                      </span>
                    </div>
                    <div className="settingRow everythingPathRow">
                      <span className="settingLabel">程序</span>
                      <span
                        className={everythingExePath ? "pathPreview configured" : "pathPreview"}
                        title={everythingExePath || everythingStatus?.installPath || undefined}
                      >
                        {everythingExePath || everythingStatus?.installPath || "自动检测 Everything.exe"}
                      </span>
                      <button type="button" onClick={chooseEverythingExePath}>
                        选择
                      </button>
                      <button
                        type="button"
                        className="secondaryButton"
                        disabled={!everythingExePath}
                        onClick={clearEverythingExePath}
                      >
                        清除
                      </button>
                    </div>
                    <small className="settingsHint">
                      选择 Portable 版或非常规安装位置的 Everything.exe；留空时使用自动检测。
                    </small>
                    <div className="sourceToggles" aria-label="Everything 高级搜索选项">
                      <button className="sourceToggle" type="button" onClick={refreshEverythingStatus}>
                        检查
                      </button>
                      {everythingStatus && !everythingStatus.installed ? (
                        <button className="sourceToggle" type="button" onClick={openEverythingDownload}>
                          下载
                        </button>
                      ) : null}
                      {everythingStatus?.installed &&
                      everythingStatus.running &&
                      !everythingStatus.httpAvailable ? (
                        <button className="sourceToggle" type="button" onClick={showEverythingHttpGuide}>
                          HTTP 指引
                        </button>
                      ) : null}
                      <ToggleButton
                        active={everythingSearchOptions.fullPath}
                        label="全路径"
                        onClick={() =>
                          updateEverythingSearchOption(
                            "fullPath",
                            !everythingSearchOptions.fullPath,
                          )
                        }
                      />
                      <ToggleButton
                        active={everythingSearchOptions.searchContent}
                        label="内容"
                        onClick={() =>
                          updateEverythingSearchOption(
                            "searchContent",
                            !everythingSearchOptions.searchContent,
                          )
                        }
                      />
                    </div>
                    <small className="settingsHint">
                      全路径会匹配完整路径；内容搜索可能较慢，需 Everything 已启用内容索引或支持内容查询。
                    </small>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>文件编辑器</strong>
                      <small>配置后，文件和目录结果的上下文菜单会显示对应编辑器动作</small>
                    </div>
                    <div className="settingRow configIoRow">
                      <span className="settingLabel">文件</span>
                      <span
                        className={fileEditorPath ? "pathPreview configured" : "pathPreview"}
                        title={fileEditorPath || undefined}
                      >
                        {fileEditorPath || "未配置文件编辑器"}
                      </span>
                      <button type="button" onClick={() => chooseEditorPath("file")}>
                        选择
                      </button>
                      <button
                        type="button"
                        className="secondaryButton"
                        disabled={!fileEditorPath}
                        onClick={() => clearEditorPath("file")}
                      >
                        清除
                      </button>
                    </div>
                    <div className="settingRow configIoRow">
                      <span className="settingLabel">目录</span>
                      <span
                        className={folderEditorPath ? "pathPreview configured" : "pathPreview"}
                        title={folderEditorPath || undefined}
                      >
                        {folderEditorPath || "未配置目录编辑器"}
                      </span>
                      <button type="button" onClick={() => chooseEditorPath("folder")}>
                        选择
                      </button>
                      <button
                        type="button"
                        className="secondaryButton"
                        disabled={!folderEditorPath}
                        onClick={() => clearEditorPath("folder")}
                      >
                        清除
                      </button>
                    </div>
                    <small className="settingsHint">
                      选择编辑器可执行文件；清除后隐藏对应菜单项。启动时会把当前文件或目录路径作为参数传给编辑器。
                    </small>
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "tools" ? (
                <>
                  <div className="settingsHeader">
                    <strong>工具设置</strong>
                    <small>输入总入口查看工具菜单；输入 enc、dec、pwd 或 time 进入单个工具</small>
                  </div>
                  <div className="configGuide">
                    <span>
                      <strong>入口</strong>
                      <code>{toolMenuAlias || "/"}</code> 严格匹配时展示工具清单；选择条目后进入对应快捷指令。
                    </span>
                    <span>
                      <strong>转换</strong>
                      <code>enc 内容</code> 返回编码和摘要；<code>dec 内容</code> 返回解码、HTML
                      实体和 URL 参数 JSON 解析。
                    </span>
                    <span>
                      <strong>其他</strong>
                      <code>pwd</code> 按下方默认策略生成密码；<code>time 1717555200</code>{" "}
                      自动识别时间戳或日期时间。
                    </span>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>工具总入口</strong>
                      <small>默认 /；保存后只有完整输入该 alias 才显示工具菜单</small>
                    </div>
                    <div className="settingRow shortcutRow">
                      <span className="settingLabel">Alias</span>
                      <input
                        aria-label="工具总入口 Alias"
                        value={toolMenuAlias}
                        onChange={(event) => setToolMenuAlias(event.target.value)}
                        placeholder="/"
                      />
                      <button type="button" onClick={saveToolMenuAlias}>
                        应用
                      </button>
                    </div>
                    <small className="settingsHint">
                      不能包含空格，也不能使用 enc、dec、pwd、time 或 tools，避免和搜索词或短指令冲突。
                    </small>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>随机密码默认值</strong>
                      <small>可在搜索框输入 pwd 20 临时覆盖长度</small>
                    </div>
                    <div className="settingRow shortcutRow">
                      <span className="settingLabel">长度</span>
                      <input
                        aria-label="随机密码默认长度"
                        type="number"
                        min="4"
                        max="128"
                        step="1"
                        value={passwordOptions.length}
                        onChange={(event) =>
                          setPasswordOptions((current) => ({
                            ...current,
                            length: Number(event.target.value),
                          }))
                        }
                      />
                      <button type="button" onClick={() => updatePasswordOptions(passwordOptions)}>
                        应用
                      </button>
                    </div>
                    <div className="sourceToggles" aria-label="随机密码字符集">
                      <ToggleButton
                        active={passwordOptions.uppercase}
                        label="大写 U"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            uppercase: !passwordOptions.uppercase,
                          })
                        }
                      />
                      <ToggleButton
                        active={passwordOptions.lowercase}
                        label="小写 W"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            lowercase: !passwordOptions.lowercase,
                          })
                        }
                      />
                      <ToggleButton
                        active={passwordOptions.digits}
                        label="数字 D"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            digits: !passwordOptions.digits,
                          })
                        }
                      />
                      <ToggleButton
                        active={passwordOptions.hyphen}
                        label="减号 -"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            hyphen: !passwordOptions.hyphen,
                          })
                        }
                      />
                      <ToggleButton
                        active={passwordOptions.underscore}
                        label="下划线 W"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            underscore: !passwordOptions.underscore,
                          })
                        }
                      />
                      <ToggleButton
                        active={passwordOptions.special}
                        label="特殊 E"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            special: !passwordOptions.special,
                          })
                        }
                      />
                      <ToggleButton
                        active={passwordOptions.brackets}
                        label="括号 B"
                        onClick={() =>
                          updatePasswordOptions({
                            ...passwordOptions,
                            brackets: !passwordOptions.brackets,
                          })
                        }
                      />
                    </div>
                    <small className="settingsHint">
                      后端会把长度限制在 4-128；如果关闭全部字符集，会自动恢复大写、小写和数字。
                    </small>
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "ai" ? (
                <>
                  <div className="settingsHeader">
                    <strong>AI 设置</strong>
                    <small>先配置供应商和模型，再创建助手并绑定已启用模型</small>
                  </div>
                  <SegmentedControl<AiSettingsTab>
                    ariaLabel="AI 设置子页"
                    value={activeAiSettingsTab}
                    options={[
                      { value: "providers", label: "供应商模型" },
                      { value: "assistants", label: "助手" },
                    ]}
                    onChange={setActiveAiSettingsTab}
                  />
                  {activeAiSettingsTab === "providers" ? (
                    <>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>AI 服务供应商</strong>
                      <small>OpenAI 兼容接口 · {aiProviders.length} 个供应商</small>
                    </div>
                    <div className="aiConfigLine aiProviderLine">
                      <input
                        aria-label="供应商名称"
                        value={aiProviderDraft.name}
                        onChange={(event) =>
                          setAiProviderDraft((current) => ({
                            ...current,
                            name: event.target.value,
                          }))
                        }
                        placeholder="供应商名称"
                      />
                      <input
                        aria-label="供应商 Base URL"
                        value={aiProviderDraft.baseUrl}
                        onChange={(event) => {
                          const baseUrl = event.target.value;
                          setAiProviderDraft((current) => ({
                            ...current,
                            baseUrl,
                          }));
                        }}
                        placeholder="Base URL"
                      />
                      <span className="aiSecretInput">
                        <input
                          aria-label="供应商 API Key"
                          type={showAiApiKey ? "text" : "password"}
                          value={aiProviderDraft.apiKey}
                          onChange={(event) =>
                            setAiProviderDraft((current) => ({
                              ...current,
                              apiKey: event.target.value,
                            }))
                          }
                          placeholder="API Key"
                        />
                        <button type="button" onClick={() => setShowAiApiKey((visible) => !visible)}>
                          {showAiApiKey ? "隐藏" : "显示"}
                        </button>
                      </span>
                      <button type="button" onClick={saveAiProvider}>
                        保存
                      </button>
                    </div>
                    <div className="aiSettingsList">
                      {aiProviders.map((provider) => (
                        <div
                          className={
                            selectedAiProvider?.id === provider.id
                              ? "aiSettingsItem active"
                              : "aiSettingsItem"
                          }
                          key={provider.id}
                          onClick={() => editAiProvider(provider)}
                          onKeyDown={(event) => {
                            if (event.key === "Enter" || event.key === " ") {
                              event.preventDefault();
                              editAiProvider(provider);
                            }
                          }}
                          role="button"
                          tabIndex={0}
                        >
                          <span>
                            <strong>{provider.name}</strong>
                            <small>
                              {provider.baseUrl || "未配置地址"} ·{" "}
                              {enabledModelCountByProvider[provider.id] ?? 0} 个启用模型
                            </small>
                          </span>
                          <button
                            type="button"
                            onClick={(event) => {
                              event.stopPropagation();
                              editAiProvider(provider);
                            }}
                          >
                            选择/编辑
                          </button>
                          <button
                            type="button"
                            onClick={(event) => {
                              event.stopPropagation();
                              deleteAiProvider(provider.id);
                            }}
                          >
                            删除
                          </button>
                        </div>
                      ))}
                      <button className="sourceToggle" type="button" onClick={newAiProviderDraft}>
                        新建供应商
                      </button>
                    </div>
                  </div>

                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>模型管理</strong>
                      <small>
                        当前供应商：{selectedAiProvider?.name ?? "未选择供应商"}
                      </small>
                    </div>
                    <div className="aiConfigLine aiModelManagerLine">
                      <input
                        aria-label="模型搜索"
                        value={aiModelSearch}
                        onChange={(event) => setAiModelSearch(event.target.value)}
                        placeholder="搜索模型"
                      />
                      <input
                        aria-label="手动添加模型"
                        value={manualAiModelName}
                        onChange={(event) => setManualAiModelName(event.target.value)}
                        placeholder="手动添加模型"
                      />
                      <button
                        type="button"
                        onClick={fetchAiProviderModels}
                        disabled={isFetchingAiModels}
                      >
                        {isFetchingAiModels ? "获取中" : "获取当前供应商模型"}
                      </button>
                      <button type="button" onClick={addManualAiProviderModel}>
                        手动添加模型
                      </button>
                    </div>
                    <div className="aiModelChecklist">
                      {filteredAiProviderModels.map((model) => (
                        <label className="aiModelCheck" key={model.id}>
                          <input
                            type="checkbox"
                            checked={model.enabled}
                            onChange={(event) => toggleAiProviderModel(model, event.target.checked)}
                          />
                          <span>{model.modelName}</span>
                        </label>
                      ))}
                      {filteredAiProviderModels.length === 0 ? (
                        <div className="settingsEmpty">
                          {selectedAiProvider
                            ? "暂无模型，可以获取模型或手动添加模型名称"
                            : "请先保存并选择供应商"}
                        </div>
                      ) : null}
                    </div>
                  </div>
                    </>
                  ) : null}

                  {activeAiSettingsTab === "assistants" ? (
                    <>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>助手</strong>
                      <small>聊天助手定义；划词显示请到“划词”页设置</small>
                    </div>
                    {enabledAiModelBindings.length === 0 ? (
                      <div className="settingsEmpty">
                        请先在“供应商模型”中启用至少一个模型。
                      </div>
                    ) : null}
                    <div className="aiConfigLine">
                      <input
                        aria-label="助手名称"
                        value={aiAssistantDraft.name}
                        onChange={(event) =>
                          setAiAssistantDraft((current) => ({ ...current, name: event.target.value }))
                        }
                        placeholder="助手名称"
                      />
                      <input
                        aria-label="助手图标"
                        value={aiAssistantDraft.icon}
                        onChange={(event) =>
                          setAiAssistantDraft((current) => ({ ...current, icon: event.target.value }))
                        }
                        placeholder="AI"
                      />
                      <select
                        aria-label="绑定模型"
                        value={selectedAssistantModelBinding}
                        onChange={(event) =>
                          setAiAssistantDraft((current) => ({
                            ...current,
                            modelProfileId: event.target.value,
                          }))
                        }
                        disabled={enabledAiModelBindings.length === 0}
                      >
                        {enabledAiModelBindings.map((binding) => (
                          <option key={binding.value} value={binding.value}>
                            {binding.label}
                          </option>
                        ))}
                      </select>
                      <button type="button" onClick={saveAiAssistant}>
                        保存助手
                      </button>
                    </div>
                    <div className="aiConfigLine aiAssistantMetaLine">
                      <input
                        aria-label="助手描述"
                        value={aiAssistantDraft.description}
                        onChange={(event) =>
                          setAiAssistantDraft((current) => ({
                            ...current,
                            description: event.target.value,
                          }))
                        }
                        placeholder="助手描述"
                      />
                      <input
                        aria-label="助手排序"
                        type="number"
                        step="1"
                        value={aiAssistantDraft.sortOrder}
                        onChange={(event) =>
                          setAiAssistantDraft((current) => ({
                            ...current,
                            sortOrder: integerFromInput(event.target.value, current.sortOrder),
                          }))
                        }
                        placeholder="排序"
                      />
                      <label className="aiStreamToggle">
                        <input
                          type="checkbox"
                          checked={aiAssistantDraft.enabled}
                          onChange={(event) =>
                            setAiAssistantDraft((current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                        />
                        <span>启用助手</span>
                      </label>
                    </div>
                    <textarea
                      className="aiPromptInput"
                      aria-label="助手系统提示词"
                      value={aiAssistantDraft.systemPrompt}
                      onChange={(event) =>
                        setAiAssistantDraft((current) => ({
                          ...current,
                          systemPrompt: event.target.value,
                        }))
                      }
                      placeholder="系统提示词"
                    />
                    <div className="aiSettingsList">
                      {aiAssistants.map((assistant) => {
                        return (
                          <div className="aiSettingsItem assistantItem" key={assistant.id}>
                            <span>
                              <strong>{assistant.icon} {assistant.name}</strong>
                              <small>
                                {assistant.description || "无描述"} ·{" "}
                                {assistantModelSummary(
                                  assistant,
                                  aiModelProfiles,
                                  aiProviders,
                                  enabledAiProviderModels,
                                )}
                              </small>
                            </span>
                            <button type="button" onClick={() => editAiAssistant(assistant)}>
                              编辑
                            </button>
                            <button
                              type="button"
                              onClick={() => deleteAiAssistant(assistant.id)}
                              disabled={isBuiltinSelectionAssistantId(assistant.id)}
                            >
                              删除
                            </button>
                          </div>
                        );
                      })}
                      <button className="sourceToggle" type="button" onClick={newAiAssistantDraft}>
                        新建助手
                      </button>
                    </div>
                  </div>
                    </>
                  ) : null}
                </>
              ) : null}

              {activeSettingsSection === "selection" ? (
                <>
                  <div className="settingsHeader">
                    <strong>划词设置</strong>
                    <small>控制划词触发方式、浮窗入口和每个助手在划词场景中的模型</small>
                  </div>
                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>触发方式</strong>
                      <small>开启后，按住 Ctrl 并用鼠标划词会弹出划词 AI 浮窗</small>
                    </div>
                    <div className="settingRow settingButtonRow">
                      <span className="settingLabel">划词功能</span>
                      <button
                        className={selectionEnabled ? "sourceToggle active" : "sourceToggle"}
                        type="button"
                        aria-pressed={selectionEnabled}
                        onClick={toggleSelectionEnabled}
                      >
                        {selectionEnabled ? "已开启" : "已关闭"}
                      </button>
                      <button className="sourceToggle" type="button" onClick={captureSelectionManually}>
                        立即读取选区
                      </button>
                    </div>
                    <div className="settingRow settingButtonRow">
                      <span className="settingLabel">触发</span>
                      <button
                        className={
                          selectionTriggerMode === "ctrl_mouse"
                            ? "sourceToggle active"
                            : "sourceToggle"
                        }
                        type="button"
                        aria-pressed={selectionTriggerMode === "ctrl_mouse"}
                        onClick={() => updateSelectionTriggerMode("ctrl_mouse")}
                      >
                        Ctrl + 鼠标划词
                      </button>
                    </div>
                  </div>

                  <div className="settingsSubsection">
                    <div className="settingsHeader compact">
                      <strong>可用助手</strong>
                      <small>
                        已显示 {visibleSelectionActions.length} 个；浮窗默认展示前 5 个，其余通过横向滚动访问
                      </small>
                    </div>
                    {enabledAiModelBindings.length === 0 ? (
                      <div className="settingsEmpty">
                        请先在 AI / 供应商模型 中启用至少一个模型
                      </div>
                    ) : null}
                    {aiAssistants.length === 0 ? (
                      <div className="settingsEmpty">
                        没有可用助手，请先到 AI / 助手 创建并启用助手
                      </div>
                    ) : null}
                    <div className="aiSettingsList">
                      {selectionActionRows.map((action) => (
                        <div className="aiSettingsItem selectionActionItem" key={action.assistantId}>
                          <label className="aiStreamToggle compact">
                            <input
                              type="checkbox"
                              checked={action.showInSelection}
                              disabled={!action.assistantEnabled}
                              onChange={(event) =>
                                toggleSelectionAction(action, event.target.checked)
                              }
                              aria-label="显示"
                            />
                          </label>
                          <span>
                            <strong>
                              {action.assistantIcon} {action.assistantName}
                            </strong>
                            <small>{action.assistantDescription || "无描述"}</small>
                          </span>
                          <input
                            aria-label={`${action.assistantName} 划词显示名称`}
                            defaultValue={action.selectionLabel}
                            onBlur={(event) =>
                              updateSelectionActionLabel(
                                action,
                                event.currentTarget.value.trim() || action.assistantName,
                              )
                            }
                            placeholder="显示名称"
                          />
                          <input
                            aria-label={`${action.assistantName} 划词排序`}
                            type="number"
                            step="1"
                            defaultValue={action.sortOrder}
                            onBlur={(event) =>
                              updateSelectionActionSortOrder(
                                action,
                                integerFromInput(event.currentTarget.value, action.sortOrder),
                              )
                            }
                            placeholder="排序"
                          />
                        </div>
                      ))}
                    </div>
                    <small className="settingsHint">
                      系统提示词、助手说明和模型在 AI / 助手 中统一编辑；划词浮窗里切换模型会同步修改同一个助手模型。
                    </small>
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "commands" ? (
                <>
                  <div className="settingsHeader">
                    <strong>自定义命令</strong>
                    <small>{customCommands.length} 个固定入口</small>
                  </div>
                  <div className="customCommandPane">
                    <div className="configGuide">
                      <span>
                        <strong>示例</strong>
                        URL 可填 <code>https://example.com</code>，程序可填{" "}
                        <code>C:\Windows\System32\notepad.exe</code>。
                      </span>
                      <span>
                        <strong>规则</strong>
                        名称不允许重复；自定义命令不支持 <code>{"{query}"}</code>，需要搜索词时请使用网页模板。
                      </span>
                      <span>
                        <strong>导入导出</strong>
                        配置导出会包含自定义命令；导入时同 ID 覆盖更新，不同 ID 但名称重复会被拒绝。
                      </span>
                    </div>
                    <div className="customCommandEditor">
                      <input
                        aria-label="自定义命令名称"
                        value={customCommandDraft.name}
                        onChange={(event) =>
                          setCustomCommandDraft((current) => ({
                            ...current,
                            name: event.target.value,
                          }))
                        }
                        placeholder="命令名称"
                      />
                      <SegmentedControl<CustomCommandType>
                        ariaLabel="自定义命令类型"
                        value={customCommandDraft.commandType}
                        options={[
                          { value: "url", label: "URL" },
                          { value: "file", label: "文件" },
                          { value: "program", label: "程序" },
                        ]}
                        onChange={(commandType) =>
                          setCustomCommandDraft((current) => ({
                            ...current,
                            commandType,
                          }))
                        }
                      />
                      <input
                        aria-label="自定义命令目标"
                        value={customCommandDraft.target}
                        onChange={(event) =>
                          setCustomCommandDraft((current) => ({
                            ...current,
                            target: event.target.value,
                          }))
                        }
                        placeholder="固定 URL、文件路径或程序路径"
                      />
                      <button type="button" onClick={saveCustomCommand}>
                        {customCommandDraft.id ? "更新" : "新增"}
                      </button>
                    </div>

                    {customCommands.length > 0 ? (
                      <div className="customCommandList">
                        {customCommands.map((command) => (
                          <div className="customCommandItem" key={command.id}>
                            <span>
                              <strong>{command.name}</strong>
                              <small>
                                {command.commandType} · {command.target}
                              </small>
                            </span>
                            <button type="button" onClick={() => editCustomCommand(command)}>
                              编辑
                            </button>
                            <button type="button" onClick={() => deleteCustomCommand(command)}>
                              删除
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="settingsEmpty">暂无自定义命令</div>
                    )}
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "phrases" ? (
                <>
                  <div className="settingsHeader">
                    <strong>快捷短语</strong>
                    <small>{phrases.length} 条常用文本</small>
                  </div>
                  <div className="phrasePane">
                    <div className="configGuide">
                      <span>
                        <strong>示例</strong>
                        标题可填 <code>邮箱签名</code>，内容可填需要一键复制的常用文本。
                      </span>
                      <span>
                        <strong>规则</strong>
                        标题不允许重复；内容最多 4000 个字符。
                      </span>
                      <span>
                        <strong>导入导出</strong>
                        配置导出会包含快捷短语；导入时同 ID 覆盖更新，不同 ID 但标题重复会被拒绝。
                      </span>
                    </div>
                    <div className="phraseEditor">
                      <input
                        aria-label="快捷短语标题"
                        value={phraseDraft.title}
                        onChange={(event) =>
                          setPhraseDraft((current) => ({ ...current, title: event.target.value }))
                        }
                        placeholder="短语标题"
                      />
                      <input
                        aria-label="快捷短语内容"
                        value={phraseDraft.text}
                        onChange={(event) =>
                          setPhraseDraft((current) => ({ ...current, text: event.target.value }))
                        }
                        placeholder="常用文本内容"
                      />
                      <button type="button" onClick={savePhrase}>
                        {phraseDraft.id ? "更新" : "新增"}
                      </button>
                    </div>

                    {phrases.length > 0 ? (
                      <div className="phraseList">
                        {phrases.map((phrase) => (
                          <div className="phraseItem" key={phrase.id}>
                            <span>
                              <strong>{phrase.title}</strong>
                              <small>{phrase.text}</small>
                            </span>
                            <button type="button" onClick={() => editPhrase(phrase)}>
                              编辑
                            </button>
                            <button type="button" onClick={() => deletePhrase(phrase)}>
                              删除
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="settingsEmpty">暂无快捷短语</div>
                    )}
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "webSearch" ? (
                <>
                  <div className="settingsHeader">
                    <strong>网页搜索模板</strong>
                    <small>{webSearchTemplates.length} 个搜索模板</small>
                  </div>
                  <div className="phrasePane">
                    <div className="configGuide">
                      <span>
                        <strong>示例</strong>
                        关键词 <code>gh</code>，模板{" "}
                        <code>https://github.com/search?q={"{query}"}</code>。
                      </span>
                      <span>
                        <strong>搜索框用法</strong>
                        输入 <code>gh rust tauri</code> 或 <code>gh:rust tauri</code> 会用 gh 模板搜索
                        rust tauri。
                      </span>
                      <span>
                        <strong>强制网页模板</strong>
                        如果关键词和 app、file、cmd 等内置前缀冲突，可输入 <code>web gh rust</code> 或{" "}
                        <code>web:gh rust</code>。
                      </span>
                      <span>
                        <strong>规则</strong>
                        关键词不允许重复，只能包含字母、数字、<code>-</code> 或 <code>_</code>；URL 必须以
                        http/https 开头并包含 <code>{"{query}"}</code>。
                      </span>
                      <span>
                        <strong>导入导出</strong>
                        配置导出会包含网页模板；导入时同 ID 覆盖更新，不同 ID 但关键词重复会被拒绝。
                      </span>
                    </div>
                    <div className="phraseEditor">
                      <input
                        aria-label="网页搜索关键词"
                        value={webSearchTemplateDraft.keyword}
                        onChange={(event) =>
                          setWebSearchTemplateDraft((current) => ({
                            ...current,
                            keyword: event.target.value,
                          }))
                        }
                        placeholder="关键词，例如 gh"
                      />
                      <input
                        aria-label="网页搜索名称"
                        value={webSearchTemplateDraft.name}
                        onChange={(event) =>
                          setWebSearchTemplateDraft((current) => ({
                            ...current,
                            name: event.target.value,
                          }))
                        }
                        placeholder="模板名称"
                      />
                      <input
                        aria-label="网页搜索模板"
                        value={webSearchTemplateDraft.urlTemplate}
                        onChange={(event) =>
                          setWebSearchTemplateDraft((current) => ({
                            ...current,
                            urlTemplate: event.target.value,
                          }))
                        }
                        placeholder="URL 模板，必须包含 {query}"
                      />
                      <button type="button" onClick={saveWebSearchTemplate}>
                        {webSearchTemplateDraft.id ? "更新" : "新增"}
                      </button>
                    </div>

                    {webSearchTemplates.length > 0 ? (
                      <div className="phraseList">
                        {webSearchTemplates.map((template) => (
                          <div className="phraseItem" key={template.id}>
                            <span>
                              <strong>{template.name}</strong>
                              <small>
                                {template.keyword} · {template.urlTemplate}
                              </small>
                            </span>
                            <button type="button" onClick={() => editWebSearchTemplate(template)}>
                              编辑
                            </button>
                            <button type="button" onClick={() => deleteWebSearchTemplate(template)}>
                              删除
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="settingsEmpty">暂无网页搜索模板</div>
                    )}
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "exclusions" ? (
                <>
                  <div className="settingsHeader">
                    <strong>隐藏规则</strong>
                    <small>{exclusionRules.length} 条排除规则</small>
                  </div>
                  <div className="exclusionPane">
                    <div className="exclusionGuide">
                      <span>
                        <strong>结果 ID</strong>
                        精确隐藏单个结果，适合应用、命令或固定文件项。
                      </span>
                      <span>
                        <strong>路径规则</strong>
                        可用 <code>*</code> 匹配路径片段，例如 <code>C:\Temp\*</code>。
                      </span>
                      <span>
                        <strong>恢复显示</strong>
                        删除对应规则后，匹配结果会重新参与搜索。
                      </span>
                    </div>
                    <div className="exclusionEditor">
                      <SegmentedControl<ExclusionMatchType>
                        ariaLabel="排除规则类型"
                        value={exclusionRuleDraft.matchType}
                        options={[
                          { value: "result_id", label: "结果 ID" },
                          { value: "path_pattern", label: "路径规则" },
                        ]}
                        onChange={(matchType) =>
                          setExclusionRuleDraft((current) => ({
                            ...current,
                            matchType,
                          }))
                        }
                      />
                      <input
                        aria-label="排除规则内容"
                        value={exclusionRuleDraft.pattern}
                        onChange={(event) =>
                          setExclusionRuleDraft((current) => ({
                            ...current,
                            pattern: event.target.value,
                          }))
                        }
                        placeholder={
                          exclusionRuleDraft.matchType === "result_id"
                            ? "例如 app:notepad"
                            : "例如 C:\\Temp\\*"
                        }
                      />
                      <button type="button" onClick={saveExclusionRule}>
                        {exclusionRuleDraft.id ? "更新" : "新增"}
                      </button>
                    </div>

                    {exclusionRules.length > 0 ? (
                      <div className="exclusionList">
                        {exclusionRules.map((rule) => (
                          <div className="exclusionItem" key={rule.id}>
                            <span>
                              <strong>
                                {rule.matchType === "result_id" ? "结果 ID" : "路径规则"}
                              </strong>
                              <small>{rule.pattern}</small>
                            </span>
                            <button type="button" onClick={() => editExclusionRule(rule)}>
                              编辑
                            </button>
                            <button type="button" onClick={() => deleteExclusionRule(rule)}>
                              删除
                            </button>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="settingsEmpty">暂无隐藏规则</div>
                    )}
                  </div>
                </>
              ) : null}

              {activeSettingsSection === "updates" ? (
                <>
                  <div className="settingsHeader">
                    <strong>更新</strong>
                    <small>当前版本 {appVersion}</small>
                  </div>
                  <div className={`updateStatusGuide ${updateStatusGuide.tone}`}>
                    <strong>{updateStatusGuide.title}</strong>
                    <span>{updateStatusGuide.detail}</span>
                  </div>
                  <div className="settingRow systemButtonRow">
                    <span className="settingLabel">Release</span>
                    <div className="systemButtonGroup">
                      <button
                        type="button"
                        onClick={checkForUpdates}
                        disabled={isCheckingUpdates}
                      >
                        {isCheckingUpdates ? "正在检查" : "检查更新"}
                      </button>
                      <button
                        className="secondaryButton"
                        type="button"
                        onClick={openLatestReleasePage}
                        disabled={!updateCheckResult?.releaseUrl}
                      >
                        打开 Release
                      </button>
                      <button
                        className="secondaryButton"
                        type="button"
                        onClick={copyUpdateDownloadUrl}
                        disabled={!updateCheckResult?.assetDownloadUrl}
                      >
                        复制 MSI 链接
                      </button>
                      <button
                        className="secondaryButton"
                        type="button"
                        onClick={dismissLatestUpdate}
                        disabled={
                          !updateCheckResult?.latestTag ||
                          !updateCheckResult?.isNewer ||
                          dismissedUpdateTag === updateCheckResult.latestTag
                        }
                      >
                        忽略此版本
                      </button>
                    </div>
                  </div>
                  <div className="settingRow settingButtonRow">
                    <span className="settingLabel">预发布</span>
                    <button
                      className={includePrereleaseUpdates ? "sourceToggle active" : "sourceToggle"}
                      type="button"
                      aria-pressed={includePrereleaseUpdates}
                      onClick={toggleIncludePrereleaseUpdates}
                    >
                      包含预发布{includePrereleaseUpdates ? "开" : "关"}
                    </button>
                  </div>
                  <div className="updateMetaGrid">
                    <span>
                      <strong>最新版本</strong>
                      <small>{updateCheckResult?.latestTag ?? "尚未检查"}</small>
                    </span>
                    <span>
                      <strong>发布时间</strong>
                      <small>
                        {updateCheckResult?.publishedAt
                          ? formatDateTime(updateCheckResult.publishedAt)
                          : "未知"}
                      </small>
                    </span>
                    <span>
                      <strong>安装包</strong>
                      <small>{updateCheckResult?.assetName ?? "暂无 MSI asset"}</small>
                    </span>
                  </div>
                  <small className="settingsHint">
                    更新检查只请求 GitHub Releases API；下载安装仍由用户从 Release 页面确认。
                  </small>
                </>
              ) : null}

              {activeSettingsSection === "backup" ? (
                <>
                  <div className="settingsHeader">
                    <strong>配置文件</strong>
                    <small>导出当前配置，或从 JSON 文件导入</small>
                  </div>
                  <div className="configGuide">
                    <span>
                      <strong>导出内容</strong>
                      包含快捷键、搜索源、权重、Everything 选项、自定义命令、短语、网页模板和隐藏规则。
                    </span>
                    <span>
                      <strong>安全</strong>
                      AI API Key 不会写入导出文件；导入会跳过不支持的设置项。
                    </span>
                    <span>
                      <strong>冲突</strong>
                      同 ID 的条目会覆盖更新；不同 ID 但名称、标题或关键词重复时，导入会提示冲突并停止。
                    </span>
                  </div>
                  <div className="settingRow configIoRow">
                    <span className="settingLabel">路径</span>
                    <input
                      aria-label="配置 JSON 路径"
                      value={importPath}
                      onChange={(event) => setImportPath(event.target.value)}
                      placeholder="导入 JSON 路径"
                    />
                    <button type="button" onClick={exportConfig}>
                      导出
                    </button>
                    <button type="button" onClick={importConfig}>
                      导入
                    </button>
                  </div>
                </>
              ) : null}
              </section>
            </div>
          </>
        ) : null}

        {viewMode === "ai" && !showSettings ? (
          <div className={aiProfileMissing ? "aiPanel aiPanelSetup" : "aiPanel"}>
            <aside className="aiAssistantColumn">
              <div className="aiSidebarHeader">
                <strong>AI</strong>
                <button type="button" onClick={() => openSettingsPanel("ai")}>
                  设置
                </button>
              </div>
              {aiProfileMissing ? (
                <div className="aiEmptyState">
                  <strong>模型配置未完成</strong>
                  <small>需要 Base URL 和模型名称。</small>
                  <button type="button" onClick={() => openSettingsPanel("ai")}>
                    打开 AI 设置
                  </button>
                </div>
              ) : (
                <>
                  <div className="aiSidebarSectionHeader">
                    <span>
                      <strong>助手</strong>
                      <small>{aiAssistants.length} 个可用</small>
                    </span>
                  </div>
                  <div className="aiAssistantList">
                    {aiAssistants.map((assistant) => (
                      <button
                        className={
                          selectedAiAssistant?.id === assistant.id
                            ? "aiAssistantItem active"
                            : "aiAssistantItem"
                        }
                        key={assistant.id}
                        type="button"
                        onClick={() => selectAiAssistant(assistant.id)}
                      >
                        <span>{assistant.icon}</span>
                        <strong>{assistant.name}</strong>
                      </button>
                    ))}
                  </div>
                </>
              )}
            </aside>
            {!aiProfileMissing ? (
              <aside className="aiConversationColumn">
                <div className="aiSidebarSectionHeader conversationHeader">
                  <span>
                    <strong>{selectedAiAssistant?.name ?? "会话"}</strong>
                    <small>{aiConversations.length > 0 ? `${aiConversations.length} 条历史` : "暂无历史"}</small>
                  </span>
                  <button type="button" onClick={startNewAiConversation}>
                    新建
                  </button>
                </div>
                <div className="aiConversationList">
                  {aiConversations.map((conversation) => (
                    <div
                      className={
                        selectedAiConversation?.id === conversation.id
                          ? "aiConversationItem active"
                          : "aiConversationItem"
                      }
                      key={conversation.id}
                      role="button"
                      tabIndex={0}
                      onClick={() => {
                        if (renamingAiConversationId !== conversation.id) {
                          selectAiConversation(conversation.id);
                        }
                      }}
                      onKeyDown={(event) => {
                        if (
                          (event.key === "Enter" || event.key === " ") &&
                          renamingAiConversationId !== conversation.id
                        ) {
                          event.preventDefault();
                          selectAiConversation(conversation.id);
                        }
                      }}
                    >
                      <div className="aiConversationMain">
                        {renamingAiConversationId === conversation.id ? (
                          <input
                            aria-label="会话标题"
                            autoFocus
                            value={renamingAiConversationTitle}
                            onClick={(event) => event.stopPropagation()}
                            onChange={(event) => setRenamingAiConversationTitle(event.target.value)}
                            onBlur={saveAiConversationTitle}
                            onKeyDown={(event) => {
                              if (event.key === "Enter") {
                                event.preventDefault();
                                saveAiConversationTitle();
                              }
                              if (event.key === "Escape") {
                                event.preventDefault();
                                setRenamingAiConversationId(null);
                              }
                            }}
                          />
                        ) : (
                          <strong>
                            {conversation.title || "新会话"}
                          </strong>
                        )}
                        <small>{formatDateTime(conversation.lastMessageAt ?? conversation.updatedAt)}</small>
                      </div>
                      <div className="aiConversationActions">
                        <button
                          className="miniAction"
                          type="button"
                          onClick={(event) => {
                            event.stopPropagation();
                            startRenameAiConversation(conversation);
                          }}
                        >
                          改名
                        </button>
                        <button
                          className="miniAction"
                          type="button"
                          onClick={(event) => {
                            event.stopPropagation();
                            deleteAiConversation(conversation.id);
                          }}
                        >
                          删除
                        </button>
                      </div>
                    </div>
                  ))}
                  {aiConversations.length === 0 ? (
                    <div className="aiConversationEmpty">
                      <strong>还没有这个助手的会话</strong>
                      <small>直接在右侧输入消息，会自动保存为历史。</small>
                    </div>
                  ) : null}
                </div>
              </aside>
            ) : null}
            <section className="aiChatPane" aria-label="AI 聊天">
              <div className="aiChatHeader">
                <strong>{selectedAiAssistant?.name ?? "默认助手"}</strong>
                {selectedAiConversation ? (
                  renamingAiConversationId === selectedAiConversation.id ? (
                    <input
                      aria-label="会话标题"
                      autoFocus
                      value={renamingAiConversationTitle}
                      onChange={(event) => setRenamingAiConversationTitle(event.target.value)}
                      onBlur={saveAiConversationTitle}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          saveAiConversationTitle();
                        }
                        if (event.key === "Escape") {
                          event.preventDefault();
                          setRenamingAiConversationId(null);
                        }
                      }}
                    />
                  ) : (
                    <button type="button" onClick={() => startRenameAiConversation(selectedAiConversation)}>
                      {selectedAiConversation.title || "新会话"}
                    </button>
                  )
                ) : (
                  <small>新会话</small>
                )}
              </div>
              <div className="aiMessageList">
                {aiMessages.map((message) => (
                  <article className={`aiMessage ${message.role}`} key={message.id}>
                    <div>
                      <strong>{message.role === "user" ? "你" : "AI"}</strong>
                      <button type="button" onClick={() => deleteAiMessage(message.id)}>
                        删除
                      </button>
                    </div>
                    <p>{message.content || (message.status === "streaming" ? "..." : "")}</p>
                    {message.error ? <small>{message.error}</small> : null}
                  </article>
                ))}
                {aiMessages.length === 0 ? (
                  <div className="aiEmptyState">
                    <strong>{selectedAiAssistant ? "新会话已准备好" : "选择助手后开始聊天"}</strong>
                    <small>
                      {selectedAiAssistant
                        ? `正在使用 ${selectedAiAssistant.name}，输入第一条消息即可开始。`
                        : "最近使用的助手会排在前面。"}
                    </small>
                  </div>
                ) : null}
              </div>
              <div className="aiComposer">
                <textarea
                  autoFocus
                  ref={aiInputRef}
                  value={aiInput}
                  onChange={(event) => setAiInput(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey) {
                      event.preventDefault();
                      sendAiMessage();
                    }
                  }}
                  placeholder="输入消息"
                />
                {activeAiRequestId ? (
                  <button type="button" onClick={cancelAiMessage}>
                    取消
                  </button>
                ) : (
                  <button type="button" onClick={sendAiMessage} disabled={!selectedAiAssistant || aiProfileMissing}>
                    发送
                  </button>
                )}
              </div>
            </section>
          </div>
        ) : null}

        {viewMode === "launcher" && !showSettings ? (
          <div className={contextSession ? "resultList contextResultList" : "resultList"}>
            {contextSession ? (
              <div className="contextHeader">
                <span>
                  <strong>{contextSession.result.title}</strong>
                  <small>{contextSession.result.subtitle}</small>
                </span>
                <button
                  type="button"
                  onClick={() => {
                    setContextSession(null);
                    setSelectedIndex(0);
                    setActionMessage("已返回搜索结果");
                  }}
                >
                  返回
                </button>
              </div>
            ) : null}
            {displayResults.map((result, index) => (
              <ResultRow
                actions={
                  contextSession ? (
                    <span className="resultAction">执行</span>
                  ) : (
                    <span className="resultAction">{actionLabels[result.action]}</span>
                  )
                }
                className={[
                  contextSession ? "contextActionItem" : "",
                  contextSession && visibleContextActions[index]?.danger ? "dangerAction" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                icon={resultIcon(result)}
                iconPath={result.iconPath}
                iconRetryKey={resultsRevision}
                key={result.id}
                onClick={async () => {
                  setSelectedIndex(index);
                  if (contextSession) {
                    const action = visibleContextActions[index];
                    if (action) {
                      await executeContextAction(action);
                    }
                    return;
                  }
                  await executeSelectedResult(result);
                }}
                onContextMenu={(event) => {
                  if (contextSession) {
                    return;
                  }
                  event.preventDefault();
                  setSelectedIndex(index);
                  openResultContextMenu(result);
                }}
                onKeyDown={(event) => {
                  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    handleSearchNavigationKeyDown(event);
                    focusSearchInput();
                    return;
                  }

                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setSelectedIndex(index);
                    if (contextSession) {
                      const action = visibleContextActions[index];
                      if (action) {
                        executeContextAction(action);
                      }
                      return;
                    }
                    if (event.key === "Enter" && event.shiftKey) {
                      openResultContextMenu(result);
                      return;
                    }
                    executeSelectedResult(result);
                  }
                }}
                role="button"
                ref={(element) => {
                  resultItemRefs.current[index] = element;
                }}
                selected={index === selectedIndex}
              >
                <strong>{result.title}</strong>
                <small>{result.subtitle}</small>
                {result.fileMetadata ? (
                  <span className="fileMetaLine">{formatFileMetadata(result.fileMetadata)}</span>
                ) : null}
              </ResultRow>
            ))}

            {displayResults.length === 0 ? <div className="emptyState">没有结果</div> : null}
          </div>
        ) : null}
      </section>
    </main>
  );
}

type SelectionActionSession = {
  conversationId: string | null;
  messages: AiMessage[];
  input: string;
  error: string | null;
};

type SelectionViewMode = "picker" | "result";

function SelectionAssistantApp() {
  const [capture, setCapture] = useState<SelectionCaptureEvent | null>(null);
  const [actions, setActions] = useState<AiSelectionAction[]>([]);
  const [providers, setProviders] = useState<AiProvider[]>([]);
  const [models, setModels] = useState<AiProviderModel[]>([]);
  const [selectedActionId, setSelectedActionId] = useState<string | null>(null);
  const [selectedProviderId, setSelectedProviderId] = useState<string>("");
  const [selectedModelName, setSelectedModelName] = useState<string>("");
  const [sessions, setSessions] = useState<Record<string, SelectionActionSession>>({});
  const [activeRequest, setActiveRequest] = useState<{ requestId: string; assistantId: string } | null>(
    null,
  );
  const [viewMode, setViewMode] = useState<SelectionViewMode>("picker");
  const [showMoreActions, setShowMoreActions] = useState(false);
  const [status, setStatus] = useState("等待选择动作");
  const [languageOption, setLanguageOption] = useState<LanguageOption>("system");
  const activeRequestRef = useRef<{ requestId: string; assistantId: string } | null>(null);
  const pendingSelectionRequestKeys = useRef<Set<string>>(new Set());
  const startedInitialSelectionKeys = useRef<Set<string>>(new Set());
  const displayLanguage = resolveDisplayLanguage(languageOption);
  useDisplayTranslations(displayLanguage);

  const enabledModels = models.filter((model) => model.enabled);
  const providerOptions = providers.filter((provider) =>
    enabledModels.some((model) => model.providerId === provider.id),
  );
  const currentAction = actions.find((action) => action.assistantId === selectedActionId) ?? null;
  const currentSession = selectedActionId ? sessions[selectedActionId] : null;
  const selectedProviderModels = enabledModels.filter(
    (model) => model.providerId === selectedProviderId,
  );
  const primarySelectionActions = actions.slice(0, SELECTION_PRIMARY_ACTION_LIMIT);
  const overflowSelectionActions = actions.slice(SELECTION_PRIMARY_ACTION_LIMIT);
  const sourceText = capture?.result.ok ? capture.result.text : "";
  useEffect(() => {
    invoke<string | null>("get_setting", {
      key: "ui.language",
    })
      .then((value) => setLanguageOption(normalizeLanguagePreference(value)))
      .catch(() => {
        setLanguageOption("system");
      });

    loadSelectionAssistantData().catch((error) => {
      setStatus(errorMessage(error, "划词助手数据读取失败"));
    });

    invoke<SelectionCaptureEvent | null>("get_pending_selection_capture")
      .then((event) => {
        if (event) {
          applySelectionCapture(event);
        }
      })
      .catch(() => {
        // Browser preview has no backend pending selection.
      });

    const unlisteners: Array<() => void> = [];
    listen<SelectionCaptureEvent>("selection-captured", (event) => {
      applySelectionCapture(event.payload);
    }).then((handler) => unlisteners.push(handler));
    listen<AiChatDeltaEvent>("ai-chat-delta", (event) => {
      if (activeRequestRef.current?.requestId !== event.payload.requestId) {
        return;
      }
      updateSelectionMessage(event.payload.messageId, (message) => ({
        ...message,
        content: `${message.content}${event.payload.delta}`,
        status: "streaming",
      }));
    }).then((handler) => unlisteners.push(handler));
    listen<AiChatDoneEvent>("ai-chat-done", (event) => {
      if (activeRequestRef.current?.requestId !== event.payload.requestId) {
        return;
      }
      activeRequestRef.current = null;
      setActiveRequest((current) =>
        current?.requestId === event.payload.requestId ? null : current,
      );
      updateSelectionMessage(event.payload.messageId, (message) => ({
        ...message,
        content: event.payload.content,
        status: "complete",
        error: null,
      }));
      setStatus("AI 回复完成");
    }).then((handler) => unlisteners.push(handler));
    listen<AiChatErrorEvent>("ai-chat-error", (event) => {
      if (activeRequestRef.current?.requestId !== event.payload.requestId) {
        return;
      }
      activeRequestRef.current = null;
      setActiveRequest((current) =>
        current?.requestId === event.payload.requestId ? null : current,
      );
      updateSelectionMessage(event.payload.messageId, (message) => ({
        ...message,
        status: "error",
        error: event.payload.error,
      }));
      setStatus(event.payload.error);
    }).then((handler) => unlisteners.push(handler));
    listen<{ requestId: string; messageId: string }>("ai-chat-cancelled", (event) => {
      if (activeRequestRef.current?.requestId !== event.payload.requestId) {
        return;
      }
      activeRequestRef.current = null;
      setActiveRequest((current) =>
        current?.requestId === event.payload.requestId ? null : current,
      );
      updateSelectionMessage(event.payload.messageId, (message) => ({
        ...message,
        status: "error",
        error: "AI 请求已取消",
      }));
      setStatus("AI 回复已取消");
    }).then((handler) => unlisteners.push(handler));

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeSelectionWindow();
      }
    }
    window.addEventListener("keydown", handleKeyDown, true);

    return () => {
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, []);

  useEffect(() => {
    resizeSelectionWindow(viewMode);
  }, [viewMode, actions.length, showMoreActions]);

  async function loadSelectionAssistantData() {
    const [loadedActions, loadedProviders, loadedModels] = await Promise.all([
      invoke<AiSelectionAction[]>("list_visible_ai_selection_actions"),
      invoke<AiProvider[]>("list_ai_providers"),
      invoke<AiProviderModel[]>("list_enabled_ai_provider_models"),
    ]);
    setActions(loadedActions);
    setProviders(loadedProviders);
    setModels(loadedModels);
    const firstAction = loadedActions[0] ?? null;
    if (firstAction) {
      restoreActionModel(firstAction, loadedModels);
    }
  }

  function applySelectionCapture(event: SelectionCaptureEvent) {
    cancelActiveSelectionRequestSilently();
    setCapture(event);
    setSessions({});
    setSelectedActionId(null);
    setShowMoreActions(false);
    setViewMode("picker");
    startedInitialSelectionKeys.current.clear();
    setStatus(event.result.message);
    if (!event.result.ok) {
      return;
    }
  }

  function updateSelectionMessage(
    messageId: string,
    updater: (message: AiMessage) => AiMessage,
  ) {
    setSessions((current) => {
      const next: Record<string, SelectionActionSession> = {};
      for (const [assistantId, session] of Object.entries(current)) {
        next[assistantId] = {
          ...session,
          messages: session.messages.map((message) =>
            message.id === messageId ? updater(message) : message,
          ),
        };
      }
      return next;
    });
  }

  function modelForAction(action: AiSelectionAction, modelList = enabledModels) {
    if (action.lastProviderId && action.lastModelName) {
      const lastModel = modelList.find(
        (model) =>
          model.providerId === action.lastProviderId && model.modelName === action.lastModelName,
      );
      if (lastModel) {
        return lastModel;
      }
    }

    const inheritedBinding = profileIdToModelBindingValue(
      action.assistantModelProfileId,
      [],
      providers,
      modelList,
    );
    const inherited = parseModelBindingValue(inheritedBinding);
    if (inherited) {
      return (
        modelList.find(
          (model) =>
            model.providerId === inherited.providerId && model.modelName === inherited.modelName,
        ) ?? null
      );
    }

    return modelList[0] ?? null;
  }

  function restoreActionModel(action: AiSelectionAction, modelList = enabledModels) {
    const target = modelForAction(action, modelList);
    setSelectedProviderId(target?.providerId ?? "");
    setSelectedModelName(target?.modelName ?? "");
  }

  async function updateActionModelSelection(
    action: AiSelectionAction,
    providerId: string,
    modelName: string,
  ) {
    if (!providerId || !modelName) {
      return;
    }
    try {
      const updatedAction = await invoke<AiSelectionAction>("set_ai_selection_action_model", {
        assistantId: action.assistantId,
        providerId,
        modelName,
      });
      setActions((current) =>
        current.map((item) =>
          item.assistantId === action.assistantId
            ? { ...item, ...updatedAction }
            : item,
        ),
      );
    } catch (error) {
      setStatus(errorMessage(error, "划词模型保存失败"));
    }
  }

  async function updateCurrentActionModelSelection(providerId: string, modelName: string) {
    if (!currentAction) {
      return;
    }
    await updateActionModelSelection(currentAction, providerId, modelName);
  }

  function resizeSelectionWindow(mode: SelectionViewMode) {
    const windowHandle = getCurrentWindow();
    if (mode === "picker") {
      const pickerHeight =
        SELECTION_PICKER_WINDOW_HEIGHT +
        (showMoreActions ? Math.min(overflowSelectionActions.length, 6) * 30 + 8 : 0);
      windowHandle
        .setMinSize(null)
        .then(() =>
          windowHandle.setSize(
            new LogicalSize(SELECTION_PICKER_WINDOW_WIDTH, pickerHeight),
          ),
        )
        .catch(() => {
          // Browser preview has no desktop selection window to resize.
        });
      return;
    }

    windowHandle
      .setMinSize(new LogicalSize(SELECTION_RESULT_WINDOW_WIDTH, SELECTION_RESULT_WINDOW_HEIGHT))
      .then(() =>
        windowHandle.setSize(
          new LogicalSize(SELECTION_RESULT_WINDOW_WIDTH, SELECTION_RESULT_WINDOW_HEIGHT),
        ),
      )
      .catch(() => {
        // Browser preview has no desktop selection window to resize.
      });
  }

  async function selectAction(action: AiSelectionAction) {
    if (activeRequest) {
      await cancelSelectionRequest();
    }
    setViewMode("result");
    setShowMoreActions(false);
    setSelectedActionId(action.assistantId);
    const targetModel = modelForAction(action);
    setSelectedProviderId(targetModel?.providerId ?? "");
    setSelectedModelName(targetModel?.modelName ?? "");
    const existingSession = sessions[action.assistantId];
    if (!existingSession && sourceText.trim()) {
      window.setTimeout(() => {
        startSelectionRequest(action, undefined, targetModel?.providerId, targetModel?.modelName, true);
      }, 0);
    }
  }

  async function startSelectionRequest(
    action = currentAction,
    followup?: string,
    providerOverride?: string,
    modelOverride?: string,
    ignoreActiveRequest = false,
  ) {
    if (!action || !sourceText.trim() || (activeRequest && !ignoreActiveRequest)) {
      return;
    }
    const session = sessions[action.assistantId] ?? {
      conversationId: null,
      messages: [],
      input: "",
      error: null,
    };
    const providerId = providerOverride ?? selectedProviderId;
    const modelName = modelOverride ?? selectedModelName;
    if (!providerId || !modelName) {
      setStatus("请先在 AI / 供应商模型 中启用至少一个模型");
      return;
    }
    const requestKey = [
      action.assistantId,
      session.conversationId ?? "new",
      followup?.trim() || "initial",
      cleanedSelectionText(sourceText),
    ].join(":");
    const isInitialRequest = !followup?.trim() && !session.conversationId;
    if (isInitialRequest && startedInitialSelectionKeys.current.has(requestKey)) {
      return;
    }
    if (pendingSelectionRequestKeys.current.has(requestKey)) {
      return;
    }
    pendingSelectionRequestKeys.current.add(requestKey);
    if (isInitialRequest) {
      startedInitialSelectionKeys.current.add(requestKey);
    }
    const requestId = `selection-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const request = { requestId, assistantId: action.assistantId };
    activeRequestRef.current = request;
    setActiveRequest(request);
    setStatus(`${action.selectionLabel} 正在生成`);
    try {
      const started = await invoke<AiChatStarted>("send_ai_selection_message", {
        request: {
          requestId,
          assistantId: action.assistantId,
          providerId,
          modelName,
          selectionText: sourceText,
          conversationId: session.conversationId,
          message: followup ?? null,
        },
      });
      setSessions((current) => ({
        ...current,
        [action.assistantId]: {
          conversationId: started.conversationId,
          messages: mergeAiMessages([
            ...(current[action.assistantId]?.messages ?? []).filter(
              (message) =>
                message.id !== started.userMessage.id &&
                message.id !== started.assistantMessage.id,
            ),
            started.userMessage,
            started.assistantMessage,
          ]),
          input: "",
          error: null,
        },
      }));
    } catch (error) {
      if (isInitialRequest) {
        startedInitialSelectionKeys.current.delete(requestKey);
      }
      setActiveRequest(null);
      setSessions((current) => ({
        ...current,
        [action.assistantId]: {
          ...session,
          error: errorMessage(error, "划词 AI 请求失败"),
        },
      }));
      setStatus(errorMessage(error, "划词 AI 请求失败"));
    } finally {
      pendingSelectionRequestKeys.current.delete(requestKey);
    }
  }

  async function sendFollowup() {
    if (!currentAction || !currentSession?.input.trim()) {
      return;
    }
    const message = currentSession.input.trim();
    setSessions((current) => ({
      ...current,
      [currentAction.assistantId]: {
        ...(current[currentAction.assistantId] ?? {
          conversationId: null,
          messages: [],
          error: null,
        }),
        input: "",
      },
    }));
    await startSelectionRequest(currentAction, message);
  }

  async function cancelSelectionRequest() {
    const request = activeRequestRef.current;
    if (!request) {
      return;
    }
    try {
      await invoke("cancel_ai_chat_message", { requestId: request.requestId });
      setActiveRequest((current) => (current?.requestId === request.requestId ? null : current));
      setStatus("正在取消 AI 回复");
    } catch (error) {
      setStatus(errorMessage(error, "取消失败"));
      activeRequestRef.current = null;
      setActiveRequest((current) => (current?.requestId === request.requestId ? null : current));
    }
  }

  function cancelActiveSelectionRequestSilently() {
    const request = activeRequestRef.current;
    if (!request) {
      return;
    }
    activeRequestRef.current = null;
    setActiveRequest((current) => (current?.requestId === request.requestId ? null : current));
    invoke("cancel_ai_chat_message", { requestId: request.requestId }).catch(() => {
      // The request may have finished between capture and cancellation.
    });
  }

  async function copyAiReply(content: string) {
    try {
      await invoke("copy_path", { path: content });
      setStatus("AI 回复已复制");
    } catch (error) {
      setStatus(errorMessage(error, "复制失败"));
    }
  }

  async function openAiSettings() {
    try {
      await invoke("show_ai_settings_window");
    } catch (error) {
      setStatus(errorMessage(error, "打开设置失败"));
    }
  }

  function closeSelectionWindow() {
    cancelActiveSelectionRequestSilently();
    invoke("hide_selection_window")
      .catch(() => {
        // Browser preview has no desktop selection window to hide.
      });
  }

  function setCurrentSessionInput(value: string) {
    if (!currentAction) {
      return;
    }
    setSessions((current) => ({
      ...current,
      [currentAction.assistantId]: {
        ...(current[currentAction.assistantId] ?? {
          conversationId: null,
          messages: [],
          error: null,
        }),
        input: value,
      },
    }));
  }

  function startWindowDrag(event: ReactMouseEvent<HTMLElement>) {
    if (
      event.target instanceof HTMLInputElement ||
      event.target instanceof HTMLButtonElement ||
      event.target instanceof HTMLTextAreaElement ||
      event.target instanceof HTMLSelectElement
    ) {
      return;
    }
    getCurrentWindow()
      .startDragging()
      .catch(() => {
        // Browser preview has no desktop window to drag.
      });
  }

  function startWindowResize(event: ReactMouseEvent<HTMLButtonElement>) {
    event.preventDefault();
    event.stopPropagation();
    getCurrentWindow()
      .startResizeDragging("SouthEast")
      .catch(() => {
        // Browser preview has no desktop window to resize.
      });
  }

  if (viewMode === "picker") {
    const pickerMessage =
      capture && !capture.result.ok
        ? capture.result.message
        : actions.length === 0
          ? "没有可用划词助手"
          : status;

    return (
      <main className="selectionAssistantShell picker" onMouseDown={startWindowDrag}>
        <section className="selectionPickerPanel" aria-label="划词动作选择">
          {primarySelectionActions.length > 0 && capture?.result.ok ? (
            <div className="selectionPickerActions" role="group" aria-label="划词动作">
              {primarySelectionActions.map((action) => (
                <button
                  type="button"
                  key={action.assistantId}
                  onClick={() => selectAction(action)}
                  disabled={!capture?.result.ok}
                  title={action.assistantDescription || action.selectionLabel}
                >
                  {action.selectionLabel}
                </button>
              ))}
              {overflowSelectionActions.length > 0 ? (
                <div className="selectionMoreActions">
                  <button
                    type="button"
                    aria-haspopup="menu"
                    aria-expanded={showMoreActions}
                    onClick={() => setShowMoreActions((shown) => !shown)}
                    disabled={!capture?.result.ok}
                    title="更多助手"
                  >
                    ...
                  </button>
                  {showMoreActions ? (
                    <div className="selectionMoreMenu" role="menu">
                      {overflowSelectionActions.map((action) => (
                        <button
                          type="button"
                          role="menuitem"
                          key={action.assistantId}
                          onClick={() => selectAction(action)}
                          title={action.assistantDescription || action.selectionLabel}
                        >
                          {action.selectionLabel}
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}
            </div>
          ) : (
            <span className="selectionPickerStatus">{pickerMessage}</span>
          )}
          <button type="button" className="selectionPickerClose" onClick={closeSelectionWindow} aria-label="关闭">
            x
          </button>
        </section>
      </main>
    );
  }

  return (
    <main className="selectionAssistantShell result" onMouseDown={startWindowDrag}>
      <section className="selectionAssistantPanel" aria-label="划词 AI 小对话框">
        <header className="selectionAssistantHeader">
          <span>
            <strong>{currentAction?.selectionLabel ?? "划词 AI"}</strong>
            <small>{status}</small>
          </span>
          <button type="button" onClick={closeSelectionWindow} aria-label="关闭">
            关闭
          </button>
        </header>

        <div className="selectionActionTabs" role="group" aria-label="划词动作">
          {primarySelectionActions.map((action) => (
            <button
              className={action.assistantId === selectedActionId ? "active" : ""}
              type="button"
              key={action.assistantId}
              onClick={() => selectAction(action)}
            >
              {action.selectionLabel}
            </button>
          ))}
          {overflowSelectionActions.length > 0 ? (
            <select
              aria-label="更多划词动作"
              value={
                overflowSelectionActions.some((action) => action.assistantId === selectedActionId)
                  ? selectedActionId ?? ""
                  : ""
              }
              onChange={(event) => {
                const action = overflowSelectionActions.find(
                  (item) => item.assistantId === event.currentTarget.value,
                );
                if (action) {
                  selectAction(action);
                }
              }}
            >
              <option value="">更多</option>
              {overflowSelectionActions.map((action) => (
                <option key={action.assistantId} value={action.assistantId}>
                  {action.selectionLabel}
                </option>
              ))}
            </select>
          ) : null}
        </div>

        {enabledModels.length === 0 ? (
          <div className="selectionModelMissing">
            <strong>请先在 AI / 供应商模型 中启用至少一个模型</strong>
            <button type="button" onClick={openAiSettings}>
              打开设置
            </button>
          </div>
        ) : (
          <div className="selectionModelBar">
            <select
              aria-label="供应商"
              value={selectedProviderId}
              onChange={(event) => {
                const providerId = event.target.value;
                const nextModel = enabledModels.find((model) => model.providerId === providerId);
                setSelectedProviderId(providerId);
                setSelectedModelName(nextModel?.modelName ?? "");
                if (nextModel) {
                  updateCurrentActionModelSelection(providerId, nextModel.modelName);
                }
              }}
            >
              {providerOptions.map((provider) => (
                <option key={provider.id} value={provider.id}>
                  {provider.name}
                </option>
              ))}
            </select>
            <select
              aria-label="模型"
              value={selectedModelName}
              onChange={(event) => {
                setSelectedModelName(event.target.value);
                updateCurrentActionModelSelection(selectedProviderId, event.target.value);
              }}
            >
              {selectedProviderModels.map((model) => (
                <option key={model.id} value={model.modelName}>
                  {model.modelName}
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="selectionChatList">
          {currentSession?.messages.map((message) => (
            <article className={`selectionChatMessage ${message.role}`} key={message.id}>
              <div>
                <strong>{message.role === "user" ? "你" : "AI"}</strong>
                {message.role === "assistant" && message.content ? (
                  <button type="button" onClick={() => copyAiReply(message.content)}>
                    复制
                  </button>
                ) : null}
              </div>
              <p>{message.content || (message.status === "streaming" ? "正在生成" : "")}</p>
              {message.error ? <small>{message.error}</small> : null}
            </article>
          ))}
          {currentSession?.error ? (
            <div className="selectionInlineError">{currentSession.error}</div>
          ) : null}
          {currentAction && !currentSession && enabledModels.length > 0 ? (
            <div className="selectionEmptyState">
              <span>正在准备请求</span>
            </div>
          ) : null}
        </div>

        <footer className="selectionComposer">
          <textarea
            autoFocus
            value={currentSession?.input ?? ""}
            onChange={(event) => setCurrentSessionInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                sendFollowup();
              }
            }}
            placeholder="继续追问"
          />
          {activeRequest ? (
            <button type="button" onClick={cancelSelectionRequest}>
              停止
            </button>
          ) : (
            <button type="button" onClick={sendFollowup} disabled={!currentSession?.input.trim()}>
              发送
            </button>
          )}
        </footer>
        <button
          type="button"
          className="selectionResizeHandle"
          onMouseDown={startWindowResize}
          aria-label="调整窗口大小"
          title="调整窗口大小"
        />
      </section>
    </main>
  );
}

function ResultRow({
  actions,
  children,
  className = "",
  icon,
  iconPath,
  iconRetryKey,
  onClick,
  onContextMenu,
  onKeyDown,
  ref,
  role = "button",
  selected,
}: {
  actions: ReactNode;
  children: ReactNode;
  className?: string;
  icon: string;
  iconPath?: string | null;
  iconRetryKey: number;
  onClick: () => void | Promise<void>;
  onContextMenu?: (event: ReactMouseEvent<HTMLDivElement>) => void;
  onKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
  ref?: (element: HTMLDivElement | null) => void;
  role?: string;
  selected: boolean;
}) {
  const iconSrc = useMemo(() => fileSrcForIconPath(iconPath), [iconPath]);
  const [imageRetry, setImageRetry] = useState(0);
  const [loadedIconSrc, setLoadedIconSrc] = useState<string | null>(null);
  const [failedIconSrc, setFailedIconSrc] = useState<string | null>(null);
  const imageFailedRef = useRef(false);
  const attemptedIconSrc = useMemo(
    () => appendIconRetryParam(iconSrc, imageRetry),
    [iconSrc, imageRetry],
  );
  const imageFailed = Boolean(attemptedIconSrc && failedIconSrc === attemptedIconSrc);
  const imageLoaded = Boolean(attemptedIconSrc && loadedIconSrc === attemptedIconSrc);

  useEffect(() => {
    imageFailedRef.current = false;
    setImageRetry(0);
  }, [iconSrc]);

  useEffect(() => {
    if (!imageFailedRef.current) {
      return;
    }

    imageFailedRef.current = false;
    setImageRetry(0);
    setFailedIconSrc(null);
  }, [iconRetryKey]);

  useEffect(() => {
    if (!iconSrc || !imageFailed || imageRetry >= 2) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setImageRetry((current) => current + 1);
    }, 250 * (imageRetry + 1));

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [iconSrc, imageFailed, imageRetry]);

  const showImageIcon = Boolean(attemptedIconSrc && imageLoaded && !imageFailed);

  return (
    <div
      aria-selected={selected}
      className={["resultItem", selected ? "selected" : "", className].filter(Boolean).join(" ")}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onKeyDown={onKeyDown}
      ref={ref}
      role={role}
      tabIndex={0}
    >
      <span className={showImageIcon ? "resultIcon imageResultIcon" : "resultIcon"}>
        {attemptedIconSrc && !imageFailed ? (
          <img
            alt=""
            aria-hidden="true"
            className={showImageIcon ? "resultIconImage" : "resultIconImage loadingResultIconImage"}
            draggable={false}
            onLoad={() => {
              imageFailedRef.current = false;
              setLoadedIconSrc(attemptedIconSrc);
              setFailedIconSrc(null);
            }}
            onError={() => {
              if ((import.meta as unknown as { env?: { DEV?: boolean } }).env?.DEV) {
                console.debug("[icons] failed to load result icon", {
                  iconPath,
                  iconSrc: attemptedIconSrc,
                  retry: imageRetry,
                });
              }
              imageFailedRef.current = true;
              setLoadedIconSrc(null);
              setFailedIconSrc(attemptedIconSrc);
            }}
            src={attemptedIconSrc}
          />
        ) : null}
        {showImageIcon ? null : (
          icon
        )}
      </span>
      <span className="resultText">{children}</span>
      <span className="resultActions">{actions}</span>
    </div>
  );
}

function isInteractiveDragTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    Boolean(
      target.closest(
        "button, input, textarea, select, label, nav, [role='button'], .aiSettingsItem, .resultItem",
      ),
    )
  );
}

function ErrorNotice({ error, onDismiss }: { error: AppError; onDismiss: () => void }) {
  return (
    <div className="errorNotice" role="alert">
      <span className="errorScope">{error.scope}</span>
      <span className="errorText">
        <strong>{error.title}</strong>
        <small>{error.message}</small>
      </span>
      <button type="button" onClick={onDismiss} aria-label="关闭错误提示">
        关闭
      </button>
    </div>
  );
}

function ToggleButton({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      className={active ? "sourceToggle active" : "sourceToggle"}
      type="button"
      aria-pressed={active}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

function WeightInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="weightInput">
      <span>{label}</span>
      <input
        aria-label={`${label}搜索权重`}
        type="number"
        min="0.1"
        max="3"
        step="0.1"
        value={Number.isFinite(value) ? value : 1}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

function SegmentedControl<TValue extends string>({
  ariaLabel,
  value,
  options,
  onChange,
}: {
  ariaLabel: string;
  value: TValue;
  options: Array<{ value: TValue; label: string }>;
  onChange: (value: TValue) => void;
}) {
  return (
    <div className="segmentedControl" role="group" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          className={option.value === value ? "segmentButton active" : "segmentButton"}
          type="button"
          aria-pressed={option.value === value}
          key={option.value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function errorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return error.trim();
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }

  return fallback;
}

function fallbackResults(query: string): SearchResult[] {
  const allResults: SearchResult[] = [
    {
      id: "fallback-app-vscode",
      title: "Visual Studio Code",
      subtitle: "前端 mock 应用结果",
      kind: "app",
      action: "launchApp",
      source: "应用",
      score: 0.98,
      shortcut: "Enter",
    },
    {
      id: "fallback-file-plan",
      title: "product-plan.md",
      subtitle: "docs/archive/f1-flow/product-plan-f1-flow.md",
      kind: "file",
      action: "openFile",
      source: "文件",
      score: 0.86,
      shortcut: "Enter",
    },
    {
      id: "fallback-command-settings",
      title: "打开系统设置",
      subtitle: "ms-settings:",
      kind: "command",
      action: "runCommand",
      source: "系统命令",
      score: 0.8,
      shortcut: "Enter",
    },
  ];

  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery) {
    return allResults;
  }

  return allResults.filter(
    (result) =>
      result.title.toLowerCase().includes(normalizedQuery) ||
      result.subtitle.toLowerCase().includes(normalizedQuery),
  );
}

function mergeIconUpdates(results: SearchResult[], updates: SearchIconUpdate[]): SearchResult[] {
  if (updates.length === 0) {
    return results;
  }

  const iconPaths = new Map(updates.map((update) => [update.resultId, update.iconPath]));
  let changed = false;
  const nextResults = results.map((result) => {
    const iconPath = iconPaths.get(result.id);
    if (!iconPath || result.iconPath === iconPath) {
      return result;
    }
    changed = true;
    return { ...result, iconPath };
  });

  return changed ? nextResults : results;
}

function mergeResultsPreservingIcons(
  nextResults: SearchResult[],
  currentResults: SearchResult[],
  cachedIconPaths?: Map<string, string>,
): SearchResult[] {
  if (nextResults.length === 0) {
    return nextResults;
  }

  const currentIconPaths = new Map<string, string>();
  if (cachedIconPaths) {
    for (const [key, iconPath] of cachedIconPaths) {
      currentIconPaths.set(key, iconPath);
    }
  }
  cacheResultIcons(currentResults, currentIconPaths);

  if (currentIconPaths.size === 0) {
    return nextResults;
  }

  let changed = false;
  const mergedResults = nextResults.map((result) => {
    if (result.iconPath) {
      return result;
    }

    const iconPath = cachedIconPathForResult(result, currentIconPaths);
    if (!iconPath) {
      return result;
    }

    changed = true;
    return { ...result, iconPath };
  });

  return changed ? mergedResults : nextResults;
}

function cacheResultIcons(results: SearchResult[], iconPaths: Map<string, string>) {
  for (const result of results) {
    if (result.iconPath) {
      for (const key of resultIconCacheKeys(result)) {
        iconPaths.set(key, result.iconPath);
      }
    }
  }
}

function cachedIconPathForResult(result: SearchResult, iconPaths: Map<string, string>) {
  for (const key of resultIconCacheKeys(result)) {
    const iconPath = iconPaths.get(key);
    if (!iconPath) {
      continue;
    }
    return iconPath;
  }

  return null;
}

function resultIconCacheKeys(result: SearchResult): string[] {
  const keys = [`id:${result.id}`];
  const path = normalizeResultIconPath(result.fileMetadata?.fullPath ?? result.subtitle);
  if (path && (result.kind === "file" || result.kind === "app")) {
    keys.push(`path:${path}`);
  }

  return keys;
}

function normalizeResultIconPath(path: string): string {
  let normalized = path.trim().replace(/\//g, "\\").toLowerCase();
  while (normalized.length > 3 && normalized.endsWith("\\")) {
    normalized = normalized.slice(0, -1);
  }
  return normalized;
}

function fileSrcForIconPath(iconPath?: string | null): string | null {
  if (!iconPath) {
    return null;
  }

  const tauriWindow = window as Window & { __TAURI_INTERNALS__?: unknown };
  if (!tauriWindow.__TAURI_INTERNALS__) {
    return null;
  }

  try {
    return convertFileSrc(iconPath);
  } catch {
    return null;
  }
}

function appendIconRetryParam(iconSrc: string | null, retry: number): string | undefined {
  if (!iconSrc) {
    return undefined;
  }
  if (retry <= 0) {
    return iconSrc;
  }

  const separator = iconSrc.includes("?") ? "&" : "?";
  return `${iconSrc}${separator}retry=${retry}`;
}

function providerToDraft(provider: AiProvider): AiProviderDraft {
  return {
    id: provider.id,
    name: provider.name,
    providerType: provider.providerType,
    baseUrl: provider.baseUrl,
    apiKey: provider.apiKey,
    enabled: provider.enabled,
    sortOrder: provider.sortOrder,
  };
}

function modelBindingValue(providerId: string, modelName: string): string {
  return `${providerId}${MODEL_BINDING_SEPARATOR}${modelName}`;
}

function parseModelBindingValue(value: string): { providerId: string; modelName: string } | null {
  const separatorIndex = value.indexOf(MODEL_BINDING_SEPARATOR);
  if (separatorIndex < 0) {
    return null;
  }

  const providerId = value.slice(0, separatorIndex).trim();
  const modelName = value.slice(separatorIndex + MODEL_BINDING_SEPARATOR.length).trim();
  return providerId && modelName ? { providerId, modelName } : null;
}

function providerModelProfileId(providerId: string, modelName: string): string {
  return `ai-model-profile:${providerId}:${encodeURIComponent(modelName)}`;
}

function profileIdToModelBindingValue(
  profileId: string,
  profiles: AiModelProfile[],
  providers: AiProvider[],
  enabledModels: AiProviderModel[],
): string {
  if (parseModelBindingValue(profileId)) {
    return profileId;
  }

  const generatedProfileModel = enabledModels.find(
    (model) => providerModelProfileId(model.providerId, model.modelName) === profileId,
  );
  if (generatedProfileModel) {
    return modelBindingValue(generatedProfileModel.providerId, generatedProfileModel.modelName);
  }

  const profile = profiles.find((item) => item.id === profileId);
  if (!profile) {
    const firstModel = enabledModels[0];
    return firstModel ? modelBindingValue(firstModel.providerId, firstModel.modelName) : "";
  }

  const provider = providers.find(
    (item) => item.baseUrl.trim() === profile.baseUrl.trim() && item.providerType === "openai_compatible",
  );
  if (!provider) {
    const firstModel = enabledModels[0];
    return firstModel ? modelBindingValue(firstModel.providerId, firstModel.modelName) : "";
  }

  const matchingModel = enabledModels.find(
    (model) => model.providerId === provider.id && model.modelName === profile.modelName,
  );
  return matchingModel ? modelBindingValue(provider.id, matchingModel.modelName) : "";
}

function assistantModelSummary(
  assistant: AiAssistant,
  profiles: AiModelProfile[],
  providers: AiProvider[],
  models: AiProviderModel[],
): string {
  const bindingValue = profileIdToModelBindingValue(
    assistant.modelProfileId,
    profiles,
    providers,
    models,
  );
  const binding = parseModelBindingValue(bindingValue);
  if (!binding) {
    return "未绑定可用模型";
  }
  const provider = providers.find((item) => item.id === binding.providerId);
  return `${provider?.name ?? "未知供应商"} / ${binding.modelName}`;
}

function inferProviderName(baseUrl: string): string {
  const withoutScheme = baseUrl.trim().replace(/^https?:\/\//i, "");
  const host = withoutScheme.split("/")[0]?.split("@").pop()?.split(":")[0] ?? "";
  const parts = host.split(".").filter(Boolean);
  if (parts.length >= 3 && parts[0].toLowerCase() === "api") {
    return parts[1];
  }
  if (parts.length >= 2) {
    return parts[parts.length - 2];
  }
  return host;
}

function cleanedSelectionText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

function mergeAiMessages(messages: AiMessage[]): AiMessage[] {
  const merged = new Map<string, AiMessage>();
  for (const message of messages) {
    merged.set(message.id, message);
  }
  return Array.from(merged.values());
}

function isBuiltinSelectionAssistantId(id: string): boolean {
  return BUILTIN_SELECTION_ASSISTANT_IDS.has(id);
}

function integerFromInput(value: string, fallback: number): number {
  const parsed = Number(value.trim());
  return Number.isFinite(parsed) ? Math.trunc(parsed) : fallback;
}

function assistantToDraft(assistant: AiAssistant): AiAssistantDraft {
  return {
    id: assistant.id,
    name: assistant.name,
    icon: assistant.icon,
    description: assistant.description,
    modelProfileId: assistant.modelProfileId,
    systemPrompt: assistant.systemPrompt,
    enabled: assistant.enabled,
    sortOrder: assistant.sortOrder,
  };
}

function contextActionsForResult(
  result: SearchResult,
  fileEditorPath = "",
  folderEditorPath = "",
  pinnedResults: PinnedResult[] = [],
  resultAliases: ResultAlias[] = [],
): ResultContextAction[] {
  const actions: ResultContextAction[] = [
    {
      id: "execute",
      title: actionLabels[result.action],
      subtitle: result.title,
    },
  ];

  if (matchesRankableResult(result)) {
    const pinned = pinnedResults.some((item) => item.resultId === result.id);
    actions.push({
      id: pinned ? "unpinResult" : "pinResult",
      title: pinned ? "取消固定" : "固定到顶部",
      subtitle: pinned ? "恢复为普通排序" : "匹配搜索时优先显示",
    });
    actions.push({
      id: "addAlias",
      title: "添加 alias",
      subtitle: "输入短词快速命中此结果",
    });
    const aliases = resultAliases.filter((alias) => alias.resultId === result.id);
    if (aliases.length > 0) {
      actions.push({
        id: "deleteAlias",
        title: "删除 alias",
        subtitle: aliases.map((alias) => alias.alias).join(", "),
      });
    }
  }

  if (matchesPathBackedResult(result)) {
    actions.push(
      {
        id: "openParent",
        title: "打开所在目录",
        subtitle: result.subtitle,
      },
      {
        id: "revealPath",
        title: "在资源管理器中选中",
        subtitle: result.subtitle,
      },
      {
        id: "copyPath",
        title: "复制完整路径",
        subtitle: result.subtitle,
      },
      {
        id: "copyName",
        title: "复制名称",
        subtitle: resultName(result),
      },
      {
        id: "copyFile",
        title: "复制文件或目录本体",
        subtitle: result.subtitle,
      },
      {
        id: "showNativeContextMenu",
        title: "显示 Windows 原生右键菜单",
        subtitle: result.subtitle,
      },
      {
        id: "openTerminal",
        title: "在当前目录打开终端",
        subtitle: result.subtitle,
      },
    );
  }

  if (result.kind === "file") {
    const editorPath = isDirectoryResult(result) ? folderEditorPath : fileEditorPath;
    if (editorPath.trim()) {
      actions.push({
        id: "openConfiguredEditor",
        title: isDirectoryResult(result) ? "用配置目录编辑器打开" : "用配置编辑器打开",
        subtitle: editorPath,
      });
    }
  }

  if (result.kind === "file" && isDirectoryResult(result)) {
    actions.push(
      {
        id: "addQuickAccess",
        title: "添加到快速访问",
        subtitle: result.subtitle,
      },
      {
        id: "removeQuickAccess",
        title: "从快速访问移除",
        subtitle: result.subtitle,
      },
    );
  }

  if (result.kind === "file") {
    actions.push({
      id: "openWith",
      title: "打开方式",
      subtitle: result.subtitle,
    });
  }

  if (result.kind === "app") {
    actions.push({
      id: "runAsAdmin",
      title: "以管理员运行",
      subtitle: result.subtitle,
    });
    actions.push({
      id: "runAsUser",
      title: "以其他用户运行",
      subtitle: "将打开 Windows runas 凭据提示",
    });
    if (result.subtitle.toLowerCase().endsWith(".lnk")) {
      actions.push({
        id: "openShortcutTargetParent",
        title: "打开目标所在目录",
        subtitle: result.subtitle,
      });
    }
  }

  if (matchesExcludableResult(result)) {
    actions.push({
      id: "deletePath",
      title: "删除文件或目录",
      subtitle: result.subtitle,
      danger: true,
    });
    actions.push({
      id: "hideResult",
      title: "隐藏此结果",
      subtitle: "写入隐藏规则，可在设置中恢复",
      danger: true,
    });
  }

  if (result.kind === "command") {
    actions.push({
      id: "copyPath",
      title: "复制命令文本",
      subtitle: result.subtitle,
    });
  }

  if (result.kind === "webSearch") {
    actions.push({
      id: "copyPath",
      title: "复制 URL",
      subtitle: result.subtitle,
    });
  }

  return actions;
}

function contextActionResult(sourceResult: SearchResult, action: ResultContextAction): SearchResult {
  return {
    id: `context:${sourceResult.id}:${action.id}`,
    title: action.title,
    subtitle: action.subtitle,
    kind: action.danger ? "command" : sourceResult.kind,
    action: "runCommand",
    source: "上下文菜单",
    score: 1,
  };
}

function matchesPathBackedResult(result: SearchResult): boolean {
  return result.kind === "app" || result.kind === "file";
}

function matchesExcludableResult(result: SearchResult): boolean {
  return result.kind === "app" || result.kind === "file";
}

function matchesRankableResult(result: SearchResult): boolean {
  return !result.id.startsWith("internal:") && ["app", "file", "command", "webSearch"].includes(result.kind);
}

function resultRankingInput(result: SearchResult) {
  return {
    resultId: result.id,
    kind: result.kind,
    title: result.title,
    target: result.subtitle,
  };
}

function isDirectoryResult(result: SearchResult): boolean {
  return result.fileMetadata?.isDir === true || result.source.includes("目录");
}

function resultName(result: SearchResult): string {
  const pathName = result.subtitle.split(/[\\/]/).pop()?.trim();
  return pathName || result.title;
}

function toolCommandFromResult(result: SearchResult): string | null {
  const command = result.id.replace(/^tool-(entry|hint):/, "").trim();
  return command ? `${command} ` : null;
}

function formatFileMetadata(metadata: FileMetadata): string {
  const parts = [
    metadata.isDir ? "目录" : metadata.extension ? metadata.extension.toUpperCase() : "文件",
    metadata.isDir ? null : formatFileSize(metadata.sizeBytes),
    formatModifiedTime(metadata.modifiedUnixSeconds),
    metadata.fullPath,
  ].filter((part): part is string => Boolean(part));

  return parts.join(" · ");
}

function resultIcon(result: SearchResult): string {
  if (result.kind === "file") {
    if (result.fileMetadata?.isDir || result.source.includes("目录")) {
      return "DIR";
    }

    const extension = result.fileMetadata?.extension || extensionFromPath(result.subtitle);
    return fileExtensionIcon(extension);
  }

  if (result.kind === "app") {
    return result.subtitle.toLowerCase().endsWith(".lnk") ? "LNK" : "APP";
  }

  if (result.id.startsWith("custom-command:")) {
    return "CMD";
  }

  if (result.kind === "webSearch") {
    return webSearchIcon(result);
  }

  return kindMarks[result.kind];
}

function webSearchIcon(result: SearchResult): string {
  const text = `${result.title} ${result.subtitle}`.toLowerCase();
  if (text.includes("github.com") || text.includes("github")) {
    return "GH";
  }
  if (text.includes("bing.com") || text.includes("bing") || text.includes("必应")) {
    return "BING";
  }
  if (text.includes("google.com") || text.includes("google")) {
    return "GO";
  }
  if (text.includes("stackoverflow.com") || text.includes("stack overflow")) {
    return "SO";
  }
  if (text.includes("developer.mozilla.org") || text.includes("mdn")) {
    return "MDN";
  }

  return "WEB";
}


function everythingStatusGuideFromStatus(
  status: EverythingStatus | null,
): EverythingStatusGuide {
  if (!status) {
    return {
      tone: "neutral",
      title: "尚未检查 Everything",
      detail: "应用、命令、计算、短语等搜索源仍可使用；点击“检查”刷新文件搜索状态。",
    };
  }

  if (!status.installed) {
    return {
      tone: "warning",
      title: "未检测到 Everything",
      detail: "文件搜索需要 Everything；其他搜索源仍可使用。点击“下载”打开安装页。",
    };
  }

  if (!status.running) {
    return {
      tone: "warning",
      title: "Everything 已安装但未运行",
      detail: "启动 Everything 后再点击“检查”；在此之前应用、命令和其他来源仍会继续搜索。",
    };
  }

  return {
    tone: "ok",
    title: "Everything 文件搜索可用",
    detail: status.httpAvailable
      ? "IPC 搜索优先，HTTP 仅作为备用通道；可按需启用全路径或内容搜索。"
      : "IPC 搜索可用；HTTP 备用接口未开启，不影响默认文件搜索。",
  };
}

function updateStatusGuideFromResult(
  result: UpdateCheckResult | null,
  checking: boolean,
  dismissedTag: string | null,
  lastCheckedAt: string | null,
): UpdateStatusGuide {
  if (checking) {
    return {
      tone: "neutral",
      title: "正在检查更新",
      detail: "正在读取 GitHub Releases 元数据。",
    };
  }

  const lastChecked = lastCheckedAt ? `上次检查：${formatDateTime(lastCheckedAt)}` : "尚未检查";
  if (!result) {
    return {
      tone: "neutral",
      title: "尚未检查更新",
      detail: `${lastChecked}。检查更新只访问 GitHub Releases API。`,
    };
  }

  if (result.error && !result.releaseUrl) {
    return {
      tone: "warning",
      title: "检查失败",
      detail: result.error,
    };
  }

  const latest = result.latestTag ?? result.latestVersion ?? "未知版本";
  if (result.isNewer === true) {
    if (dismissedTag === result.latestTag) {
      return {
        tone: "neutral",
        title: `已忽略版本 ${latest}`,
        detail: "本次不会再突出提醒；仍可打开 Release 页面查看。",
      };
    }

    return {
      tone: "warning",
      title: `发现新版本 ${latest}`,
      detail: result.assetName
        ? `Release 已发布 MSI：${result.assetName}。`
        : "Release 页面可查看版本说明；当前未发现 MSI asset。",
    };
  }

  if (result.isNewer === false) {
    return {
      tone: "ok",
      title: "已是最新版本",
      detail: `当前版本 ${result.currentVersion}，最新 Release 为 ${latest}。`,
    };
  }

  return {
    tone: "warning",
    title: "发现发布信息但无法比较",
    detail: result.error ?? "Release tag 不是三段数字版本，可打开 Release 页面手动确认。",
  };
}

function extensionFromPath(path: string): string | null {
  const name = path.split(/[\\/]/).pop() ?? path;
  const index = name.lastIndexOf(".");
  return index > -1 && index < name.length - 1 ? name.slice(index + 1) : null;
}

function fileExtensionIcon(extension?: string | null): string {
  const normalized = extension?.trim().toLowerCase();
  if (!normalized) {
    return "FILE";
  }

  const iconMap: Record<string, string> = {
    exe: "EXE",
    lnk: "LNK",
    html: "HTML",
    htm: "HTML",
    js: "JS",
    jsx: "JSX",
    ts: "TS",
    tsx: "TSX",
    json: "JSON",
    md: "MD",
    pdf: "PDF",
    png: "IMG",
    jpg: "IMG",
    jpeg: "IMG",
    webp: "IMG",
    svg: "SVG",
    zip: "ZIP",
    rar: "RAR",
    "7z": "7Z",
  };

  return iconMap[normalized] ?? normalized.slice(0, 4).toUpperCase();
}

function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatFileSize(sizeBytes?: number | null): string | null {
  if (sizeBytes === null || sizeBytes === undefined) {
    return null;
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = sizeBytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const formatted = unitIndex === 0 ? `${value}` : value.toFixed(value >= 10 ? 1 : 2);
  return `${formatted.replace(/\.0+$/, "")} ${units[unitIndex]}`;
}

function formatModifiedTime(unixSeconds?: number | null): string | null {
  if (!unixSeconds) {
    return null;
  }

  const date = new Date(unixSeconds * 1000);
  if (Number.isNaN(date.getTime())) {
    return null;
  }

  return date.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

export default App;
