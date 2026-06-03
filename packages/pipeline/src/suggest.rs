//! Editor-suggestion pipeline.
//!
//! When a user saves a law in the editor, the editor-api enqueues one suggest
//! job per kind. A worker spawns a headless LLM (`claude -p`) that runs a skill
//! against the saved law on the traject's own branch and produces *suggestions*
//! — it never commits to the law itself. The skill writes findings as JSON; we
//! convert those to a W3C Web Annotation sidecar (`creator: { name: "claude-…" }`)
//! anchored to the law text via a `TextQuoteSelector`, and commit that sidecar
//! to the traject branch. The editor reads it back and renders accept/reject UI.
//!
//! Two kinds:
//! - [`SuggestKind::Guidelines`] — check article text against the Aanwijzingen
//!   voor de regelgeving (the `law-aanwijzingen` skill).
//! - [`SuggestKind::MachineReadable`] — propose `machine_readable` sections (the
//!   `law-generate` pipeline), surfaced as suggestions instead of a direct commit.
//!
//! The subprocess/timeout/kill handling and provider selection live in
//! [`crate::llm`]; this module only builds the prompt, parses the skill output,
//! and shapes the annotation sidecar.

use std::path::{Path, PathBuf};
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::llm::{LlmInvocation, LlmProvider, LlmRunner};

/// Which suggestion to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestKind {
    /// Toets de wettekst tegen de Aanwijzingen voor de regelgeving.
    Guidelines,
    /// Genereer voorgestelde machine_readable-secties.
    MachineReadable,
}

impl SuggestKind {
    /// Short slug for log lines, branch hints, and sidecar naming.
    pub fn slug(self) -> &'static str {
        match self {
            SuggestKind::Guidelines => "guidelines",
            SuggestKind::MachineReadable => "machine_readable",
        }
    }

    /// W3C annotation `creator.name` for findings of this kind.
    fn creator_name(self) -> &'static str {
        match self {
            SuggestKind::Guidelines => "claude-aanwijzingen",
            SuggestKind::MachineReadable => "claude-machine-readable",
        }
    }
}

/// Payload for a suggest job, stored as JSON in the job queue.
///
/// `traject_branch` is the *resolved* git branch the editor-api saved the law
/// to (`traject/{slug}-{8hex}`). The worker cannot derive it from `traject_ref`
/// alone (it needs the traject name + id), so the editor-api passes it in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestPayload {
    pub law_id: String,
    /// Relative path to the law YAML within the repo.
    pub yaml_path: String,
    /// Traject reference (short id) the save belongs to. Used for the
    /// unique-active-job index and sidecar scoping.
    pub traject_ref: String,
    /// Resolved traject git branch to check out and commit the sidecar to.
    pub traject_branch: String,
    pub kind: SuggestKind,
    /// Optional single article to scope the suggestion to. None = whole law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article_number: Option<String>,
}

/// Configuration for suggestion execution. Editor suggestions use Claude
/// (`claude -p`) by default; the env vars mirror the enrich worker's.
#[derive(Debug, Clone)]
pub struct SuggestConfig {
    pub provider: LlmProvider,
    pub timeout: Duration,
}

impl SuggestConfig {
    pub fn from_env() -> Self {
        let timeout = std::env::var("SUGGEST_TIMEOUT_SECS")
            .or_else(|_| std::env::var("LLM_TIMEOUT_SECS"))
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);

        let provider = LlmProvider::Claude {
            path: std::env::var("CLAUDE_PATH")
                .or_else(|_| std::env::var("LLM_PATH"))
                .unwrap_or_else(|_| "claude".into())
                .into(),
            model: std::env::var("CLAUDE_MODEL")
                .or_else(|_| std::env::var("LLM_MODEL"))
                .ok(),
        };

        Self {
            provider,
            timeout: Duration::from_secs(timeout),
        }
    }
}

// ---------------------------------------------------------------------------
// Skill output contract (matches .claude/skills/law-aanwijzingen/SKILL.md)
// ---------------------------------------------------------------------------

/// One finding as written by the skill to its JSON output file.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillFinding {
    pub article_number: String,
    pub exact_quote: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub aanwijzing_nr: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    pub suggestion_text: String,
    #[serde(default)]
    pub proposed_replacement: Option<String>,
}

