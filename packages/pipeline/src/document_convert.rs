//! Document-convert jobs: turn an uploaded document of any format into a clean
//! markdown werkdocument.
//!
//! Known formats (docx/office via `pandoc`, PDF via `pdftotext`) are converted
//! deterministically with a real tool — reliable, offline, and keeping the
//! untrusted document away from the Bash-enabled LLM. Any other format (the
//! upload boundary no longer gates on an allow-list) falls back to the LLM
//! agent subprocess that enrich uses (see
//! [`crate::enrich::run_llm_subprocess`]), which decides for itself how to
//! convert (pick a tool, or read it directly). This module owns the payload
//! type, the transient upload storage helpers, the status-list query for the
//! editor, and the conversion orchestration. The worker (see `worker.rs`) drives
//! it and delivers the produced markdown as a job-blob + review-taak
//! (`worker::finish_document_convert_task_job`); the editor commits it to the
//! traject repo namens de gebruiker after approval.
//!
//! Per het worker/traject-contract (zie de crate-doc in `lib.rs`) bevat deze
//! module bewust géén git-push-pad: een sessieloze worker schrijft nooit met
//! een server-token naar een traject-repo. De guard die dat afdwingt is
//! [`DocumentConvertPayload::require_task_delivery`].

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

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
    /// `"task"` ⇒ resultaat als job_blobs + review-taak, géén push (taak-flow;
    /// editor-api zet dit op elke upload). Elke andere waarde — inclusief
    /// afwezig, het pre-taken-mechanisme directe-push-gedrag — wordt door de
    /// worker terminaal geweigerd: zie [`Self::require_task_delivery`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver: Option<String>,
}

impl DocumentConvertPayload {
    /// Taak-flow: resultaat naar Postgres + taak i.p.v. push naar git.
    pub fn deliver_as_task(&self) -> bool {
        self.deliver.as_deref() == Some("task")
    }

    /// Contract-guard op jobniveau: een document-convert-job heeft altijd een
    /// traject-doel (`traject_id`), en per het worker/traject-contract (zie de
    /// crate-doc) mag een sessieloze worker nooit met een server-token naar
    /// een traject-repo pushen. Oplevering kan dus uitsluitend via de
    /// taak-flow: `deliver: "task"` mét een `requested_by` als assignee van de
    /// review-taak. Al het andere — het verwijderde directe-push-gedrag van
    /// jobs van vóór het taken-mechanisme incluis — hoort terminaal en luid te
    /// falen, vóór de (kostbare) conversie. Spiegel van de `law_convert`-gate
    /// in `worker.rs`.
    pub fn require_task_delivery(&self) -> Result<()> {
        if !self.deliver_as_task() || self.requested_by.is_none() {
            return Err(PipelineError::Worker(
                "document_convert vereist deliver=task met requested_by: een traject-write \
                 loopt altijd via een review-taak (editor-commit namens de gebruiker), nooit \
                 via een directe push vanuit de worker"
                    .to_string(),
            ));
        }
        Ok(())
    }
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
/// document_convert or law_convert job. This garbage-collects bytes orphaned
/// when a worker crashed mid-conversion and the generic orphan reaper failed
/// the job without running the type-specific [`delete_upload`]. Returns the
/// number of rows removed.
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
              WHERE j.job_type IN ('document_convert', 'law_convert')
                AND j.status IN ('pending', 'processing')
                AND j.payload->>'upload_id' = du.id::text
          )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
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
/// status UI.
///
/// * `include_failed = false` (taken-mechanisme, the editor-api's mode): only
///   still-active jobs (`pending`, `processing`). A completed job is
///   represented by the actual `.md` in the documents list; a terminally
///   failed job is not shown here at all - the uploader instead gets a
///   wegklikbare `job_failed` taak (see
///   `worker::process_next_document_convert_job`), so a failure no longer
///   lingers forever in this status block. `error` is always `None` in this
///   mode (the query excludes failed rows entirely).
/// * `include_failed = true` (pre-taken-mechanisme behaviour): everything not
///   `completed` (pending, processing, failed), with `error` populated from
///   `result->>'error'` for failed rows so an inline failure UI has
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

