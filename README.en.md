# Easy Launcher

English | [简体中文](README.md)

Easy Launcher is a fast command center for Windows. Press one shortcut, search apps, files, commands, web searches, AI actions, and snippets, then run the thing you need.

Its goal is simple: turn the tiny actions you repeat all day into one quick input box. Less icon hunting, less folder digging, less copy-paste busywork, and fewer jumps between tools.

![Easy Launcher search window](assets/screenshots/launcher-search-redacted.png)

## Why I Recommend It

Windows already has plenty of good tools: Start Menu launches apps, Everything finds files, browsers search the web, AI tools handle text, and system commands are all somewhere.

The problem is that those tools are scattered. Easy Launcher brings the common paths into a small, calm window that is always one shortcut away:

- Open VS Code, WeChat, Terminal, or any other app without hunting for icons.
- Search files from the same box when Everything is available.
- Translate, summarize, or explain selected text from any app.
- Calculate expressions, open system tools, copy snippets, or run custom commands.
- Use your own OpenAI-compatible service by setting the Base URL, API Key, and model.

This is not a concept demo. It is already useful as a daily Windows desktop tool, especially if you launch apps, find files, process text, and switch work contexts many times a day.

## Highlights

- Show or hide the launcher with `Alt+1`.
- Search Start Menu, Desktop, and common installation paths.
- Integrate with Everything for fast file search.
- Open files, open containing folders, and copy file paths.
- Run calculator results, system commands, web searches, snippets, and custom commands.
- Open the selection action menu with `Ctrl+Shift+Space`.
- Translate, summarize, explain, search, or copy selected text.
- Use OpenAI-compatible Chat Completions with streaming output and cancellation.
- Configure search sources, shortcuts, AI settings, Everything, and startup behavior.
- Import and export settings. API Keys are not exported by default.
- Stay in the tray with quick actions for opening the app, settings, Everything status, and exit.

## Who It Is For

Easy Launcher is a good fit if:

- You mainly use Windows 10 or Windows 11.
- You like keyboard-first app and file launching.
- You already use Everything, or want faster local file search.
- You often translate, summarize, explain, copy, or search selected text.
- You want AI features to use your own OpenAI-compatible provider instead of a fixed hosted service.

If you need enterprise deployment, automatic updates, code signing, cross-platform support, or credential-vault-level API Key protection, this version is not ready for that kind of production use yet.

## Quick Start

Default shortcuts:

- Launcher: `Alt+1`
- Selection menu: `Ctrl+Shift+Space`

Launcher:

1. Press `Alt+1` to open Easy Launcher.
2. Type an app name, file name, command, expression, snippet, or web search.
3. Select a result with arrow keys and press `Enter`.
4. File results can be opened, revealed in their folder, or copied as paths.
5. Open settings to adjust sources, shortcuts, AI, Everything, and startup behavior.

Selection menu:

1. Select text in any app.
2. Press `Ctrl+Shift+Space`.
3. Choose translate, summarize, explain, web search, or copy.

You can also switch the selection trigger to Ctrl-drag in settings. Hold `Ctrl`, drag-select text, and release the mouse to open the menu.

## Requirements

- Windows 10 or Windows 11.
- Optional: Everything for faster file search.
- Optional: an OpenAI-compatible API service for AI translation, summarization, and explanation.

For development or local builds:

- Node.js 18 or newer.
- Rust stable toolchain.
- Microsoft C++ Build Tools 2022 with MSVC and Windows SDK.

## Run From Source

Install dependencies:

```powershell
npm install
```

Check the Tauri desktop environment:

```powershell
npm run tauri -- info
```

Run the desktop app:

```powershell
npm run tauri -- dev
```

You can also use the task helper:

```powershell
npm run task
```

Run only the web UI:

```powershell
npm run dev
```

Vite dev server:

```text
http://127.0.0.1:1420/
```

## Build and Test

Build the frontend:

```powershell
npm run build
```

Run Rust tests:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Build the desktop app and MSI:

```powershell
npm run tauri -- build
```

Or use the packaging helper:

```powershell
npm run build:msi
```

MSI output path:

```text
src-tauri\target\release\bundle\msi\
```

If `cargo` is not on your system `PATH`, install Rust or set `CARGO_HOME`, `RUSTUP_HOME`, and the Cargo `bin` path in your current shell.

