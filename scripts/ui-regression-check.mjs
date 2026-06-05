import { readFile } from "node:fs/promises";

const checks = [
  {
    file: "src/App.tsx",
    label: "launcher/settings page switch keeps distinct containers",
    patterns: ['? "settingsPage"', ': "launcherPage"', 'viewMode === "clipboard"', 'viewMode === "selection"'],
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
      "contextActionsForResult(result, fileEditorPath, folderEditorPath)",
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
    patterns: ["everythingStatusGuide", "configGuide", "exclusionGuide", "clipboardStatusHint"],
  },
  {
    file: "src/App.tsx",
    label: "configured editor settings remain mounted",
    patterns: ["file.editor.path", "folder.editor.path", "文件编辑器", "用配置编辑器打开"],
  },
  {
    file: "src/App.tsx",
    label: "clipboard panel keeps paste as primary action",
    patterns: [
      "pasteClipboardItem(item)",
      "paste_clipboard_item",
      "Ctrl+Enter 复制",
      "删除这条剪贴板历史",
      "clearClipboardItems",
      "addClipboardItemToPhrase",
      "visibleClipboardText",
      "clipboard.paste.restore_previous",
      "粘贴后恢复原剪贴板",
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
