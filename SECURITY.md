# Security Policy

## 支持范围

当前安全说明适用于公开仓库中的最新 `main` 分支和已发布的 v0.1.x 版本。

Easy Launcher 目前定位为个人本机试用工具，不适合受管企业环境、共享电脑或高敏感凭据长期保存场景。

## 报告安全问题

如果你发现安全问题，请优先使用 GitHub 的 private vulnerability reporting 功能提交报告。如果仓库暂未开启该功能，请创建 issue，并只描述影响范围和复现条件，不要公开 API Key、真实 token、本机数据库、完整 Authorization header、敏感文件路径或用户文本内容。

报告中建议包含：

- 受影响版本或提交。
- Windows 版本和安装方式。
- 是否启用 Everything、AI、划词菜单或剪贴板相关功能。
- 最小复现步骤。
- 你认为可能泄露、破坏或误用的数据类型。

本项目目前没有漏洞赏金计划。

## 当前安全边界

### API Key

v0.1.x 没有接入 Windows Credential Manager。

用户配置的 AI API Key 会明文保存在本机 SQLite 数据库中：

```text
%LocalAppData%\EasyLauncher\data.db
```

默认配置导出不会包含 `ai.api_key`，但不要在共享电脑、受管设备或不可信环境中保存重要 API Key。

### AI 请求

AI 功能使用用户配置的 OpenAI 兼容 Chat Completions 服务。翻译、总结和解释等动作会把用户选择或输入的文本发送到该服务。项目不附带 API Key，也不代理用户请求到项目维护者的服务器。

### 本机数据

运行时数据目录：

```text
%LocalAppData%\EasyLauncher\
```

SQLite 数据库会保存设置、最近使用记录、应用索引和 AI 配置。配置导出默认写入：

```text
%LocalAppData%\EasyLauncher\exports\
```

### Everything 搜索

Everything 文件搜索依赖用户本机安装的 Everything。Easy Launcher 会优先使用 IPC，并可在 IPC 不可用时访问本机 Everything HTTP Server，默认地址为 `127.0.0.1:8080`。文件索引不会上传到项目维护者服务器。

### 划词和剪贴板

划词取词会临时模拟 `Ctrl+C`，等待剪贴板文本变化后尝试恢复原文本剪贴板。当前版本不能完整恢复非文本剪贴板内容。

如果启用 `Ctrl+划词` 模式，应用会在 Windows 后台监听全局键鼠事件，用于判断用户是否按住 `Ctrl` 并完成拖选。该模式可能受安全软件、远程桌面、管理员权限边界或特殊应用输入模型影响。

剪贴板历史没有敏感内容过滤。不要把它用于处理密码、私钥、生产凭据或其他高敏感内容。

### Windows 安装器

v0.1.x MSI 暂未签名。Windows 可能显示 SmartScreen 或安全提示。请只从项目 Release 页面或可信的本地源码构建获取安装包。
