# Claude Bar

Claude Bar 是一个 轻量化仅有1.5MB的Windows 托盘应用，用来统计本机 Codex 和 Claude 的 token 使用情况。它不会登录账号，也不会请求远程额度接口，而是直接扫描你电脑上的本地 JSONL 日志，把最近一段时间的用量汇总成一个轻量的悬浮面板。

安装包内的应用名是 **Token Ledger**。

## 这个产品做什么

很多人会同时在本机使用 Codex 和 Claude Code，但 token 用量分散在不同工具的本地日志里。Claude Bar 的目标很简单：把这些本地记录汇总到一个 Windows 托盘入口里，让你快速看到 Codex 和 Claude 分别用了多少 token、主要消耗在哪里、最近的使用趋势如何。

它适合想要做本地 AI 编程用量观察的人，尤其是需要在不同工具之间对比输入、缓存、输出和模型使用情况时。

## 产品特点

- **托盘常驻，随手查看**：应用运行在 Windows 系统托盘中，打开后显示一个紧凑的悬浮面板，不需要进入大型仪表盘。
- **同时统计 Codex 和 Claude**：扫描 Codex session 日志和 Claude project 日志，合并展示两类工具的 token 使用情况。
- **多时间窗口**：支持最近 30 天、本月、今天三个统计窗口。
- **高信号用量指标**：展示总 token、输入 token、缓存 token、输出 token、Codex / Claude 占比、每日趋势、模型用量和扫描文件数量。
- **本地优先和隐私友好**：只读取本机 JSONL 日志，不读取聊天正文，不展示 prompt 或 response。
- **无需账号授权**：不需要 OAuth，不导入浏览器 Cookie，不读取 Codex `auth.json`。
- **无远程调用**：不调用 OpenAI、Anthropic、Claude 或 ChatGPT 服务，也不上传遥测数据。
- **容错扫描**：缺少某个工具日志、部分 JSONL 损坏或目录不可读时，应用会尽量展示可用数据并给出 provider 级别的提示。

## 界面信息

悬浮面板会展示：

- 当前时间窗口的总 token
- Codex 和 Claude 各自的 token 总量
- 输入、缓存、输出 token 拆分
- Codex / Claude 使用占比
- 最近每日用量趋势
- 模型维度的 token 排名
- 扫描文件数量、有效文件数量和解析警告
- 最近扫描时间

## 安装

下载并运行 Windows 安装包：

- [Token Ledger 0.1.3 x64 setup](installers/Token%20Ledger_0.1.3_x64-setup.exe)
- Size: 1.08 MiB
- SHA256: `403F481F0658474F4D17B2E567AF97E2BF1A50A0C061FCF1E0C617E084CB2AB2`

## 本地数据源

Codex：

- `%USERPROFILE%\.codex\sessions`
- `%USERPROFILE%\.codex\archived_sessions`
- `%CODEX_HOME%\sessions` when `CODEX_HOME` is set
- `%CODEX_HOME%\archived_sessions` when `CODEX_HOME` is set

Claude：

- `%USERPROFILE%\.claude\projects`
- `%USERPROFILE%\.config\claude\projects`
- `<root>\projects` for each comma-separated `CLAUDE_CONFIG_DIR` root

## 开发

```powershell
npm install
npm run dev
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```
