import { useEffect } from "react";

export type LanguagePreference = "system" | "zh-CN" | "en-US";
export type DisplayLanguage = "zh-CN" | "en-US";

const LANGUAGE_SETTING_VALUES = new Set<LanguagePreference>(["system", "zh-CN", "en-US"]);

const textNodeOriginals = new WeakMap<Text, string>();

function isChineseLocale(locale: string): boolean {
  return locale.trim().toLowerCase().startsWith("zh");
}

export function normalizeLanguagePreference(value?: string | null): LanguagePreference {
  return value && LANGUAGE_SETTING_VALUES.has(value as LanguagePreference)
    ? (value as LanguagePreference)
    : "system";
}

export function resolveDisplayLanguage(preference: LanguagePreference): DisplayLanguage {
  if (preference === "zh-CN" || preference === "en-US") {
    return preference;
  }

  const languages =
    typeof navigator === "undefined"
      ? []
      : navigator.languages && navigator.languages.length > 0
        ? navigator.languages
        : [navigator.language];

  return languages.some((language) => isChineseLocale(language)) ? "zh-CN" : "en-US";
}

const exactText: Record<string, string> = {
  "后端未验证": "Backend not verified",
  "输入关键词搜索": "Type to search",
  "关闭": "Close",
  "设置": "Settings",
  "打开 Easy Launcher 设置": "Open Easy Launcher settings",
  "搜索，输入 option 打开设置": "Search, type option to open settings",
  "筛选操作：": "Filter actions:",
  "Slash Board 范围": "Slash Board scopes",
  "综合": "Overview",
  "文本": "Text",
  "最近打开": "Recent",
  "常用打开": "Open",
  "没有内容": "No content",
  "Slash Board 加载失败": "Slash Board failed to load",
  "设置分组": "Settings sections",
  "设置操作区": "Settings content",
  "搜索设置": "Search Settings",
  "搜索": "Search",
  "配置": "Config",
  "隐藏": "Hidden",
  "通用设置": "General Settings",
  "搜索入口、启动方式和本机状态": "Search entry, startup behavior, and local status",
  "状态": "Status",
  "检查后端": "Check Backend",
  "检查 Everything": "Check Everything",
  "SQLite 未验证": "SQLite not verified",
  "主快捷键": "Main Shortcut",
  "快速唤起": "Quick Launch",
  "系统": "System",
  "AI 快捷键": "AI Shortcut",
  "应用": "Apply",
  "文件": "File",
  "计算": "Calculator",
  "命令": "Command",
  "短语": "Phrase",
  "网页": "Web",
  "工具": "Tool",
  "选择参与搜索的来源，并调整结果排序权重":
    "Choose searchable sources and tune result ranking weights",
  "搜索源开关": "Search Sources",
  "搜索权重": "Search Weights",
  "Everything 高级选项": "Everything Advanced Options",
  "用于文件搜索的 Everything 查询参数": "Everything query options for file search",
  "尚未检查 Everything": "Everything has not been checked",
  "应用、命令、计算、短语等搜索源仍可使用；点击“检查”刷新文件搜索状态。":
    "Apps, commands, calculator, phrases, and other sources still work. Click Check to refresh file search status.",
  "未检测到 Everything": "Everything was not detected",
  "文件搜索需要 Everything；其他搜索源仍可使用。点击“下载”打开安装页。":
    "File search requires Everything. Other search sources still work. Click Download to open the installer page.",
  "Everything 已安装但未运行": "Everything is installed but not running",
  "启动 Everything 后再点击“检查”；在此之前应用、命令和其他来源仍会继续搜索。":
    "Start Everything, then click Check again. Apps, commands, and other sources continue to work meanwhile.",
  "Everything 文件搜索可用": "Everything file search is available",
  "IPC 搜索优先，HTTP 仅作为备用通道；可按需启用全路径或内容搜索。":
    "IPC search is preferred. HTTP is only a fallback. Enable full-path or content search as needed.",
  "IPC 搜索可用；HTTP 备用接口未开启，不影响默认文件搜索。":
    "IPC search is available. The HTTP fallback is not enabled, which does not affect default file search.",
  "版本支持": "Version Support",
  "支持普通 Installer / Portable 版 Everything；不建议使用 Lite 版，因为 Lite 移除了 IPC 和 HTTP Server，文件搜索无法正常关联。":
    "Regular Installer and Portable builds of Everything are supported. Lite is not recommended because it removes IPC and HTTP Server, so file search cannot integrate reliably.",
  "关联逻辑": "Integration",
  "Easy Launcher 会连接正在运行的": "Easy Launcher connects to the running",
  "，优先使用 IPC 查询；IPC 失败时才尝试本机": ", uses IPC first, and only tries the local",
  "的 HTTP Server 备用接口。": "HTTP Server fallback when IPC fails.",
  "Everything 高级搜索选项": "Everything advanced search options",
  "全路径会匹配完整路径；内容搜索可能较慢，需 Everything 已启用内容索引或支持内容查询。":
    "Full path matches complete paths. Content search can be slower and requires Everything content indexing or content-query support.",
  "检查": "Check",
  "下载": "Download",
  "HTTP 指引": "HTTP Guide",
  "全路径": "Full Path",
  "内容": "Content",
  "文件编辑器": "File Editors",
  "配置后，文件和目录结果的上下文菜单会显示对应编辑器动作":
    "After configuration, file and folder result context menus show matching editor actions",
  "文件编辑器路径": "File editor path",
  "目录编辑器路径": "Folder editor path",
  "例如 C:\\Program Files\\Microsoft VS Code\\Code.exe":
    "Example: C:\\Program Files\\Microsoft VS Code\\Code.exe",
  "未配置文件编辑器": "No file editor configured",
  "未配置目录编辑器": "No folder editor configured",
  "选择": "Choose",
  "清除": "Clear",
  "选择编辑器可执行文件；清除后隐藏对应菜单项。启动时会把当前文件或目录路径作为参数传给编辑器。":
    "Choose the editor executable. Clear it to hide the matching menu item. The current file or folder path is passed to the editor when launched.",
  "留空则隐藏对应菜单项；启动时会把当前文件或目录路径作为参数传给编辑器。":
    "Leave blank to hide the menu item. The current file or folder path is passed to the editor when launched.",
  "目录": "Folder",
  "保存": "Save",
  "工具设置": "Tool Settings",
  "输入快捷入口查看分类；输入 enc、dec、pwd 或 time 进入单个工具":
    "Type the quick entry alias to show categories. Type enc, dec, pwd, or time to open a specific tool.",
  "入口": "Entry",
  "打开快捷入口；选择 tools 后进入工具清单。":
    "opens quick entry. Select tools to show the tool list.",
  "转换": "Conversion",
  "返回编码和摘要；": "returns encodings and summaries.",
  "返回解码、HTML 实体和 URL 参数 JSON 解析。":
    "returns decoded text, HTML entities, and URL-parameter JSON parsing.",
  "其他": "Other",
  "按下方默认策略生成密码；": "generates a password using the default policy below.",
  "自动识别时间戳或日期时间。": "auto-detects timestamps or date/time values.",
  "快捷入口 Alias": "Quick Entry Alias",
  "默认 /；保存后输入该 alias 会显示命令、短语、网页搜索、工具和最近打开分类":
    "Default is /. After saving, typing this alias shows commands, phrases, web search, tools, and recent-opened categories.",
  "快捷入口保存失败": "Failed to save quick entry alias",
  "快捷入口不能为空": "Quick entry alias cannot be empty",
  "快捷入口不能包含空格": "Quick entry alias cannot contain spaces",
  "快捷入口不能使用已有快捷指令": "Quick entry alias cannot use an existing shortcut command",
  "Alias 与快捷入口冲突": "Alias conflicts with the quick entry alias",
  "不能包含空格，也不能使用 enc、dec、pwd、time 或 tools，避免和搜索词或短指令冲突。":
    "Cannot contain spaces or use enc, dec, pwd, time, or tools, to avoid conflicts with search terms and short commands.",
  "随机密码默认值": "Random Password Defaults",
  "可在搜索框输入 pwd 20 临时覆盖长度": "Type pwd 20 in search to temporarily override the length.",
  "随机密码默认长度": "Default random password length",
  "随机密码字符集": "Random password character sets",
  "大写 U": "Uppercase U",
  "小写 W": "Lowercase W",
  "数字 D": "Digits D",
  "减号 -": "Hyphen -",
  "下划线 W": "Underscore W",
  "特殊 E": "Special E",
  "括号 B": "Brackets B",
  "后端会把长度限制在 4-128；如果关闭全部字符集，会自动恢复大写、小写和数字。":
    "The backend clamps length to 4-128. If all character sets are disabled, uppercase, lowercase, and digits are restored automatically.",
  "长度": "Length",
  "大写": "Uppercase",
  "小写": "Lowercase",
  "数字": "Digits",
  "连字符": "Hyphen",
  "下划线": "Underscore",
  "特殊字符": "Special",
  "括号": "Brackets",
  "AI 设置": "AI Settings",
  "AI 设置子页": "AI settings tab",
  "先配置供应商和模型，再创建助手并绑定已启用模型":
    "Configure providers and models first, then create assistants and bind enabled models.",
  "供应商模型": "Providers and Models",
  "AI 服务供应商": "AI Service Providers",
  "OpenAI 兼容接口": "OpenAI-compatible API",
  "供应商名称": "Provider name",
  "供应商 Base URL": "Provider Base URL",
  "供应商 API Key": "Provider API Key",
  "显示": "Show",
  "选择/编辑": "Select/Edit",
  "新建供应商": "New Provider",
  "模型管理": "Model Management",
  "当前供应商：": "Current provider:",
  "未选择供应商": "No provider selected",
  "模型搜索": "Model search",
  "搜索模型": "Search models",
  "手动添加模型": "Add model manually",
  "获取中": "Fetching",
  "获取当前供应商模型": "Fetch Provider Models",
  "暂无模型，可以获取模型或手动添加模型名称":
    "No models yet. Fetch models or add a model name manually.",
  "请先保存并选择供应商": "Save and select a provider first",
  "助手": "Assistants",
  "聊天助手定义；划词显示请到“划词”页设置":
    "Chat assistant definitions. Configure selection visibility on the Selection page.",
  "请先在“供应商模型”中启用至少一个模型。":
    "Enable at least one model in Providers and Models first.",
  "助手名称": "Assistant name",
  "助手图标": "Assistant icon",
  "绑定模型": "Bind model",
  "保存助手": "Save Assistant",
  "助手描述": "Assistant description",
  "助手排序": "Assistant order",
  "排序": "Order",
  "启用助手": "Enable Assistant",
  "助手系统提示词": "Assistant system prompt",
  "系统提示词": "System prompt",
  "无描述": "No description",
  "新建助手": "New Assistant",
  "划词": "Selection",
  "划词设置": "Selection Settings",
  "控制划词触发方式、浮窗入口和每个助手在划词场景中的模型":
    "Control selection triggers, popup entries, and each assistant's model in selection scenarios.",
  "触发方式": "Trigger Mode",
  "开启后，按住 Ctrl 并用鼠标划词会弹出划词 AI 浮窗":
    "When enabled, hold Ctrl and select text with the mouse to show the selection AI popup.",
  "划词功能": "Selection Feature",
  "已开启": "Enabled",
  "已关闭": "Disabled",
  "立即读取选区": "Read Selection Now",
  "触发": "Trigger",
  "Ctrl + 鼠标划词": "Ctrl + mouse selection",
  "可用助手": "Available Assistants",
  "显示名称": "Display name",
  "系统提示词、助手说明和模型在 AI / 助手 中统一编辑；划词浮窗里切换模型会同步修改同一个助手模型。":
    "System prompts, assistant descriptions, and models are edited in AI / Assistants. Switching models in the selection popup updates the same assistant model.",
  "自定义命令": "Custom Commands",
  "示例": "Example",
  "规则": "Rules",
  "URL 可填": "URL can be",
  "，程序可填": ", and a program can be",
  "名称不允许重复；自定义命令不支持": "Names cannot be duplicated. Custom commands do not support",
  "，需要搜索词时请使用网页模板。": ". Use web templates when you need query text.",
  "配置导出会包含自定义命令；导入时同 ID 覆盖更新，不同 ID 但名称重复会被拒绝。":
    "Config export includes custom commands. Import updates matching IDs; different IDs with duplicate names are rejected.",
  "自定义命令名称": "Custom command name",
  "命令名称": "Command name",
  "自定义命令类型": "Custom command type",
  "自定义命令目标": "Custom command target",
  "固定 URL、文件路径或程序路径": "Fixed URL, file path, or program path",
  "暂无自定义命令": "No custom commands",
  "程序": "Program",
  "快捷短语": "Phrases",
  "标题可填": "Title can be",
  "，内容可填需要一键复制的常用文本。":
    ", and content can be reusable text you want to copy with one action.",
  "标题不允许重复；内容最多 4000 个字符。":
    "Titles cannot be duplicated. Content can be up to 4000 characters.",
  "配置导出会包含快捷短语；导入时同 ID 覆盖更新，不同 ID 但标题重复会被拒绝。":
    "Config export includes phrases. Import updates matching IDs; different IDs with duplicate titles are rejected.",
  "快捷短语标题": "Phrase title",
  "短语标题": "Phrase title",
  "快捷短语内容": "Phrase content",
  "常用文本内容": "Reusable text content",
  "暂无快捷短语": "No phrases",
  "网页搜索模板": "Web Search Templates",
  "网页搜索关键词": "Web search keyword",
  "关键词，例如 gh": "Keyword, e.g. gh",
  "网页搜索名称": "Web search name",
  "模板名称": "Template name",
  "搜索框用法": "Search Box Usage",
  "输入": "Type",
  "会用 gh 模板搜索 rust tauri。": "to search rust tauri with the gh template.",
  "强制网页模板": "Force Web Template",
  "如果关键词和 app、file、cmd 等内置前缀冲突，可输入":
    "If a keyword conflicts with built-in prefixes such as app, file, or cmd, type",
  "URL 模板，必须包含 {query}": "URL template, must include {query}",
  "关键词不允许重复，只能包含字母、数字、": "Keywords cannot be duplicated and may contain only letters, digits,",
  "或": "or",
  "；URL 必须以 http/https 开头并包含": ". URLs must start with http/https and include",
  "配置导出会包含网页模板；导入时同 ID 覆盖更新，不同 ID 但关键词重复会被拒绝。":
    "Config export includes web templates. Import updates matching IDs; different IDs with duplicate keywords are rejected.",
  "暂无网页搜索模板": "No web search templates",
  "隐藏规则": "Hidden Rules",
  "结果 ID": "Result ID",
  "精确隐藏单个结果，适合应用、命令或固定文件项。":
    "Hide one exact result. Useful for apps, commands, or pinned file items.",
  "路径规则": "Path Rule",
  "可用": "Use",
  "匹配路径片段，例如": "to match path fragments, for example",
  "恢复显示": "Restore Display",
  "删除对应规则后，匹配结果会重新参与搜索。":
    "After deleting a rule, matching results are included in search again.",
  "排除规则类型": "Exclusion rule type",
  "排除规则内容": "Exclusion rule content",
  "暂无隐藏规则": "No hidden rules",
  "配置文件": "Config File",
  "导出当前配置，或从 JSON 文件导入": "Export current config or import from a JSON file",
  "导出内容": "Export Contents",
  "包含快捷键、搜索源、权重、Everything 选项、自定义命令、短语、网页模板和隐藏规则。":
    "Includes shortcuts, search sources, weights, Everything options, custom commands, phrases, web templates, and hidden rules.",
  "安全": "Security",
  "AI API Key 不会写入导出文件；导入会跳过不支持的设置项。":
    "AI API keys are not written to export files. Import skips unsupported settings.",
  "冲突": "Conflicts",
  "同 ID 的条目会覆盖更新；不同 ID 但名称、标题或关键词重复时，导入会提示冲突并停止。":
    "Items with the same ID are updated. Duplicate names, titles, or keywords with different IDs stop import with a conflict.",
  "配置 JSON 路径": "Config JSON path",
  "导入 JSON 路径": "Import JSON path",
  "邮箱签名": "Email signature",
  "路径": "Path",
  "导出": "Export",
  "导入": "Import",
  "更新": "Update",
  "新增": "Add",
  "编辑": "Edit",
  "删除": "Delete",
  "改名": "Rename",
  "新建": "New",
  "返回": "Back",
  "执行": "Run",
  "启动": "Launch",
  "打开": "Open",
  "运行": "Run",
  "翻译": "Translate",
  "总结": "Summarize",
  "没有结果": "No results",
  "浏览器预览模式": "Browser preview mode",
  "后端连接失败": "Backend connection failed",
  "快捷键状态读取失败": "Failed to read shortcut status",
  "本地设置读取失败": "Failed to read local settings",
  "Everything 状态读取失败": "Failed to read Everything status",
  "未检测到 Everything，请安装后使用文件搜索":
    "Everything was not detected. Install it to use file search.",
  "Everything 正在运行，HTTP 备用接口可用":
    "Everything is running and the HTTP fallback is available",
  "Everything 正在运行，HTTP 备用接口未开启":
    "Everything is running and the HTTP fallback is not enabled",
  "模型配置未完成": "Model setup is incomplete",
  "需要 Base URL 和模型名称。": "Base URL and model name are required.",
  "打开 AI 设置": "Open AI Settings",
  "会话": "Conversations",
  "暂无历史": "No history",
  "会话标题": "Conversation title",
  "新会话": "New conversation",
  "还没有会话": "No conversations yet",
  "还没有这个助手的会话": "No conversations for this assistant yet",
  "直接在右侧输入消息，会自动保存为历史。":
    "Type a message on the right and it will be saved to history.",
  "默认助手": "Default Assistant",
  "未选择助手": "No assistant selected",
  "未知助手": "Unknown assistant",
  "聊天供应商": "Chat provider",
  "聊天模型": "Chat model",
  "未选择模型": "No model selected",
  "聊天模型切换失败": "Failed to switch chat model",
  "你": "You",
  "新会话已准备好": "New conversation is ready",
  "选择助手后开始聊天": "Choose an assistant to start chatting",
  "最近使用的助手会排在前面。": "Recently used assistants appear first.",
  "输入消息": "Type a message",
  "取消": "Cancel",
  "发送": "Send",
  "划词动作选择": "Selection action picker",
  "划词动作": "Selection actions",
  "保存当前划词原文": "Save selected text",
  "记录": "Mark",
  "记录中": "Saving",
  "正在记录": "Saving mark",
  "已记录": "Marked",
  "设置提醒时间并加入待办": "Set a reminder and add todo",
  "待办": "Todo",
  "加入待办": "Add Todo",
  "划词待办时间选择": "Selection todo time picker",
  "提醒时间": "Reminder time",
  "10 分钟后": "In 10 minutes",
  "30 分钟后": "In 30 minutes",
  "1 小时后": "In 1 hour",
  "今晚 20:00": "Tonight 20:00",
  "明晚 20:00": "Tomorrow night 20:00",
  "明早 09:00": "Tomorrow 09:00",
  "自定义": "Custom",
  "提醒日期": "Reminder date",
  "提醒时间必须晚于当前时间": "Reminder time must be later than now",
  "选择提醒时间": "Choose reminder time",
  "加入中": "Adding",
  "待办保存失败": "Failed to save todo",
  "待办提醒": "Todo reminder",
  "等待提醒": "Waiting for reminder",
  "到时间了": "Due now",
  "已过期": "Overdue",
  "已完成": "Done",
  "暂无待处理提醒": "No pending reminder",
  "稍后": "Snooze",
  "完成": "Done",
  "修改提醒时间": "Edit reminder time",
  "修改内容": "Edit content",
  "没有可用划词助手": "No selection assistants available",
  "更多助手": "More assistants",
  "划词 AI 小对话框": "Selection AI dialog",
  "划词 AI": "Selection AI",
  "更多划词动作": "More selection actions",
  "更多": "More",
  "请先在 AI / 供应商模型 中启用至少一个模型":
    "Enable at least one model in AI / Providers and Models first",
  "未配置地址": "No URL configured",
  "打开设置": "Open Settings",
  "供应商": "Provider",
  "模型": "Model",
  "复制": "Copy",
  "正在生成": "Generating",
  "正在准备请求": "Preparing request",
  "继续追问": "Follow up",
  "停止": "Stop",
  "调整窗口大小": "Resize window",
  "关闭错误提示": "Dismiss error",
  "通用": "General",
  "快捷键、系统": "Shortcuts, system",
  "来源、权重": "Sources, weights",
  "搜索来源": "Search sources",
  "关闭后对应来源不参与普通搜索；Slash 直接入口不受影响":
    "Disabled sources are excluded from normal search; direct Slash entries are unaffected",
  "权重微调": "Weight tuning",
  "只有需要改变默认排序倾向时再调整；1.0 表示默认权重":
    "Adjust only when you need to change ranking preference; 1.0 is the default weight",
  "密码、转换": "Password, conversion",
  "模型、助手": "Models, assistants",
  "触发、浮窗": "Trigger, popup",
  "划词记录": "Selection marks",
  "记录只通过 /mark 取回，不进入普通搜索，也不会写入配置导出":
    "Marks are retrieved only through /mark, stay out of plain search, and are not exported",
  "划词记录操作": "Selection mark actions",
  "清空划词记录": "Clear selection marks",
  "划词记录保存失败": "Failed to save selection mark",
  "划词记录清空失败": "Failed to clear selection marks",
  "待办只通过 /todo 管理；已完成项可定期清理":
    "Todos are managed through /todo; completed items can be cleared periodically",
  "待办提醒操作": "Todo reminder actions",
  "清空已完成待办": "Clear completed todos",
  "已完成待办清空失败": "Failed to clear completed todos",
  "固定入口": "Pinned entries",
  "常用文本": "Reusable text",
  "搜索模板": "Search templates",
  "排除规则": "Exclusion rules",
  "导入导出": "Import/export",
  "语言": "Language",
  "跟随系统": "Follow System",
  "中文": "Chinese",
  "English": "English",
  "显示语言": "Display Language",
  "默认跟随系统；系统语言不是中文时显示英文界面。":
    "Defaults to system language; non-Chinese system languages use English.",
  "应用设置后仅影响界面展示文案，不会修改快捷键、命令或已有常用英语。":
    "This only changes interface text, not shortcuts, commands, or existing common English terms.",
};

