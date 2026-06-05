# Easy Launcher

Easy Launcher 是一个 Windows-first 桌面启动器，使用 Tauri 2、Rust、React、TypeScript 和 Vite 构建。

v0.1 目标是做一个可日常试用的紧凑启动器：全局搜索、应用启动、Everything 文件搜索、划词菜单、AI 文本处理、计算器、系统命令、托盘常驻、开机自启动和配置导入导出。

## 当前状态

已实现：

- `Alt+1` 呼出或隐藏启动器主窗口。
- 搜索并启动开始菜单、桌面和常见安装目录中的应用。
- 通过 Everything IPC 优先、HTTP 备用搜索文件。
- 打开文件、打开所在目录、复制文件路径。
- 选中文本后按 `Ctrl+Shift+Space` 呼出划词动作菜单。
- 划词菜单支持翻译、总结、解释、网页搜索和复制。
- OpenAI 兼容接口 AI 调用，支持流式输出和取消。
- 计算器、系统命令。
- 搜索源开关、快捷键设置、AI 配置、开机自启动。
- 配置导入导出，默认不导出 API Key。
- 系统托盘菜单：打开主窗口、打开设置、检查 Everything 状态、退出。
- 统一错误提示。

发布准备：

- 已配置 GitHub Actions Windows 构建。
- 已生成未签名 Windows MSI。
- 已补充 v0.1 发布检查清单。
- 部分桌面能力仍建议在干净 Windows 用户环境中手动验证，见“验证建议”。

## 系统要求

- Windows 10 或 Windows 11。
- Node.js 18 或更新版本。
- Rust stable toolchain。
- Microsoft C++ Build Tools 2022，包含 MSVC 和 Windows SDK。
- 可选：Everything，用于文件搜索。
- 可选：OpenAI 兼容 API 服务，用于翻译、总结、解释。

## 安装依赖

```powershell
npm install
```

检查 Tauri 桌面开发环境：

```powershell
npm run tauri -- info
```

## 开发运行

启动桌面应用：

```powershell
npm run tauri -- dev
```

也可以使用统一任务脚本交互选择启动或打包：

```powershell
npm run task
```

直接启动桌面开发项目：

```powershell
npm run start:desktop
```

只启动 Web UI：

```powershell
npm run dev
```

Vite 开发地址：

```text
http://127.0.0.1:1420/
```

构建前端：

```powershell
npm run build
```

运行 Rust 测试：

```powershell
cargo test
```

如果 `cargo` 不在系统 `PATH` 中，请先安装 Rust，或在当前 shell 中设置 `CARGO_HOME`、`RUSTUP_HOME` 并把 Cargo 的 `bin` 目录加入 `PATH`。

## 本机 Rust 包装脚本

`npm run tauri -- ...` 会通过以下脚本启动 Tauri CLI：

```text
scripts/tauri-with-rust-env.mjs
```

脚本会使用当前 shell 或 `.env.local` 中已有的 `CARGO_HOME` 和 `RUSTUP_HOME`。如果没有设置这些变量，则直接依赖系统 `PATH` 中可用的 `cargo`。

如果你的 Rust 安装在非默认目录，建议在启动开发命令前设置好环境变量，例如：

```powershell
$env:CARGO_HOME='<path-to-cargo-home>'
$env:RUSTUP_HOME='<path-to-rustup-home>'
$env:Path='<path-to-cargo-bin>;' + $env:Path
```

## 使用说明

默认快捷键：

- 主启动器：`Alt+1`
- 划词菜单：`Ctrl+Shift+Space`

主启动器：

1. 按 `Alt+1` 打开或隐藏启动器。
2. 输入关键词搜索应用、文件、命令、计算器结果、快捷短语、网页搜索或 AI 动作。
3. 使用方向键选择结果，按 `Enter` 执行。
4. 文件结果支持打开、打开目录和复制路径。
5. 点击“设置”展开紧凑设置面板。

划词菜单：

1. 默认模式下，在任意应用中选中文本后按 `Ctrl+Shift+Space`。
2. 也可以在设置中把“划词触发”切到“Ctrl+划词”，之后按住 `Ctrl` 并用鼠标拖选文本，松开鼠标后自动呼出划词菜单。
3. 在启动器内选择翻译、总结、解释、搜索或复制。
4. 取词时会临时模拟 `Ctrl+C`，等待系统剪贴板内容变化后恢复原文本剪贴板。

托盘：

- 右键或左键点击托盘图标打开菜单。
- 菜单包含打开主窗口、打开设置、检查 Everything 状态和退出。
- Windows 上双击托盘图标会打开主窗口。

## Everything 文件搜索

Easy Launcher v0.1 使用 Everything 做 Windows 文件搜索：

- 优先使用 Everything IPC。
- 如果 IPC 不可用，可使用 Everything HTTP Server 作为备用。
- HTTP Server 默认访问 `127.0.0.1:8080`。

