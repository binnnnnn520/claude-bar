# Local Token Ledger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows tray app that shows a Claude-inspired floating popover with local Codex and Claude token usage for the last 30 days.

**Architecture:** Use a Tauri desktop shell with a React frontend and a Rust backend scanner. The Rust scanner reads only local JSONL logs, aggregates token usage, and exposes snapshots through Tauri commands. The React UI renders the approved Claude-style tray popover and a simple dashboard view from the snapshot.

**Tech Stack:** Tauri 2, React 18, TypeScript, Vite, Rust, serde_json, chrono, walkdir.

---

## File Structure

Create these files:

- `.gitignore` - ignores build outputs, Node modules, Rust target, and local brainstorm sessions.
- `package.json` - npm scripts and frontend/Tauri dependencies.
- `index.html` - Vite entry.
- `tsconfig.json` - TypeScript settings.
- `vite.config.ts` - Vite dev server config for Tauri.
- `src/main.tsx` - React entry.
- `src/App.tsx` - top-level app state and Tauri command wiring.
- `src/types.ts` - frontend snapshot types matching Rust output.
- `src/components/FloatingPopover.tsx` - approved popover UI.
- `src/components/Dashboard.tsx` - secondary larger dashboard.
- `src/styles.css` - Claude-inspired UI styles, adapted from `ui-floating-popover-claude.html`.
- `src-tauri/Cargo.toml` - Rust crate dependencies.
- `src-tauri/tauri.conf.json` - Tauri app config.
- `src-tauri/src/main.rs` - Tauri entry.
- `src-tauri/src/lib.rs` - app setup and command registration.
- `src-tauri/src/scanner/mod.rs` - scanner module exports and `scan_usage`.
- `src-tauri/src/scanner/types.rs` - Rust data model serialized to the frontend.
- `src-tauri/src/scanner/roots.rs` - local log root discovery.
- `src-tauri/src/scanner/time.rs` - timestamp parsing and last-30-days filtering.
- `src-tauri/src/scanner/claude.rs` - Claude JSONL parser.
- `src-tauri/src/scanner/codex.rs` - Codex JSONL parser.
- `src-tauri/src/tray.rs` - tray icon, popover show/hide, and pin behavior.
- `src-tauri/src/commands.rs` - Tauri command functions.
- `README.md` - local-only product and dev instructions.

Modify these files after creation:

- `docs/superpowers/specs/2026-05-04-local-token-ledger-design.md` - add implementation status after MVP is complete.

---

### Task 1: Scaffold the Tauri React App

