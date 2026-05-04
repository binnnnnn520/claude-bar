# Local Token Ledger Design

Date: 2026-05-04

## Goal

Build a Windows tray app that shows a Claude-inspired floating popover for local AI coding token usage. The app answers one question clearly:

How many tokens did local Codex and Claude sessions use over the selected time window?

The first supported window is the last 30 days. The UI may also expose Today and This Month once the scanner supports those buckets.

## Non-Goals

- No OAuth login.
- No browser cookie import.
- No remote quota or remaining-limit calls.
- No reading or displaying chat content.
- No use of `%USERPROFILE%\.codex\auth.json` or Claude credentials.
- No multi-provider dashboard beyond Codex and Claude in the initial version.

## Product Shape

The primary surface is a Windows system tray app with a small floating popover anchored near the tray icon. The popover is the default interaction, not a large dashboard.

The approved visual direction is the Claude-inspired version in:

`ui-floating-popover-claude.html`

Key traits:

- Warm off-white canvas and paper-like panel surfaces.
- Brown-orange Claude-style accent as the primary visual color.
- Muted green for Codex, so the two providers are distinguishable without making the UI loud.
- Thin borders, soft shadows, and restrained density.
- Compact tray-panel layout with high-signal totals.

## Popover Content

The popover shows:

- Last 30 days total tokens across Codex and Claude.
- Codex total tokens.
- Claude total tokens.
- Provider share bar.
- Per-provider input, cache, and output token buckets.
- Small recent-activity sparkline per provider.
- Last scan time.
- Local-only badges: No auth and No network.
- Actions:
  - Refresh scan.
  - Pin floating window.
  - Close.
  - Open full dashboard.

The full dashboard is secondary. It can reuse the broader direction from `ui-token-ledger.html`, but the tray popover is the main MVP target.

## Data Sources

All data comes from known local JSONL logs.

Codex roots:

- `%USERPROFILE%\.codex\sessions\**\*.jsonl`
- `%USERPROFILE%\.codex\archived_sessions\*.jsonl`
- If `CODEX_HOME` is set, use `%CODEX_HOME%\sessions` and `%CODEX_HOME%\archived_sessions` instead.

Claude roots:

- `%USERPROFILE%\.claude\projects\**\*.jsonl`
- `%USERPROFILE%\.config\claude\projects\**\*.jsonl`
- If `CLAUDE_CONFIG_DIR` is set, treat each comma-separated root as a Claude config root and scan `<root>\projects`.

The scanner may later support additional local sources such as pi session logs, but those are out of scope for the first version.

## Parsing Rules

### Codex

Read JSONL line by line. Only inspect structured fields needed for usage aggregation.

Codex usage is based on:

- `event_msg` entries with token usage data, especially token-count events.
- model/context markers such as `turn_context` when present.

The parser should aggregate:

- input tokens
- cached input tokens, when available
- output tokens
- total tokens
- model bucket, when available
- timestamp bucket

If a Codex line does not contain recognized usage fields, skip it.

### Claude

Read JSONL line by line. Only inspect structured fields needed for usage aggregation.

Claude usage is based on assistant messages with:

- `type: "assistant"`
- `message.usage`

The parser should aggregate:

- input tokens
- cache read tokens
- cache creation tokens
- output tokens
- total tokens
- model bucket, when available
- timestamp bucket

If streaming chunks repeat cumulative usage for the same message, deduplicate by stable IDs such as `message.id` and request ID when available.

## Privacy Rules

The app must be able to run in a fully offline mode.

It must not:

- call OpenAI, Anthropic, Claude, or ChatGPT endpoints
- read OAuth tokens
- read browser cookies
- upload telemetry
- render raw prompts, responses, file contents, or message text

The UI should label the data source as local JSONL scan and make the privacy boundary visible.

## Window Behavior

The Windows app should behave like a macOS-style menu bar popover, implemented with Windows-native tray and a frameless floating window.

Expected behavior:

- Click tray icon to show the popover near the tray area.
- Popover is frameless, fixed-size, shadowed, and not shown as a normal taskbar window.
- Losing focus closes the popover unless it is pinned.
- `Esc` closes the popover.
- Pin action keeps the window visible and always on top.
- Dashboard action opens the larger app window.
- Refresh action runs a local rescan.

Positioning should account for:

- taskbar on bottom, top, left, or right
- multiple monitors
- hidden tray icon fallback, using cursor position or primary monitor taskbar corner

## Suggested Implementation Stack

Recommended stack:

- Tauri desktop shell
- React UI
- Rust scanner backend

Reasons:

- Tauri gives reliable Windows tray integration with a small footprint.
- React is practical for the polished UI surface.
- Rust is a good fit for fast, safe JSONL scanning and file walking.

The scanner should be a separate module from the UI so it can be tested without launching a window.

## Data Model

Core output shape:

```text
UsageSnapshot
  window
  scannedAt
  providers[]

ProviderUsage
  provider: codex | claude
  totalTokens
  inputTokens
  cacheReadTokens
  cacheCreationTokens
  cachedInputTokens
  outputTokens
  filesScanned
  filesWithUsage
  daily[]
  models[]
  errors[]
```

Codex may use `cachedInputTokens`; Claude may use `cacheReadTokens` and `cacheCreationTokens`. The UI can display these under a single Cache label while keeping the backend buckets separate.

## Error Handling

The scanner should tolerate partial failures.

Examples:

- Missing Codex logs: show Codex as not found, still show Claude.
- Missing Claude logs: show Claude as not found, still show Codex.
- Malformed JSONL line: skip the line and count it as a parse warning.
- Permission error: report which root failed, without crashing the app.
- No usage rows found: show "No local usage found" for that provider.

Errors should be shown as compact provider-level warnings, not modal dialogs.

## Refresh and Caching

Initial scan runs when the app starts.

Manual refresh is available from the popover.

Recommended caching:

- cache the most recent aggregated snapshot in app data
- store file metadata fingerprints to avoid rescanning unchanged files later
- keep a simple full rescan path for correctness

For MVP, a full local scan is acceptable if performance is good on the user's logs.

## Testing

Scanner tests:

- Codex fixture with token-count lines.
- Codex fixture with model/context markers.
- Claude fixture with assistant `message.usage`.
- Claude fixture with repeated streaming/cumulative usage, verifying deduplication.
- Missing directory.
- Malformed JSONL.
- Mixed old and recent timestamps, verifying last-30-days filtering.

UI tests:

- Popover renders total, Codex, Claude, input/cache/output.
- Empty provider state renders correctly.
- Provider warning state renders correctly.
- Refresh button triggers scanner command.
- No login, cookie, or network controls appear in the MVP UI.

Manual checks:

- Open popover from tray.
- Close on blur.
- Close on Esc.
- Pin keeps the popover visible.
- Dashboard button opens larger window.
- Popover remains readable at 100%, 125%, and 150% Windows scaling.