/// Cancel a not-yet-completed document-convert job for a traject — e.g. one the
/// user wants to kill because it has been stuck for hours. Deletes the job row
/// and its now-orphaned source upload. Scoped to the traject's own
/// document-convert jobs so a member can't cancel another traject's job by id.
/// Returns `true` when a job was removed, `false` when none matched (already
/// gone / completed / wrong traject) so the caller can stay idempotent.
pub async fn cancel_traject_document_job(
    pool: &PgPool,
    traject_ref: &str,
    job_id: Uuid,
) -> Result<bool> {
    let deleted: Option<(Option<Uuid>,)> = sqlx::query_as(
        r#"
        DELETE FROM jobs
        WHERE id = $1
          AND traject_ref = $2
          AND job_type = 'document_convert'
          AND status <> 'completed'
        RETURNING (payload->>'upload_id')::uuid
        "#,
    )
    .bind(job_id)
    .bind(traject_ref)
    .fetch_optional(pool)
    .await?;

    let Some((upload_id,)) = deleted else {
        return Ok(false);
    };
    // Best-effort clean-up of the orphaned source bytes; the job is already gone
    // (the user's intent) and `cleanup_orphaned_uploads` is the backstop, so a
    // failure here must not fail the cancel.
    if let Some(uid) = upload_id {
        let _ = delete_upload(pool, uid).await;
    }
    Ok(true)
}

