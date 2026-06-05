# Contributing to Easy Launcher

感谢你愿意参与 Easy Launcher。

本项目当前是 Windows-first 的个人桌面启动器，第一阶段优先保证本机试用、可构建、可验证和安全边界清晰。提交 issue 或 PR 前，请先阅读 README 中的“已知限制”和安全说明，避免把当前明确不支持的范围误判为 bug。

## 开发环境

推荐环境：

- Windows 10 或 Windows 11。
- Node.js 18 或更新版本。
- Rust stable toolchain。
- Microsoft C++ Build Tools 2022，包含 MSVC 和 Windows SDK。
- 可选：Everything，用于验证文件搜索。
- 可选：OpenAI 兼容 API 服务，用于验证 AI 文本处理。

安装依赖：

```powershell
npm install
```

检查 Tauri 环境：

```powershell
npm run tauri -- info
```

启动桌面应用：

```powershell
npm run tauri -- dev
```

只启动 Web UI：

```powershell
npm run dev
```

## 分支和提交

建议从公开默认分支创建功能分支：

```powershell
git switch main
git pull
git switch -c fix/short-description
```

提交信息使用 Conventional Commit 风格，例如：

```text
fix(search): improve Everything fallback handling
feat(settings): add startup toggle validation
docs: clarify unsigned MSI warning
```

保持 PR 聚焦在一个行为或一组紧密相关的改动上。不要在同一个 PR 中混合功能、格式化、依赖升级和大量无关重构。

## 代码风格

前端使用 React function components、hooks 和 TypeScript。Tauri command 的输入输出类型要显式描述，TypeScript 字段使用 camelCase。

Rust 使用 edition 2021 风格：函数和模块使用 `snake_case`，结构体和枚举使用 `PascalCase`。改动共享行为、解析逻辑、搜索排序、存储、命令执行或 Windows 集成时，请优先补 Rust 单元测试。

JSON 文件保留 2 空格缩进。Rust 代码提交前运行格式化：

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
```

## 验证

常规验证命令：

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

桌面打包验证：

```powershell
npm run tauri -- build
```

如果只改文档，可以不运行完整构建，但请在 PR 中说明没有运行的原因。

涉及 UI、快捷键、托盘、Everything、AI、剪贴板或安装器的改动，PR 中请写明手动验证步骤和 Windows 版本。UI 改动请附截图或录屏。

## 安全和隐私

不要在 issue、PR、截图、日志或导出配置中提交 API Key、Authorization header、本机数据库、真实用户文件路径或敏感剪贴板内容。

当前版本的 API Key 会明文保存在本机 SQLite 数据库中。默认配置导出不会包含 API Key，但用户手动提供的日志和截图仍可能泄露敏感信息。

安全问题请优先按 `SECURITY.md` 的说明报告。

## 沟通规则

请保持讨论具体、可复现、可验证。提交 bug 时尽量提供系统版本、安装方式、复现步骤、期望结果、实际结果和相关截图。对暂不支持的平台或能力，可以提交 feature request，但不要把未支持范围包装成紧急缺陷。
