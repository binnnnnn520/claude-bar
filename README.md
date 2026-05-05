# Claude Bar

Windows tray app for local Codex and Claude token accounting.

## Scope

Claude Bar is local-only. It does not require login, OAuth, cookies, remote calls, or credentials. It does not read `%USERPROFILE%\.codex\auth.json`, and it does not render raw prompts or responses.

## Install

Download and run the Windows installer:

- [Token Ledger 0.1.2 x64 setup](installers/Token%20Ledger_0.1.2_x64-setup.exe)
- Size: 1.06 MiB
- SHA256: `5DC27665E8EAA482E05D9037F60E2F8E2477C7FDA2121EAB264724D00C86DE37`

## Local Sources

Codex:

- `%USERPROFILE%\.codex\sessions`
- `%USERPROFILE%\.codex\archived_sessions`
- `%CODEX_HOME%\sessions` when `CODEX_HOME` is set
- `%CODEX_HOME%\archived_sessions` when `CODEX_HOME` is set

Claude:

- `%USERPROFILE%\.claude\projects`
- `%USERPROFILE%\.config\claude\projects`
- `<root>\projects` for each comma-separated `CLAUDE_CONFIG_DIR` root

## Supported Windows

- Last 30 days
- Current month
- Today

## Development

```sh
npm install
npm run dev
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```
