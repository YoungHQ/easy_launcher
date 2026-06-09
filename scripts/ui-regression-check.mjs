import { readFile } from "node:fs/promises";

const checks = [
  {
    file: "src/App.tsx",
    label: "main window keeps launcher, ai, and settings containers distinct",
    patterns: [
      '? "settingsPage"',
      '"launcherPage aiPage"',
      '"launcherPage contextMode"',
      ': "launcherPage"',
      'viewMode === "launcher"',
      'viewMode === "ai"',
    ],
  },
  {
    file: "src/App.tsx",
    label: "settings navigation exposes expected sections",
    patterns: [
      '{ id: "general"',
      '{ id: "search"',
      '{ id: "commands"',
      '{ id: "phrases"',
      '{ id: "webSearch"',
      '{ id: "exclusions"',
      '{ id: "backup"',
    ],
  },
  {
    file: "src/App.tsx",
    label: "core result actions remain available through context menu",
    patterns: [
      "openResultContextMenu(result)",
      "contextActionsForResult(",
      "openResultParent(result)",
      "runResultAsAdmin(result)",
      "hideResultFromSearch(result)",
      "revealResultPath(result)",
      "deleteResultPath(result)",
      "openShortcutTargetParent(result)",
      "runResultAsUser(result)",
      "copyFileResult(result)",
      "openResultWithDialog(result)",
      "openTerminalAtResult(result)",
      "openResultWithConfiguredEditor(result)",
      "open_configured_editor",
      "addQuickAccess",
      "removeQuickAccess",
      "set_quick_access",
    ],
  },
  {
    file: "src/App.tsx",
    label: "settings guidance blocks remain mounted",
    patterns: ["everythingStatusGuide", "configGuide", "exclusionGuide", "settingsHint"],
  },
  {
    file: "src/App.tsx",
    label: "configured editor settings remain mounted",
    patterns: ["file.editor.path", "folder.editor.path", "文件编辑器", "用配置编辑器打开"],
  },
  {
    file: "src/App.tsx",
    label: "selection assistant stays in the dedicated window flow",
    patterns: [
      'if (label === "selection")',
      'if (label === "reminder")',
      "return <TodoReminderApp />",
      "selectionAssistantShell picker",
      "selectionAssistantShell result",
      "list_visible_ai_selection_actions",
      "send_ai_selection_message",
      "hide_selection_window",
    ],
  },
  {
    file: "src/App.tsx",
    label: "quick entry results keep enter behavior",
    patterns: [
      "isQuickEntryCategoryResult(result)",
      "isQuickEntryWebTemplateResult(result)",
      "quickEntryCategoryQuery(result, toolMenuAlias)",
      "webTemplateQueryFromResult(result, toolMenuAlias)",
      "quickEntryToolQueryFromResult(result, toolMenuAlias)",
      'return command ? `${entryAlias}tools ${command} ` : null',
      'result.id.startsWith("tool-entry:")',
      'return "进入"',
      "快捷入口 Alias",
    ],
  },
  {
    file: "src/App.tsx",
    label: "slash board keeps two-column scoped preview flow",
    patterns: [
      "type SlashBoardScope",
      "type SlashBoardScopeSetting",
      "slashBoardActive",
      "visibleSlashBoardScopes.map",
      "slashBoardRail",
      "slashBoardPane",
      "invokeSlashBoardSearch",
      "slashBoardPreviewQueries",
      "slashBoardStaticResults",
      "settingsSearchResult",
      "mergeSlashBoardResultGroups",
      "nextSlashBoardScope",
      "slashBoardScopesFromSettings",
      'event.key === "ArrowRight"',
      'event.key === "ArrowLeft"',
      "!slashBoardActive && (contextSession || query.trim().length > 0)",
    ],
  },
  {
    file: "src/App.tsx",
    label: "slash board menu settings can hide and reorder scopes",
    patterns: [
      "slash.board.scopes",
      "Slash Board 菜单",
      "saveSlashBoardScopeSettings",
      "updateSlashBoardScopeVisibility",
      "reorderSlashBoardScope",
      "handleSlashBoardScopePointerDown",
      "handleSlashBoardScopePointerEnter",
      "slashBoardSettingsOrder",
      "slashBoardSettingsHandle",
      "onPointerEnter",
      "normalizeSlashBoardScopeSettings",
      "至少保留一个范围",
    ],
  },
  {
    file: "src/styles.css",
    label: "slash board keeps compact rail and dense results",
    patterns: [
      ".slashBoard",
      "grid-template-columns: 108px minmax(0, 1fr)",
      ".slashBoardRail",
      ".slashBoardScope",
      ".slashBoardPane",
      ".slashBoardResultItem",
      "min-height: 38px",
      ".slashBoardSettingsList",
      "grid-auto-flow: column",
    ],
  },
  {
    file: "src/styles.css",
    label: "result list keeps bottom radius clipping",
    patterns: [".launcherPage", "overflow: hidden", ".resultList", "border-radius: 0 0 9px 9px"],
  },
  {
    file: "src/styles.css",
    label: "settings layout keeps scrollable content and compact nav",
    patterns: [".settingsPane", "grid-template-columns: 168px minmax(0, 1fr)", ".settingsContent", "overflow: auto"],
  },
  {
    file: "src/styles.css",
    label: "button and guide styles use existing palette variables",
    patterns: [
      "var(--color-accent)",
      "var(--color-surface-subtle)",
      "var(--color-danger-soft)",
      ".configGuide",
    ],
  },
];

let failed = false;

for (const check of checks) {
  const content = await readFile(check.file, "utf8");
  const missing = check.patterns.filter((pattern) => !content.includes(pattern));

  if (missing.length > 0) {
    failed = true;
    console.error(`UI regression check failed: ${check.label}`);
    for (const pattern of missing) {
      console.error(`  missing ${JSON.stringify(pattern)} in ${check.file}`);
    }
  } else {
    console.log(`ok: ${check.label}`);
  }
}

if (failed) {
  process.exitCode = 1;
}
