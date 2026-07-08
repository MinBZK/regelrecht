//! Document-convert jobs: turn an uploaded PDF/Word document into a clean
//! markdown werkdocument.
//!
//! The heavy lifting is done by the same LLM agent subprocess that enrich uses
//! (see [`crate::enrich::run_llm_subprocess`]); the agent decides for itself how
//! to convert the document (pick/install a tool, or read it directly). This
//! module owns the payload type, the transient upload storage helpers, the
//! status-list query for the editor, and the conversion orchestration. The
//! worker (see `worker.rs`) drives it and writes the produced markdown back to
//! the traject's git corpus.

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
        SELECT source_id, name, source_type, gh_owner, gh_repo, gh_branch,
               gh_path, gh_ref, local_path, priority, auth_ref
        FROM traject_corpus_sources
        WHERE traject_id = $1 AND is_writable_own = TRUE
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

    let mut backend = create_backend(&source, token.as_deref())?;
    backend.ensure_ready().await?;

    let relative_path = Path::new("documents")
        .join(&payload.traject_ref)
        .join(&payload.target_path);
    backend.write_file(&relative_path, markdown).await?;

    let message = format!("document-convert: {}", payload.target_path);
    backend
        .persist(&WriteContext {
            message,
            author: None,
        })
        .await?;
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
    /// Failure reason (from `jobs.result->>'error'`), present when `status = failed`.
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// List the traject's document-convert jobs that are still relevant to show:
/// everything that is not `completed` (pending, processing, failed). A completed
/// job is represented by the actual `.md` in the documents list, so it drops out
/// of this view.
pub async fn list_traject_document_jobs(
    pool: &PgPool,
    traject_ref: &str,
) -> Result<Vec<TrajectJobView>> {
    let rows = sqlx::query_as::<_, TrajectJobView>(
        r#"
        SELECT id,
               payload->>'target_path' AS target_path,
               status,
               result->>'error'        AS error,
               created_at
        FROM jobs
        WHERE traject_ref = $1
          AND job_type = 'document_convert'
          AND status <> 'completed'
        ORDER BY created_at DESC
        "#,
    )
    .bind(traject_ref)
    .fetch_all(pool)
    .await?;
    Ok(rows)
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
            Some(input_file),
            work_dir,
            config,
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
         2. Decide on and carry out the best way to convert it to Markdown. You may use \
            any command-line tool you consider suitable — and install one if needed \
            (for example via `pip install --user`) — or read the document yourself and \
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
        let upload = Upload {
            filename: "report.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            bytes: b"%PDF-1.4 fake".to_vec(),
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
            filename: "report.pdf".to_string(),
            content_type: "application/pdf".to_string(),
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
}