const replacements: Array<[RegExp, string]> = [
  [/^SQLite 已初始化：(.+)$/, "SQLite initialized: $1"],
  [/^(.+) 用于呼出启动器$/, "$1 opens the launcher"],
  [/^(.+) 用于打开 AI 面板$/, "$1 opens the AI panel"],
  [/^(.+) 用于呼出\/隐藏启动器$/, "$1 opens/hides the launcher"],
  [/^浏览器预览模式，快捷键仅在桌面端生效$/, "Browser preview mode; shortcuts only work in the desktop app"],
  [/^浏览器预览模式，AI 快捷键仅在桌面端生效$/, "Browser preview mode; AI shortcut only works in the desktop app"],
  [/^Enter 执行，Esc 清空$/, "Enter to run, Esc to clear"],
  [/^没有匹配结果$/, "No matching results"],
  [/^已补充文件结果$/, "Added file results"],
  [/^浏览器预览模式，使用 mock 结果$/, "Browser preview mode, using mock results"],
  [/^已返回搜索结果$/, "Returned to search results"],
  [/^AI 回复完成$/, "AI response complete"],
  [/^AI 回复已取消$/, "AI response canceled"],
  [/^AI 请求已取消$/, "AI request canceled"],
  [/^主快捷键已更新$/, "Main shortcut updated"],
  [/^编辑器路径已保存$/, "Editor paths saved"],
  [/^Ctrl\+鼠标划词已开启$/, "Ctrl + mouse selection trigger enabled"],
  [/^划词功能已开启$/, "Selection feature enabled"],
  [/^划词功能已关闭$/, "Selection feature disabled"],
  [/^双击 Alt 唤起已开启$/, "Double Alt launch enabled"],
  [/^双击 Alt 唤起已关闭$/, "Double Alt launch disabled"],
  [/^开机自启动已开启$/, "Launch at startup enabled"],
  [/^开机自启动已关闭$/, "Launch at startup disabled"],
  [/^会话已重命名$/, "Conversation renamed"],
  [/^已导出 (\d+) 项配置$/, "Exported $1 config items"],
  [/^已导入 (\d+) 项配置$/, "Imported $1 config items"],
  [/^自定义命令已保存$/, "Custom command saved"],
  [/^快捷短语已保存$/, "Phrase saved"],
  [/^已清空划词记录：(\d+) 条$/, "Cleared $1 selection marks"],
  [/^已清空已完成待办：(\d+) 条$/, "Cleared $1 completed todos"],
  [/^网页搜索模板已保存$/, "Web search template saved"],
  [/^排除规则已保存$/, "Exclusion rule saved"],
  [/^搜索源设置已保存$/, "Search source settings saved"],
  [/^搜索权重已保存$/, "Search weight settings saved"],
  [/^工具设置已保存$/, "Tool settings saved"],
  [/^Everything 搜索选项已保存$/, "Everything search options saved"],
  [/^Everything 设置路径：工具 > 选项 > HTTP 服务器$/, "Everything path: Tools > Options > HTTP Server"],
  [/^已打开 Everything 下载页$/, "Opened Everything download page"],
  [/^快捷入口已保存：(.+)$/, "Quick entry alias saved: $1"],
  [/^(.+) 个供应商$/, "$1 providers"],
  [/^(.+) 个启用模型$/, "$1 enabled models"],
  [/^已获取 (\d+) 个模型，请勾选需要启用的模型$/, "Fetched $1 models. Check the models you want to enable."],
  [/^模型已启用：(.+)$/, "Model enabled: $1"],
  [/^模型已停用：(.+)$/, "Model disabled: $1"],
  [/^供应商已保存：(.+)$/, "Provider saved: $1"],
  [/^正在编辑供应商：(.+)$/, "Editing provider: $1"],
  [/^供应商已删除$/, "Provider deleted"],
  [/^助手已保存：(.+)$/, "Assistant saved: $1"],
  [/^正在编辑助手：(.+)$/, "Editing assistant: $1"],
  [/^助手已删除$/, "Assistant deleted"],
  [/^聊天模型已切换：(.+)$/, "Chat model switched: $1"],
  [/^已显示划词动作：(.+)$/, "Selection action shown: $1"],
  [/^已隐藏划词动作：(.+)$/, "Selection action hidden: $1"],
  [/^已显示 (\d+) 个；浮窗默认展示前 5 个，其余通过横向滚动访问$/,
    "$1 shown. The popup shows the first 5 by default; the rest are available by horizontal scrolling."],
  [/^正在编辑：(.+)$/, "Editing: $1"],
  [/^正在编辑排除规则：(.+)$/, "Editing exclusion rule: $1"],
  [/^已删除：(.+)$/, "Deleted: $1"],
  [/^已删除排除规则：(.+)$/, "Deleted exclusion rule: $1"],
  [/^(.+) 个可用$/, "$1 available"],
  [/^(.+) 条历史$/, "$1 history items"],
  [/^全部 (\d+) 条$/, "All $1"],
  [/^(.+) 个固定入口$/, "$1 pinned entries"],
  [/^(.+) 条常用文本$/, "$1 phrases"],
  [/^(.+) 个搜索模板$/, "$1 search templates"],
  [/^(.+) 条排除规则$/, "$1 exclusion rules"],
  [/^正在使用 (.+)，输入第一条消息即可开始。$/, "Using $1. Type the first message to start."],
  [/^(.+) 正在生成$/, "$1 is generating"],
  [/^AI 回复已复制$/, "AI response copied"],
  [/^正在取消 AI 回复$/, "Canceling AI response"],
  [/^等待选择动作$/, "Waiting for an action"],
  [/^已加入待办，将于 (.+) 提醒$/, "Added todo. Reminder at $1"],
  [/^已完成待办：(.+)$/, "Completed todo: $1"],
  [/^已稍后 (\d+) 分钟：(.+)$/, "Snoozed $2 for $1 minutes"],
  [/^已稍后 (\d+) 分钟$/, "Snoozed for $1 minutes"],
  [/^已修改提醒时间：(.+)$/, "Updated reminder time: $1"],
  [/^已修改待办：(.+)$/, "Updated todo: $1"],
  [/^已删除待办：(.+)$/, "Deleted todo: $1"],
  [/^已取消修改提醒时间：(.+)$/, "Canceled reminder time edit: $1"],
  [/^已取消修改待办：(.+)$/, "Canceled todo edit: $1"],
  [/^已取消删除待办：(.+)$/, "Canceled todo deletion: $1"],
  [/^已获取选中文本$/, "Selected text captured"],
  [/^没有读取到选中文本$/, "No selected text was captured"],
  [/^没有读取到选中文本，请确认当前应用中已有文本选区$/, "No selected text was captured. Make sure text is selected in the current app."],
];

