use std::path::Path;

use crate::error::{CorpusError, Result};

/// Auth configuration for a corpus source, loaded from `corpus-auth.yaml`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthConfig {
    pub sources: Vec<SourceAuth>,
}

/// Auth entry for a single source.
#[derive(Clone, serde::Deserialize)]
pub struct SourceAuth {
    pub id: String,
    pub token: String,
}

impl std::fmt::Debug for SourceAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceAuth")
            .field("id", &self.id)
            .field("token", &"***")
            .finish()
    }
}

/// Look up a GitHub token for a source.
///
/// Uses `auth_ref` as the lookup key when provided, otherwise falls back
/// to `source_id`. This matches the RFC-010 design where `auth_ref` in the
/// manifest references an entry in the auth config.
///
/// Resolution order:
/// 1. Environment variable `CORPUS_AUTH_{KEY}_TOKEN` (uppercased, hyphens → underscores)
/// 2. `corpus-auth.yaml` file (if it exists)
/// 3. None (unauthenticated, lower rate limits)
pub fn resolve_token_for_source(
    source_id: &str,
    auth_ref: Option<&str>,
    auth_file: Option<&Path>,
) -> Result<Option<String>> {
    let key = auth_ref.unwrap_or(source_id);
    resolve_token(key, auth_file)
}

/// Look up a GitHub token by key.
///
/// Resolution order:
/// 1. Per-source environment variable `CORPUS_AUTH_{KEY}_TOKEN`
///    (uppercased, hyphens → underscores). Use this for multi-source
///    setups that need isolation between source repos.
/// 2. `corpus-auth.yaml` file (if it exists) — explicit per-source
///    entries, same purpose as (1) but file-backed.
/// 3. Legacy shared environment variable `CORPUS_GIT_TOKEN`. The
///    harvester era used this single global token for one upstream
///    repo. We accept it as a fallback so deployments that still set
///    only the legacy variable keep working for *every* GitHub source
///    without needing to duplicate the secret under each per-source
///    name. Per-source vars set above still win.
/// 4. None (unauthenticated, lower rate limits).
pub fn resolve_token(source_id: &str, auth_file: Option<&Path>) -> Result<Option<String>> {
    // 1. Per-source environment variable
    let env_key = format!(
        "CORPUS_AUTH_{}_TOKEN",
        source_id.to_uppercase().replace('-', "_")
    );
    if let Ok(token) = std::env::var(&env_key) {
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }

    // 2. Auth file
    if let Some(path) = auth_file {
        if path.exists() {
            let content = std::fs::read_to_string(path).map_err(|e| {
                CorpusError::Config(format!(
                    "Failed to read auth file {}: {}",
                    path.display(),
                    e
                ))
            })?;

            let config: AuthConfig = serde_yaml_ng::from_str(&content).map_err(|e| {
                CorpusError::Config(format!(
                    "Failed to parse auth file {}: {}",
                    path.display(),
                    e
                ))
            })?;

            if let Some(entry) = config.sources.iter().find(|s| s.id == source_id) {
                return Ok(Some(entry.token.clone()));
            }
        }
    }

    // 3. Legacy shared CORPUS_GIT_TOKEN (harvester-era single token)
    if let Ok(token) = std::env::var("CORPUS_GIT_TOKEN") {
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }

    // 4. No token
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    // Tests in this module mutate the process-wide environment via
    // `CORPUS_GIT_TOKEN` / per-source vars. cargo's default parallel
    // test runner would then race two tests reading/writing the same
    // env vars and produce flakes. Serialize the env-mutating tests
    // behind one mutex so they take turns. Tests that only read a
    // unique-named var don't need it.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_resolve_token_from_env() {
        // Use a unique env var name to avoid test interference
        let key = "CORPUS_AUTH_TEST_SOURCE_123_TOKEN";
        unsafe { std::env::set_var(key, "env-token-value") };

        let result = resolve_token("test-source-123", None).unwrap();
        assert_eq!(result, Some("env-token-value".to_string()));

        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn test_resolve_token_from_file() {
        let yaml = r#"
sources:
  - id: amsterdam
    token: file-token-123
  - id: rotterdam
    token: file-token-456
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let result = resolve_token("amsterdam", Some(file.path())).unwrap();
        assert_eq!(result, Some("file-token-123".to_string()));

        let result = resolve_token("rotterdam", Some(file.path())).unwrap();
        assert_eq!(result, Some("file-token-456".to_string()));
    }

    #[test]
    fn test_resolve_token_none() {
        let _g = ENV_LOCK.lock().unwrap();
        // Make sure the legacy CORPUS_GIT_TOKEN doesn't leak from the
        // test runner's environment and turn this assertion into a flake.
        let legacy_was_set = std::env::var("CORPUS_GIT_TOKEN").ok();
        unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") };
        let result = resolve_token("nonexistent", None).unwrap();
        if let Some(prev) = legacy_was_set {
            unsafe { std::env::set_var("CORPUS_GIT_TOKEN", prev) };
        }
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_token_falls_back_to_legacy_corpus_git_token() {
        let _g = ENV_LOCK.lock().unwrap();
        // No per-source var, no auth file → resolver picks up
        // CORPUS_GIT_TOKEN so deployments that still set only the legacy
        // single-token variable keep working across all sources.
        let prev = std::env::var("CORPUS_GIT_TOKEN").ok();
        unsafe { std::env::set_var("CORPUS_GIT_TOKEN", "legacy-token-value") };
        let result = resolve_token("any-source-here", None).unwrap();
        match prev {
            Some(v) => unsafe { std::env::set_var("CORPUS_GIT_TOKEN", v) },
            None => unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") },
        }
        assert_eq!(result, Some("legacy-token-value".to_string()));
    }

    #[test]
    fn test_per_source_env_wins_over_legacy_corpus_git_token() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_legacy = std::env::var("CORPUS_GIT_TOKEN").ok();
        let per_source_key = "CORPUS_AUTH_PRIORITY_TEST_TOKEN";
        unsafe { std::env::set_var("CORPUS_GIT_TOKEN", "legacy-loses") };
        unsafe { std::env::set_var(per_source_key, "per-source-wins") };
        let result = resolve_token("priority-test", None).unwrap();
        unsafe { std::env::remove_var(per_source_key) };
        match prev_legacy {
            Some(v) => unsafe { std::env::set_var("CORPUS_GIT_TOKEN", v) },
            None => unsafe { std::env::remove_var("CORPUS_GIT_TOKEN") },
        }
        assert_eq!(result, Some("per-source-wins".to_string()));
    }

    #[test]
    fn test_resolve_token_missing_file_is_none() {
        let result = resolve_token("test", Some(Path::new("/nonexistent/auth.yaml"))).unwrap();
        assert_eq!(result, None);
    }
}
