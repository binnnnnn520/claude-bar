# Claude Bar

Windows tray app for local Codex and Claude token accounting.

## Scope

Claude Bar is local-only. It does not require login, OAuth, cookies, remote calls, or credentials. It does not read `%USERPROFILE%\.codex\auth.json`, and it does not render raw prompts or responses.

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