export function translateDisplayText(text: string, language: DisplayLanguage): string {
  if (language !== "en-US") {
    return text;
  }

  const compact = text.replace(/\s+/g, " ").trim();
  if (!compact) {
    return text;
  }

  const exact = exactText[compact];
  if (exact) {
    return text.replace(compact, exact);
  }

  for (const [pattern, replacement] of replacements) {
    if (pattern.test(compact)) {
      return text.replace(compact, compact.replace(pattern, replacement));
    }
  }

  return text;
}

function translateTextNode(node: Text, language: DisplayLanguage) {
  const current = node.nodeValue ?? "";
  const storedOriginal = textNodeOriginals.get(node);
  const storedTranslation = storedOriginal
    ? translateDisplayText(storedOriginal, "en-US")
    : undefined;
  const original =
    storedOriginal && (current === storedOriginal || current === storedTranslation)
      ? storedOriginal
      : current;
  textNodeOriginals.set(node, original);
  const next = language === "en-US" ? translateDisplayText(original, language) : original;
  if (current !== next) {
    node.nodeValue = next;
  }
}

function translateAttribute(element: Element, name: string, language: DisplayLanguage) {
  if (!["aria-label", "placeholder", "title"].includes(name)) {
    return;
  }

  const value = element.getAttribute(name);
  if (!value) {
    return;
  }

  const originalKey = `data-i18n-original-${name}`;
  const storedOriginal = element.getAttribute(originalKey);
  const storedTranslation = storedOriginal
    ? translateDisplayText(storedOriginal, "en-US")
    : undefined;
  const original =
    storedOriginal && (value === storedOriginal || value === storedTranslation)
      ? storedOriginal
      : value;
  if (storedOriginal !== original) {
    element.setAttribute(originalKey, original);
  }

  const next = language === "en-US" ? translateDisplayText(original, language) : original;
  if (value !== next) {
    element.setAttribute(name, next);
  }
}