如果未安装 Everything，启动器会显示下载入口。Everything 已运行但 HTTP 备用接口未开启时，界面会提示：

```text
工具 > 选项 > HTTP 服务器
```

第一版不内置打包 Everything，也不会自动安装 Everything。

如果 Everything 安装在非常规位置，可以在启动应用前设置环境变量：

```powershell
$env:EASY_LAUNCHER_EVERYTHING_EXE='<path-to-Everything.exe>'
```

也可以写入本机 `.env.local`，该文件已被 `.gitignore` 排除：

```text
EASY_LAUNCHER_EVERYTHING_EXE=<path-to-Everything.exe>
```

## AI 配置

AI 功能使用 OpenAI 兼容 Chat Completions 接口。需要在设置面板中填写：

- Base URL，例如 `https://api.openai.com`、`https://api.example.com/v1` 或完整 `/v1/chat/completions` 地址。
- API Key。
- Model，例如 `gpt-4.1-mini` 或兼容服务提供的模型名。

支持的动作：

- 翻译
- 总结
- 解释

AI 请求默认走 Rust 后端发起，前端通过 Tauri event 接收流式输出。

## API Key 安全说明

v0.1 没有接入 Windows Credential Manager。

当前 API Key 会明文保存在本机 SQLite 数据库中：

```text
%LocalAppData%\EasyLauncher\data.db
```

默认配置导出不会包含 `ai.api_key`。导出内容只包含非敏感设置，例如快捷键、搜索源开关、AI Base URL、AI Model 和开机自启动开关。

这个版本只适合个人本机试用。不要在共享电脑、受管设备或不可信环境中保存重要 API Key。

## 本地数据

运行时数据目录：

```text
%LocalAppData%\EasyLauncher\
```

主要文件：

```text
data.db              SQLite 数据库
exports\             配置导出目录
logs\                日志目录
```

SQLite 中会保存：

- 设置项
- 最近使用记录
- 应用索引
- AI 配置

划词触发模式默认是 `shortcut`。设置为 `ctrl_mouse` 后，Windows 后台会监听全局键鼠事件，只在检测到 `Ctrl` 按住、左键拖拽距离达到阈值并释放后触发取词；`Ctrl+Shift+Space` 仍保留为兜底快捷键。

## 配置导入导出

设置面板中可以导出配置 JSON。导出文件默认写入：

```text
%LocalAppData%\EasyLauncher\exports\
```

导入时需要输入 JSON 文件路径。导入只接受当前版本支持的白名单设置，非法 JSON、未知版本、非法快捷键或非法布尔值会显示错误提示。

默认不导出：

- API Key
- 最近使用记录
- 应用索引

## 构建

构建前端：

```powershell
npm run build
```

构建桌面应用：

```powershell
npm run tauri -- build
```

也可以使用打包脚本：

```powershell
npm run build:msi
```

MSI 产物位置：

```text
src-tauri\target\release\bundle\msi\
```

构建桌面调试程序但不打包安装器：

```powershell
npm run tauri -- build --debug --no-bundle
```

调试可执行文件位置：

```text
src-tauri\target\debug\easy-launcher.exe
```

v0.1 计划生成未签名 Windows MSI。未签名安装包会触发 Windows 安全提示，这是当前版本的已知发布限制。

## 许可证

本项目使用 MIT License，见 `LICENSE`。

## 已知限制

- 第一版只支持 Windows。
- macOS 和 Linux 暂不支持。
- UWP / Microsoft Store 应用扫描暂不支持。
- Everything HTTP Server 需要用户手动在 Everything 中开启。
- AI 真实调用需要用户自行配置可用的 OpenAI 兼容服务。
- API Key 明文保存在本机 SQLite 中。
- 划词取词通过模拟 `Ctrl+C` 实现，已增加剪贴板变化检测和等待重试，但部分应用仍可能禁止复制或无法读取选中文本。
- `Ctrl+划词` 使用 Windows 全局键鼠监听实现，可能受安全软件、远程桌面、管理员权限边界或特殊应用输入模型影响；误触场景可切回默认快捷键模式。
- 模拟复制会短暂占用剪贴板，程序会尝试恢复原文本剪贴板内容；如果原剪贴板不是文本内容，当前版本只能恢复为空文本。
- MSI 暂未签名，Windows 可能显示安全提示。
- 尚未完成自动更新和代码签名。

## 验证建议

发布前建议手动验证：

- `Alt+1` 呼出和隐藏主窗口。
- 设置中修改主快捷键后无需重启即可生效。
- 搜索并启动 3 个本机应用。
- Everything 文件搜索、打开文件、打开目录和复制路径。
- 配置导出、导入和非法 JSON 错误提示。
- 开机自启动注册表写入和删除。
- 托盘菜单的打开主窗口、打开设置、检查 Everything 状态和退出。
- 配置真实 AI 服务后的翻译、总结、解释、流式输出和取消（延后验证）。
- 选中文本后 `Ctrl+Shift+Space` 取词和剪贴板恢复（延后验证）。
