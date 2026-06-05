# Release Process

本文档记录 Easy Launcher 公开版本的手动发布流程。当前项目优先发布 Windows x64 未签名 MSI。

## 版本号

发布前同时确认这些位置的版本号：

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

版本标签使用 `vMAJOR.MINOR.PATCH`，例如：

```text
v0.1.0
```

## 发布前检查

确认工作区干净：

```powershell
git status --short
```

确认公开分支没有内部文档、本地配置或构建产物：

```powershell
git ls-tree -r --name-only HEAD | rg "^(docs/|node_modules/|dist/|\.env|AGENTS\.md|\.ai-handoffs/)"
```

上面的命令应该没有输出。

扫描常见敏感关键词：

```powershell
rg -n "api[_-]?key|apikey|secret|token|password|sk-|Bearer|Authorization" -g "!node_modules" -g "!dist" -g "!src-tauri/target"
```

允许命中安全说明、字段名、测试假值和文档提示；不要发布真实凭据、真实 token、完整 Authorization header、本机数据库或用户隐私文本。

## 构建验证

安装依赖：

```powershell
npm ci
```

构建前端：

```powershell
npm run build
```

运行 Rust 测试：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

构建 MSI：

```powershell
npm run tauri -- build
```

MSI 默认输出位置：

```text
src-tauri\target\release\bundle\msi\
```

## 手动回归

发布前至少验证：

- `Alt+1` 呼出和隐藏主窗口。
- 搜索并启动本机应用。
- Everything 文件搜索、打开文件、打开目录和复制路径。
- 设置中修改快捷键后无需重启即可生效。
- 配置导出、导入和非法 JSON 错误提示。
- 托盘菜单：打开主窗口、打开设置、检查 Everything 状态、退出。
- 如配置了 AI 服务，验证翻译、总结、解释、流式输出和取消。
- 如验证划词菜单，确认 `Ctrl+Shift+Space` 取词和剪贴板恢复行为。

## GitHub Release

建议 Release 文案包含：

- 版本摘要。
- 下载文件名。
- Windows 版本要求。
- 未签名 MSI 的 SmartScreen 或安全提示说明。
- 已知限制。
- SHA256 校验值。

生成 SHA256：

```powershell
Get-FileHash .\src-tauri\target\release\bundle\msi\<installer>.msi -Algorithm SHA256
```

## 未签名安装包说明

v0.1.x MSI 暂未签名。用户安装时可能看到 Windows SmartScreen 或其他安全提示。Release 页面不要把该版本描述为企业级、生产稳定或已完成安全加固的安装包。

建议文案：

```text
This MSI is currently unsigned. Windows may show SmartScreen or security warnings. Only install it if you trust this repository, or build from source.
```
