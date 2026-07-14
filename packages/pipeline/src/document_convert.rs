//! Document-convert jobs: turn an uploaded PDF/Word document into a clean
//! markdown werkdocument.
//!
//! Known formats (docx/office via `pandoc`, PDF via `pdftotext`) are converted
//! deterministically with a real tool — reliable, offline, and keeping the
//! untrusted document away from the Bash-enabled LLM. Only when no tool fits
//! does it fall back to the LLM agent subprocess that enrich uses (see
//! [`crate::enrich::run_llm_subprocess`]), which decides for itself how to
//! convert (pick a tool, or read it directly). This module owns the payload
//! type, the transient upload storage helpers, the status-list query for the
//! editor, and the conversion orchestration. The worker (see `worker.rs`) drives
//! it and writes the produced markdown back to the traject's git corpus.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use regelrecht_corpus::backend::{create_backend, WriteContext};
use regelrecht_corpus::models::{GitHubSource, LocalSource, Source, SourceType};

use crate::enrich::{run_llm_subprocess, EnrichConfig};
use crate::error::{PipelineError, Result};
use crate::models::JobStatus;

/// Payload carried by a `document_convert` job. The raw bytes live in
/// `document_uploads` (referenced by `upload_id`), not in the job payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentConvertPayload {
    /// Row id in `document_uploads` holding the uploaded bytes.
    pub upload_id: Uuid,
    /// Owning traject's id — used by the worker to look up the writable-own
    /// corpus source directly (no ref-parsing round-trip).
    pub traject_id: Uuid,
    /// Owning traject ref (also mirrored onto `jobs.traject_ref`); used to build
    /// the `documents/<traject_ref>/…` write path and for the status UI.
    pub traject_ref: String,
    /// Target werkdocument path, relative to `documents/<traject_ref>/`,
    /// e.g. `"report.md"`. Already sanitized + collision-resolved by the
    /// editor-api upload handler.
    pub target_path: String,
    /// LLM provider override (`"claude"` / `"opencode"`); `None` uses the
    /// worker default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Account dat de upload deed; krijgt bij terminaal falen een
    /// `job_failed`-taak. `None` voor jobs van vóór dit veld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<Uuid>,
}