/// Target `.md` paths of the traject's not-yet-completed document-convert jobs
/// (pending, processing AND failed). A failed job still shows a row under its
/// name, so reserving it too keeps a new upload from deriving the same name and
/// spawning a confusing duplicate. Completed jobs are excluded: their `.md` is a
/// real document, caught by the on-disk check.
/// Used by the upload handler to make the derived target collision-safe
/// against conversions that are enqueued but haven't committed their `.md` yet —
/// without this, two uploads that derive the same name (e.g. two `report.pdf`)
/// would both target `report.md` and the second conversion would overwrite the
/// first once both commit.
pub async fn reserved_target_paths(pool: &PgPool, traject_ref: &str) -> Result<Vec<String>> {
    let rows = sqlx::query_as::<_, (Option<String>,)>(
        r#"
        SELECT payload->>'target_path'
        FROM jobs
        WHERE traject_ref = $1
          AND job_type = 'document_convert'
          AND status <> 'completed'
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
pub(crate) fn extension_for(filename: &str, content_type: &str) -> String {
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

/// The deterministic command-line converter that handles a given extension.
/// `pandoc` reads the office/markup formats; `pdftotext` (poppler) reads PDF.
enum DeterministicTool {
    Pandoc,
    PdfToText,
}

/// The deterministic converter for `ext`, or `None` when no tool fits and the
/// agentic (LLM) fallback must run. Single source of truth for "which formats
/// take the fast, offline route" — both [`try_deterministic_convert`] and
/// [`has_deterministic_converter`] read it, so the routing decision and the
/// command dispatch can never drift apart.
fn deterministic_tool_for(ext: &str) -> Option<DeterministicTool> {
    match ext {
        // pandoc reads these natively and emits GitHub-flavored Markdown.
        "docx" | "odt" | "rtf" | "html" | "htm" | "epub" | "fb2" => Some(DeterministicTool::Pandoc),
        // pandoc cannot read PDF; poppler's pdftotext extracts the text layer.
        "pdf" => Some(DeterministicTool::PdfToText),
        _ => None,
    }
}

/// Whether a deterministic converter handles `ext`. Formats without one take the
/// agentic (LLM) fallback route instead. Public so the upload boundary and tests
/// can reason about which route a newly-accepted format will follow without
/// spawning a tool.
pub fn has_deterministic_converter(ext: &str) -> bool {
    deterministic_tool_for(ext).is_some()
}

/// Try to convert `input_file` to markdown at `output_path` with a deterministic
/// command-line tool, chosen by extension. Returns `Some(markdown)` on success,
/// or `None` when the format is not handled, the tool is absent, it exits
/// non-zero, or it produces empty output — every `None` case falls through to the
/// agentic converter, so a missing tool or an odd file never hard-fails here.
///
/// `pandoc` handles the office/markup formats; `pdftotext` (poppler) handles
/// PDF. Both are baked into the enrich-worker image.
pub(crate) async fn try_deterministic_convert(
    input_file: &Path,
    ext: &str,
    output_path: &Path,
) -> Option<String> {
    let mut cmd = match deterministic_tool_for(ext)? {
        DeterministicTool::Pandoc => {
            let mut c = tokio::process::Command::new("pandoc");
            c.arg(input_file)
                .arg("-t")
                .arg("gfm")
                .arg("--wrap=none")
                .arg("-o")
                .arg(output_path);
            c
        }
        DeterministicTool::PdfToText => {
            let mut c = tokio::process::Command::new("pdftotext");
            c.arg("-layout").arg(input_file).arg(output_path);
            c
        }
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
            deliver: None,
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
            deliver: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("provider").is_none());
    }

    #[test]
    fn payload_deliver_task_fields_roundtrip_and_backcompat() {
        // Oude payloads (zonder deliver-veld) moeten blijven deserialiseren —
        // maar hun directe-push-gedrag bestaat niet meer: de guard weigert ze
        // terminaal (zie `require_task_delivery_rejects_non_task_payloads`).
        let old = serde_json::json!({
            "upload_id": Uuid::nil(),
            "traject_id": Uuid::nil(),
            "traject_ref": "abcd1234",
            "target_path": "report.md",
        });
        let parsed: DocumentConvertPayload = serde_json::from_value(old).unwrap();
        assert!(parsed.deliver.is_none());
        assert!(!parsed.deliver_as_task());

        // Nieuwe payloads dragen het deliver-veld mee.
        let account = Uuid::new_v4();
        let new = DocumentConvertPayload {
            upload_id: Uuid::nil(),
            traject_id: Uuid::nil(),
            traject_ref: "abcd1234".to_string(),
            target_path: "report.md".to_string(),
            provider: None,
            requested_by: Some(account),
            deliver: Some("task".to_string()),
        };
        let roundtrip: DocumentConvertPayload =
            serde_json::from_value(serde_json::to_value(&new).unwrap()).unwrap();
        assert_eq!(roundtrip.requested_by, Some(account));
        assert!(roundtrip.deliver_as_task());
    }

    /// Basispayload met traject-doel voor de guard-tests hieronder.
    fn traject_payload(
        deliver: Option<&str>,
        requested_by: Option<Uuid>,
    ) -> DocumentConvertPayload {
        DocumentConvertPayload {
            upload_id: Uuid::nil(),
            traject_id: Uuid::new_v4(),
            traject_ref: "voorbeeld-abcd1234".to_string(),
            target_path: "report.md".to_string(),
            provider: None,
            requested_by,
            deliver: deliver.map(str::to_string),
        }
    }

    #[test]
    fn require_task_delivery_rejects_non_task_payloads() {
        // Het worker/traject-contract: een document-convert-job (altijd een
        // traject-doel) mag uitsluitend via de taak-flow opleveren. Zonder
        // `deliver: "task"` — het verwijderde pre-taken directe-push-gedrag —
        // moet de guard weigeren, wélke andere waarde er ook staat.
        let account = Uuid::new_v4();
        for deliver in [None, Some("push"), Some("Task"), Some("")] {
            let err = traject_payload(deliver, Some(account))
                .require_task_delivery()
                .unwrap_err();
            assert!(
                err.to_string().contains("deliver=task"),
                "deliver={deliver:?} moet met een duidelijke contractfout geweigerd worden, kreeg: {err}"
            );
        }
    }

    #[test]
    fn require_task_delivery_rejects_task_without_requested_by() {
        // Zonder aanvrager is er geen assignee voor de review-taak, dus ook
        // geen aflever-pad — net zo terminaal als een niet-task-payload.
        let err = traject_payload(Some("task"), None)
            .require_task_delivery()
            .unwrap_err();
        assert!(err.to_string().contains("requested_by"));
    }

    #[test]
    fn require_task_delivery_accepts_the_task_flow() {
        traject_payload(Some("task"), Some(Uuid::new_v4()))
            .require_task_delivery()
            .expect("taak-flow-payload met aanvrager is het enige geldige aflever-pad");
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

    #[test]
    fn routes_formats_between_deterministic_and_agentic() {
        // Formats a real tool handles take the fast, offline route. This covers
        // the newly-accepted office/markup formats (odt/rtf/html/epub/…) that the
        // upload boundary used to reject before the allow-list was dropped.
        for ext in ["docx", "odt", "rtf", "html", "htm", "epub", "fb2", "pdf"] {
            assert!(
                has_deterministic_converter(ext),
                "{ext} should take the deterministic route"
            );
        }
        // Everything else — images, spreadsheets, presentations, unknown/binary
        // junk — has no deterministic tool and falls back to the agentic
        // (enricher/LLM) converter, which returns a reviewable markdown task.
        for ext in ["png", "jpg", "xlsx", "pptx", "csv", "bin", "", "exe"] {
            assert!(
                !has_deterministic_converter(ext),
                "{ext} should take the agentic fallback route"
            );
        }
    }

    #[tokio::test]
    async fn convert_in_dir_uses_agentic_fallback_for_new_nonconvertible_format() {
        let dir = tempfile::tempdir().unwrap();
        // A `.pptx` is a format the upload boundary now accepts but no
        // deterministic tool here reads, so the agentic fallback (FakeConverter)
        // must run and produce the reviewable markdown.
        let upload = Upload {
            filename: "deck.pptx".to_string(),
            content_type:
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                    .to_string(),
            bytes: b"raw slide bytes".to_vec(),
        };
        let converter = FakeConverter {
            markdown: "# Deck\n\nSlide one.\n".to_string(),
        };
        let md = convert_in_dir(
            dir.path(),
            &upload,
            "deck.md",
            &EnrichConfig::for_test(crate::enrich::LlmProvider::Claude {
                path: "claude".into(),
                model: None,
            }),
            &converter,
        )
        .await
        .unwrap();
        assert_eq!(md, "# Deck\n\nSlide one.\n");
    }

    /// End-to-end deterministic route for a newly-accepted format (`.html`): when
    /// `pandoc` is present the converter is never consulted, proving the fast
    /// route is taken. Skipped when `pandoc` is absent (it lives in the
    /// enrich-worker image but not every test host), so the suite never depends
    /// on an external tool being installed.
    #[tokio::test]
    async fn convert_in_dir_uses_deterministic_route_for_new_html_format() {
        if tokio::process::Command::new("pandoc")
            .arg("--version")
            .output()
            .await
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            eprintln!("skipping: pandoc not installed");
            return;
        }

        struct PanickingConverter;
        #[async_trait::async_trait]
        impl DocumentConverter for PanickingConverter {
            async fn convert(
                &self,
                _input_file: &Path,
                _work_dir: &Path,
                _output_path: &Path,
                _config: &EnrichConfig,
            ) -> Result<()> {
                panic!("the agentic converter must not run for a deterministic format");
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let upload = Upload {
            filename: "page.html".to_string(),
            content_type: "text/html".to_string(),
            bytes: b"<h1>Titel</h1><p>Inhoud.</p>".to_vec(),
        };
        let md = convert_in_dir(
            dir.path(),
            &upload,
            "page.md",
            &EnrichConfig::for_test(crate::enrich::LlmProvider::Claude {
                path: "claude".into(),
                model: None,
            }),
            &PanickingConverter,
        )
        .await
        .unwrap();
        // pandoc turns the <h1> into an ATX heading.
        assert!(
            md.contains("Titel"),
            "expected converted markdown, got: {md}"
        );
    }
}
