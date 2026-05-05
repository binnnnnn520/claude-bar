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
