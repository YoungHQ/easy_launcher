# Easy Launcher

Easy Launcher 是一个 Windows-first 桌面启动器。按下快捷键后，可以在一个紧凑输入框里搜索应用、文件、系统命令、网页搜索、快捷短语、计算结果和 AI 文本动作。

更完整的产品说明见：[产品文档](product-html/index.html)。

## 核心功能

- `Alt+1` 呼出或隐藏主启动器。
- 搜索并启动开始菜单、桌面、常见安装目录和 PATH 中的应用。
- 集成 Everything 文件搜索，支持打开文件、打开所在目录和复制路径。
- 支持计算器、系统命令、网页搜索、自定义命令和快捷短语。
- 选中文本后按 `Ctrl+Shift+Space` 呼出划词菜单。
- 划词菜单支持翻译、总结、解释、网页搜索和复制。
- AI 使用 OpenAI 兼容 Chat Completions 接口，支持流式输出和取消。
- 支持搜索源开关、快捷键设置、AI 配置、开机自启动、配置导入导出和手动检查更新。

## 系统要求

- Windows 10 或 Windows 11。
- 可选：Everything，用于更快的文件搜索。
- 可选：OpenAI 兼容 API 服务，用于 AI 翻译、总结和解释。

从源码开发或构建还需要：

- Node.js 18 或更新版本。
- Rust stable toolchain。
- Microsoft C++ Build Tools 2022，包含 MSVC 和 Windows SDK。

## 开发运行

```powershell
npm install
npm run tauri -- dev
```

只启动 Web UI：

```powershell
npm run dev
```

构建前端：

```powershell
npm run build
```

运行 Rust 测试：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

构建 Windows 桌面安装包：

```powershell
npm run tauri -- build
```

如果 Rust 不在系统 `PATH` 中，可以使用 `npm run tauri -- ...`，它会通过 `scripts/tauri-with-rust-env.mjs` 读取当前 shell 或 `.env.local` 中的 `CARGO_HOME`、`RUSTUP_HOME`。

## 本地数据和安全

运行时数据目录：

```text
%LocalAppData%\EasyLauncher\
```

当前版本没有接入 Windows Credential Manager。AI API Key 会明文保存在本机 SQLite 数据库中：

```text
%LocalAppData%\EasyLauncher\data.db
```

配置导出默认不包含 `ai.api_key`、最近使用记录和应用索引。不要在 issue、PR、截图、日志或导出配置中公开 API Key、Authorization header、真实 token、本机数据库、敏感文件路径或用户隐私文本。

## 已知限制

- 当前只支持 Windows 10/11。
- UWP / Microsoft Store 应用扫描暂不支持。
- Everything HTTP Server 需要用户手动在 Everything 中开启。
- AI 功能需要用户自行配置可用的 OpenAI 兼容服务。
- 划词取词通过模拟 `Ctrl+C` 实现，部分应用可能禁止复制或无法读取选中文本。
- MSI 暂未签名，Windows 可能显示 SmartScreen 或安全提示。

## 许可证

本项目使用 MIT License，见 `LICENSE`。
