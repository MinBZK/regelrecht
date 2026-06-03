//! Shared LLM invocation primitives.
//!
//! Both the enrich pipeline (`enrich.rs`) and the editor-suggestion pipeline
//! (`suggest.rs`) spawn a headless LLM CLI (`claude -p` / `opencode run`) on a
//! checked-out corpus repo. This module holds the provider selection, the
//! environment allowlist, and the process runner so neither pipeline duplicates
//! the subprocess/timeout/kill handling.
//!
//! The runner is deliberately prompt-based: callers build the prompt themselves
//! and hand it to [`LlmRunner::run`] together with the working directory. It
//! knows nothing about enrich payloads or suggestion kinds.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// A headless LLM provider plus where its CLI lives.
///
/// Both providers manage their own authentication:
/// - **OpenCode/VLAM**: reads `~/.local/share/opencode/auth.json` (set via `opencode auth`)
/// - **Claude**: reads `~/.claude/.credentials` or `ANTHROPIC_API_KEY` env var
///
/// In Docker, mount the appropriate auth files or set env vars.
#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenCode {
        path: PathBuf,
        model: Option<String>,
    },
    Claude {
        path: PathBuf,
        model: Option<String>,
    },
}

impl LlmProvider {
    /// Short name used in branch names and metadata.
    pub fn name(&self) -> &str {
        match self {
            LlmProvider::OpenCode { .. } => "opencode",
            LlmProvider::Claude { .. } => "claude",
        }
    }

    /// Model string for metadata (provider-specific default if not set).
    pub fn model_str(&self) -> String {
        match self {
            LlmProvider::OpenCode { model, .. } => {
                model.clone().unwrap_or_else(|| "default".into())
            }
            LlmProvider::Claude { model, .. } => model.clone().unwrap_or_else(|| "default".into()),
        }
    }
}

/// What an LLM run needs: a fully-built prompt, the repo working directory, the
/// provider, the timeout, and the tools the agent may use. Kept provider- and
/// pipeline-agnostic so enrich and suggest share one runner.
pub struct LlmInvocation<'a> {
    pub prompt: &'a str,
    pub repo_path: &'a Path,
    pub provider: &'a LlmProvider,
    pub timeout: Duration,
    /// Comma-separated allowed tools for the Claude CLI (`--allowedTools`).
    /// OpenCode ignores this. Example: "Read,Edit,Write,Grep,Glob".
    pub allowed_tools: &'a str,
}

/// Trait abstracting the LLM invocation so callers can test with a fake
/// provider that doesn't spawn real processes.
#[async_trait::async_trait]
pub trait LlmRunner: Send + Sync {
    /// Run the LLM with the given invocation. Implementations respect the timeout.
    async fn run(&self, inv: &LlmInvocation<'_>) -> Result<(), LlmError>;
}

/// Error from spawning or running the LLM subprocess.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("{0}")]
    Process(String),
}

/// Default runner that spawns a real CLI process.
pub struct ProcessLlmRunner;

#[async_trait::async_trait]
impl LlmRunner for ProcessLlmRunner {
    async fn run(&self, inv: &LlmInvocation<'_>) -> Result<(), LlmError> {
        let provider_name = inv.provider.name().to_string();
        let mut cmd = build_command(inv);

        // stderr is inherited so the LLM's logging goes to the worker's stderr.
        // This avoids a deadlock: if stderr were piped, a verbose LLM (e.g. Claude CLI)
        // could fill the OS pipe buffer (64 KB) and block indefinitely.
        cmd.stderr(std::process::Stdio::inherit());
        let mut child = cmd
            .spawn()
            .map_err(|e| LlmError::Process(format!("failed to spawn {provider_name}: {e}")))?;

        let status = tokio::select! {
            result = child.wait() => {
                result.map_err(|e| {
                    LlmError::Process(format!("failed to wait for {provider_name}: {e}"))
                })?
            }
            _ = tokio::time::sleep(inv.timeout) => {
                if let Err(e) = child.kill().await {
                    tracing::warn!(error = %e, "failed to kill timed-out LLM process");
                }
                let _ = child.wait().await;
                return Err(LlmError::Process(format!(
                    "{provider_name} timed out after {:?}",
                    inv.timeout
                )));
            }
        };

        if !status.success() {
            return Err(LlmError::Process(format!(
                "{provider_name} exited with {status}"
            )));
        }

        Ok(())
    }
}

/// Allowlisted environment variable prefixes/names that are safe to pass to the
/// LLM subprocess. Everything else (DATABASE_URL, etc.) is stripped.
const LLM_ENV_ALLOWLIST: &[&str] = &[
    "HOME",
    "PATH",
    "TERM",
    "LANG",
    "USER",
    "SHELL",
    "TMPDIR",
    "XDG_",
    // Provider-specific auth
    "ANTHROPIC_API_KEY",
    "VLAM_API_KEY",
    "OPENCODE_",
];

/// Check whether an environment variable name is on the allowlist.
fn env_allowed(key: &str) -> bool {
    LLM_ENV_ALLOWLIST
        .iter()
        .any(|prefix| key == *prefix || key.starts_with(prefix))
}

/// Build the command for the configured LLM provider.
///
/// The subprocess gets a stripped environment: only variables on
/// `LLM_ENV_ALLOWLIST` are forwarded. This prevents leaking DATABASE_URL
/// and other secrets to the LLM process (which may have shell access).
fn build_command(inv: &LlmInvocation<'_>) -> tokio::process::Command {
    // Collect allowed env vars before creating the command.
    let safe_env: Vec<(String, String)> =
        std::env::vars().filter(|(k, _)| env_allowed(k)).collect();

    match inv.provider {
        LlmProvider::OpenCode { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.env_clear();
            cmd.envs(safe_env);
            cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
            cmd.arg("run")
                .arg(inv.prompt)
                .arg("--format")
                .arg("json")
                .arg("--dir")
                .arg(inv.repo_path);
            if let Some(m) = model {
                cmd.arg("-m").arg(m);
            }
            cmd
        }
        LlmProvider::Claude { path, model } => {
            let mut cmd = tokio::process::Command::new(path);
            cmd.env_clear();
            cmd.envs(safe_env);
            cmd.env("NODE_OPTIONS", "--max-old-space-size=512");
            cmd.arg("-p")
                .arg(inv.prompt)
                .arg("--allowedTools")
                .arg(inv.allowed_tools)
                .current_dir(inv.repo_path);
            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }
            cmd
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_allowlist_passes_auth_and_blocks_secrets() {
        assert!(env_allowed("ANTHROPIC_API_KEY"));
        assert!(env_allowed("OPENCODE_AUTH"));
        assert!(env_allowed("PATH"));
        assert!(!env_allowed("DATABASE_URL"));
        assert!(!env_allowed("RIG_API_KEY"));
    }

    #[test]
    fn provider_name_and_model() {
        let p = LlmProvider::Claude {
            path: "claude".into(),
            model: Some("opus".into()),
        };
        assert_eq!(p.name(), "claude");
        assert_eq!(p.model_str(), "opus");

        let o = LlmProvider::OpenCode {
            path: "opencode".into(),
            model: None,
        };
        assert_eq!(o.name(), "opencode");
        assert_eq!(o.model_str(), "default");
    }
}