/// The skill's JSON output document.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillOutput {
    #[serde(default)]
    pub law_id: String,
    #[serde(default)]
    pub findings: Vec<SkillFinding>,
}

// ---------------------------------------------------------------------------
// W3C Web Annotation sidecar (matches schema/v0.5.2/annotation-schema.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationSidecar {
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    #[serde(rename = "type")]
    pub type_: String,
    pub motivation: String,
    pub creator: Creator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    pub workflow: String,
    pub target: Target,
    pub body: Vec<Body>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creator {
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub source: String,
    pub selector: TextQuoteSelector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextQuoteSelector {
    #[serde(rename = "type")]
    pub type_: String,
    pub exact: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
    pub purpose: String,
}

/// Convert skill findings into a W3C annotation sidecar.
///
/// Quotes with empty `exact` are dropped: a selector with no `exact` cannot
/// anchor and the schema requires a non-empty value. The skill emits
/// whitespace-normalised quotes; the engine's fuzzy resolver tier anchors them
/// despite the hard line-wraps in block-scalar law text (see the plan's
/// "Verankering" note), so no normalisation is needed here.
pub fn findings_to_sidecar(
    law_id: &str,
    kind: SuggestKind,
    findings: &[SkillFinding],
    created: Option<String>,
) -> AnnotationSidecar {
    // `editing` when there's a concrete replacement to apply, else `commenting`.
    let annotations = findings
        .iter()
        .filter(|f| !f.exact_quote.trim().is_empty())
        .map(|f| {
            let has_replacement = f
                .proposed_replacement
                .as_ref()
                .is_some_and(|r| !r.trim().is_empty());
            let motivation = if has_replacement {
                "editing"
            } else {
                "commenting"
            };

            // The body carries the human-readable suggestion. For machine_readable
            // suggestions the proposed YAML lives in suggestion_text already; for a
            // guideline finding with a replacement we append it so the editor can
            // offer "apply" without a second field.
            let mut value = f.suggestion_text.clone();
            if let Some(rep) = &f.proposed_replacement {
                if !rep.trim().is_empty() {
                    value.push_str("\n\nVoorgestelde tekst: ");
                    value.push_str(rep);
                }
            }
            if let Some(nr) = &f.aanwijzing_nr {
                if !nr.trim().is_empty() {
                    value = format!("[Aanwijzing {nr}] {value}");
                }
            }

            Annotation {
                type_: "Annotation".into(),
                motivation: motivation.into(),
                creator: Creator {
                    type_: "Agent".into(),
                    name: kind.creator_name().into(),
                },
                created: created.clone(),
                workflow: "open".into(),
                target: Target {
                    source: format!("regelrecht://{law_id}"),
                    selector: TextQuoteSelector {
                        type_: "TextQuoteSelector".into(),
                        exact: f.exact_quote.clone(),
                        prefix: f.prefix.clone(),
                        suffix: f.suffix.clone(),
                    },
                },
                body: vec![Body {
                    type_: "TextualBody".into(),
                    value,
                    purpose: motivation.into(),
                }],
            }
        })
        .collect();

    AnnotationSidecar { annotations }
}

/// Repo-relative path of the AI-suggestion sidecar for a law. Kept separate
/// from the human `annotations/{law_id}/annotations.yaml` so AI suggestions
/// never pollute curated notes and can be cleaned up independently.
pub fn suggestions_sidecar_path(law_id: &str) -> String {
    format!("annotations/{law_id}/suggestions.yaml")
}

/// Path the skill writes its JSON findings to, inside the repo checkout.
fn skill_output_path(repo_path: &Path, law_id: &str, kind: SuggestKind) -> PathBuf {
    repo_path.join(format!(".suggest-{}-{}.json", kind.slug(), law_id))
}

/// Create a `CorpusClient` checked out on the traject branch for a suggest job.
///
/// Unlike enrich (which works on `enrich/{provider}` and falls back to
/// `development`), suggestions run on the traject's own branch — the editor just
/// saved the law there, so the branch is guaranteed to exist. Sparse-checks out
/// the law directory, `features/` (the skills read example features), and the
/// law's `annotations/` directory so an existing sidecar can be overwritten in
/// place. A per-job checkout dir keeps concurrent suggest jobs isolated.
pub async fn create_suggest_corpus(
    base_config: &CorpusConfig,
    branch: &str,
    job_id: Uuid,
    yaml_path: &str,
    law_id: &str,
) -> Result<CorpusClient> {
    let mut config = base_config.clone();
    config.branch = branch.into();

    // Validate + strip legacy absolute prefixes before the path touches git,
    // exactly as the enrich path does (rejects `..`, absolute paths, and shell
    // metacharacters). Defense-in-depth: the value is server-derived today, but
    // the pipeline `/suggest` endpoint trusts the field blind.
    let normalized = crate::enrich::normalize_yaml_path(yaml_path)?;

    let law_dir = Path::new(&normalized)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|d| !d.is_empty());

    let mut sparse = vec!["features".to_string(), format!("annotations/{law_id}")];
    if let Some(dir) = law_dir {
        sparse.push(dir);
    }
    config.sparse_paths = Some(sparse);

    // Separate checkout dir per branch + job so concurrent suggest jobs (and the
    // enrich worker) never clobber each other's working tree.
    let dir_name = format!("suggest-{}-{}", branch.replace('/', "-"), job_id);
    let base_dir = config
        .repo_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("/tmp"));
    config.repo_path = base_dir.join(dir_name);

    let mut client = CorpusClient::new(config);
    client.ensure_repo().await?;
    client
        .checkout_from_branch(branch, &[normalized.as_str()])
        .await?;

    Ok(client)
}