**Files:**
- Create: `.gitignore`
- Create: `package.json`
- Create: `index.html`
- Create: `tsconfig.json`
- Create: `vite.config.ts`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/types.ts`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `.gitignore`**

```gitignore
node_modules/
dist/
src-tauri/target/
src-tauri/gen/
.env
.env.local
.superpowers/brainstorm/*/state/
*.log
```

- [ ] **Step 2: Create `package.json`**

```json
{
  "name": "claude-bar",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "tauri dev",
    "build": "tauri build",
    "dev:ui": "vite --host 127.0.0.1 --port 1420",
    "build:ui": "tsc && vite build",
    "test": "cargo test --manifest-path src-tauri/Cargo.toml",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@types/react": "^18.3.18",
    "@types/react-dom": "^18.3.5",
    "@vitejs/plugin-react": "^4.3.4",
    "typescript": "^5.7.3",
    "vite": "^6.0.7"
  }
}
```

- [ ] **Step 3: Create `index.html`**

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Token Ledger</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 4: Create `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2020"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"],
  "references": []
}
```

- [ ] **Step 5: Create `vite.config.ts`**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1"
  },
  envPrefix: ["VITE_", "TAURI_"]
});
```

- [ ] **Step 6: Create initial React entry files**

`src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

`src/types.ts`:

```ts
export type ProviderId = "codex" | "claude";

export interface TokenBucket {
  inputTokens: number;
  cachedInputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  outputTokens: number;
  totalTokens: number;
}

export interface DailyUsage extends TokenBucket {
  date: string;
}

export interface ModelUsage extends TokenBucket {
  model: string;
}

export interface ProviderUsage extends TokenBucket {
  provider: ProviderId;
  filesScanned: number;
  filesWithUsage: number;
  parseWarnings: number;
  daily: DailyUsage[];
  models: ModelUsage[];
  errors: string[];
}

export interface UsageSnapshot {
  window: "last30d" | "month" | "today";
  scannedAt: string;
  providers: ProviderUsage[];
}
```

`src/App.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FloatingPopover } from "./components/FloatingPopover";
import { Dashboard } from "./components/Dashboard";
import type { UsageSnapshot } from "./types";

const emptySnapshot: UsageSnapshot = {
  window: "last30d",
  scannedAt: new Date().toISOString(),
  providers: []
};

export default function App() {
  const [snapshot, setSnapshot] = useState<UsageSnapshot>(emptySnapshot);
  const [view, setView] = useState<"popover" | "dashboard">("popover");
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setIsLoading(true);
    setError(null);
    try {
      const next = await invoke<UsageSnapshot>("scan_usage", { window: "last30d" });
      setSnapshot(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  const content = useMemo(() => {
    if (view === "dashboard") {
      return <Dashboard snapshot={snapshot} onRefresh={refresh} onBack={() => setView("popover")} isLoading={isLoading} error={error} />;
    }
    return <FloatingPopover snapshot={snapshot} onRefresh={refresh} onOpenDashboard={() => setView("dashboard")} isLoading={isLoading} error={error} />;
  }, [view, snapshot, isLoading, error]);

  return content;
}
```

- [ ] **Step 7: Create `src-tauri/Cargo.toml`**

```toml
[package]
name = "claude-bar"
version = "0.1.0"
description = "Local Codex and Claude token ledger"
authors = ["binnnnnn520"]
edition = "2021"

[lib]
name = "claude_bar_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "claude-bar"
path = "src/main.rs"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
thiserror = "2"
walkdir = "2"
```

- [ ] **Step 8: Create `src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Token Ledger",
  "version": "0.1.0",
  "identifier": "dev.local.tokenledger",
  "build": {
    "beforeDevCommand": "npm run dev:ui",
    "devUrl": "http://127.0.0.1:1420",
    "beforeBuildCommand": "npm run build:ui",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Token Ledger",
        "width": 386,
        "height": 520,
        "resizable": false,
        "fullscreen": false,
        "visible": false,
        "decorations": false,
        "transparent": true,
        "skipTaskbar": true,
        "alwaysOnTop": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

- [ ] **Step 9: Create Rust entry files**

`src-tauri/src/main.rs`:

```rust
fn main() {
    claude_bar_lib::run();
}
```

`src-tauri/src/lib.rs`:

```rust
mod commands;
mod scanner;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::scan_usage])
        .setup(|app| {
            tray::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Token Ledger");
}
```

- [ ] **Step 10: Run install/build bootstrap**

Run:

```powershell
npm install
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected:

- `npm install` installs dependencies.
- `npm run typecheck` fails until components and styles are created.
- `cargo test` fails until scanner and command modules are created.

- [ ] **Step 11: Commit scaffold**

```powershell
git add .gitignore package.json index.html tsconfig.json vite.config.ts src src-tauri
git commit -m "Scaffold Tauri React token ledger app"
```

---

### Task 2: Add Scanner Types, Root Discovery, and Time Helpers

**Files:**
- Create: `src-tauri/src/scanner/types.rs`
- Create: `src-tauri/src/scanner/roots.rs`
- Create: `src-tauri/src/scanner/time.rs`
- Create: `src-tauri/src/scanner/mod.rs`

- [ ] **Step 1: Write failing unit tests for root discovery and token math**

Add this to `src-tauri/src/scanner/mod.rs`:

```rust
pub mod roots;
pub mod time;
pub mod types;

#[cfg(test)]
mod tests {
    use super::types::{ProviderId, ProviderUsage, TokenBucket};

    #[test]
    fn token_bucket_total_includes_input_cache_and_output() {
        let bucket = TokenBucket {
            input_tokens: 10,
            cached_input_tokens: 3,
            cache_read_tokens: 4,
            cache_creation_tokens: 5,
            output_tokens: 7,
            total_tokens: 0,
        }
        .with_total();

        assert_eq!(bucket.total_tokens, 29);
    }

    #[test]
    fn provider_usage_adds_buckets() {
        let mut usage = ProviderUsage::empty(ProviderId::Codex);
        usage.add_bucket(TokenBucket {
            input_tokens: 10,
            cached_input_tokens: 2,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            output_tokens: 5,
            total_tokens: 17,
        });

        assert_eq!(usage.total_tokens, 17);
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.cached_input_tokens, 2);
        assert_eq!(usage.output_tokens, 5);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml scanner::tests -- --nocapture
```

Expected:

- FAIL because `ProviderId`, `ProviderUsage`, and `TokenBucket` do not exist.

- [ ] **Step 3: Implement `src-tauri/src/scanner/types.rs`**

```rust
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Codex,
    Claude,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenBucket {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

impl TokenBucket {
    pub fn with_total(mut self) -> Self {
        self.total_tokens = self.input_tokens
            + self.cached_input_tokens
            + self.cache_read_tokens
            + self.cache_creation_tokens
            + self.output_tokens;
        self
    }

    pub fn add(&mut self, other: &TokenBucket) {
        self.input_tokens += other.input_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
        self.output_tokens += other.output_tokens;
        self.total_tokens += other.total_tokens;
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    #[serde(flatten)]
    pub bucket: TokenBucket,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    #[serde(flatten)]
    pub bucket: TokenBucket,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub provider: ProviderId,
    #[serde(flatten)]
    pub bucket: TokenBucket,
    pub files_scanned: u64,
    pub files_with_usage: u64,
    pub parse_warnings: u64,
    pub daily: Vec<DailyUsage>,
    pub models: Vec<ModelUsage>,
    pub errors: Vec<String>,
    #[serde(skip)]
    pub daily_buckets: BTreeMap<String, TokenBucket>,
    #[serde(skip)]
    pub model_buckets: BTreeMap<String, TokenBucket>,
}

impl ProviderUsage {
    pub fn empty(provider: ProviderId) -> Self {
        Self {
            provider,
            bucket: TokenBucket::default(),
            files_scanned: 0,
            files_with_usage: 0,
            parse_warnings: 0,
            daily: Vec::new(),
            models: Vec::new(),
            errors: Vec::new(),
            daily_buckets: BTreeMap::new(),
            model_buckets: BTreeMap::new(),
        }
    }

    pub fn add_bucket(&mut self, bucket: TokenBucket) {
        self.bucket.add(&bucket);
    }

    pub fn add_daily_bucket(&mut self, date: String, bucket: TokenBucket) {
        self.add_bucket(bucket.clone());
        self.daily_buckets.entry(date).or_default().add(&bucket);
    }

    pub fn add_model_bucket(&mut self, model: String, bucket: TokenBucket) {
        self.model_buckets.entry(model).or_default().add(&bucket);
    }

    pub fn finalize(mut self) -> Self {
        self.daily = self
            .daily_buckets
            .iter()
            .map(|(date, bucket)| DailyUsage {
                date: date.clone(),
                bucket: bucket.clone(),
            })
            .collect();
        self.models = self
            .model_buckets
            .iter()
            .map(|(model, bucket)| ModelUsage {
                model: model.clone(),
                bucket: bucket.clone(),
            })
            .collect();
        self.models.sort_by(|a, b| b.bucket.total_tokens.cmp(&a.bucket.total_tokens));
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub window: String,
    pub scanned_at: String,
    pub providers: Vec<ProviderUsage>,
}
```

- [ ] **Step 4: Implement `src-tauri/src/scanner/roots.rs`**

```rust
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoots {
    pub codex_roots: Vec<PathBuf>,
    pub claude_roots: Vec<PathBuf>,
}

pub fn discover_roots() -> ScanRoots {
    ScanRoots {
        codex_roots: codex_roots(),
        claude_roots: claude_roots(),
    }
}

fn user_home() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
}

fn codex_roots() -> Vec<PathBuf> {
    if let Some(home) = env::var_os("CODEX_HOME").map(PathBuf::from) {
        return vec![home.join("sessions"), home.join("archived_sessions")];
    }

    user_home()
        .map(|home| {
            vec![
                home.join(".codex").join("sessions"),
                home.join(".codex").join("archived_sessions"),
            ]
        })
        .unwrap_or_default()
}

fn claude_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(config) = env::var_os("CLAUDE_CONFIG_DIR") {
        for raw in config.to_string_lossy().split(',') {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                roots.push(PathBuf::from(trimmed).join("projects"));
            }
        }
    }

    if let Some(home) = user_home() {
        roots.push(home.join(".claude").join("projects"));
        roots.push(home.join(".config").join("claude").join("projects"));
    }

    roots
}
```

- [ ] **Step 5: Implement `src-tauri/src/scanner/time.rs`**

```rust
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde_json::Value;
use std::path::Path;

pub fn cutoff_last_30_days(now: DateTime<Utc>) -> DateTime<Utc> {
    now - Duration::days(30)
}

pub fn parse_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    for key in ["timestamp", "ts", "time", "created_at", "createdAt"] {
        if let Some(raw) = value.get(key).and_then(Value::as_str) {
            if let Ok(ts) = DateTime::parse_from_rfc3339(raw) {
                return Some(ts.with_timezone(&Utc));
            }
        }
    }
    None
}

pub fn date_key(ts: DateTime<Utc>) -> String {
    ts.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

pub fn codex_date_from_path(path: &Path) -> Option<String> {
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect();

    for window in parts.windows(3) {
        let year = &window[0];
        let month = &window[1];
        let day = &window[2];
        if year.len() == 4
            && month.len() == 2
            && day.len() == 2
            && year.chars().all(|c| c.is_ascii_digit())
            && month.chars().all(|c| c.is_ascii_digit())
            && day.chars().all(|c| c.is_ascii_digit())
        {
            let key = format!("{year}-{month}-{day}");
            if NaiveDate::parse_from_str(&key, "%Y-%m-%d").is_ok() {
                return Some(key);
            }
        }
    }

    None
}
```

- [ ] **Step 6: Run tests to verify pass**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml scanner::tests -- --nocapture
```

Expected:

- PASS for token bucket math tests.

- [ ] **Step 7: Commit scanner foundations**

```powershell
git add src-tauri/src/scanner
git commit -m "Add token scanner foundations"
```

---

### Task 3: Implement Claude JSONL Scanner

**Files:**
- Create: `src-tauri/src/scanner/claude.rs`
- Modify: `src-tauri/src/scanner/mod.rs`

- [ ] **Step 1: Write failing Claude parser tests**

Append to `src-tauri/src/scanner/mod.rs` test module:

```rust
#[test]
fn claude_parser_extracts_assistant_usage() {
    let line = r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":100,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":40}}}"#;
    let parsed = super::claude::parse_claude_line(line).expect("usage should parse");

    assert_eq!(parsed.bucket.input_tokens, 100);
    assert_eq!(parsed.bucket.cache_creation_tokens, 20);
    assert_eq!(parsed.bucket.cache_read_tokens, 30);
    assert_eq!(parsed.bucket.output_tokens, 40);
    assert_eq!(parsed.bucket.total_tokens, 190);
    assert_eq!(parsed.model.as_deref(), Some("claude-sonnet"));
    assert_eq!(parsed.dedupe_key.as_deref(), Some("msg_1"));
}

#[test]
fn claude_parser_skips_non_assistant_rows() {
    let line = r#"{"type":"user","timestamp":"2026-05-01T12:00:00Z","message":{"content":"hidden"}}"#;
    assert!(super::claude::parse_claude_line(line).is_none());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml claude_parser -- --nocapture
```

Expected:

- FAIL because `scanner::claude` does not exist.

- [ ] **Step 3: Implement `src-tauri/src/scanner/claude.rs`**

```rust
use super::time::parse_timestamp;
use super::types::TokenBucket;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ParsedUsage {
    pub timestamp: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub dedupe_key: Option<String>,
    pub bucket: TokenBucket,
}

pub fn parse_claude_line(line: &str) -> Option<ParsedUsage> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }

    let message = value.get("message")?;
    let usage = message.get("usage")?;

    let bucket = TokenBucket {
        input_tokens: usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
        cached_input_tokens: 0,
        cache_read_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_creation_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
        total_tokens: 0,
    }
    .with_total();

    if bucket.total_tokens == 0 {
        return None;
    }

    Some(ParsedUsage {
        timestamp: parse_timestamp(&value),
        model: message.get("model").and_then(Value::as_str).map(str::to_owned),
        dedupe_key: message
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| value.get("requestId").and_then(Value::as_str))
            .map(str::to_owned),
        bucket,
    })
}
```

- [ ] **Step 4: Export Claude module**

Modify `src-tauri/src/scanner/mod.rs` module list:

```rust
pub mod claude;
pub mod roots;
pub mod time;
pub mod types;
```

- [ ] **Step 5: Run tests to verify pass**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml claude_parser -- --nocapture
```

Expected:

- PASS.

- [ ] **Step 6: Commit Claude parser**

```powershell
git add src-tauri/src/scanner/claude.rs src-tauri/src/scanner/mod.rs
git commit -m "Parse local Claude usage logs"
```

---

### Task 4: Implement Codex JSONL Scanner

**Files:**
- Create: `src-tauri/src/scanner/codex.rs`
- Modify: `src-tauri/src/scanner/mod.rs`

- [ ] **Step 1: Write failing Codex parser tests**

Append to `src-tauri/src/scanner/mod.rs` test module:

```rust
#[test]
fn codex_parser_extracts_payload_token_count() {
    let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"event_msg","payload":{"type":"token_count","input_tokens":100,"cached_input_tokens":25,"output_tokens":40,"model":"gpt-5.3-codex"}}"#;
    let parsed = super::codex::parse_codex_line(line).expect("usage should parse");

    assert_eq!(parsed.bucket.input_tokens, 100);
    assert_eq!(parsed.bucket.cached_input_tokens, 25);
    assert_eq!(parsed.bucket.output_tokens, 40);
    assert_eq!(parsed.bucket.total_tokens, 165);
    assert_eq!(parsed.model.as_deref(), Some("gpt-5.3-codex"));
}

#[test]
fn codex_parser_skips_non_usage_rows() {
    let line = r#"{"timestamp":"2026-05-01T12:00:00Z","type":"response_item","payload":{"type":"message","content":"hidden"}}"#;
    assert!(super::codex::parse_codex_line(line).is_none());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml codex_parser -- --nocapture
```

Expected:

- FAIL because `scanner::codex` does not exist.

- [ ] **Step 3: Implement `src-tauri/src/scanner/codex.rs`**

```rust
use super::time::parse_timestamp;
use super::types::TokenBucket;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ParsedUsage {
    pub timestamp: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub bucket: TokenBucket,
}

pub fn parse_codex_line(line: &str) -> Option<ParsedUsage> {
    let value: Value = serde_json::from_str(line).ok()?;
    let payload = value.get("payload").or_else(|| value.get("msg"))?;
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str));

    let looks_like_token_count = payload_type == Some("token_count")
        || payload
            .get("event")
            .and_then(Value::as_str)
            .is_some_and(|event| event.contains("token_count"));

    if !looks_like_token_count {
        return None;
    }

    let usage = payload.get("usage").unwrap_or(payload);
    let bucket = TokenBucket {
        input_tokens: number_at_any(usage, &["input_tokens", "inputTokens", "prompt_tokens", "promptTokens"]),
        cached_input_tokens: number_at_any(
            usage,
            &["cached_input_tokens", "cachedInputTokens", "cache_read_input_tokens", "cacheReadInputTokens"],
        ),
        cache_read_tokens: 0,
        cache_creation_tokens: number_at_any(
            usage,
            &["cache_creation_input_tokens", "cacheCreationInputTokens"],
        ),
        output_tokens: number_at_any(
            usage,
            &["output_tokens", "outputTokens", "completion_tokens", "completionTokens"],
        ),
        total_tokens: number_at_any(usage, &["total_tokens", "totalTokens"]),
    };

    let bucket = if bucket.total_tokens == 0 {
        bucket.with_total()
    } else {
        bucket
    };

    if bucket.total_tokens == 0 {
        return None;
    }

    Some(ParsedUsage {
        timestamp: parse_timestamp(&value),
        model: string_at_any(payload, &["model", "model_slug", "modelSlug"])
            .or_else(|| string_at_any(usage, &["model", "model_slug", "modelSlug"])),
        bucket,
    })
}

fn number_at_any(value: &Value, keys: &[&str]) -> u64 {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(Value::as_u64) {
            return number;
        }
    }
    0
}

fn string_at_any(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            return Some(text.to_owned());
        }
    }
    None
}
```

- [ ] **Step 4: Export Codex module**

Modify `src-tauri/src/scanner/mod.rs` module list:

```rust
pub mod claude;
pub mod codex;
pub mod roots;
pub mod time;
pub mod types;
```

- [ ] **Step 5: Run tests to verify pass**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml codex_parser -- --nocapture
```

Expected:

- PASS.

- [ ] **Step 6: Commit Codex parser**

```powershell
git add src-tauri/src/scanner/codex.rs src-tauri/src/scanner/mod.rs
git commit -m "Parse local Codex usage logs"
```

---

### Task 5: Aggregate Scanner Results Across Local Files

**Files:**
- Modify: `src-tauri/src/scanner/mod.rs`
- Modify: `src-tauri/src/scanner/types.rs`

- [ ] **Step 1: Write failing integration-style unit test**

Append to `src-tauri/src/scanner/mod.rs` test module:

```rust
#[test]
fn scan_lines_aggregates_provider_usage() {
    let lines = vec![
        r#"{"type":"assistant","timestamp":"2026-05-01T12:00:00Z","message":{"id":"msg_1","model":"claude-sonnet","usage":{"input_tokens":10,"output_tokens":5}}}"#.to_string(),
        r#"{"type":"assistant","timestamp":"2026-05-01T12:01:00Z","message":{"id":"msg_2","model":"claude-sonnet","usage":{"input_tokens":20,"output_tokens":7}}}"#.to_string(),
    ];

    let usage = super::scan_claude_lines_for_test(lines);
    assert_eq!(usage.files_scanned, 1);
    assert_eq!(usage.files_with_usage, 1);
    assert_eq!(usage.bucket.total_tokens, 42);
    assert_eq!(usage.daily.len(), 1);
    assert_eq!(usage.models[0].model, "claude-sonnet");
    assert_eq!(usage.models[0].bucket.total_tokens, 42);
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml scan_lines_aggregates_provider_usage -- --nocapture
```

Expected:

- FAIL because `scan_claude_lines_for_test` does not exist.

- [ ] **Step 3: Implement scanner aggregation in `src-tauri/src/scanner/mod.rs`**

```rust
pub mod claude;
pub mod codex;
pub mod roots;
pub mod time;
pub mod types;

use chrono::Utc;
use roots::discover_roots;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use time::{codex_date_from_path, cutoff_last_30_days, date_key};
use types::{ProviderId, ProviderUsage, UsageSnapshot};
use walkdir::WalkDir;

pub fn scan_usage(window: &str) -> UsageSnapshot {
    let now = Utc::now();
    let cutoff = cutoff_last_30_days(now);
    let roots = discover_roots();

    let codex = scan_codex_roots(&roots.codex_roots, cutoff).finalize();
    let claude = scan_claude_roots(&roots.claude_roots, cutoff).finalize();

    UsageSnapshot {
        window: window.to_owned(),
        scanned_at: now.to_rfc3339(),
        providers: vec![codex, claude],
    }
}

fn jsonl_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path.to_path_buf());
            }
        }
    }
    files
}

fn scan_codex_roots(roots: &[PathBuf], cutoff: chrono::DateTime<Utc>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Codex);
    for path in jsonl_files(roots) {
        usage.files_scanned += 1;
        scan_codex_file(&path, cutoff, &mut usage);
    }
    usage
}

fn scan_claude_roots(roots: &[PathBuf], cutoff: chrono::DateTime<Utc>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Claude);
    for path in jsonl_files(roots) {
        usage.files_scanned += 1;
        scan_claude_file(&path, cutoff, &mut usage);
    }
    usage
}

fn scan_codex_file(path: &Path, cutoff: chrono::DateTime<Utc>, usage: &mut ProviderUsage) {
    let Ok(file) = File::open(path) else {
        usage.errors.push(format!("Cannot read {}", path.display()));
        return;
    };
    let mut found_usage = false;
    let fallback_date = codex_date_from_path(path);

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Some(parsed) = codex::parse_codex_line(&line) else {
            continue;
        };
        if parsed.timestamp.is_some_and(|ts| ts < cutoff) {
            continue;
        }
        let date = parsed
            .timestamp
            .map(date_key)
            .or_else(|| fallback_date.clone())
            .unwrap_or_else(|| "unknown".to_string());
        usage.add_daily_bucket(date, parsed.bucket.clone());
        if let Some(model) = parsed.model {
            usage.add_model_bucket(model, parsed.bucket);
        }
        found_usage = true;
    }

    if found_usage {
        usage.files_with_usage += 1;
    }
}

fn scan_claude_file(path: &Path, cutoff: chrono::DateTime<Utc>, usage: &mut ProviderUsage) {
    let Ok(file) = File::open(path) else {
        usage.errors.push(format!("Cannot read {}", path.display()));
        return;
    };

    let mut found_usage = false;
    let mut seen = HashSet::new();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Some(parsed) = claude::parse_claude_line(&line) else {
            continue;
        };
        if let Some(key) = &parsed.dedupe_key {
            if !seen.insert(key.clone()) {
                continue;
            }
        }
        if parsed.timestamp.is_some_and(|ts| ts < cutoff) {
            continue;
        }
        let date = parsed
            .timestamp
            .map(date_key)
            .unwrap_or_else(|| "unknown".to_string());
        usage.add_daily_bucket(date, parsed.bucket.clone());
        if let Some(model) = parsed.model {
            usage.add_model_bucket(model, parsed.bucket);
        }
        found_usage = true;
    }

    if found_usage {
        usage.files_with_usage += 1;
    }
}

#[cfg(test)]
pub fn scan_claude_lines_for_test(lines: Vec<String>) -> ProviderUsage {
    let mut usage = ProviderUsage::empty(ProviderId::Claude);
    usage.files_scanned = 1;
    let mut seen = HashSet::new();

    for line in lines {
        let Some(parsed) = claude::parse_claude_line(&line) else {
            continue;
        };
        if let Some(key) = &parsed.dedupe_key {
            if !seen.insert(key.clone()) {
                continue;
            }
        }
        let date = parsed
            .timestamp
            .map(date_key)
            .unwrap_or_else(|| "unknown".to_string());
        usage.add_daily_bucket(date, parsed.bucket.clone());
        if let Some(model) = parsed.model {
            usage.add_model_bucket(model, parsed.bucket);
        }
    }

    if usage.bucket.total_tokens > 0 {
        usage.files_with_usage = 1;
    }

    usage.finalize()
}
```

- [ ] **Step 4: Remove unused imports if compiler reports them**

If `serde_json::Value` is unused, remove this line:

```rust
use serde_json::Value;
```

- [ ] **Step 5: Run scanner tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml scanner -- --nocapture
```

Expected:

- PASS.

- [ ] **Step 6: Commit scanner aggregation**

```powershell
git add src-tauri/src/scanner
git commit -m "Aggregate local token usage snapshots"
```

---

### Task 6: Expose Scanner Through Tauri Commands

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create command implementation**

`src-tauri/src/commands.rs`:

```rust
use crate::scanner;
use crate::scanner::types::UsageSnapshot;

#[tauri::command]
pub async fn scan_usage(window: String) -> Result<UsageSnapshot, String> {
    let normalized = match window.as_str() {
        "last30d" | "month" | "today" => window,
        other => return Err(format!("Unsupported usage window: {other}")),
    };

    tauri::async_runtime::spawn_blocking(move || scanner::scan_usage(&normalized))
        .await
        .map_err(|err| err.to_string())
}
```

- [ ] **Step 2: Confirm `src-tauri/src/lib.rs` command registration**

`src-tauri/src/lib.rs` should contain:

```rust
mod commands;
mod scanner;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::scan_usage])
        .setup(|app| {
            tray::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Token Ledger");
}
```

- [ ] **Step 3: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected:

- PASS.

- [ ] **Step 4: Commit Tauri command**

```powershell
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "Expose local usage scanner command"
```

---

### Task 7: Implement React Popover and Dashboard UI

**Files:**
- Create: `src/components/FloatingPopover.tsx`
- Create: `src/components/Dashboard.tsx`
- Create: `src/styles.css`
- Modify: `src/App.tsx`

- [ ] **Step 1: Create `src/components/FloatingPopover.tsx`**

```tsx
import type { ProviderUsage, UsageSnapshot } from "../types";

interface FloatingPopoverProps {
  snapshot: UsageSnapshot;
  onRefresh: () => void;
  onOpenDashboard: () => void;
  isLoading: boolean;
  error: string | null;
}

function fmt(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${Math.round(value / 1_000)}k`;
  return String(value);
}

function provider(snapshot: UsageSnapshot, id: "codex" | "claude"): ProviderUsage | undefined {
  return snapshot.providers.find((item) => item.provider === id);
}

function total(snapshot: UsageSnapshot): number {
  return snapshot.providers.reduce((sum, item) => sum + item.totalTokens, 0);
}

function ProviderCard({ item, tone }: { item: ProviderUsage | undefined; tone: "codex" | "claude" }) {
  const empty = item === undefined;
  const label = tone === "codex" ? "Codex" : "Claude";
  const classes = `dot ${tone === "claude" ? "claude" : ""}`;

  return (
    <article className="provider">
      <div className="provider-line">
        <div className="provider-name">
          <span className={classes} />
          {label}
        </div>
        <div className={`spark ${tone}`} aria-label={`${label} daily sparkline`}>
          {Array.from({ length: 12 }).map((_, index) => (
            <i key={index} />
          ))}
        </div>
        <div className="provider-total">{empty ? "0" : fmt(item.totalTokens)}</div>
      </div>
      <div className="details">
        <div className="detail">
          <span className="label">Input</span>
          <strong>{empty ? "0" : fmt(item.inputTokens)}</strong>
        </div>
        <div className="detail">
          <span className="label">Cache</span>
          <strong>{empty ? "0" : fmt(item.cachedInputTokens + item.cacheReadTokens + item.cacheCreationTokens)}</strong>
        </div>
        <div className="detail">
          <span className="label">Output</span>
          <strong>{empty ? "0" : fmt(item.outputTokens)}</strong>
        </div>
      </div>
      {item?.errors.length ? <p className="provider-warning">{item.errors[0]}</p> : null}
    </article>
  );
}

export function FloatingPopover({ snapshot, onRefresh, onOpenDashboard, isLoading, error }: FloatingPopoverProps) {
  const codex = provider(snapshot, "codex");
  const claude = provider(snapshot, "claude");
  const totalTokens = total(snapshot);
  const codexShare = totalTokens > 0 && codex ? Math.round((codex.totalTokens / totalTokens) * 100) : 0;
  const claudeShare = totalTokens > 0 && claude ? 100 - codexShare : 0;

  return (
    <main className="stage app-stage">
      <section className="popover app-popover" aria-label="Local token popover">
        <header className="popover-head">
          <div className="title-group">
            <div className="glyph">
              <span className="glyph-mark" />
            </div>
            <div className="app-title">
              <strong>Token Ledger</strong>
              <span>local JSONL scan · {isLoading ? "scanning" : "ready"}</span>
            </div>
          </div>
          <div className="icon-actions">
            <button className="icon-button" title="重新扫描" type="button" onClick={onRefresh}>
              <span className="ico refresh" />
            </button>
            <button className="icon-button" title="钉住窗口" type="button">
              <span className="ico pin" />
            </button>
            <button className="icon-button" title="关闭" type="button">
              <span className="ico close" />
            </button>
          </div>
        </header>

        <div className="body">
          <div className="total-block">
            <div>
              <div className="label">最近 30 天总 token</div>
              <div className="total">{fmt(totalTokens)}</div>
            </div>
            <div className="local-badge">No auth</div>
          </div>

          <div className="stack" aria-label="Codex and Claude share">
            <span className="codex" style={{ width: `${codexShare}%` }} />
            <span className="claude" style={{ width: `${claudeShare}%` }} />
          </div>

          {error ? <div className="error-box">{error}</div> : null}

          <div className="providers">
            <ProviderCard item={codex} tone="codex" />
            <ProviderCard item={claude} tone="claude" />
          </div>
        </div>

        <footer className="footer">
          <button className="dashboard-button" type="button" onClick={onOpenDashboard}>
            Dashboard <span className="arrow" />
          </button>
          <div className="path-status">%USERPROFILE%\.codex + %USERPROFILE%\.claude</div>
          <span className="local-badge">No network</span>
        </footer>
      </section>
    </main>
  );
}
```

- [ ] **Step 2: Create `src/components/Dashboard.tsx`**

```tsx
import type { UsageSnapshot } from "../types";

interface DashboardProps {
  snapshot: UsageSnapshot;
  onRefresh: () => void;
  onBack: () => void;
  isLoading: boolean;
  error: string | null;
}

function fmt(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${Math.round(value / 1_000)}k`;
  return String(value);
}

export function Dashboard({ snapshot, onRefresh, onBack, isLoading, error }: DashboardProps) {
  const total = snapshot.providers.reduce((sum, item) => sum + item.totalTokens, 0);

  return (
    <main className="dashboard-shell">
      <header className="dashboard-head">
        <div>
          <div className="eyebrow">local-only token accounting</div>
          <h1>Codex / Claude 本地 Token 总账</h1>
        </div>
        <div className="dashboard-actions">
          <button className="secondary-button" type="button" onClick={onBack}>返回悬浮窗</button>
          <button className="dashboard-button" type="button" onClick={onRefresh}>{isLoading ? "扫描中" : "重新扫描"}</button>
        </div>
      </header>
      {error ? <div className="error-box">{error}</div> : null}
      <section className="dashboard-card">
        <div className="label">最近 30 天总 token</div>
        <div className="dashboard-total">{fmt(total)}</div>
      </section>
      <section className="dashboard-grid">
        {snapshot.providers.map((provider) => (
          <article className="dashboard-card" key={provider.provider}>
            <h2>{provider.provider === "codex" ? "Codex" : "Claude"}</h2>
            <p>{fmt(provider.totalTokens)} tokens</p>
            <p>{provider.filesScanned} files scanned · {provider.filesWithUsage} with usage</p>
          </article>
        ))}
      </section>
    </main>
  );
}
```

- [ ] **Step 3: Create `src/styles.css`**

Copy the visual system from `ui-floating-popover-claude.html` into `src/styles.css`, then apply these app-specific edits:

```css
html,
body,
#root {
  margin: 0;
  min-height: 100vh;
}

.app-stage {
  min-height: 100vh;
  padding: 0;
  background: transparent;
}

.app-popover {
  position: relative;
  right: auto;
  bottom: auto;
  width: 386px;
  margin: 0;
}

.app-popover::after {
  display: none;
}

.error-box {
  border: 1px solid rgba(197, 111, 63, .32);
  background: rgba(197, 111, 63, .10);
  color: var(--claude-deep);
  border-radius: 8px;
  padding: 10px;
  font-size: 12px;
}

.provider-warning {
  margin: 9px 0 0;
  color: var(--claude-deep);
  font-size: 12px;
}

.dashboard-shell {
  min-height: 100vh;
  padding: 28px;
  background: var(--canvas);
  color: var(--ink);
}

.dashboard-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 18px;
  margin-bottom: 18px;
}

.dashboard-head h1 {
  margin: 0;
  font-family: Georgia, "Times New Roman", "Songti SC", serif;
  font-size: 34px;
  font-weight: 500;
}

.dashboard-actions {
  display: flex;
  gap: 8px;
}

.secondary-button {
  height: 30px;
  border: 1px solid var(--line-strong);
  background: var(--paper);
  color: var(--ink);
  border-radius: 7px;
  padding: 0 10px;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.dashboard-card {
  background: var(--paper);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 16px;
  box-shadow: 0 18px 45px rgba(86, 61, 39, .12);
}

.dashboard-total {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 52px;
  font-weight: 700;
}

.dashboard-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  margin-top: 14px;
}
```

- [ ] **Step 4: Run frontend typecheck**

Run:

```powershell
npm run typecheck
```

Expected:

- PASS.

- [ ] **Step 5: Commit UI implementation**

```powershell
git add src
git commit -m "Implement Claude-style token popover UI"
```

---

### Task 8: Implement Tray Window Behavior

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/tray.rs`**

```rust
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, PhysicalPosition};

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit])?;
    let handle = app.handle().clone();

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Token Ledger")
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
            }
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_main_window(&handle, position.x as i32, position.y as i32);
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_main_window(app: &AppHandle, x: i32, y: i32) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let width = 386;
    let height = 520;
    let target_x = (x - width + 28).max(0);
    let target_y = (y - height - 18).max(0);

    let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
    let _ = window.show();
    let _ = window.set_focus();
}
```

- [ ] **Step 2: Ensure setup is registered**

`src-tauri/src/lib.rs` must call:

```rust
.setup(|app| {
    tray::setup(app)?;
    Ok(())
})
```

- [ ] **Step 3: Build Rust app to catch Tauri API mismatches**

Run:

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected:

- PASS. If Tauri API names differ for the installed version, adjust imports and event fields while preserving behavior.

- [ ] **Step 4: Run dev app**

Run:

```powershell
npm run dev
```

Expected:

- App starts.
- No normal taskbar window is shown at startup.
- Tray icon appears.
- Clicking tray icon shows the popover.

- [ ] **Step 5: Commit tray behavior**

```powershell
git add src-tauri/src/tray.rs src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "Add Windows tray popover behavior"
```

---

### Task 9: Add README, Final Verification, and Push

**Files:**
- Create: `README.md`
- Modify: `docs/superpowers/specs/2026-05-04-local-token-ledger-design.md`

- [ ] **Step 1: Create `README.md`**

```markdown
# Claude Bar

Windows tray app for local Codex and Claude token accounting.

## Scope

Claude Bar is local-only. It scans JSONL logs on this machine and summarizes token usage.

It does not:

- perform OAuth login
- import browser cookies
- read Codex `auth.json`
- call OpenAI, Anthropic, Claude, or ChatGPT services
- display prompts or responses

## Local Sources

Codex:

- `%USERPROFILE%\.codex\sessions`
- `%USERPROFILE%\.codex\archived_sessions`
- `%CODEX_HOME%\sessions` when `CODEX_HOME` is set

Claude:

- `%USERPROFILE%\.claude\projects`
- `%USERPROFILE%\.config\claude\projects`
- `<root>\projects` for each root in `CLAUDE_CONFIG_DIR`

## Development

```powershell
npm install
npm run dev
```

## Test

```powershell
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```
```

- [ ] **Step 2: Update spec status**

Append to `docs/superpowers/specs/2026-05-04-local-token-ledger-design.md`:

```markdown
## Implementation Status

MVP implementation follows `docs/superpowers/plans/2026-05-04-local-token-ledger-implementation.md`.
```

- [ ] **Step 3: Run full verification**

Run:

```powershell
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
npm run build:ui
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected:

- All commands pass.

- [ ] **Step 4: Commit docs and verification status**

```powershell
git add README.md docs/superpowers/specs/2026-05-04-local-token-ledger-design.md
git commit -m "Document local-only token ledger app"
```

- [ ] **Step 5: Push to remote**

Run only after the user approves pushing:

```powershell
git push -u origin main
```

Expected:

- Branch `main` is pushed to `https://github.com/binnnnnn520/claude-bar.git`.

---

## Self-Review

Spec coverage:

- Local-only Codex and Claude token scanning is covered by Tasks 2 through 6.
- Claude-inspired floating popover is covered by Task 7.
- Windows tray behavior is covered by Task 8.
- No auth, no cookies, no network is covered in the UI badges, README, and scanner-only architecture.
- Tests for parser behavior, aggregation, and build verification are included.

Placeholder scan:

- No unresolved placeholder markers are present.
- The only conditional instruction is the expected Tauri API adjustment in Task 8, which is constrained to preserving the specified tray behavior after `cargo check`.

Type consistency:

- Rust output uses camelCase serde names matching `src/types.ts`.
- `ProviderId` serializes as `codex` and `claude`, matching frontend provider IDs.
- `scan_usage` accepts `window` and returns `UsageSnapshot`, matching frontend `invoke<UsageSnapshot>("scan_usage", { window: "last30d" })`.