function translateTree(root: ParentNode, language: DisplayLanguage) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    const parent = node.parentElement;
    if (parent && !parent.closest("input, textarea, code")) {
      translateTextNode(node as Text, language);
    }
    node = walker.nextNode();
  }

  if (root instanceof Element) {
    translateAttribute(root, "aria-label", language);
    translateAttribute(root, "placeholder", language);
    translateAttribute(root, "title", language);
  }

  root.querySelectorAll?.("[aria-label], [placeholder], [title]").forEach((element) => {
    translateAttribute(element, "aria-label", language);
    translateAttribute(element, "placeholder", language);
    translateAttribute(element, "title", language);
  });
}

export function useDisplayTranslations(language: DisplayLanguage) {
  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    const root = document.getElementById("root") ?? document.body;
    translateTree(root, language);

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "characterData" && mutation.target instanceof Text) {
          const parent = mutation.target.parentElement;
          if (parent && !parent.closest("input, textarea, code")) {
            translateTextNode(mutation.target, language);
          }
          continue;
        }

        if (mutation.type === "attributes" && mutation.target instanceof Element) {
          translateAttribute(mutation.target, mutation.attributeName ?? "", language);
          continue;
        }

        mutation.addedNodes.forEach((node) => {
          if (node instanceof Text) {
            const parent = node.parentElement;
            if (parent && !parent.closest("input, textarea, code")) {
              translateTextNode(node, language);
            }
          } else if (node instanceof Element) {
            translateTree(node, language);
          }
        });
      }
    });

    observer.observe(root, {
      attributes: true,
      attributeFilter: ["aria-label", "placeholder", "title"],
      characterData: true,
      childList: true,
      subtree: true,
    });

    return () => observer.disconnect();
  }, [language]);
}