/// Build the prompt that drives the skill for a given kind.
fn build_prompt(
    yaml_path: &str,
    output_path: &str,
    kind: SuggestKind,
    article_number: Option<&str>,
) -> String {
    let article_clause = match article_number {
        Some(nr) => format!("ARTICLE_NUMBER={nr}. "),
        None => String::new(),
    };
    match kind {
        SuggestKind::Guidelines => format!(
            r#"Voer de law-aanwijzingen skill uit. Lees .claude/skills/law-aanwijzingen/SKILL.md
en .claude/skills/law-aanwijzingen/reference.md en volg de instructies volledig.

LAW_YAML={yaml_path}. {article_clause}OUTPUT_PATH={output_path}.

Schrijf het JSON-resultaat met bevindingen naar OUTPUT_PATH. Wijzig de wet-YAML NIET.
Werk autonoom; stel geen vragen."#
        ),
        SuggestKind::MachineReadable => format!(
            r#"Je stelt machine_readable-secties VOOR voor een Nederlandse wet — je commit NIETS
en je wijzigt de wet-YAML NIET.

LAW_YAML={yaml_path}. {article_clause}

Lees .claude/skills/law-generate/SKILL.md, reference.md en examples.md. Bepaal voor elk
uitvoerbaar artikel de machine_readable-logica zoals de skill voorschrijft, maar schrijf
de YAML NIET naar de wet. Lever in plaats daarvan per artikel een voorstel als JSON naar
OUTPUT_PATH={output_path}, met exact dit contract:

{{"law_id": "<$id>", "findings": [
  {{"article_number": "<nr>", "exact_quote": "<eerste ~8 woorden van de artikeltekst, genormaliseerde witruimte>",
    "prefix": "", "suffix": "", "aanwijzing_nr": null, "severity": "suggestie",
    "suggestion_text": "Voorgestelde machine_readable (YAML):\n<de voorgestelde yaml>",
    "proposed_replacement": null}}
]}}

`exact_quote` is een letterlijke (witruimte-genormaliseerde) passage uit de artikeltekst
om de suggestie aan te verankeren. Werk autonoom; stel geen vragen."#
        ),
    }
}

/// Allowed tools for the suggestion run. Guidelines is read-only plus the
/// single Write for its JSON output; machine_readable needs the generate
/// skill's full toolset (it validates with Bash) but still must not commit.
fn allowed_tools(kind: SuggestKind) -> &'static str {
    match kind {
        SuggestKind::Guidelines => "Read,Grep,Glob,Write",
        SuggestKind::MachineReadable => "Read,Grep,Glob,Write,Bash",
    }
}

/// Result of a successful suggestion run.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestResult {
    pub law_id: String,
    pub kind: SuggestKind,
    pub findings_count: usize,
    /// Repo-relative path of the sidecar that was written.
    pub sidecar_path: String,
}

