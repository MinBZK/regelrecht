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
/// 1. Environment variable `CORPUS_AUTH_{KEY}_TOKEN` (uppercased, hyphens → underscores)
/// 2. `corpus-auth.yaml` file (if it exists)
/// 3. None (unauthenticated, lower rate limits)
pub fn resolve_token(source_id: &str, auth_file: Option<&Path>) -> Result<Option<String>> {
    // 1. Environment variable
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

    // 3. No token
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
        let result = resolve_token("nonexistent", None).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_token_missing_file_is_none() {
        let result = resolve_token("test", Some(Path::new("/nonexistent/auth.yaml"))).unwrap();
        assert_eq!(result, None);
    }
}
