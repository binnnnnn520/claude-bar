use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoots {
    pub codex_roots: Vec<PathBuf>,
    pub claude_roots: Vec<PathBuf>,
}

pub fn discover_roots() -> ScanRoots {
    roots_from_env(
        env::var_os("CODEX_HOME"),
        env::var_os("CLAUDE_CONFIG_DIR"),
        env::var_os("USERPROFILE"),
        env::var_os("HOME"),
    )
}

fn roots_from_env(
    codex_home: Option<OsString>,
    claude_config_dir: Option<OsString>,
    user_profile: Option<OsString>,
    home: Option<OsString>,
) -> ScanRoots {
    let user_home = user_home_from_env(user_profile, home);

    ScanRoots {
        codex_roots: codex_roots(codex_home, user_home.as_deref()),
        claude_roots: claude_roots(claude_config_dir, user_home.as_deref()),
    }
}

fn env_path(value: Option<OsString>) -> Option<PathBuf> {
    value.and_then(|raw| {
        let trimmed = raw.to_string_lossy().trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    })
}

fn user_home_from_env(user_profile: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    env_path(user_profile).or_else(|| env_path(home))
}

fn codex_roots(codex_home: Option<OsString>, user_home: Option<&Path>) -> Vec<PathBuf> {
    let roots = if let Some(home) = env_path(codex_home) {
        vec![home.join("sessions"), home.join("archived_sessions")]
    } else {
        user_home
            .map(|home| {
                vec![
                    home.join(".codex").join("sessions"),
                    home.join(".codex").join("archived_sessions"),
                ]
            })
            .unwrap_or_default()
    };

    dedupe_preserving_order(roots)
}

fn claude_roots(claude_config_dir: Option<OsString>, user_home: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(config) = claude_config_dir {
        for raw in config.to_string_lossy().split(',') {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                roots.push(PathBuf::from(trimmed).join("projects"));
            }
        }
    }

    if let Some(home) = user_home {
        roots.push(home.join(".claude").join("projects"));
        roots.push(home.join(".config").join("claude").join("projects"));
    }

    dedupe_preserving_order(roots)
}

fn dedupe_preserving_order(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    roots
        .into_iter()
        .filter(|root| seen.insert(root.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::roots_from_env;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn roots_from_env_ignores_empty_home_values() {
        let roots = roots_from_env(
            Some(OsString::from("")),
            None,
            Some(OsString::from("")),
            Some(OsString::from("/home/example")),
        );

        assert_eq!(
            roots.codex_roots,
            vec![
                PathBuf::from("/home/example")
                    .join(".codex")
                    .join("sessions"),
                PathBuf::from("/home/example")
                    .join(".codex")
                    .join("archived_sessions"),
            ]
        );
        assert_eq!(
            roots.claude_roots,
            vec![
                PathBuf::from("/home/example")
                    .join(".claude")
                    .join("projects"),
                PathBuf::from("/home/example")
                    .join(".config")
                    .join("claude")
                    .join("projects"),
            ]
        );
    }

    #[test]
    fn roots_from_env_deduplicates_claude_roots_preserving_order() {
        let roots = roots_from_env(
            None,
            Some(OsString::from(
                "/home/example/.claude,/home/example/.config/claude,/home/example/.claude",
            )),
            Some(OsString::from("/home/example")),
            None,
        );

        assert_eq!(
            roots.claude_roots,
            vec![
                PathBuf::from("/home/example")
                    .join(".claude")
                    .join("projects"),
                PathBuf::from("/home/example")
                    .join(".config")
                    .join("claude")
                    .join("projects"),
            ]
        );
    }
}