/// Execute a suggestion using the default process-based LLM runner.
pub async fn execute_suggest(
    payload: &SuggestPayload,
    repo_path: &Path,
    config: &SuggestConfig,
    created: Option<String>,
) -> Result<(SuggestResult, PathBuf)> {
    execute_suggest_with_runner(
        payload,
        repo_path,
        config,
        created,
        &crate::llm::ProcessLlmRunner,
    )
    .await
}

/// Execute a suggestion: run the LLM skill, read its JSON findings, convert them
/// to an annotation sidecar, and write the sidecar into the repo checkout.
///
/// Returns the result summary and the absolute path of the written sidecar (for
/// the caller to stage + commit to the traject branch). Accepts a `runner` so
/// tests can avoid spawning real processes.
pub async fn execute_suggest_with_runner(
    payload: &SuggestPayload,
    repo_path: &Path,
    config: &SuggestConfig,
    created: Option<String>,
    runner: &dyn LlmRunner,
) -> Result<(SuggestResult, PathBuf)> {
    // Normalize the same way the checkout did, so we look for the law at the
    // path that was actually materialized (and reject traversal/absolute paths).
    let normalized_path = crate::enrich::normalize_yaml_path(&payload.yaml_path)?;
    let yaml_abs = repo_path.join(&normalized_path);
    if !yaml_abs.exists() {
        return Err(PipelineError::Suggest(format!(
            "law YAML file not found: {}",
            yaml_abs.display()
        )));
    }

    let output_path = skill_output_path(repo_path, &payload.law_id, payload.kind);
    // Remove a stale output from a previous attempt so a silent LLM failure
    // can't make us read old findings.
    let _ = tokio::fs::remove_file(&output_path).await;

    let prompt = build_prompt(
        &normalized_path,
        &output_path.to_string_lossy(),
        payload.kind,
        payload.article_number.as_deref(),
    );

    tracing::info!(
        law_id = %payload.law_id,
        kind = payload.kind.slug(),
        traject = %payload.traject_ref,
        "starting suggestion run"
    );

    runner
        .run(&LlmInvocation {
            prompt: &prompt,
            repo_path,
            provider: &config.provider,
            timeout: config.timeout,
            allowed_tools: allowed_tools(payload.kind),
        })
        .await?;

    // Read and parse the skill's JSON output.
    let raw = tokio::fs::read_to_string(&output_path).await.map_err(|e| {
        PipelineError::Suggest(format!(
            "skill produced no output at {}: {e}",
            output_path.display()
        ))
    })?;
    let skill_output: SkillOutput = serde_json::from_str(&raw)
        .map_err(|e| PipelineError::Suggest(format!("skill output is not valid JSON: {e}")))?;

    let law_id = if skill_output.law_id.is_empty() {
        payload.law_id.clone()
    } else {
        skill_output.law_id.clone()
    };

    let sidecar = findings_to_sidecar(&law_id, payload.kind, &skill_output.findings, created);
    let findings_count = sidecar.annotations.len();

    // Write the sidecar into the repo checkout for the caller to commit.
    let sidecar_rel = suggestions_sidecar_path(&payload.law_id);
    let sidecar_abs = repo_path.join(&sidecar_rel);
    if let Some(parent) = sidecar_abs.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let sidecar_yaml = serde_yaml_ng::to_string(&sidecar)?;
    tokio::fs::write(&sidecar_abs, &sidecar_yaml).await?;

    tracing::info!(
        law_id = %payload.law_id,
        kind = payload.kind.slug(),
        findings = findings_count,
        "suggestion run completed"
    );

    let result = SuggestResult {
        law_id: payload.law_id.clone(),
        kind: payload.kind,
        findings_count,
        sidecar_path: sidecar_rel,
    };

    // Clean up the transient skill output so it never gets staged.
    let _ = tokio::fs::remove_file(&output_path).await;

    Ok((result, sidecar_abs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_payload_serde_roundtrip() {
        let payload = SuggestPayload {
            law_id: "zorgtoeslagwet".into(),
            yaml_path: "regulation/nl/wet/zorgtoeslag/2025-01-01.yaml".into(),
            traject_ref: "abc123".into(),
            traject_branch: "traject/tarief-2025-3f4a8b2c".into(),
            kind: SuggestKind::Guidelines,
            article_number: Some("2".into()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"kind\":\"guidelines\""));
        let back: SuggestPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, SuggestKind::Guidelines);
        assert_eq!(back.traject_branch, "traject/tarief-2025-3f4a8b2c");
    }

    #[test]
    fn payload_omits_none_article() {
        let payload = SuggestPayload {
            law_id: "x".into(),
            yaml_path: "y.yaml".into(),
            traject_ref: "t".into(),
            traject_branch: "traject/x".into(),
            kind: SuggestKind::MachineReadable,
            article_number: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("article_number"));
        assert!(json.contains("\"kind\":\"machine_readable\""));
    }

    #[test]
    fn findings_become_annotations_with_agent_creator() {
        let findings = vec![SkillFinding {
            article_number: "2".into(),
            exact_quote: "de verzekerde".into(),
            prefix: "heeft ".into(),
            suffix: " aanspraak".into(),
            aanwijzing_nr: Some("3.56".into()),
            severity: Some("suggestie".into()),
            suggestion_text: "Gebruik consistente terminologie.".into(),
            proposed_replacement: Some("de aanvrager".into()),
        }];
        let sidecar = findings_to_sidecar(
            "zorgtoeslagwet",
            SuggestKind::Guidelines,
            &findings,
            Some("2026-06-03T00:00:00Z".into()),
        );
        assert_eq!(sidecar.annotations.len(), 1);
        let a = &sidecar.annotations[0];
        assert_eq!(a.creator.name, "claude-aanwijzingen");
        assert_eq!(a.creator.type_, "Agent");
        // Has a replacement -> editing motivation.
        assert_eq!(a.motivation, "editing");
        assert_eq!(a.target.source, "regelrecht://zorgtoeslagwet");
        assert_eq!(a.target.selector.exact, "de verzekerde");
        assert_eq!(a.workflow, "open");
        // Body carries aanwijzing nr + replacement.
        assert!(a.body[0].value.contains("Aanwijzing 3.56"));
        assert!(a.body[0].value.contains("de aanvrager"));
    }

    #[test]
    fn finding_without_replacement_is_commenting() {
        let findings = vec![SkillFinding {
            article_number: "1".into(),
            exact_quote: "iets".into(),
            prefix: String::new(),
            suffix: String::new(),
            aanwijzing_nr: None,
            severity: None,
            suggestion_text: "Let op.".into(),
            proposed_replacement: None,
        }];
        let sidecar = findings_to_sidecar("x", SuggestKind::Guidelines, &findings, None);
        assert_eq!(sidecar.annotations[0].motivation, "commenting");
    }

    #[test]
    fn empty_exact_quote_is_dropped() {
        let findings = vec![SkillFinding {
            article_number: "1".into(),
            exact_quote: "   ".into(),
            prefix: String::new(),
            suffix: String::new(),
            aanwijzing_nr: None,
            severity: None,
            suggestion_text: "x".into(),
            proposed_replacement: None,
        }];
        let sidecar = findings_to_sidecar("x", SuggestKind::Guidelines, &findings, None);
        assert!(sidecar.annotations.is_empty());
    }

    #[test]
    fn sidecar_serializes_to_valid_w3c_shape() {
        let findings = vec![SkillFinding {
            article_number: "2".into(),
            exact_quote: "de verzekerde".into(),
            prefix: String::new(),
            suffix: String::new(),
            aanwijzing_nr: None,
            severity: None,
            suggestion_text: "x".into(),
            proposed_replacement: None,
        }];
        let sidecar = findings_to_sidecar("law", SuggestKind::Guidelines, &findings, None);
        let yaml = serde_yaml_ng::to_string(&sidecar).unwrap();
        assert!(yaml.contains("type: Annotation"));
        assert!(yaml.contains("type: TextQuoteSelector"));
        assert!(yaml.contains("exact: de verzekerde"));
        // Empty prefix/suffix are skipped.
        assert!(!yaml.contains("prefix:"));
    }

    #[test]
    fn suggestions_sidecar_path_is_separate_from_notes() {
        assert_eq!(
            suggestions_sidecar_path("zorgtoeslagwet"),
            "annotations/zorgtoeslagwet/suggestions.yaml"
        );
    }

    #[test]
    fn build_prompt_references_skill_and_output() {
        let p = build_prompt(
            "law.yaml",
            "/tmp/out.json",
            SuggestKind::Guidelines,
            Some("2"),
        );
        assert!(p.contains("law-aanwijzingen/SKILL.md"));
        assert!(p.contains("ARTICLE_NUMBER=2"));
        assert!(p.contains("/tmp/out.json"));
        assert!(p.contains("LAW_YAML=law.yaml"));

        let m = build_prompt(
            "law.yaml",
            "/tmp/out.json",
            SuggestKind::MachineReadable,
            None,
        );
        assert!(m.contains("law-generate/SKILL.md"));
        assert!(!m.contains("ARTICLE_NUMBER="));
    }

    /// Fake runner that writes a fixed skill output JSON, simulating the LLM.
    struct FakeSkillRunner {
        json: String,
    }

    #[async_trait::async_trait]
    impl LlmRunner for FakeSkillRunner {
        async fn run(
            &self,
            inv: &LlmInvocation<'_>,
        ) -> std::result::Result<(), crate::llm::LlmError> {
            // The prompt contains `OUTPUT_PATH=<path>`; pull out the path and write
            // the fixed findings there, mimicking what the real skill would do.
            let marker = "OUTPUT_PATH=";
            let start = inv.prompt.find(marker).unwrap() + marker.len();
            let path = inv.prompt[start..]
                .split_whitespace()
                .next()
                .unwrap()
                .trim_end_matches('.');
            std::fs::write(path, &self.json).unwrap();
            Ok(())
        }
    }

    #[tokio::test]
    async fn execute_suggest_writes_sidecar_from_findings() {
        let dir = tempfile::tempdir().unwrap();
        let law_rel = "regulation/nl/wet/test/2025-01-01.yaml";
        let law_abs = dir.path().join(law_rel);
        tokio::fs::create_dir_all(law_abs.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&law_abs, "articles: []\n").await.unwrap();

        let json = r#"{"law_id":"testwet","findings":[
            {"article_number":"2","exact_quote":"de verzekerde","prefix":"","suffix":"",
             "aanwijzing_nr":null,"severity":"suggestie","suggestion_text":"Let op."}
        ]}"#;

        let payload = SuggestPayload {
            law_id: "testwet".into(),
            yaml_path: law_rel.into(),
            traject_ref: "t1".into(),
            traject_branch: "traject/test".into(),
            kind: SuggestKind::Guidelines,
            article_number: Some("2".into()),
        };
        let config = SuggestConfig {
            provider: LlmProvider::Claude {
                path: "fake".into(),
                model: None,
            },
            timeout: Duration::from_secs(60),
        };

        let runner = FakeSkillRunner {
            json: json.to_string(),
        };
        let (result, sidecar_abs) =
            execute_suggest_with_runner(&payload, dir.path(), &config, None, &runner)
                .await
                .unwrap();

        assert_eq!(result.findings_count, 1);
        assert_eq!(result.sidecar_path, "annotations/testwet/suggestions.yaml");
        assert!(sidecar_abs.exists());

        let written = tokio::fs::read_to_string(&sidecar_abs).await.unwrap();
        assert!(written.contains("claude-aanwijzingen"));
        assert!(written.contains("regelrecht://testwet"));

        // The transient skill output JSON must be cleaned up.
        let out = skill_output_path(dir.path(), "testwet", SuggestKind::Guidelines);
        assert!(!out.exists());
    }

    #[tokio::test]
    async fn execute_suggest_errors_when_law_missing() {
        let dir = tempfile::tempdir().unwrap();
        let payload = SuggestPayload {
            law_id: "x".into(),
            yaml_path: "nope.yaml".into(),
            traject_ref: "t".into(),
            traject_branch: "traject/x".into(),
            kind: SuggestKind::Guidelines,
            article_number: None,
        };
        let config = SuggestConfig {
            provider: LlmProvider::Claude {
                path: "fake".into(),
                model: None,
            },
            timeout: Duration::from_secs(60),
        };
        let runner = FakeSkillRunner { json: "{}".into() };
        let err = execute_suggest_with_runner(&payload, dir.path(), &config, None, &runner)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("law YAML file not found"));
    }
}