/// An uploaded document loaded from `document_uploads`.
pub struct Upload {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// Load the raw bytes of an uploaded document.
pub async fn load_upload(pool: &PgPool, upload_id: Uuid) -> Result<Upload> {
    let row = sqlx::query_as::<_, (String, String, Vec<u8>)>(
        "SELECT filename, content_type, bytes FROM document_uploads WHERE id = $1",
    )
    .bind(upload_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PipelineError::Enrich(format!("document upload {upload_id} not found")))?;
    Ok(Upload {
        filename: row.0,
        content_type: row.1,
        bytes: row.2,
    })
}

/// Delete an uploaded document's bytes. Called by the worker after a successful
/// conversion or a terminal failure — the transient blob is never kept.
pub async fn delete_upload(pool: &PgPool, upload_id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM document_uploads WHERE id = $1")
        .bind(upload_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete upload rows no longer referenced by an active (`pending`/`processing`)
/// document_convert job. This garbage-collects bytes orphaned when a worker
/// crashed mid-conversion and the generic orphan reaper failed the job without
/// running the type-specific [`delete_upload`]. Returns the number of rows
/// removed.
///
/// Safe because editor-api inserts the upload row and its job in a single
/// transaction, so a row with no active job is genuinely orphaned — the short
/// age grace is only belt-and-suspenders against clock/visibility skew.
pub async fn cleanup_orphaned_uploads(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM document_uploads du
        WHERE du.created_at < now() - interval '15 minutes'
          AND NOT EXISTS (
              SELECT 1 FROM jobs j
              WHERE j.job_type = 'document_convert'
                AND j.status IN ('pending', 'processing')
                AND j.payload->>'upload_id' = du.id::text
          )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Row for a traject's writable-own corpus source, from `traject_corpus_sources`.
#[derive(sqlx::FromRow)]
struct WritableOwnSourceRow {
    source_id: String,
    name: String,
    source_type: String,
    gh_owner: Option<String>,
    gh_repo: Option<String>,
    gh_branch: Option<String>,
    gh_path: Option<String>,
    gh_ref: Option<String>,
    local_path: Option<String>,
    priority: i32,
    auth_ref: Option<String>,
}

impl WritableOwnSourceRow {
    /// Mirror of editor-api's `TrajectSourceRow::to_source`. Scopes are omitted
    /// (they only gate reads; the worker only writes).
    fn to_source(&self) -> Source {
        let source_type = match self.source_type.as_str() {
            "github" => SourceType::GitHub {
                github: GitHubSource {
                    owner: self.gh_owner.clone().unwrap_or_default(),
                    repo: self.gh_repo.clone().unwrap_or_default(),
                    branch: self.gh_branch.clone().unwrap_or_default(),
                    path: self.gh_path.clone(),
                    git_ref: self.gh_ref.clone(),
                },
            },
            _ => SourceType::Local {
                local: LocalSource {
                    path: std::path::PathBuf::from(self.local_path.clone().unwrap_or_default()),
                },
            },
        };
        Source {
            id: self.source_id.clone(),
            name: self.name.clone(),
            source_type,
            scopes: Vec::new(),
            priority: self.priority.max(0) as u32,
            auth_ref: self.auth_ref.clone(),
        }
    }
}

/// Build a corpus [`Source`] for the traject's single writable-own source.
async fn load_writable_own_source(pool: &PgPool, traject_id: Uuid) -> Result<Source> {
    let row = sqlx::query_as::<_, WritableOwnSourceRow>(
        r#"
        SELECT source_id, name, source_type::text AS source_type,
               gh_owner, gh_repo, gh_branch, gh_path, gh_ref, local_path,
               priority, auth_ref
        FROM traject_corpus_sources
        WHERE traject_id = $1 AND is_writable_own = TRUE
        -- A partial unique index already guarantees at most one such row per
        -- traject; the deterministic order + LIMIT is defensive belt-and-braces.
        ORDER BY priority DESC, source_id
        LIMIT 1
        "#,
    )
    .bind(traject_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        PipelineError::Enrich(format!(
            "traject {traject_id} has no writable-own corpus source"
        ))
    })?;
    Ok(row.to_source())
}

/// Write the converted markdown into the traject's writable-own corpus and push
/// it. This is a service-account commit straight to the source's branch (no PR):
/// the async worker has no user session, so it mirrors how the enrich worker
/// writes to the corpus rather than the editor's per-session PR flow. The file
/// lands at `documents/<traject_ref>/<target_path>` — the same location the
/// editor's document handler writes to — so it shows up as a normal werkdocument.
pub async fn write_markdown_to_traject(
    pool: &PgPool,
    payload: &DocumentConvertPayload,
    markdown: &str,
) -> Result<()> {
    let source = load_writable_own_source(pool, payload.traject_id).await?;
    // Resolve a push token strictly by the source's auth key (env var
    // `CORPUS_AUTH_<key>_TOKEN`); no auth file in the worker and no shared-token
    // fallback — matches the editor's writable-own resolution.
    let auth_key = source.auth_ref.clone().unwrap_or_else(|| source.id.clone());
    let token = regelrecht_corpus::auth::resolve_token_strict(&auth_key, None)?;

    // A GitHub source with no push token would only commit into the worker's
    // throwaway checkout (GitBackend goes local-only) and never reach the
    // traject — silent data loss. Fail loudly instead, so the failure is visible
    // in the werkdocumenten status block and the ops fix (set the token) is
    // obvious. Local sources need no token (the local write IS the persistence).
    if token.is_none() && matches!(source.source_type, SourceType::GitHub { .. }) {
        return Err(PipelineError::Enrich(format!(
            "no push token for traject source '{auth_key}' (expected env {}); the converted \
             document cannot be persisted to the traject repository",
            regelrecht_corpus::auth::token_env_name(&auth_key),
        )));
    }

    let mut backend = create_backend(&source, token.as_deref())?;
    backend.ensure_ready().await?;

    let relative_path = Path::new("documents")
        .join(&payload.traject_ref)
        .join(&payload.target_path);
    backend.write_file(&relative_path, markdown).await?;

    let message = format!("document-convert: {}", payload.target_path);
    backend.persist(&WriteContext::new(message, None)).await?;
    Ok(())
}

/// A trimmed view of a `document_convert` job for the werkdocumenten status UI.
/// Deliberately does not expose the full [`crate::models::Job`] (payload,
/// attempts, timestamps beyond what the UI needs).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TrajectJobView {
    pub id: Uuid,
    /// Target `.md` path (from the job payload); `None` only for a malformed payload.
    pub target_path: Option<String>,
    pub status: JobStatus,
    /// Failure reason (from `jobs.result->>'error'`). Only ever populated for
    /// a `failed` row, and only when `include_failed` was true - see
    /// `list_traject_document_jobs`. `None` otherwise.
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// List the traject's document-convert jobs relevant to the werkdocumenten
/// status UI. `include_failed` mirrors the `tasks.job_review` feature flag,
/// inverted (caller passes `!flag_enabled`) - the flag decides which of two
/// mutually exclusive failure UIs is active, so exactly one of "taak" or
/// "inline status row" ever shows a given failure:
///
/// * `include_failed = false` (flag ON, taken-mechanisme actief): only
///   still-active jobs (`pending`, `processing`). A completed job is
///   represented by the actual `.md` in the documents list; a terminally
///   failed job is not shown here at all - the uploader instead gets a
///   wegklikbare `job_failed` taak (see
///   `worker::process_next_document_convert_job`), so a failure no longer
///   lingers forever in this status block. `error` is always `None` in this
///   mode (the query excludes failed rows entirely).
/// * `include_failed = true` (flag OFF): pre-taken-mechanisme behaviour
///   restored byte-for-byte - everything not `completed` (pending,
///   processing, failed), with `error` populated from `result->>'error'` for
///   failed rows so the old inline failure UI (`ConversionStatus.vue`) has
///   something to render.
pub async fn list_traject_document_jobs(
    pool: &PgPool,
    traject_ref: &str,
    include_failed: bool,
) -> Result<Vec<TrajectJobView>> {
    let (status_filter, error_column) = if include_failed {
        ("status <> 'completed'", "result->>'error'")
    } else {
        ("status IN ('pending', 'processing')", "NULL::text")
    };
    // `status_filter`/`error_column` are fixed literals selected above, never
    // caller-supplied - no injection surface despite the format!.
    let query = format!(
        r#"
        SELECT id,
               payload->>'target_path' AS target_path,
               status,
               {error_column}          AS error,
               created_at
        FROM jobs
        WHERE traject_ref = $1
          AND job_type = 'document_convert'
          AND {status_filter}
        ORDER BY created_at DESC
        LIMIT 100
        "#
    );
    let rows = sqlx::query_as::<_, TrajectJobView>(&query)
        .bind(traject_ref)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Target `.md` paths of the traject's still-pending/processing document-convert
/// jobs. Used by the upload handler to make the derived target collision-safe
/// against conversions that are enqueued but haven't committed their `.md` yet —
/// without this, two uploads that derive the same name (e.g. two `report.pdf`)
/// would both target `report.md` and the second conversion would overwrite the
/// first once both commit.
pub async fn pending_target_paths(pool: &PgPool, traject_ref: &str) -> Result<Vec<String>> {
    let rows = sqlx::query_as::<_, (Option<String>,)>(
        r#"
        SELECT payload->>'target_path'
        FROM jobs
        WHERE traject_ref = $1
          AND job_type = 'document_convert'
          AND status IN ('pending', 'processing')
        "#,
    )
    .bind(traject_ref)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(|(p,)| p).collect())
}

/// Abstraction over the actual document→markdown conversion so the orchestration
/// can be unit-tested without spawning a real LLM subprocess (mirrors the
/// [`crate::enrich::LlmRunner`] seam).
#[async_trait::async_trait]
pub trait DocumentConverter: Send + Sync {
    /// Convert `input_file` (inside `work_dir`) to markdown, written to
    /// `output_path`. The agent runs with `work_dir` as its working directory.
    async fn convert(
        &self,
        input_file: &Path,
        work_dir: &Path,
        output_path: &Path,
        config: &EnrichConfig,
    ) -> Result<()>;
}

/// Production converter: prompts the LLM agent to figure out and perform the
/// conversion itself, then write the markdown to `output_path`.
pub struct LlmDocumentConverter;

#[async_trait::async_trait]
impl DocumentConverter for LlmDocumentConverter {
    async fn convert(
        &self,
        input_file: &Path,
        work_dir: &Path,
        output_path: &Path,
        config: &EnrichConfig,
    ) -> Result<()> {
        let prompt = build_convert_prompt(input_file, output_path);
        run_llm_subprocess(
            &config.provider,
            &prompt,
            // Deliberately NOT passed as OpenCode's `-f`: the input is a binary
            // PDF/Word file, and `-f` feeds a file as text context (enrich only
            // ever passed text YAML). The agent instead reads the file — named in
            // the prompt — from its working directory, which works for both the
            // opencode (`--dir`) and claude (`current_dir`) providers.
            None,
            work_dir,
            config,
            // The agent may run/install a converter (e.g. pdftotext, pandoc),
            // so the claude provider needs shell access here.
            true,
        )
        .await
    }
}

/// Build the conversion prompt. The agent is deliberately given full latitude to
/// decide the conversion approach (spec: "het taalmodel mag het zelf bedenken").
fn build_convert_prompt(input_file: &Path, output_path: &Path) -> String {
    let input = input_file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("input");
    let output = output_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output.md");
    format!(
        "You are converting an uploaded document to clean Markdown.\n\n\
         The document is the file `{input}` in your current working directory.\n\n\
         Do the following:\n\
         1. Inspect `{input}` and determine its format (for example PDF or Word).\n\
         2. Decide on and carry out the best way to convert it to Markdown. The tools \
            `pandoc` (Word/HTML/… → Markdown, e.g. `pandoc {input} -t gfm --wrap=none -o {output}`) \
            and `pdftotext` (PDF → text) are already installed — prefer them. Do NOT rely on \
            network access: this environment has no internet egress, so do not try to install \
            packages (e.g. via `pip`). If no tool fits, read the document yourself and \
            transcribe it. Choose whatever yields the most faithful result.\n\
         3. Produce clean, well-structured GitHub-flavored Markdown that preserves the \
            document's headings, lists, tables and paragraph structure. Do NOT summarize, \
            translate, add commentary, or omit content — transcribe faithfully.\n\
         4. Write the resulting Markdown to a file named `{output}` in your current \
            working directory. Write ONLY the Markdown document to that file — no \
            surrounding code fences, no explanations.\n\n\
         The final deliverable is the file `{output}`."
    )
}

/// Pick a file extension for the scratch input file from the original filename,
/// falling back to the content type. Keeps the extension the agent sees honest
/// so it can detect the format.
fn extension_for(filename: &str, content_type: &str) -> String {
    if let Some(ext) = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
    {
        return ext.to_ascii_lowercase();
    }
    match content_type {
        "application/pdf" => "pdf",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        _ => "bin",
    }
    .to_string()
}

/// Load the upload, run the conversion in a fresh working directory, and return
/// the produced markdown. The caller (worker) is responsible for writing the
/// markdown to git and for cleaning up the upload row + job.
pub async fn execute_document_convert(
    pool: &PgPool,
    payload: &DocumentConvertPayload,
    config: &EnrichConfig,
    converter: &dyn DocumentConverter,
) -> Result<String> {
    let upload = load_upload(pool, payload.upload_id).await?;
    let work_dir = std::env::temp_dir().join(format!("docconvert-{}", payload.upload_id));
    // Clear any stale directory left by a previous attempt of this same job.
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    tokio::fs::create_dir_all(&work_dir).await?;

    let result = convert_in_dir(&work_dir, &upload, &payload.target_path, config, converter).await;

    // Always remove the working directory, success or failure.
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    result
}

/// Try to convert `input_file` to markdown at `output_path` with a deterministic
/// command-line tool, chosen by extension. Returns `Some(markdown)` on success,
/// or `None` when the format is not handled, the tool is absent, it exits
/// non-zero, or it produces empty output — every `None` case falls through to the
/// agentic converter, so a missing tool or an odd file never hard-fails here.
///
/// `pandoc` handles the office/markup formats; `pdftotext` (poppler) handles
/// PDF. Both are baked into the enrich-worker image.
async fn try_deterministic_convert(
    input_file: &Path,
    ext: &str,
    output_path: &Path,
) -> Option<String> {
    let mut cmd = match ext {
        // pandoc reads these natively and emits GitHub-flavored Markdown.
        "docx" | "odt" | "rtf" | "html" | "htm" | "epub" | "fb2" => {
            let mut c = tokio::process::Command::new("pandoc");
            c.arg(input_file)
                .arg("-t")
                .arg("gfm")
                .arg("--wrap=none")
                .arg("-o")
                .arg(output_path);
            c
        }
        // pandoc cannot read PDF; poppler's pdftotext extracts the text layer.
        "pdf" => {
            let mut c = tokio::process::Command::new("pdftotext");
            c.arg("-layout").arg(input_file).arg(output_path);
            c
        }
        _ => return None,
    };

    match cmd.status().await {
        Ok(status) if status.success() => {
            let markdown = tokio::fs::read_to_string(output_path).await.ok()?;
            if markdown.trim().is_empty() {
                None
            } else {
                Some(markdown)
            }
        }
        // Tool missing (spawn error) or a non-zero exit → let the agent try.
        _ => None,
    }
}

/// The pure filesystem half of the conversion, split out so it can be unit-tested
/// with a fake converter and a synthetic upload (no DB, no LLM).
async fn convert_in_dir(
    work_dir: &Path,
    upload: &Upload,
    target_path: &str,
    config: &EnrichConfig,
    converter: &dyn DocumentConverter,
) -> Result<String> {
    let ext = extension_for(&upload.filename, &upload.content_type);
    let input_file = work_dir.join(format!("input.{ext}"));
    tokio::fs::write(&input_file, &upload.bytes).await?;

    let output_name = Path::new(target_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output.md");
    let output_path: PathBuf = work_dir.join(output_name);

    // Deterministic-first: for known formats convert with a real tool
    // (`pandoc`/`pdftotext`) instead of the agent. This is reliable (the agent
    // proved flaky at reliably writing the output file), needs no network, and —
    // crucially — keeps the untrusted document away from the Bash-enabled LLM,
    // shrinking the prompt-injection surface. The agentic path stays as a
    // fallback for formats the tools don't handle (or when they are absent).
    if let Some(markdown) = try_deterministic_convert(&input_file, &ext, &output_path).await {
        return Ok(markdown);
    }

    converter
        .convert(&input_file, work_dir, &output_path, config)
        .await?;

    let markdown = tokio::fs::read_to_string(&output_path).await.map_err(|e| {
        PipelineError::Enrich(format!(
            "conversion produced no readable markdown at {}: {e}",
            output_path.display()
        ))
    })?;
    if markdown.trim().is_empty() {
        return Err(PipelineError::Enrich(
            "conversion produced empty markdown".to_string(),
        ));
    }
    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_roundtrips_through_json() {
        let payload = DocumentConvertPayload {
            upload_id: Uuid::nil(),
            traject_id: Uuid::nil(),
            traject_ref: "abcd1234".to_string(),
            target_path: "report.md".to_string(),
            provider: Some("claude".to_string()),
            requested_by: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["upload_id"], "00000000-0000-0000-0000-000000000000");
        assert_eq!(json["target_path"], "report.md");
        let back: DocumentConvertPayload = serde_json::from_value(json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn payload_omits_absent_provider() {
        let payload = DocumentConvertPayload {
            upload_id: Uuid::nil(),
            traject_id: Uuid::nil(),
            traject_ref: "abcd1234".to_string(),
            target_path: "report.md".to_string(),
            provider: None,
            requested_by: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("provider").is_none());
    }

    #[test]
    fn extension_prefers_filename_then_content_type() {
        assert_eq!(
            extension_for("Report.PDF", "application/octet-stream"),
            "pdf"
        );
        assert_eq!(extension_for("brief.docx", "text/plain"), "docx");
        assert_eq!(extension_for("noext", "application/pdf"), "pdf");
        assert_eq!(extension_for("noext", "application/msword"), "doc");
    }

    #[test]
    fn prompt_names_input_and_output_files() {
        let prompt = build_convert_prompt(
            Path::new("/tmp/wd/input.pdf"),
            Path::new("/tmp/wd/report.md"),
        );
        assert!(prompt.contains("`input.pdf`"));
        assert!(prompt.contains("`report.md`"));
        // The agent must be told to transcribe faithfully, not summarize.
        assert!(prompt.to_lowercase().contains("do not summarize"));
    }

    /// Fake converter that writes fixed markdown, proving the orchestration reads
    /// back exactly what the agent produced and reports the input it received.
    struct FakeConverter {
        markdown: String,
    }

    #[async_trait::async_trait]
    impl DocumentConverter for FakeConverter {
        async fn convert(
            &self,
            input_file: &Path,
            _work_dir: &Path,
            output_path: &Path,
            _config: &EnrichConfig,
        ) -> Result<()> {
            // The input file must have been materialized before we're called.
            assert!(
                input_file.exists(),
                "input file should exist for the converter"
            );
            tokio::fs::write(output_path, &self.markdown).await.unwrap();
            Ok(())
        }
    }

    #[tokio::test]
    async fn convert_in_dir_returns_produced_markdown() {
        let dir = tempfile::tempdir().unwrap();
        // A `.bin` extension no deterministic tool handles, so the agentic
        // fallback (the FakeConverter) runs and we test that orchestration.
        let upload = Upload {
            filename: "report.bin".to_string(),
            content_type: "application/octet-stream".to_string(),
            bytes: b"raw bytes".to_vec(),
        };
        let converter = FakeConverter {
            markdown: "# Report\n\nBody.\n".to_string(),
        };
        let md = convert_in_dir(
            dir.path(),
            &upload,
            "report.md",
            &EnrichConfig::for_test(crate::enrich::LlmProvider::Claude {
                path: "claude".into(),
                model: None,
            }),
            &converter,
        )
        .await
        .unwrap();
        assert_eq!(md, "# Report\n\nBody.\n");
    }

    #[tokio::test]
    async fn convert_in_dir_errors_on_empty_output() {
        let dir = tempfile::tempdir().unwrap();
        let upload = Upload {
            filename: "report.bin".to_string(),
            content_type: "application/octet-stream".to_string(),
            bytes: b"x".to_vec(),
        };
        let converter = FakeConverter {
            markdown: "   \n".to_string(),
        };
        let err = convert_in_dir(
            dir.path(),
            &upload,
            "report.md",
            &EnrichConfig::for_test(crate::enrich::LlmProvider::Claude {
                path: "claude".into(),
                model: None,
            }),
            &converter,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("empty markdown"));
    }

    #[tokio::test]
    async fn deterministic_convert_returns_none_for_unhandled_format() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.bin");
        tokio::fs::write(&input, b"data").await.unwrap();
        let output = dir.path().join("out.md");
        // Unknown extension → no tool is even spawned → None (agentic fallback).
        assert!(try_deterministic_convert(&input, "bin", &output)
            .await
            .is_none());
        assert!(!output.exists());
    }
}