`npm run tauri -- ...` uses `scripts/tauri-with-rust-env.mjs`. The script reads `CARGO_HOME` and `RUSTUP_HOME` from the current shell or `.env.local`; otherwise it relies on `cargo` from the system `PATH`.

## Everything File Search

Everything is an optional enhancement, not a hard requirement for launching Easy Launcher.

When Everything is installed, Easy Launcher prefers IPC for file search. If IPC is unavailable, it can use Everything HTTP Server as a fallback at `127.0.0.1:8080`.

If Everything is running but HTTP fallback is disabled, the app points you to:

```text
Tools > Options > HTTP Server
```

If Everything is installed in a non-standard location, set this environment variable before launching the app:

```powershell
$env:EASY_LAUNCHER_EVERYTHING_EXE='<path-to-Everything.exe>'
```

You can also put it in local `.env.local`, which is ignored by Git:

```text
EASY_LAUNCHER_EVERYTHING_EXE=<path-to-Everything.exe>
```

## AI Setup

AI features use an OpenAI-compatible Chat Completions API. The project does not ship an API Key and does not proxy your requests through a maintainer-owned server.

Configure these fields in settings:

- Base URL, such as `https://api.openai.com`, `https://api.example.com/v1`, or a full `/v1/chat/completions` endpoint.
- API Key.
- Model, such as `gpt-4.1-mini` or any model provided by your compatible service.

Supported actions:

- Translate
- Summarize
- Explain

AI requests are sent from the Rust backend. The frontend receives streaming output through Tauri events.

## Local Data and Security

Runtime data directory:

```text
%LocalAppData%\EasyLauncher\
```

Main files:

```text
data.db              SQLite database
exports\             exported settings
logs\                logs
```

SQLite stores:

- Settings
- Recent items
- App index
- AI configuration

This version does not use Windows Credential Manager. API Keys are stored in plaintext in the local SQLite database:

```text
%LocalAppData%\EasyLauncher\data.db
```

Settings export does not include `ai.api_key` by default. It only includes non-sensitive settings such as shortcuts, source toggles, AI Base URL, AI Model, and startup behavior.

Selection capture temporarily simulates `Ctrl+C`, waits for clipboard text to change, and then tries to restore the previous text clipboard. Clipboard history does not filter sensitive content.

See `SECURITY.md` for the full security boundary and reporting guidance.

## Import and Export

The settings panel can export configuration as JSON. Exports are written to:

```text
%LocalAppData%\EasyLauncher\exports\
```

Import requires a JSON file path. Only supported allowlisted settings are accepted. Invalid JSON, unknown versions, invalid shortcuts, and invalid boolean values show errors.

Not exported by default:

- API Key
- Recent items
- App index

## Known Limitations

- Windows 10/11 only.
- macOS and Linux are not supported yet.
- UWP / Microsoft Store app scanning is not supported yet.
- Everything HTTP Server must be enabled manually in Everything.
- AI calls require a user-configured OpenAI-compatible service.
- API Keys are stored in plaintext in local SQLite.
- Selection capture relies on simulated `Ctrl+C`; some apps may block copy or selected text access.
- Ctrl-drag selection uses Windows global keyboard and mouse hooks, which can be affected by security software, Remote Desktop, privilege boundaries, or unusual input models.
- Simulated copy briefly uses the clipboard. Non-text clipboard content cannot be fully restored in this version.
- MSI packages are currently unsigned, so Windows may show SmartScreen or security warnings.
- Automatic updates and code signing are not implemented yet.

## Contributing

Bug reports, feature requests, and PRs are welcome. Please read `CONTRIBUTING.md` first for setup, branch guidance, style, validation commands, PR expectations, and security notes.

When reporting issues, include your Windows version, installation method, Everything status, whether AI is enabled, reproduction steps, expected behavior, and actual behavior. Screenshots or recordings are especially helpful for UI, shortcut, tray, installer, Everything, AI, and selection-menu issues.

Do not publish API Keys, Authorization headers, real tokens, local databases, sensitive file paths, or private text in issues, PRs, screenshots, logs, or exported settings.

## Release

Release steps, checksum commands, and release-note boundaries are documented in `RELEASE.md`.

Current MSI packages are unsigned. Only install from the project Release page or build from trusted local source.

## License

MIT License. See `LICENSE`.
