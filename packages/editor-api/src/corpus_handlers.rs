use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Extension, Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_sessions::Session;

use regelrecht_auth::handlers::{
    SESSION_KEY_EMAIL, SESSION_KEY_EMAIL_VERIFIED, SESSION_KEY_NAME, SESSION_KEY_SUB,
};
use regelrecht_corpus::annotation_schema::{
    append_notes_to_sidecar, first_note_not_targeting_law, parse_and_validate_annotation_yaml,
    validate_annotation_doc, AppendOutcome,
};
use regelrecht_corpus::backend::{EditorUser, PersistOutcome, RepoBackend, WriteContext};
use regelrecht_corpus::dto::{build_source_summaries, PaginationParams, SourceSummary};
use regelrecht_corpus::source_map::{
    collect_law_outputs, extract_law_id, validate_yaml_syntax, LoadedLaw,
};
use regelrecht_corpus::timing;
use regelrecht_corpus::CorpusError;

use crate::accounts::AccountRecord;
use crate::github_oauth;
use crate::state::{AppState, CorpusState};
use crate::traject_corpus::{ScenarioListEntry, TrajectCorpus, TrajectCorpusError};
use crate::trajects::resolve_traject_ref;
use crate::user_notes;

/// Response body for a successful save.
///
/// `pr` is populated when the traject's writable backend opened (or
/// updated) a pull request for this save — currently a future enhancement;
/// the default traject backend commits straight to the configured branch
/// and returns `None`.
#[derive(Debug, Serialize)]
pub struct SaveResponse {
    pub pr: Option<SavePrInfo>,
    /// `true` when the **sidecar/git side** of a notes save was a no-op:
    /// nothing was written/committed to the branch — either every
    /// submitted public note was already present, or the save carried no
    /// public notes at all. It says nothing about personal notes: a save
    /// can be `no_change: true` AND have `personal_saved > 0`. Clients
    /// must gate success signals on the combination, not on `!no_change`
    /// alone. Lets the frontend show "al opgeslagen" and keep any
    /// existing PR badge instead of treating a PR-less 200 as a lost
    /// save (review finding NEW-2). Always `false` for law/scenario
    /// saves; those clients ignore it.
    pub no_change: bool,
    /// How many notes in an annotations save were routed to the caller's
    /// personal store (`regelrecht:visibility: personal`) instead of the
    /// git sidecar. Only set by `save_annotations`; omitted elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personal_saved: Option<usize>,
    /// The new ETag after a law/scenario save — clients keep it for the
    /// next PUT's `If-Match` header (same optimistic-concurrency chain
    /// as [`SaveDocumentResponse::etag`]). `None` for handlers that
    /// don't participate in If-Match concurrency (annotations, deletes),
    /// and omitted from the JSON in that case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SavePrInfo {
    pub url: String,
    pub number: u64,
}

/// A law entry with source provenance.
#[derive(Debug, Serialize)]
pub struct CorpusLawEntry {
    pub law_id: String,
    pub name: Option<String>,
    /// Resolved human-readable name. For laws with a literal `name:` field
    /// this equals `name`. For laws with `name: '#output_ref'` this is the
    /// resolved value from the matching action output. Falls back to `None`
    /// when the reference cannot be resolved.
    pub display_name: Option<String>,
    pub source_id: String,
    pub source_name: String,
    /// Priority of the providing source (lower = higher priority). The
    /// search UI groups results by source and orders the groups by this
    /// value, so the traject's own writable repo (priority 0) sorts above
    /// the seeded central corpus.
    pub source_priority: u32,
}

/// A parameter required by the execution block that declares an output.
#[derive(Debug, Serialize)]
pub struct LawParamEntry {
    pub name: String,
    pub param_type: String,
}

/// An output entry from a law's machine_readable.execution.output.
#[derive(Debug, Serialize)]
pub struct LawOutputEntry {
    pub name: String,
    pub output_type: String,
    pub article_number: String,
    /// Parameters required by the article's execution block. The caller
    /// must supply these via `source.parameters` when referencing this output.
    pub parameters: Vec<LawParamEntry>,
}

/// Read-time scope for the corpus endpoints. Either the per-traject
/// corpus (membership-checked) or the global corpus state under a read
/// lock (anonymous / no-traject browsing). The variant is determined by
/// the route — `/api/corpus/...` always lands in `Global`,
/// `/api/trajects/{tid}/corpus/...` always lands in `Traject` — so a
/// single handler body can serve both via the route-specific extractor
/// that produced the scope.
enum ReadScope {
    Traject(Arc<TrajectCorpus>),
    Global(tokio::sync::OwnedRwLockReadGuard<CorpusState>),
}

impl ReadScope {
    fn corpus(&self) -> &CorpusState {
        match self {
            ReadScope::Traject(t) => &t.corpus,
            ReadScope::Global(g) => g,
        }
    }

    /// Look up a law's YAML content within the active scope. For a
    /// traject, the read-your-writes overlay (populated by `save_law`)
    /// takes precedence over the source_map snapshot, so a save +
    /// re-open in the same traject returns the new content without a
    /// full source_map rebuild.
    async fn law_yaml(
        &self,
        law_id: &str,
    ) -> Result<Option<String>, regelrecht_corpus::error::CorpusError> {
        match self {
            ReadScope::Traject(t) => t.law_yaml(law_id).await,
            // The global corpus is fully loaded up front, so there's no lazy
            // fetch that could fail — a miss is always a genuine miss.
            ReadScope::Global(g) => {
                Ok(g.source_map.get_law(law_id).map(|l| l.yaml_content.clone()))
            }
        }
    }

    /// Look up *all* versions of a law's YAML content within the active scope.
    /// Mirrors [`Self::law_yaml`] but returns the full version set (newest-first)
    /// so the scenario loader can hand them all to the engine. An unknown law is
    /// an empty vec, not an error.
    async fn law_yaml_versions(
        &self,
        law_id: &str,
    ) -> Result<Vec<String>, regelrecht_corpus::error::CorpusError> {
        match self {
            ReadScope::Traject(t) => t.law_yaml_versions(law_id).await,
            // The global corpus is fully loaded up front (like `law_yaml`), so
            // eagerly-loaded (local-source) bodies are present; filter any
            // metadata-only sentinel so the loader never receives an empty YAML
            // string. Asymmetry to note: GitHub-backed versions are metadata-
            // only in the global view (never lazily fetched here, just as
            // `law_yaml`'s Global branch can't fetch them), so they are omitted
            // from the global endpoint. The traject-scoped path *does* lazily
            // fetch them — and scenario execution always runs under a traject,
            // so the omission never affects the dependency loader.
            ReadScope::Global(g) => Ok(g
                .source_map
                .get_law_versions(law_id)
                .map(|vs| {
                    vs.iter()
                        .map(|l| l.yaml_content.clone())
                        .filter(|y| !y.is_empty())
                        .collect()
                })
                .unwrap_or_default()),
        }
    }
}

/// Read a law's YAML within a scope, mapping the outcome to an HTTP error:
/// a backend failure (lazy fetch threw) becomes 502 "failed to load" so it's
/// distinguishable from a genuine 404 miss; the error is logged for operators.
async fn read_law_yaml(scope: &ReadScope, law_id: &str) -> Result<String, (StatusCode, String)> {
    scope
        .law_yaml(law_id)
        .await
        .map_err(|e| {
            tracing::warn!(law_id = %law_id, error = %e, "failed to load law body");
            (
                StatusCode::BAD_GATEWAY,
                format!("Kon wet '{law_id}' niet laden"),
            )
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{law_id}' not found")))
}

/// Global read scope: no traject, no overlay. Used by every public
/// `/api/corpus/...` GET — no membership check, no DB hit.
async fn global_scope(state: &AppState) -> ReadScope {
    ReadScope::Global(state.corpus.clone().read_owned().await)
}

/// Traject read scope: looks up the per-traject corpus, verifying the
/// caller's membership against `traject_members`. Used by both
/// `/api/trajects/{ref}/corpus/...` reads and the write handlers (writes
/// also need the membership check before touching the branch).
///
/// The `traject_ref` is the URL form `{slug}-{8hex}` — resolved to a
/// UUID before the membership query (see `resolve_traject_ref`). Returns
/// 403 when the caller is not a member, 404 when the ref doesn't match
/// any known traject, 400 when the ref is malformed.
async fn require_traject_scope(
    state: &AppState,
    session: &Session,
    traject_ref: &str,
) -> Result<ReadScope, (StatusCode, String)> {
    let traject = require_traject_corpus_from_ref(state, session, traject_ref).await?;
    Ok(ReadScope::Traject(traject))
}

/// GET /api/sources — list all registered corpus sources (global).
pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<SourceSummary>>, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    Ok(Json(list_sources_in_scope(&scope)))
}

/// GET /api/trajects/{traject_id}/sources — same shape as `/api/sources`,
/// but routed through the traject's per-source backends.
pub async fn list_traject_sources(
    State(state): State<AppState>,
    session: Session,
    Path(traject_ref): Path<String>,
) -> Result<Json<Vec<SourceSummary>>, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    Ok(Json(list_sources_in_scope(&scope)))
}

fn list_sources_in_scope(scope: &ReadScope) -> Vec<SourceSummary> {
    let corpus = scope.corpus();
    build_source_summaries(&corpus.registry, &corpus.source_map)
}

/// GET /api/corpus/laws — list loaded laws with source metadata (global view).
///
/// Supports pagination via `?offset=0&limit=100`. Default limit is 100,
/// maximum is 1000.
pub async fn list_corpus_laws(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CorpusLawEntry>>, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    Ok(Json(list_corpus_laws_in_scope(&scope, params)))
}

/// GET /api/trajects/{traject_id}/corpus/laws — same as `/api/corpus/laws`
/// but the source_map comes from the traject's per-source backends.
pub async fn list_traject_corpus_laws(
    State(state): State<AppState>,
    session: Session,
    Path(traject_ref): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<CorpusLawEntry>>, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    Ok(Json(list_corpus_laws_in_scope(&scope, params)))
}

fn list_corpus_laws_in_scope(scope: &ReadScope, params: PaginationParams) -> Vec<CorpusLawEntry> {
    let corpus = scope.corpus();
    let limit = params.effective_limit();

    // Exact-id filter (highest precedence). The library sidebar sends the
    // user's favorites + traject edits as `?ids=a,b,c` so it resolves metadata
    // for just those laws — it never has to load the whole corpus and filter
    // client-side, and a favorite that sorts past any page cap still resolves.
    let id_filter: Option<std::collections::HashSet<&str>> = params
        .ids
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .collect()
        })
        .filter(|s: &std::collections::HashSet<&str>| !s.is_empty());

    // Optional server-side search. The corpus index can hold thousands of
    // laws, so the editor sends `?q=` and we filter here rather than shipping
    // every law to the browser to filter client-side. Underscores in the
    // `law_id` are treated as spaces so "wet op de zorgtoeslag" matches
    // `wet_op_de_zorgtoeslag`; loaded laws also match on their `name`.
    let needle = params
        .q
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    let mut entries: Vec<CorpusLawEntry> = corpus
        .source_map
        .laws()
        .filter(|law| {
            if let Some(ids) = &id_filter {
                return ids.contains(law.law_id.as_str());
            }
            match &needle {
                None => true,
                Some(n) => {
                    law.law_id.replace('_', " ").to_lowercase().contains(n)
                        || law
                            .name
                            .as_deref()
                            .is_some_and(|name| name.to_lowercase().contains(n))
                }
            }
        })
        .map(|law| CorpusLawEntry {
            law_id: law.law_id.clone(),
            name: law.name.clone(),
            // Precomputed at index/load time (see `LoadedLaw::display_name`)
            // so the unfiltered list doesn't re-scan every law body — let
            // alone fully re-parse the `name: '#ref'` ones — per page load.
            display_name: law.display_name.clone(),
            source_id: law.source_id.clone(),
            source_name: law.source_name.clone(),
            source_priority: law.source_priority,
        })
        .collect();

    if id_filter.is_some() || needle.is_some() {
        // Filtered (by ids or search): order so the grouped UI gets the
        // highest-priority sources first (the traject's own repo before the
        // central corpus), and the result cap can't starve a high-priority
        // source. No offset paging — return the matching set.
        entries.sort_by(|a, b| {
            a.source_priority
                .cmp(&b.source_priority)
                .then_with(|| a.law_id.cmp(&b.law_id))
        });
        entries.into_iter().take(limit).collect()
    } else {
        entries.sort_by(|a, b| a.law_id.cmp(&b.law_id));
        entries
            .into_iter()
            .skip(params.offset)
            .take(limit)
            .collect()
    }
}

/// GET /api/trajects/{traject_ref}/corpus/changed-laws — law ids that have
/// been edited in this traject (the diff of the traject branch against its
/// base on the writable-own source, mapped back to law ids).
///
/// Feeds the library sidebar's "Bewerkt in dit traject" section. Returns an
/// empty array — not an error — when nothing has been saved yet (the
/// traject branch doesn't exist), so the frontend simply hides the section.
///
/// Goes through `require_traject_corpus_from_ref` (not `require_traject_scope`)
/// because it needs the `TrajectCorpus` directly to reach the writable-own
/// backend; the membership re-check is identical either way.
pub async fn list_traject_changed_laws(
    State(state): State<AppState>,
    session: Session,
    Path(traject_ref): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let ids = traject.changed_law_ids().await.map_err(|e| {
        // A GitHub round-trip failure (token, transport, unexpected status)
        // is upstream — surface it as 502 with a generic message; details
        // are logged for operators.
        tracing::warn!(traject_ref = %traject_ref, error = %e, "changed-laws diff failed");
        (
            StatusCode::BAD_GATEWAY,
            "Kon de gewijzigde wetten van dit traject niet ophalen".to_string(),
        )
    })?;
    Ok(Json(ids))
}

type YamlResponse = (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    String,
);

/// Raw-content response carrying an `ETag` next to the `Content-Type`.
/// Used by the law and scenario GETs so clients can echo the ETag back
/// as `If-Match` on the corresponding save (same optimistic-concurrency
/// chain as the document endpoints).
type EtaggedContentResponse = (StatusCode, [(axum::http::HeaderName, String); 2], String);

/// Build a `200 OK` raw-content response with `Content-Type` and an
/// `ETag` computed over the body (see [`document_etag`]).
fn etagged_content_response(content_type: &'static str, content: String) -> EtaggedContentResponse {
    let etag = document_etag(&content);
    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type.to_string()),
            (axum::http::header::ETAG, etag),
        ],
        content,
    )
}

/// GET /api/corpus/laws/{law_id} — return raw YAML content for a specific law (global view).
pub async fn get_corpus_law(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<EtaggedContentResponse, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    get_corpus_law_in_scope(&scope, &law_id).await
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id} — same as the
/// global GET but with the traject's read-your-writes overlay applied.
pub async fn get_traject_corpus_law(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<EtaggedContentResponse, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    get_corpus_law_in_scope(&scope, &law_id).await
}

async fn get_corpus_law_in_scope(
    scope: &ReadScope,
    law_id: &str,
) -> Result<EtaggedContentResponse, (StatusCode, String)> {
    let yaml = read_law_yaml(scope, law_id).await?;
    Ok(etagged_content_response("text/yaml; charset=utf-8", yaml))
}

/// GET /api/corpus/laws/{law_id}/versions — every version's raw YAML, newest
/// first (global view). The scenario dependency loader feeds all of these to
/// the engine so its date-aware `select_in` can pick the version in force on
/// the scenario's calculation date (rather than the single "best for today"
/// entry that the plain `/laws/{id}` GET returns). An unknown law is an empty
/// array, not a 404 — the loader treats that as a missing dependency.
pub async fn get_corpus_law_versions(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    get_corpus_law_versions_in_scope(&scope, &law_id).await
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id}/versions — same as the
/// global GET but routed through the traject's per-source backends and
/// read-your-writes overlay.
pub async fn get_traject_corpus_law_versions(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    get_corpus_law_versions_in_scope(&scope, &law_id).await
}

async fn get_corpus_law_versions_in_scope(
    scope: &ReadScope,
    law_id: &str,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let versions = scope.law_yaml_versions(law_id).await.map_err(|e| {
        // A backend failure (lazy fetch threw) is a 502, distinguishable from
        // a genuine "no such law" empty array; logged for operators.
        tracing::warn!(law_id = %law_id, error = %e, "failed to load law versions");
        (
            StatusCode::BAD_GATEWAY,
            format!("Kon versies van wet '{law_id}' niet laden"),
        )
    })?;
    Ok(Json(versions))
}

/// GET /api/corpus/laws/{law_id}/outputs — list all outputs declared across articles (global view).
pub async fn list_law_outputs(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<LawOutputEntry>>, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    list_law_outputs_in_scope(&scope, &law_id).await
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id}/outputs — same as
/// global but with the traject overlay.
pub async fn list_traject_law_outputs(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<Json<Vec<LawOutputEntry>>, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    list_law_outputs_in_scope(&scope, &law_id).await
}

async fn list_law_outputs_in_scope(
    scope: &ReadScope,
    law_id: &str,
) -> Result<Json<Vec<LawOutputEntry>>, (StatusCode, String)> {
    let yaml = read_law_yaml(scope, law_id).await?;

    let outputs: Vec<LawOutputEntry> = collect_law_outputs(&yaml)
        .into_iter()
        .map(|out| LawOutputEntry {
            name: out.name,
            output_type: out.output_type,
            article_number: out.article_number,
            parameters: out
                .parameters
                .into_iter()
                .map(|(name, param_type)| LawParamEntry { name, param_type })
                .collect(),
        })
        .collect();

    Ok(Json(outputs))
}

/// GET /api/corpus/laws/{law_id}/implementors — law ids whose articles
/// declare `implements` an `open_term` of `{law_id}` (the IoC reverse link).
///
/// Computed server-side over the already-in-memory corpus. This exists so
/// the editor's scenario dependency loader can find implementing regulations
/// with a single request instead of fetching and parsing every law in the
/// corpus over HTTP — which, for a traject federating the full central
/// corpus, was hundreds of round-trips per scenario-panel open.
pub async fn list_law_implementors(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    Ok(Json(implementors_in_scope(&scope, &law_id).await))
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id}/implementors — same
/// as the global view but resolved through the traject's federated corpus.
pub async fn list_traject_law_implementors(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    Ok(Json(implementors_in_scope(&scope, &law_id).await))
}

async fn implementors_in_scope(scope: &ReadScope, law_id: &str) -> Vec<String> {
    match scope {
        // Traject: federated laws are metadata-only until first read, so
        // the lookup goes through the per-snapshot implements index
        // (built once per snapshot — the only O(corpus) body scan — and
        // invalidated with it; see `TrajectCorpus::implementors_of`).
        // Laws whose body fetch failed during the scan are *reported*
        // rather than silently passed off as "no implementors"; the
        // response shape (a bare id array) has no room for a non-breaking
        // partiality flag. The index build already warns once (with the
        // skipped/indexed counts) when it records fetch failures, so the
        // per-request signal here is debug-only — a warn per lookup would
        // re-log the same incident on every panel open until the next
        // rebuild self-heals it.
        ReadScope::Traject(traject) => {
            let result = traject.implementors_of(law_id).await;
            if result.skipped_count > 0 {
                tracing::debug!(
                    law_id = %law_id,
                    skipped = result.skipped_count,
                    found = result.implementors.len(),
                    "implements index was built with unreadable law bodies (index-wide count, not specific to this law); this lookup may therefore be incomplete"
                );
            }
            result.implementors
        }
        // Global: bodies are fully loaded up front and each law's
        // `implements` list was parsed once at load time, so this is an
        // in-memory reverse scan — no per-request YAML parsing.
        ReadScope::Global(corpus) => {
            let mut out: Vec<String> = corpus
                .source_map
                .laws()
                .filter(|law| law.law_id != law_id && law.implements.iter().any(|i| i == law_id))
                .map(|law| law.law_id.clone())
                .collect();
            out.sort();
            out
        }
    }
}

/// Law ids a scenario file evaluates, extracted from its execution steps.
///
/// A target is the law named in an `I evaluate "<output>" of "<law_id>"`
/// step. The Gherkin keyword may be `When`, `Then`, `And`, `But` or `*` —
/// the frontend step matcher (`frontend/src/gherkin/steps.js`) matches step
/// text without its keyword, so all of these run as execution steps.
/// `Given law "…" is loaded` lines are dependencies, not targets.
/// Deduplicated, order of first occurrence preserved.
fn extract_target_law_ids(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        let Some(step) = ["When ", "Then ", "And ", "But ", "* "]
            .iter()
            .find_map(|kw| trimmed.strip_prefix(kw))
        else {
            continue;
        };
        let Some(rest) = step.trim_start().strip_prefix("I evaluate \"") else {
            continue;
        };
        // rest = `<output>" of "<law_id>"…`
        let Some((_, after_output)) = rest.split_once('"') else {
            continue;
        };
        let Some(rest) = after_output.strip_prefix(" of \"") else {
            continue;
        };
        let Some((law_id, _)) = rest.split_once('"') else {
            continue;
        };
        if !law_id.is_empty() && !out.iter().any(|l| l == law_id) {
            out.push(law_id.to_string());
        }
    }
    out
}

/// A scenario file entry.
#[derive(Debug, Serialize)]
pub struct ScenarioEntry {
    pub filename: String,
    /// Law ids this scenario file evaluates (from its
    /// `I evaluate … of "…"` steps). Empty when the file has no parseable
    /// execution step yet (work in progress) or could not be read.
    pub target_law_ids: Vec<String>,
}

/// GET /api/corpus/laws/{law_id}/scenarios — list available scenario files (global view).
pub async fn list_scenarios(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<Json<Vec<ScenarioEntry>>, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    list_scenarios_in_scope(&scope, &law_id).await
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id}/scenarios — same as
/// global but routed through the traject's backends, so a freshly saved
/// scenario is visible without a corpus reload.
pub async fn list_traject_scenarios(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<Json<Vec<ScenarioEntry>>, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    list_scenarios_in_scope(&scope, &law_id).await
}

async fn list_scenarios_in_scope(
    scope: &ReadScope,
    law_id: &str,
) -> Result<Json<Vec<ScenarioEntry>>, (StatusCode, String)> {
    // Surface real backend errors (permissions, broken git checkout, …) as
    // 500 instead of swallowing them as "no scenarios". `list_files` itself
    // already returns `Ok(vec![])` for a missing directory, so anything that
    // does reach the error arm is a genuine fault worth telling the client.
    let list_error = |e: CorpusError| {
        tracing::warn!(law_id = %law_id, error = %e, "list_scenarios backend failure");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to list scenarios".to_string(),
        )
    };

    // Each listed file is also read to extract which law ids it evaluates
    // (`target_law_ids`). One failed read must not take down the whole
    // listing: that entry stays listed with unknown (empty) targets.
    let out: Vec<ScenarioEntry> = match scope {
        // Traject-scoped: the union of the write target's listing and
        // the seed's, mirroring the per-file routing of the scenario GET
        // / `If-Match` check (`read_traject_file_via_write_target`): a
        // scenario saved on the traject branch is listed even when the
        // law file itself was never saved there, AND seed scenarios that
        // were never copied to the branch keep showing up (their GET
        // falls back to the seed the same way).
        ReadScope::Traject(traject) => {
            // Per-snapshot cache: the editor requests this listing on
            // every law open, and a rebuild costs one `list_files` per
            // backend plus a read per scenario file — the most GitHub-
            // round-trip-heavy read on the law-open path. Scenario
            // save/delete invalidate the entry (read-your-writes);
            // upstream edits converge at the next snapshot.
            // Captured before the cache probe (not merely before the
            // backend I/O): a scenario save/delete landing while this
            // listing is computed bumps the write generation and the
            // store below is dropped, so the (older) listing can never
            // mask the mutation. Capturing after the probe would leave a
            // window — a write landing between probe and capture would
            // hand us the post-write generation, letting a rebuild from
            // GitHub's eventually-consistent (pre-write) directory view
            // pass the guard.
            let gen_before = traject.scenario_list_write_generation();
            if let Some(cached) = traject.cached_scenario_list(law_id).await {
                return Ok(Json(
                    cached
                        .into_iter()
                        .map(|e| ScenarioEntry {
                            filename: e.filename,
                            target_law_ids: e.target_law_ids,
                        })
                        .collect(),
                ));
            }
            let law = traject_law(traject, law_id)?;
            let scenarios_dir = match law_relative_dir(law) {
                Ok(dir) => dir.join("scenarios"),
                Err(_) => return Ok(Json(Vec::new())),
            };
            let write_source_id = traject_write_source_id(traject, law);
            let mut names = std::collections::BTreeSet::new();
            // Lock order: write target first, released before the seed
            // backend is touched (writable-own → seed, never the
            // reverse — same invariant as the per-file reads).
            let seed_source_id =
                (law.source_id != write_source_id).then_some(law.source_id.as_str());
            for source_id in std::iter::once(write_source_id.as_str()).chain(seed_source_id) {
                let Some(entry) = traject.corpus.backends.get(source_id) else {
                    continue;
                };
                let backend = entry.backend.lock().await;
                let entries = backend
                    .list_files(&scenarios_dir, Some("feature"))
                    .await
                    .map_err(list_error)?;
                drop(backend);
                names.extend(entries.into_iter().map(|e| e.name));
            }
            // Content goes through the same write-target-with-seed-fallback
            // per-file routing as the scenario GET, so the extracted targets
            // reflect exactly the bytes the editor serves — and lands in the
            // same per-file cache, so a subsequent scenario GET is free.
            // Cost on a cache miss: one sequential read per file in this
            // law's `scenarios/` dir — O(folder), typically 1-3 files, NOT
            // the O(corpus) scan that hung federated trajects before
            // (#762). Revisit with a batch read or index only if per-law
            // scenario counts grow.
            let mut out = Vec::with_capacity(names.len());
            let mut complete = true;
            for filename in names {
                let relative_path = scenarios_dir.join(&filename);
                let target_law_ids =
                    match read_traject_scenario_cached(traject, law, &relative_path).await {
                        Ok(Some(content)) => extract_target_law_ids(&content),
                        // Listed but unreadable on every routing leg: a
                        // ghost. GitHub's directory listing is eventually
                        // consistent, so right after a delete commit the
                        // (stale) listing can still name the file while
                        // its content GET already 404s. Skip it — keeping
                        // it would serve a listing entry whose GET can
                        // only 404.
                        Ok(None) => continue,
                        Err((_, err)) => {
                            tracing::warn!(
                                law_id = %law_id,
                                file = %filename,
                                error = %err,
                                "scenario read failed during listing"
                            );
                            complete = false;
                            Vec::new()
                        }
                    };
                out.push(ScenarioEntry {
                    filename,
                    target_law_ids,
                });
            }
            // BTreeSet iteration is already sorted by filename. Only a
            // fully-read listing is cached: a transient per-file read
            // failure keeps its degraded (empty-targets) entry out of the
            // snapshot cache, so the next request retries instead of
            // serving the degraded listing for the whole window.
            if complete {
                traject
                    .store_scenario_list_read(
                        law_id.to_string(),
                        out.iter()
                            .map(|e| ScenarioListEntry {
                                filename: e.filename.clone(),
                                target_law_ids: e.target_law_ids.clone(),
                            })
                            .collect(),
                        gen_before,
                    )
                    .await;
            }
            out
        }
        // Global: no write target exists; keep the read-only resolution.
        ReadScope::Global(_) => {
            let resolved = resolve_backend_for_law(scope.corpus(), law_id).await?;
            let scenarios_dir = match law_relative_dir(&resolved.law) {
                Ok(dir) => dir.join("scenarios"),
                Err(_) => return Ok(Json(Vec::new())),
            };
            // Unlike the Traject branch there is only one backend here, so
            // holding its lock across the listing and the per-file reads is
            // fine: no second lock is ever taken (no ordering hazard) and
            // re-acquiring per file would only add churn.
            let backend = resolved.backend.lock().await;
            let entries = backend
                .list_files(&scenarios_dir, Some("feature"))
                .await
                .map_err(list_error)?;
            let mut out = Vec::with_capacity(entries.len());
            for e in entries {
                let target_law_ids = match backend.read_file(&scenarios_dir.join(&e.name)).await {
                    Ok(Some(content)) => extract_target_law_ids(&content),
                    Ok(None) => Vec::new(),
                    Err(err) => {
                        tracing::warn!(
                            law_id = %law_id,
                            file = %e.name,
                            error = %err,
                            "scenario read failed during listing"
                        );
                        Vec::new()
                    }
                };
                out.push(ScenarioEntry {
                    filename: e.name,
                    target_law_ids,
                });
            }
            drop(backend);
            out.sort_by(|a, b| a.filename.cmp(&b.filename));
            out
        }
    };
    Ok(Json(out))
}

/// GET /api/corpus/laws/{law_id}/scenarios/{filename} — return raw .feature content (global view).
pub async fn get_scenario(
    State(state): State<AppState>,
    Path((law_id, filename)): Path<(String, String)>,
) -> Result<EtaggedContentResponse, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    get_scenario_in_scope(&scope, &law_id, &filename).await
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id}/scenarios/{filename}
/// — traject-scoped scenario read.
pub async fn get_traject_scenario(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, law_id, filename)): Path<(String, String, String)>,
) -> Result<EtaggedContentResponse, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    get_scenario_in_scope(&scope, &law_id, &filename).await
}

async fn get_scenario_in_scope(
    scope: &ReadScope,
    law_id: &str,
    filename: &str,
) -> Result<EtaggedContentResponse, (StatusCode, String)> {
    validate_scenario_filename(filename)?;

    let content = match scope {
        // Traject-scoped: read through the same write-target-with-seed-
        // fallback routing the save's `If-Match` check uses, so the GET
        // serves (and ETags) exactly the bytes a subsequent save is
        // checked against. See `read_traject_file_via_write_target` for
        // the 412-loop this prevents. Cached per snapshot; saves keep the
        // entry coherent (see `read_traject_scenario_cached`).
        ReadScope::Traject(traject) => {
            let law = traject_law(traject, law_id)?;
            let relative_path = scenario_relative_path(law, filename)?;
            read_traject_scenario_cached(traject, law, &relative_path).await?
        }
        // Global: no write target exists; keep the read-only resolution.
        ReadScope::Global(_) => {
            let resolved = resolve_backend_for_law(scope.corpus(), law_id).await?;
            let relative_path = scenario_relative_path(&resolved.law, filename)?;
            let backend = resolved.backend.lock().await;
            // Generic message + logged details, like `corpus_write_error`:
            // the raw error can carry backend internals (paths, git/remote
            // detail) that must not reach the client.
            backend.read_file(&relative_path).await.map_err(|e| {
                tracing::warn!(law_id = %law_id, filename = %filename, error = %e, "scenario read failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to read scenario".to_string(),
                )
            })?
        }
    };

    let content = content.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Scenario '{}' not found", filename),
        )
    })?;

    Ok(etagged_content_response(
        "text/plain; charset=utf-8",
        content,
    ))
}

/// Shared backend handle: a clone of the entry's `Arc<Mutex<...>>` from
/// the corpus state. The `Mutex` is held only across a single read/write
/// to keep contention scoped to one I/O call. Aliased here because the
/// fully-spelled type trips `clippy::type_complexity` on function
/// signatures.
type SharedBackend = Arc<Mutex<Box<dyn RepoBackend>>>;

/// Resolve the backend a `get_annotations` read should hit for `law_id`,
/// mirroring the write-path routing in `resolve_traject_law_write`.
///
/// With an active traject this returns the writable backend that
/// `save_annotations` writes to (its branch), so a note just appended is
/// visible on the next refresh. Without an active traject this falls
/// back to the law's own source backend in the global corpus — matches
/// the static-mirror semantics the frontend used to rely on (central
/// main's annotations for anonymous browsing).
///
/// Synchronous: only does `HashMap` lookups against the resolved
/// `ReadScope`. No DB hit (the membership check already happened in
/// `resolve_read_corpus`).
fn resolve_annotation_read_backend(
    scope: &ReadScope,
    law_id: &str,
) -> Result<SharedBackend, (StatusCode, String)> {
    match scope {
        ReadScope::Traject(traject) => {
            let law = traject_law(traject, law_id)?;
            // Mirror `resolve_traject_law_write`: a law from a
            // non-writable_own source is routed to the writable_own
            // backend via `write_target_for_source`; a law from a
            // writable source goes to its own.
            let target_source_id = traject_write_source_id(traject, law);
            let entry = traject
                .corpus
                .backends
                .get(&target_source_id)
                .ok_or_else(|| {
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        "Writable backend not initialised".to_string(),
                    )
                })?;
            Ok(entry.backend.clone())
        }
        ReadScope::Global(corpus) => {
            // No traject: read from the law's own source. There is no
            // per-traject branch involved, so the seed/central source
            // is the right target — matches the old static-mirror
            // surface for anonymous browsing.
            let law = corpus
                .source_map
                .get_law(law_id)
                .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?;
            let entry = corpus.backends.get(&law.source_id).ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("No backend registered for source '{}'", law.source_id),
                )
            })?;
            Ok(entry.backend.clone())
        }
    }
}

/// GET /api/corpus/laws/{law_id}/annotations — return the law's stand-off
/// notes sidecar.
///
/// Routed through the same backend as `save_annotations`: with an active
/// traject the read hits that traject's writable backend (its branch),
/// so a note just appended by `save_annotations` is visible on the next
/// refresh — the gap that #662 left open when it moved law reads to the
/// API but kept annotation reads on the static `/data` mirror baked into
/// the frontend container.
///
/// Without an active traject the read degrades to the global corpus's
/// resolved backend for the law (the central source's main view), matching
/// the static-mirror semantics the frontend used to rely on.
///
/// A missing sidecar returns 404 — "law without notes" is the normal
/// case and `useNotes.js` already treats it as a non-error.
///
/// Why not reuse `resolve_backend_for_law` (which `get_scenario` uses)?
/// That helper verifies a candidate writable backend by reading the
/// *law file* from it. Scenarios live under the law's own directory so
/// the check is a reliable proxy for "this backend has this law's
/// content". Annotations live under a *separate* `annotations/{law_id}/`
/// tree, and a freshly-created traject branch can carry a saved
/// annotation without ever having received a law-content edit — the
/// verification then falls through to the read-only seed and the
/// just-saved note silently disappears on refresh. The annotation read
/// instead mirrors the write path's routing
/// (`write_target_for_source`), which is the single source of truth
/// for "which backend owns this law's writes in this traject".
pub async fn get_annotations(
    State(state): State<AppState>,
    Path(law_id): Path<String>,
) -> Result<YamlResponse, (StatusCode, String)> {
    let scope = global_scope(&state).await;
    get_annotations_in_scope(&scope, &law_id).await
}

/// GET /api/trajects/{traject_id}/corpus/laws/{law_id}/annotations — same
/// as the global GET but reads the sidecar from the traject's writable
/// backend, matching the write path. A note just appended via
/// `save_annotations` is therefore visible on the next refresh.
pub async fn get_traject_annotations(
    State(state): State<AppState>,
    session: Session,
    Extension(account): Extension<AccountRecord>,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<YamlResponse, (StatusCode, String)> {
    let scope = require_traject_scope(&state, &session, &traject_ref).await?;
    let sidecar = match get_annotations_in_scope(&scope, &law_id).await {
        Ok((_, _, content)) => Some(content),
        // Absent sidecar is fine here: the caller may still have personal
        // notes, which then make up the whole document.
        Err((StatusCode::NOT_FOUND, _)) => None,
        Err(other) => return Err(other),
    };

    // Unified read: merge the caller's personal notes (marked
    // `regelrecht:visibility: personal`) into the returned document, so
    // the client sees one list of notes regardless of where each one is
    // stored. The sidecar bytes stay verbatim; personal notes are
    // appended the same way a save would append public ones.
    let personal = match &state.pool {
        Some(pool) => user_notes::personal_annotation_values(pool, account.id, &law_id)
            .await
            .map_err(|status| (status, "Failed to read personal notes".to_string()))?,
        None => Vec::new(),
    };

    let content = if personal.is_empty() {
        sidecar.ok_or((StatusCode::NOT_FOUND, "Annotations not found".to_string()))?
    } else {
        match append_notes_to_sidecar(sidecar.as_deref(), &personal, ANNOTATION_SCHEMA_URL) {
            Ok(AppendOutcome::Write(text)) => text,
            // Every personal value carries a unique `id`, so dedup can
            // only fire on a pathological sidecar; fall back to it.
            Ok(AppendOutcome::NoChange) => {
                sidecar.ok_or((StatusCode::NOT_FOUND, "Annotations not found".to_string()))?
            }
            Err(e) => {
                // A sidecar too broken to merge into must still be
                // readable; serve it unmerged rather than failing the GET.
                tracing::warn!(law_id = %law_id, error = %e, "could not merge personal notes into sidecar");
                sidecar.ok_or((StatusCode::NOT_FOUND, "Annotations not found".to_string()))?
            }
        }
    };

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/yaml; charset=utf-8")],
        content,
    ))
}

async fn get_annotations_in_scope(
    scope: &ReadScope,
    law_id: &str,
) -> Result<YamlResponse, (StatusCode, String)> {
    let content = read_annotations_in_scope(scope, law_id)
        .await?
        .ok_or((StatusCode::NOT_FOUND, "Annotations not found".to_string()))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/yaml; charset=utf-8")],
        content,
    ))
}

/// Cache key for a law's annotations sidecar in
/// [`TrajectCorpus::cached_sidecar`]. Prefixed so it can never collide
/// with the scenario-file keys ([`scenario_cache_key`]) sharing the map.
fn annotations_cache_key(law_id: &str) -> String {
    format!("ann:{law_id}")
}

/// Read the raw annotations sidecar bytes for `law_id` within a scope.
/// `Ok(None)` = the law exists but has no sidecar (the normal case).
///
/// Traject-scoped reads are cached per index snapshot — including the
/// `None` outcome: the editor requests annotations on every law open,
/// and rediscovering "no notes yet" would otherwise cost a full GitHub
/// round-trip each time. `save_annotations` keeps the entry coherent
/// within the snapshot window by storing the just-written body
/// (read-your-writes); cross-replica edits converge at the next snapshot,
/// like law bodies. Global (no-traject) reads stay uncached: they serve
/// anonymous browsing against the central source, whose backend does its
/// own ETag handling.
async fn read_annotations_in_scope(
    scope: &ReadScope,
    law_id: &str,
) -> Result<Option<String>, (StatusCode, String)> {
    // Captured before the cache probe: `store_sidecar_read` drops the
    // store when a save bumped the generation while this (potentially
    // slow) read was in flight, so it can never mask fresher content.
    // Capturing after the probe would leave a window — a save landing
    // between probe and capture would hand us the post-save generation,
    // letting a stale backend read pass the guard and overwrite the
    // save's fresh entry.
    let mut gen_before = 0;
    if let ReadScope::Traject(traject) = scope {
        gen_before = traject.sidecar_write_generation();
        if let Some(cached) = traject.cached_sidecar(&annotations_cache_key(law_id)).await {
            return Ok(cached);
        }
    }

    let backend = resolve_annotation_read_backend(scope, law_id)?;

    // RFC-018 §1: keyed by law id at the source root, regardless of where
    // the law file lives. Same path the `save_annotations` write uses.
    let relative_path = PathBuf::from("annotations")
        .join(law_id)
        .join("annotations.yaml");

    let content = {
        let backend = backend.lock().await;
        backend.read_file(&relative_path).await.map_err(|e| {
            tracing::warn!(law_id = %law_id, error = %e, "get_annotations backend read failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read annotations".to_string(),
            )
        })?
    };

    if let ReadScope::Traject(traject) = scope {
        traject
            .store_sidecar_read(annotations_cache_key(law_id), content.clone(), gen_before)
            .await;
    }

    Ok(content)
}

// ---------------------------------------------------------------------------
// Scenario write helpers
// ---------------------------------------------------------------------------

/// Validate a scenario filename (no path traversal, must end with `.feature`).
fn validate_scenario_filename(filename: &str) -> Result<(), (StatusCode, String)> {
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.contains('\0')
    {
        return Err((StatusCode::BAD_REQUEST, "Invalid filename".to_string()));
    }
    if !filename.ends_with(".feature") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Only .feature files are supported".to_string(),
        ));
    }
    Ok(())
}

/// Extract the law-relative directory from a law's file_path.
///
/// Returns the path of the law's directory, relative to the source root.
///
/// `LoadedLaw::relative_path` is computed at load time by stripping the
/// source root (for local sources) or the in-repo subpath (for GitHub
/// sources). Taking its parent gives the directory the backend writes to,
/// without making any assumption about the structural depth of the corpus
/// layout.
fn law_relative_dir(law: &LoadedLaw) -> Result<PathBuf, (StatusCode, String)> {
    let rel = std::path::Path::new(&law.relative_path);
    rel.parent().map(PathBuf::from).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Cannot determine law directory".to_string(),
        )
    })
}

/// Resolved backend information for a law (read-path only).
///
/// The write path goes through the per-traject corpus
/// ([`require_traject_corpus`]) and returns its own resolved target shape
/// ([`EditorWriteTarget`]), so this struct doesn't need a writability flag.
struct ResolvedBackend {
    law: LoadedLaw,
    backend: Arc<Mutex<Box<dyn RepoBackend>>>,
}

/// Resolve the backend that should be used for a law's scenario files.
///
/// Both read and write handlers go through this function so the editor
/// always uses the **same** backend for `get_scenario` / `list_scenarios` /
/// `save_scenario` / `delete_scenario` on a given law. Without this single
/// source of truth, a read can end up at one on-disk location while a write
/// for the same law lands at a different one — silent data loss.
///
/// Resolution order:
///
/// 1. **Law's own writable backend.** Happy path for normal local-only dev.
/// 2. **Verified writable fallback.** When the law's own source is read-only
///    (e.g. baked-in container filesystem) we look for another writable
///    backend whose root contains the **same** law file at the same
///    `law.relative_path`. A successful read of that path proves the two
///    sources share their structural layout, so subsequent reads/writes of
///    sibling scenario files land at consistent locations.
/// 3. **Law's own read-only backend.** No writable target available. Reads
///    still work; writes will be rejected with 403 by the caller.
///
/// The verification in step 2 is essential: without it the fallback could
/// silently produce files at a path the reader never looks at.
async fn resolve_backend_for_law(
    corpus: &crate::state::CorpusState,
    law_id: &str,
) -> Result<ResolvedBackend, (StatusCode, String)> {
    let law = corpus
        .source_map
        .get_law(law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))?
        .clone();

    // 1. Prefer the law's own backend if it can accept writes.
    if let Some(entry) = corpus.backends.get(&law.source_id) {
        if entry.writable {
            return Ok(ResolvedBackend {
                law,
                backend: entry.backend.clone(),
            });
        }
    }

    // 2. Look for another writable backend that contains the same law file
    //    at the same source-relative path. Alphabetical iteration keeps the
    //    choice deterministic across restarts.
    let law_rel = std::path::Path::new(&law.relative_path);
    let mut candidate_ids: Vec<&String> = corpus.backends.keys().collect();
    candidate_ids.sort();

    for source_id in candidate_ids {
        let Some(entry) = corpus.backends.get(source_id) else {
            continue;
        };
        if !entry.writable || source_id == &law.source_id {
            continue;
        }
        let backend = entry.backend.lock().await;
        let exists = backend.read_file(law_rel).await.ok().flatten().is_some();
        drop(backend);
        if exists {
            tracing::warn!(
                law_id = %law_id,
                law_source = %law.source_id,
                fallback_source = %source_id,
                "law's own source has no writable backend; routing reads through verified-matching source"
            );
            return Ok(ResolvedBackend {
                law,
                backend: entry.backend.clone(),
            });
        }
    }

    // 3. Fall through to the law's own read-only backend so reads still
    //    work.
    if let Some(entry) = corpus.backends.get(&law.source_id) {
        return Ok(ResolvedBackend {
            law,
            backend: entry.backend.clone(),
        });
    }

    Err((
        StatusCode::NOT_FOUND,
        format!(
            "No backend registered for source '{}' (the source that owns law '{}')",
            law.source_id, law_id
        ),
    ))
}

/// Map a [`CorpusError`] from a write / delete / persist operation to an
/// HTTP error tuple.
///
/// `ReadOnly` is an expected, recoverable precondition (e.g. the resolved
/// backend is a baked-in local source on a read-only container filesystem),
/// and the message is safe to surface to the user as `403 Forbidden`.
///
/// Every other variant (IO, git command failures, push failures, …) goes
/// out as `500 Internal Server Error` with a **generic** message. The full
/// error — which can include git stderr, repository URLs that may carry
/// push tokens for local-only backends, and absolute filesystem paths — is
/// logged at warn level for operators but never returned to the client.
///
/// `kind` is the short name of the resource being written ("scenario",
/// "law", …) so logs and the user-facing 500 body name the right thing
/// regardless of which handler is on the stack. The `FnOnce` wrapper is a
/// convenience for `.map_err(corpus_write_error("law"))` at call sites.
fn corpus_write_error(kind: &'static str) -> impl FnOnce(CorpusError) -> (StatusCode, String) {
    move |e| match e {
        CorpusError::ReadOnly(_) => (StatusCode::FORBIDDEN, e.to_string()),
        // Optimistic-concurrency race (remote SHA moved between read and
        // write). The frontend needs to discriminate this from a generic
        // 500 so it can prompt a refresh-and-retry instead of an error
        // toast.
        CorpusError::Conflict(_) => (
            StatusCode::CONFLICT,
            "Concurrent edit detected, please retry".to_string(),
        ),
        _ => {
            tracing::warn!(error = %e, kind = %kind, "corpus write/persist failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error while writing {}", kind),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Save / Delete scenario endpoints
// ---------------------------------------------------------------------------

/// Pull the editor user identity out of the OIDC session, but only
/// when the IdP has marked the email as verified.
///
/// Why the strict check: this identity ends up as the commit-author on
/// every save. Without `email_verified=true` we can't claim the email
/// is really the user's — GitHub will still happily render anyone's
/// name+email on a commit, so without IdP verification an attacker
/// could change their preferred email mid-session and impersonate
/// someone else on the resulting commits. Returning `None` here makes
/// the save-handler refuse the write with a 403, which is the right
/// fail-closed behaviour for an attribution system.
///
/// Returns `None` also when the session keys aren't populated at all
/// (auth disabled, or a session created before the verified-email
/// claim was added); the caller distinguishes those cases via its own
/// error message.
async fn editor_user_from_session(session: &Session) -> Option<EditorUser> {
    let name: Option<String> = session.get(SESSION_KEY_NAME).await.ok().flatten();
    let email: Option<String> = session.get(SESSION_KEY_EMAIL).await.ok().flatten();
    let verified: Option<bool> = session.get(SESSION_KEY_EMAIL_VERIFIED).await.ok().flatten();
    match (name, email, verified) {
        (Some(name), Some(email), Some(true)) if !name.is_empty() && !email.is_empty() => {
            Some(EditorUser { name, email })
        }
        _ => None,
    }
}

/// Wrapper around [`editor_user_from_session`] for write paths that
/// REQUIRE an attributable identity. Returns 403 with a user-facing
/// message when the editor isn't fully authenticated — better than
/// silently committing under the service account, which would let an
/// unverified email leak into the git history.
///
/// Note for ops at rollout: any session that predates the deploy that
/// introduced `SESSION_KEY_EMAIL_VERIFIED` lacks the claim entirely
/// and will hit this 403 even when the user's email *is* verified at
/// the IdP. The message therefore nudges towards a re-login, which
/// re-runs the OIDC callback and populates the missing claim.
async fn require_editor_user(session: &Session) -> Result<EditorUser, (StatusCode, String)> {
    editor_user_from_session(session).await.ok_or_else(|| {
        (
            StatusCode::FORBIDDEN,
            "Je sessie heeft geen geverifieerd e-mailadres. \
             Mogelijke oorzaken: (1) je sessie is van vóór de laatste deploy — \
             log opnieuw in om de verificatie-status uit je organisatie-account \
             in te lezen; (2) je e-mail is daadwerkelijk niet geverifieerd — \
             vraag je beheerder om in Keycloak 'email_verified' aan te zetten \
             voor je account."
                .to_string(),
        )
    })
}

/// Resolved write target for editor saves: a backend lock + the file
/// path. PR info comes back via `PersistOutcome.pr` from the actual
/// `persist` call, so we don't need to flag the backend here.
struct EditorWriteTarget {
    relative_path: PathBuf,
    backend: tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>,
}

/// Resolve the per-traject corpus from the URL ref, re-checking the
/// caller's membership on every call. Bumps the traject corpus cache on
/// a miss; calls `ensure_ready` (i.e. `git clone`) for every source in
/// the traject's federated config on first use.
///
/// The `traject_ref` is the URL form `{slug}-{8hex}`. The slug part is
/// cosmetic — `resolve_traject_ref` looks up the traject by the trailing
/// 8 hex chars of the UUID. A renamed traject keeps working under the
/// old URL because the suffix never changes.
///
/// The membership re-check catches drift since the SPA loaded the
/// `/editor/{ref}/…` route — a member removed (or their traject deleted)
/// mid-session must immediately stop being able to write to the branch
/// instead of keeping a stale handle through their open tabs.
async fn require_traject_corpus_from_ref(
    state: &AppState,
    session: &Session,
    traject_ref: &str,
) -> Result<Arc<TrajectCorpus>, (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))?;
    let traject_id = resolve_traject_ref(pool, traject_ref).await?;

    // Membership re-check: a single EXISTS join keeps this on the hot
    // path while catching state drift (membership revoked, traject
    // deleted, account never linked to a sub).
    let sub: String = session
        .get(SESSION_KEY_SUB)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "session read sub in require_traject_corpus_from_ref");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "session read failed".to_string(),
            )
        })?
        .ok_or((
            StatusCode::FORBIDDEN,
            "session has no subject claim".to_string(),
        ))?;
    let (is_member,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(
             SELECT 1 FROM accounts a
             JOIN traject_members m ON m.account_id = a.id
             WHERE a.person_sub = $1 AND m.traject_id = $2
         )",
    )
    .bind(&sub)
    .bind(traject_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "membership re-check query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "membership check failed".to_string(),
        )
    })?;
    if !is_member {
        return Err((
            StatusCode::FORBIDDEN,
            "Je hebt geen toegang meer tot dit traject".to_string(),
        ));
    }

    let auth_file = {
        let corpus = state.corpus.read().await;
        corpus.auth_file.clone()
    };
    state
        .trajects
        .get_or_build(pool, traject_id, auth_file)
        .await
        .map_err(traject_corpus_error)
}

fn traject_corpus_error(e: TrajectCorpusError) -> (StatusCode, String) {
    match e {
        TrajectCorpusError::NotFound => (
            StatusCode::NOT_FOUND,
            "Actief traject niet gevonden".to_string(),
        ),
        TrajectCorpusError::NoWritableOwn => (
            StatusCode::CONFLICT,
            "Traject heeft geen eigen schrijfbare source".to_string(),
        ),
        other => {
            tracing::error!(error = %other, "traject corpus build failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Traject corpus init failed".to_string(),
            )
        }
    }
}

/// Look up a law in a traject's federated source map; 404 when absent.
fn traject_law<'a>(
    traject: &'a TrajectCorpus,
    law_id: &str,
) -> Result<&'a LoadedLaw, (StatusCode, String)> {
    traject
        .corpus
        .source_map
        .get_law(law_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Law '{}' not found", law_id)))
}

/// Write-target source id for a law in a traject — the single routing
/// rule shared by the write path (`resolve_traject_law_write`), the
/// annotation read path and the traject-scoped scenario reads.
///
/// "Save back where the law came from": laws from a source that has an
/// entry in `write_target_for_source` get routed through that mapped
/// backend (typically the writable_own's traject-branched backend on
/// the same upstream repo). Laws from a source without an entry — e.g.
/// the local source, which is natively writable on its scratch dir —
/// stay on their own source's backend.
fn traject_write_source_id(traject: &TrajectCorpus, law: &LoadedLaw) -> String {
    traject
        .write_target_for_source
        .get(&law.source_id)
        .cloned()
        .unwrap_or_else(|| law.source_id.clone())
}

/// Resolved write routing for a law in a traject: the law's index
/// entry, the id of the source whose backend the write goes to, and an
/// owned guard over that backend.
struct TrajectLawWrite {
    law: LoadedLaw,
    /// Source id of the backend behind `backend`. Differs from
    /// `law.source_id` when the law comes from a federated read-only
    /// source and its writes are routed to the traject's writable-own
    /// backend (see `write_target_for_source`).
    write_source_id: String,
    backend: tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>,
}

/// Resolve the writable-own backend within a traject's corpus. Returns
/// the looked-up law (for its `relative_path`), the write-target source
/// id, and an owned guard over the traject's writable backend.
async fn resolve_traject_law_write(
    traject: &Arc<TrajectCorpus>,
    law_id: &str,
) -> Result<TrajectLawWrite, (StatusCode, String)> {
    let law = traject_law(traject, law_id)?.clone();

    let write_target_source_id = traject_write_source_id(traject, &law);
    let entry = traject
        .corpus
        .backends
        .get(&write_target_source_id)
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "Writable backend not initialised".to_string(),
        ))?;
    if !entry.writable {
        return Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()));
    }
    // Per-source write mutex: every save to the same traject source
    // serialises here, so contention (a second save waiting out the
    // first's GitHub round-trips) shows up as a fat `lock` phase.
    let backend = timing::measure("lock", entry.backend.clone().lock_owned()).await;
    Ok(TrajectLawWrite {
        law,
        write_source_id: write_target_source_id,
        backend,
    })
}

/// Source-relative path of a law's scenario file.
fn scenario_relative_path(
    law: &LoadedLaw,
    filename: &str,
) -> Result<PathBuf, (StatusCode, String)> {
    Ok(law_relative_dir(law)?.join("scenarios").join(filename))
}

/// Read the bytes an `If-Match` check on a law/scenario save compares
/// against, coherently with the write that follows.
///
/// The write-target file (the traject branch) is authoritative: it is
/// read through the same mutex guard the caller holds for the upcoming
/// write, so a concurrent save of the same law cannot slip in between
/// the check and the write (saves of one law serialise on that mutex,
/// and the loser then sees the winner's bytes — a stale `If-Match`
/// 412s instead of silently overwriting).
///
/// When that file does not exist yet AND the law is routed from a
/// *different* source (a federated law whose first traject edit hasn't
/// landed on the writable-own branch), the client's ETag was computed
/// from the upstream body the GET served — so fall back to reading the
/// law's own source backend (see [`read_seed_fallback`]).
async fn current_content_for_write(
    traject: &Arc<TrajectCorpus>,
    write: &TrajectLawWrite,
    relative_path: &std::path::Path,
    kind: &'static str,
) -> Result<Option<String>, (StatusCode, String)> {
    if let Some(text) = write
        .backend
        .read_file(relative_path)
        .await
        .map_err(corpus_write_error(kind))?
    {
        return Ok(Some(text));
    }
    read_seed_fallback(
        traject,
        &write.law,
        &write.write_source_id,
        relative_path,
        kind,
    )
    .await
}

/// Read `relative_path` from the law's own (seed) source backend — the
/// fallback leg of the write-target routing, for laws federated from a
/// non-writable source whose file was never saved on the writable-own
/// branch. Returns `Ok(None)` when the law already writes to its own
/// source (nothing to fall back to) or the seed backend is missing.
///
/// Lock-ordering invariant: callers must NOT hold the seed backend's
/// lock. Holding the write-target lock while calling this is allowed —
/// writable-own → seed is the one legal two-backend lock order (the
/// `law.source_id != write_source_id` guard guarantees the fallback
/// locks a *different* mutex than the write target's; tokio's `Mutex`
/// is not reentrant, so re-locking a held guard would deadlock).
async fn read_seed_fallback(
    traject: &Arc<TrajectCorpus>,
    law: &LoadedLaw,
    write_source_id: &str,
    relative_path: &std::path::Path,
    kind: &'static str,
) -> Result<Option<String>, (StatusCode, String)> {
    if law.source_id == write_source_id {
        return Ok(None);
    }
    let Some(entry) = traject.corpus.backends.get(&law.source_id) else {
        return Ok(None);
    };
    let backend = entry.backend.lock().await;
    backend
        .read_file(relative_path)
        .await
        .map_err(corpus_write_error(kind))
}

/// Read a law-scoped file for a traject GET through the **same routing
/// the write path's `If-Match` check uses** ([`current_content_for_write`]):
/// the write-target backend first, then the law's own seed source when
/// the file doesn't exist on the target yet. GET, precondition check
/// and write thereby agree on the authoritative replica.
///
/// Without this, a scenario saved before the law itself was ever saved
/// in the traject (law file still absent on the writable-own branch)
/// kept being served from the seed by `resolve_backend_for_law` —
/// whose fallback rule verifies the *law* file — while the check
/// compared against the writable-own copy: the user could never see
/// their save and every If-Match retry 412'd against an ETag the GET
/// never served (a permanent conflict loop).
///
/// Lock order: the write-target lock is released before the seed
/// backend is locked (writable-own → seed, never the reverse — the
/// same invariant `current_content_for_write` relies on).
async fn read_traject_file_via_write_target(
    traject: &Arc<TrajectCorpus>,
    law: &LoadedLaw,
    relative_path: &std::path::Path,
    kind: &'static str,
) -> Result<Option<String>, (StatusCode, String)> {
    let write_source_id = traject_write_source_id(traject, law);
    let entry = traject.corpus.backends.get(&write_source_id).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Writable backend not initialised".to_string(),
    ))?;
    {
        let backend = entry.backend.lock().await;
        if let Some(text) = backend
            .read_file(relative_path)
            .await
            .map_err(corpus_write_error(kind))?
        {
            return Ok(Some(text));
        }
    }
    read_seed_fallback(traject, law, &write_source_id, relative_path, kind).await
}

/// Cache key for one scenario file's content in
/// [`TrajectCorpus::cached_sidecar`]. The relative path is unique across
/// laws (it lives under the law's own directory), and the prefix keeps it
/// disjoint from the annotations keys sharing the map.
fn scenario_cache_key(relative_path: &std::path::Path) -> String {
    format!("scn:{}", relative_path.display())
}

/// [`read_traject_file_via_write_target`] with a per-snapshot cache in
/// front, for **GET-path scenario reads only** (the single-file GET and
/// the listing's target extraction). `None` results are cached too — a
/// seed-only law's write-target miss costs a GitHub round-trip to
/// rediscover. Write-path preconditions (`current_content_for_write`,
/// the `If-Match` checks) deliberately bypass this cache: they must
/// compare against the branch as it *is*, under the write lock, not
/// against a snapshot-aged copy.
///
/// Coherence: `save_scenario` stores the just-written body under this
/// key, `delete_scenario` drops the entry (the next read may
/// legitimately fall back to a seed copy, so a negative entry would be
/// wrong). Cross-replica edits converge at the next snapshot, like law
/// bodies.
async fn read_traject_scenario_cached(
    traject: &Arc<TrajectCorpus>,
    law: &LoadedLaw,
    relative_path: &std::path::Path,
) -> Result<Option<String>, (StatusCode, String)> {
    let key = scenario_cache_key(relative_path);
    // Generation captured before the cache probe: a save/delete landing
    // while the read is in flight bumps it, and the store below is then
    // dropped so these (older) bytes can't mask the write's entry.
    // Capturing after the probe would leave a window — a save landing
    // between probe and capture would hand us the post-save generation,
    // letting a stale backend read pass the guard and overwrite the
    // save's fresh entry.
    let gen_before = traject.sidecar_write_generation();
    if let Some(cached) = traject.cached_sidecar(&key).await {
        return Ok(cached);
    }
    let content =
        read_traject_file_via_write_target(traject, law, relative_path, "scenario").await?;
    traject
        .store_sidecar_read(key, content.clone(), gen_before)
        .await;
    Ok(content)
}

/// Resolve the write target for a law's stand-off notes sidecar.
///
/// The path is `annotations/{law_id}/annotations.yaml` at the source root,
/// NOT under the law's own `regulation/...` directory: RFC-018 §1 keys the
/// sidecar by law id, independent of where the law file lives. Routing
/// and writability come from `resolve_traject_law_write` (same backend
/// the law/scenario writes use), so notes land in the same traject
/// branch/PR as the rest of the edits in the session.
async fn resolve_traject_annotation_target(
    traject: &Arc<TrajectCorpus>,
    law_id: &str,
) -> Result<EditorWriteTarget, (StatusCode, String)> {
    let write = resolve_traject_law_write(traject, law_id).await?;
    Ok(EditorWriteTarget {
        relative_path: PathBuf::from("annotations")
            .join(law_id)
            .join("annotations.yaml"),
        backend: write.backend,
    })
}

/// Build a [`SaveResponse`] for a traject write. Traject backends commit
/// straight to the configured branch without opening a PR for now, so the
/// outcome typically carries `pr: None` and the response is just `{ pr:
/// null }` — the frontend treats that as a successful save without an
/// upstream link to display.
fn save_response_from_traject(outcome: PersistOutcome) -> SaveResponse {
    SaveResponse {
        pr: outcome.pr.map(|pr| SavePrInfo {
            url: pr.html_url,
            number: pr.number,
        }),
        no_change: false,
        personal_saved: None,
        etag: None,
    }
}

/// PUT /api/trajects/{traject_id}/corpus/laws/{law_id}/scenarios/{filename}
/// — save a scenario file in the traject's writable-own backend.
///
/// The traject id comes from the URL (per-tab SPA route), and the
/// caller's membership is re-checked on every request. No traject id =
/// no route, so this handler is unreachable without one.
///
/// Honors an optional `If-Match` header for optimistic concurrency
/// (412 on mismatch; an absent header stays a permissive blind write
/// for backward compatibility — same semantics as the document PUT).
/// The new ETag is returned in both the `ETag` header and the response
/// body's `etag` field.
pub async fn save_scenario(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path((traject_ref, law_id, filename)): Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::response::IntoResponse;
    validate_scenario_filename(&filename)?;
    let author = Some(require_editor_user(&session).await?);

    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let write = resolve_traject_law_write(&traject, &law_id).await?;
    // Resolved-backend-aware: a local writable-own backend ignores the
    // override, so enforcement must not 428 a save that never reaches GitHub.
    let token_override =
        github_oauth::user_write_token_for_backend(&state, account.id, &headers, &**write.backend)
            .await?;
    let relative_path = scenario_relative_path(&write.law, &filename)?;

    // Optimistic concurrency, same semantics as documents. Only read the
    // current content when the client actually sent a precondition — a
    // header-less save stays a single write, no extra backend read.
    if let Some(if_match) = extract_if_match(&headers) {
        let current =
            current_content_for_write(&traject, &write, &relative_path, "scenario").await?;
        check_if_match(current.as_deref(), Some(&if_match), "Scenario")?;
    }

    write
        .backend
        .write_file(&relative_path, &body)
        .await
        .map_err(corpus_write_error("scenario"))?;

    let outcome = write
        .backend
        .persist(&WriteContext {
            message: format!("Update scenario {} for {}", filename, law_id),
            author,
            token_override,
        })
        .await
        .map_err(corpus_write_error("scenario"))?;

    // Keep the per-snapshot read caches coherent with the save
    // (read-your-writes): the file's content cache gets the new body and
    // the cached listing is updated in place. Surgical (not invalidate +
    // rebuild): a rebuild would re-list GitHub's eventually-consistent
    // directory view, which right after the commit can still miss this
    // save and would then be cached for the rest of the snapshot window.
    traject
        .store_sidecar(scenario_cache_key(&relative_path), Some(body.clone()))
        .await;
    traject
        .record_scenario_saved(&law_id, &filename, extract_target_law_ids(&body))
        .await;

    let new_etag = document_etag(&body);
    let mut response = save_response_from_traject(outcome);
    response.etag = Some(new_etag.clone());
    Ok(([(axum::http::header::ETAG, new_etag)], Json(response)).into_response())
}

/// Schema the produced notes document is validated against before it is
/// written. Must match the version embedded in `regelrecht-corpus`'
/// annotation validator (kept in lockstep with the engine's resolver).
const ANNOTATION_SCHEMA_URL: &str = "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.3/annotation-schema.json";

/// Upper bound on notes accepted in a single save. The body limit on the
/// route already caps raw size; this caps the *count* so a single request
/// cannot append an unreasonable number of notes in one commit.
const MAX_NOTES_PER_SAVE: usize = 500;

/// Property a submitted note carries to choose its storage side in the
/// unified save (and that the unified GET stamps on merged-in personal
/// notes). Vendor-prefixed like `regelrecht:hint` in the selector.
const VISIBILITY_KEY: &str = "regelrecht:visibility";

/// Split submitted notes into (public, personal) on
/// `regelrecht:visibility`. The marker is stripped in both directions —
/// public notes must stay valid against the annotation schema
/// (`additionalProperties: false`), personal notes are re-shaped by
/// `annotation_to_request` anyway. An unrecognised visibility is a 400,
/// not a silent default: a typo like "privat" must never publish a note
/// the author meant to keep personal.
fn partition_notes_by_visibility(
    notes: Vec<serde_json::Value>,
) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>), (StatusCode, String)> {
    let mut public = Vec::new();
    let mut personal = Vec::new();
    for mut note in notes {
        let visibility = note.as_object_mut().and_then(|o| o.remove(VISIBILITY_KEY));
        match &visibility {
            None => public.push(note),
            Some(value) => match value.as_str() {
                Some("public") => public.push(note),
                Some("personal") => personal.push(note),
                _ => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "regelrecht:visibility must be \"personal\" or \"public\"".to_string(),
                    ));
                }
            },
        }
    }
    Ok((public, personal))
}

/// PUT /api/trajects/{traject_id}/corpus/laws/{law_id}/annotations —
/// the unified note save. The body is a JSON array of *new* notes
/// (drafts); each note's `regelrecht:visibility` decides where it goes:
///
/// - `"personal"` → the caller's private store (Postgres, account-scoped;
///   never git), marker and all other handling server-side;
/// - `"public"` or absent → the stand-off sidecar in the traject's
///   writable backend (its branch), so a note and a law edit made in the
///   same session ride the same PR.
///
/// A personal-marked note can therefore never end up in git, even when a
/// client naively round-trips the merged GET document back into a save.
///
/// For the public side the handler reads the sidecar as it stands on the
/// traject branch and appends only the new, deduped notes, keeping the
/// existing bytes verbatim (RFC-018 Dec. 1 / RFC-005: per-note `git
/// blame` and the curated motivering comments must survive). Personal
/// notes are deduped the same way (an identical stored note is skipped)
/// so retries stay idempotent. Error bodies are deliberately generic —
/// schema instance paths can echo attacker-controlled map keys and would
/// flow into an nldd dialog (the self-XSS vector `save_law` also avoids).
pub async fn save_annotations(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    let author = Some(require_editor_user(&session).await?);

    // Body is a JSON array of new notes. Parse + bound it before touching
    // the backend.
    let new_notes: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|e| {
        tracing::debug!(law_id = %law_id, error = %e, "save_annotations: body is not a JSON note array");
        (
            StatusCode::BAD_REQUEST,
            "Request body must be a JSON array of notes".to_string(),
        )
    })?;
    if new_notes.len() > MAX_NOTES_PER_SAVE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Too many notes in one save (max {})", MAX_NOTES_PER_SAVE),
        ));
    }

    let (public_notes, personal_notes) = partition_notes_by_visibility(new_notes)?;

    // Membership gate first: personal notes are the caller's own, but the
    // endpoint is traject-scoped, so the same access rule applies to both
    // halves of the save.
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;

    // Personal notes go to the account-scoped store, never to git. Map
    // and validate the whole batch first, then insert atomically (one
    // transaction) — a bad note rejects the batch before anything is
    // stored, so a 400/409 never leaves a half-saved batch behind.
    let mut personal_saved = 0usize;
    if !personal_notes.is_empty() {
        let pool = state.pool.as_ref().ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "Personal notes need a database".to_string(),
        ))?;
        let mut requests = Vec::with_capacity(personal_notes.len());
        for note in &personal_notes {
            let req = user_notes::annotation_to_request(&law_id, note)
                .map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
            user_notes::validate(&req).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "A personal note is not valid".to_string(),
                )
            })?;
            requests.push(req);
        }
        let inserted = user_notes::insert_notes(pool, account.id, &law_id, requests, true)
            .await
            .map_err(|status| match status {
                StatusCode::CONFLICT => (
                    StatusCode::CONFLICT,
                    "Too many personal notes for this law".to_string(),
                ),
                StatusCode::BAD_REQUEST => (
                    StatusCode::BAD_REQUEST,
                    "A personal note is not valid".to_string(),
                ),
                other => (other, "Failed to save personal notes".to_string()),
            })?;
        personal_saved = inserted.iter().filter(|n| n.is_some()).count();
    }

    // All-personal save: nothing for the sidecar, skip the git machinery.
    if public_notes.is_empty() {
        return Ok(Json(SaveResponse {
            pr: None,
            no_change: true,
            personal_saved: Some(personal_saved),
            etag: None,
        }));
    }
    let new_notes = public_notes;
    let target = resolve_traject_annotation_target(&traject, &law_id).await?;
    let EditorWriteTarget {
        relative_path,
        backend,
    } = target;
    // Checked here — after the all-personal early return and against the
    // resolved backend — so enforcement only fires for notes that actually
    // commit to GitHub: personal notes go to the database, and a local
    // writable-own backend never uses a token at all.
    let token_override =
        github_oauth::user_write_token_for_backend(&state, account.id, &headers, &**backend)
            .await?;

    // Read the current sidecar from the traject backend (the branch this
    // traject's PR is built on — read-your-writes within the traject).
    // Absent file = first notes for this law.
    let base_text: Option<String> = backend
        .read_file(&relative_path)
        .await
        .map_err(corpus_write_error("annotations"))?;

    // Validate the EXISTING file first, before merging in the new notes.
    // The post-merge validation below cannot tell "your note is invalid"
    // from "the file on the branch was already invalid" — same generic
    // message for both, so a user with a perfectly good note would edit it
    // forever while the real fault is a pre-existing note they did not
    // write. Schema drift on a committed sidecar is rare, but when it
    // happens the user must get a distinct, actionable error. Accepted
    // limitation documented in RFC-018 §10.
    if let Some(text) = base_text.as_deref() {
        if !text.trim().is_empty() {
            if let Err(errors) = parse_and_validate_annotation_yaml(text) {
                tracing::warn!(
                    law_id = %law_id,
                    errors = ?errors,
                    "save_annotations: existing sidecar fails the current schema; blocking append"
                );
                return Err((
                    StatusCode::CONFLICT,
                    "The notes file on the branch is itself invalid against the \
                     current schema. This is not a problem with your note; the \
                     existing file must be repaired before new notes can be added."
                        .to_string(),
                ));
            }
        }
    }

    // Append-only: keep the base file's bytes verbatim (preserves the
    // curated motivering comments and per-note git blame, RFC-018 Dec. 1 /
    // RFC-005) and append only the new, deduped notes. NoChange short-
    // circuits the whole write/commit so a no-op save is silent.
    let new_text =
        match append_notes_to_sidecar(base_text.as_deref(), &new_notes, ANNOTATION_SCHEMA_URL)
            .map_err(|e| {
                tracing::error!(law_id = %law_id, error = %e, "save_annotations: append failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to prepare notes for writing".to_string(),
                )
            })? {
            AppendOutcome::NoChange => {
                // Nothing new survived dedup (re-save of already-committed
                // notes). Skip write/commit entirely — no empty commit, no
                // branch noise (review finding NEW-2). `no_change: true` tells
                // the frontend to show "al opgeslagen" and keep the existing
                // PR badge instead of treating a PR-less 200 as a lost save.
                tracing::debug!(law_id = %law_id, "save_annotations: no new notes, skipping write");
                return Ok(Json(SaveResponse {
                    pr: None,
                    no_change: true,
                    personal_saved: Some(personal_saved),
                    etag: None,
                }));
            }
            AppendOutcome::Write(text) => text,
        };

    // Validate the *resulting* document. The base was already validated
    // separately above (a pre-existing-invalid file returns a distinct
    // 409), so a failure here means the newly submitted notes are bad, or
    // serialisation produced something off. Detailed errors are logged,
    // never returned (self-XSS stance, as `save_law`).
    let doc: serde_json::Value = serde_yaml_ng::from_str(&new_text).map_err(|e| {
        tracing::error!(law_id = %law_id, error = %e, "save_annotations: produced YAML does not parse");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Produced an invalid notes file".to_string(),
        )
    })?;
    if let Err(errors) = validate_annotation_doc(&doc) {
        tracing::debug!(
            law_id = %law_id,
            errors = ?errors,
            "save_annotations rejected: resulting document failed schema validation"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            "Notes are not valid against the annotation schema".to_string(),
        ));
    }

    // Every note must be about the law this path writes to (RFC-018 §1
    // keys the sidecar by law id). Allowlist, not blocklist: a note whose
    // `target.source` is absent or not a parseable `regelrecht://{law_id}`
    // is rejected, not silently ignored — the note-side analogue of
    // `save_law`'s `$id`/path guard. Runs on the merged result so a hostile
    // appended note cannot reference another law.
    if let Some((idx, err)) = first_note_not_targeting_law(&doc, &law_id) {
        tracing::debug!(
            law_id = %law_id,
            note_index = idx,
            reason = ?err,
            "save_annotations rejected: a note's target.source does not resolve to this law"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            "A note's target does not match the law it is being saved to".to_string(),
        ));
    }

    backend
        .write_file(&relative_path, &new_text)
        .await
        .map_err(corpus_write_error("annotations"))?;

    let outcome = backend
        .persist(&WriteContext {
            message: format!("Notities bijgewerkt voor {}", law_id),
            author,
            token_override,
        })
        .await
        .map_err(corpus_write_error("annotations"))?;

    // Read-your-writes for the per-snapshot annotations cache: the next
    // GET serves the just-committed sidecar (replacing a possibly cached
    // "no annotations yet") without a GitHub round-trip.
    traject
        .store_sidecar(annotations_cache_key(&law_id), Some(new_text))
        .await;

    let mut response = save_response_from_traject(outcome);
    response.personal_saved = Some(personal_saved);
    Ok(Json(response))
}

/// PUT /api/trajects/{traject_id}/corpus/laws/{law_id} — save edited law
/// YAML content to the traject's writable-own backend (its branch on the
/// writable repo). The save does NOT mirror into
/// `state.corpus.source_map`: that cache feeds GETs against
/// `/api/corpus/...` (no traject), so pushing in-progress traject edits
/// there would leak across users. The traject overlay populated below
/// makes the new content visible to GETs under the same `/api/trajects/{tid}/...`
/// prefix without a corpus reload.
///
/// The `$id` in the body must match the path parameter: allowing them to
/// diverge would either create a phantom law (new `$id` lands on an
/// existing file) or orphan the original (old `$id` can never be fetched
/// again). We reject the mismatch up-front instead of silently corrupting
/// the source map.
pub async fn save_law(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path((traject_ref, law_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::response::IntoResponse;
    let author = Some(require_editor_user(&session).await?);

    // Validation:
    //   1. Body must parse as well-formed YAML. extract_law_id below is a
    //      line-based scanner that happily accepts "$id: foo\n<garbage>",
    //      so without this check a syntactically broken body would land on
    //      disk and corrupt the corpus source file.
    //   2. Body must have a top-level `$id` field.
    //   3. That `$id` must match the path parameter. Any mismatch is either
    //      a phantom-law attempt (new id lands on an existing file) or an
    //      orphaning (old id becomes unfetchable); reject up-front.
    //
    // We do NOT run full JSON Schema validation here — the frontend blocks
    // incomplete operation stubs (findIncompleteOperation) and the YAML
    // pane has a live parse check. Full schema validation is a separate
    // follow-up (mirroring `just validate`).
    //
    // The mismatch error body intentionally does NOT echo the user-supplied
    // `body_id`: it flows through the frontend into ndd-inline-dialog's
    // supporting-text and we don't want self-XSS if the dialog ever renders
    // that attribute as markup. The path law_id is already known to the
    // caller, so the generic message is sufficient.
    validate_yaml_syntax(&body).map_err(|e| {
        tracing::debug!(law_id = %law_id, error = %e, "save_law received malformed YAML body");
        (
            StatusCode::BAD_REQUEST,
            "Body is not valid YAML".to_string(),
        )
    })?;

    let body_id = extract_law_id(&body).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Body missing top-level `$id` field".to_string(),
        )
    })?;

    if body_id != law_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "Body $id does not match path law_id".to_string(),
        ));
    }

    // Resolve the write target AND keep a handle on the per-traject
    // corpus so we can mirror the saved body into its read-your-writes
    // overlay after `persist` succeeds.
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let write = resolve_traject_law_write(&traject, &law_id).await?;
    let token_override =
        github_oauth::user_write_token_for_backend(&state, account.id, &headers, &**write.backend)
            .await?;
    let relative_path = PathBuf::from(&write.law.relative_path);

    // Optimistic concurrency, same semantics as the document PUT: a
    // present `If-Match` must equal the current content's ETag (412 on
    // mismatch), an absent header stays a permissive blind write for
    // backward compatibility. Checked while holding the write backend's
    // mutex (acquired by `resolve_traject_law_write` above), so a
    // concurrent save cannot slip between the check and the write.
    if let Some(if_match) = extract_if_match(&headers) {
        let current = current_content_for_write(&traject, &write, &relative_path, "law").await?;
        check_if_match(current.as_deref(), Some(&if_match), "Wet")?;
    }

    let outcome = {
        write
            .backend
            .write_file(&relative_path, &body)
            .await
            .map_err(corpus_write_error("law"))?;
        write
            .backend
            .persist(&WriteContext {
                message: format!("Update law {}", law_id),
                author,
                token_override,
            })
            .await
            .map_err(corpus_write_error("law"))?
    };

    // ETag of what we just wrote — the client chains it into the next
    // save's `If-Match`. Computed before `record_save` consumes `body`.
    let new_etag = document_etag(&body);

    // The global `state.corpus.source_map` is still NOT touched here:
    // it feeds GET handlers when no traject is active, so writing a
    // traject's in-progress edits into it would leak unmerged changes
    // across users.
    //
    // We DO mirror into the per-traject overlay so a subsequent GET in
    // the same traject (any session) sees the new content — that is
    // the read-your-writes follow-up that used to be punted.
    traject.record_save(law_id.clone(), body).await;

    // This save added (or kept) this law on the traject branch — fold it
    // into the cached changed-laws diff so the sidebar's "Bewerkt in dit
    // traject" section reflects the edit on the next load, without the
    // synchronous GitHub Compare call a cache invalidation would cost
    // that load.
    traject.record_changed_law(&law_id).await;

    let mut response = save_response_from_traject(outcome);
    response.etag = Some(new_etag.clone());
    Ok(([(axum::http::header::ETAG, new_etag)], Json(response)).into_response())
}

/// DELETE /api/trajects/{traject_id}/corpus/laws/{law_id}/scenarios/{filename}
/// — delete a scenario file in the traject's writable-own backend.
pub async fn delete_scenario(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path((traject_ref, law_id, filename)): Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    validate_scenario_filename(&filename)?;
    let author = Some(require_editor_user(&session).await?);

    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let write = resolve_traject_law_write(&traject, &law_id).await?;
    let token_override =
        github_oauth::user_write_token_for_backend(&state, account.id, &headers, &**write.backend)
            .await?;
    let relative_path = scenario_relative_path(&write.law, &filename)?;

    write
        .backend
        .delete_file(&relative_path)
        .await
        .map_err(corpus_write_error("scenario"))?;

    let outcome = write
        .backend
        .persist(&WriteContext {
            message: format!("Delete scenario {} for {}", filename, law_id),
            author,
            token_override,
        })
        .await
        .map_err(corpus_write_error("scenario"))?;

    // Drop (don't negative-cache) the file's read-cache entry: after a
    // branch-side delete the next read may legitimately fall back to a
    // seed copy of the scenario.
    traject
        .invalidate_sidecar(&scenario_cache_key(&relative_path))
        .await;

    // Update the cached listing in place — an invalidate-and-rebuild
    // races GitHub's eventually-consistent directory view, which can
    // still list the deleted file right after the commit and would then
    // resurrect it in the cache for the rest of the snapshot window.
    // What "in place" means depends on whether a seed copy remains: a
    // federated law's scenario that was overridden on the branch falls
    // back to the seed after the branch delete (same routing as the GET),
    // so it stays listed with the seed's targets. The seed probe is one
    // read, only for federated laws, on the rare delete path — and the
    // legal writable-own → seed lock order is respected (`write.backend`
    // holds the write-target guard). On a probe error we still remove
    // the entry: the delete is the user's intent, and a hidden seed copy
    // reappears at the next snapshot swap.
    match read_seed_fallback(
        &traject,
        &write.law,
        &write.write_source_id,
        &relative_path,
        "scenario",
    )
    .await
    {
        Ok(Some(seed_content)) => {
            traject
                .record_scenario_saved(&law_id, &filename, extract_target_law_ids(&seed_content))
                .await;
        }
        Ok(None) => traject.record_scenario_deleted(&law_id, &filename).await,
        Err((_, err)) => {
            tracing::warn!(
                law_id = %law_id,
                file = %filename,
                error = %err,
                "seed probe after scenario delete failed; removing from cached listing"
            );
            traject.record_scenario_deleted(&law_id, &filename).await;
        }
    }

    Ok(Json(save_response_from_traject(outcome)))
}

/// POST /api/corpus/reload — refetch corpus from all sources.
///
/// Reloads the in-memory SourceMap from the registry (local + GitHub).
/// Accepts an optional JSON body with `law_ids` to include specific laws
/// that may not yet be in the corpus (e.g. freshly harvested laws).
pub async fn reload_corpus(
    State(state): State<AppState>,
    body: Option<Json<ReloadRequest>>,
) -> Result<Json<ReloadResponse>, (StatusCode, String)> {
    // Single-flight: each reload fans out to GitHub (one call per law
    // source). Without this gate, an authenticated client firing parallel
    // reloads can exhaust the 5000 req/hr token quota and break corpus
    // reads for everyone. Concurrent callers get 429 rather than being
    // serialised — a reload already in flight will pick up their changes.
    let _reload_guard = state.reload_lock.try_lock().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Corpus reload already in progress".to_string(),
        )
    })?;

    // Gather everything we need under a read lock so concurrent readers
    // (law fetches, scenario loads, dependency resolution) are not blocked
    // for the duration of the GitHub round-trip.
    let (registry, auth_file, mut law_ids) = {
        let corpus = state.corpus.read().await;
        let law_ids: std::collections::HashSet<String> =
            corpus.source_map.laws().map(|l| l.law_id.clone()).collect();
        (corpus.registry.clone(), corpus.auth_file.clone(), law_ids)
    };

    // Include any extras the caller explicitly requests (e.g. a freshly
    // harvested law not yet in the corpus).
    if let Some(Json(req)) = &body {
        for id in &req.law_ids {
            law_ids.insert(id.clone());
        }
    }

    let new_map = registry
        .load_favorites_async(&law_ids, auth_file.as_deref())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "corpus reload failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to reload corpus".to_string(),
            )
        })?;

    let law_count = new_map.len();
    {
        let mut corpus = state.corpus.write().await;
        corpus.source_map = new_map;
    }
    tracing::info!(law_count, "corpus reloaded (local + GitHub)");
    Ok(Json(ReloadResponse { law_count }))
}

#[derive(Debug, Deserialize)]
pub struct ReloadRequest {
    #[serde(default)]
    pub law_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReloadResponse {
    pub law_count: usize,
}

// ---------------------------------------------------------------------------
// Document endpoints
// ---------------------------------------------------------------------------
//
// Documents live alongside laws in the writable-own backend's source
// root under `documents/<traject-ref>/<rest>` so they share the
// traject's branch, PR review and access control with the laws
// themselves. The MVP allows two text-based extensions (`.md` and
// `.txt`); binary uploads (PDF/images) and canvas-style collaboration
// are explicit out-of-scope for fase 1.
//
// Optimistic concurrency uses a SHA-256 over the on-branch body as
// the `ETag`. `enforce_if_match` returns a 412 directly so the frontend
// can distinguish "your view is stale, reload" from "the upstream raced
// us, retry" (409 `Conflict`).

const ALLOWED_DOCUMENT_EXTENSIONS: &[&str] = &["md", "txt"];

#[derive(Debug, Serialize)]
pub struct TrajectDocumentListEntry {
    /// Path relative to `documents/<traject-ref>/`, forward slashes.
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct TrajectDocumentList {
    pub documents: Vec<TrajectDocumentListEntry>,
}

#[derive(Debug, Serialize)]
pub struct SaveDocumentResponse {
    /// The new ETag after the save. Clients keep this for the next
    /// PUT/DELETE's `If-Match` header.
    pub etag: String,
    /// Mirrors `SaveResponse.pr` — populated when the writable
    /// backend surfaced a PR link.
    pub pr: Option<SavePrInfo>,
}

/// Compute the document ETag used for optimistic-concurrency checks.
/// Wrapped in double quotes per RFC 7232 so the header value can be
/// returned verbatim.
fn document_etag(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(content.as_bytes());
    format!("\"{:x}\"", digest)
}

/// Validate a caller-supplied document path. The path lives under
/// `documents/<traject-ref>/` so a traversal escape would land in
/// another traject (worst case) or in the writable backend's law
/// tree. Rules: non-empty; no leading `/`, no `\`, no NUL; no `.`
/// or `..` segments; segments match `[a-z0-9._-]+`; the file
/// extension is one of [`ALLOWED_DOCUMENT_EXTENSIONS`].
fn validate_document_path(raw: &str) -> Result<(), (StatusCode, String)> {
    if raw.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Pad mag niet leeg zijn".to_string(),
        ));
    }
    if raw.starts_with('/') || raw.contains('\\') || raw.contains('\0') {
        return Err((
            StatusCode::BAD_REQUEST,
            "Ongeldige tekens in pad".to_string(),
        ));
    }
    let segments: Vec<&str> = raw.split('/').collect();
    for segment in &segments {
        if segment.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Pad bevat lege segmenten".to_string(),
            ));
        }
        if *segment == "." || *segment == ".." || segment.starts_with('.') {
            return Err((
                StatusCode::BAD_REQUEST,
                "Pad mag geen '.' of '..' bevatten".to_string(),
            ));
        }
        if !segment
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-'))
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "Pad mag alleen kleine letters, cijfers en '._-' bevatten".to_string(),
            ));
        }
    }
    // `segments` is non-empty because `raw` is non-empty (checked
    // above) and `split('/')` always yields at least one element.
    // Falling back to the empty string keeps the check side-effect-free
    // even if that invariant ever drifted, and the extension lookup
    // below then correctly rejects the empty filename.
    let filename = segments.last().copied().unwrap_or("");
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !ALLOWED_DOCUMENT_EXTENSIONS.contains(&ext) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Alleen bestanden met extensie {} zijn toegestaan",
                ALLOWED_DOCUMENT_EXTENSIONS
                    .iter()
                    .map(|e| format!(".{e}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    Ok(())
}

/// Source-relative base directory for documents in a traject.
fn traject_documents_base(traject_ref: &str) -> PathBuf {
    PathBuf::from("documents").join(traject_ref)
}

/// Get the writable-own backend for a traject. Documents have no
/// per-law context, so we address the writable_own source directly via
/// the id captured at `TrajectCorpus` construction time. Reading the
/// id off `traject.writable_own_source_id` makes the invariant local —
/// previously this function inferred it from
/// `write_target_for_source.values().next()`, which relied on every
/// value being identical (true today, but unenforced).
async fn resolve_traject_documents_writer(
    traject: &Arc<TrajectCorpus>,
) -> Result<tokio::sync::OwnedMutexGuard<Box<dyn RepoBackend>>, (StatusCode, String)> {
    let entry = traject
        .corpus
        .backends
        .get(&traject.writable_own_source_id)
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "Writable backend not initialised".to_string(),
        ))?;
    if !entry.writable {
        return Err((StatusCode::FORBIDDEN, "Source is read-only".to_string()));
    }
    Ok(entry.backend.clone().lock_owned().await)
}

/// Read the `If-Match` header value, trimmed. `None` when absent or empty.
fn extract_if_match(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Allowlist for the `Content-Type` of an incoming document PUT. The
/// fase-1 endpoints only accept text bodies; semicolon-parameters
/// (`; charset=utf-8`) are stripped before matching, and an empty or
/// whitespace-only header is rejected — that input is malformed and
/// must surface as `415` rather than slip through as "no content type".
fn allowed_document_content_type(value: &str) -> bool {
    let mime = value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(mime.as_str(), "text/markdown" | "text/plain")
}

/// Check a client-supplied `If-Match` against the file's current state
/// and return the current ETag (or `None` when the file does not yet
/// exist). A `412 Precondition Failed` is surfaced on mismatch so the
/// frontend can distinguish a stale-view conflict from a generic 409
/// upstream race.
///
/// **`if_match = None` is intentionally a no-op.** The documents PUT
/// has to support brand-new files where the client has no prior ETag
/// to send, and DELETE accepts an unconditional remove for "kill it
/// with fire" cleanup. Callers that need optimistic-concurrency
/// guarantees MUST send the previously-issued ETag (or `*` for
/// "match anything that exists"); silently absent headers fall
/// through to a blind overwrite. The frontend composable
/// `useTrajectDocuments` always echoes the last seen ETag, so this
/// only matters for raw API consumers (curl, future tooling).
async fn enforce_if_match(
    backend: &dyn RepoBackend,
    relative_path: &std::path::Path,
    if_match: Option<&str>,
) -> Result<Option<String>, (StatusCode, String)> {
    let current = backend
        .read_file(relative_path)
        .await
        .map_err(corpus_write_error("document"))?;
    check_if_match(current.as_deref(), if_match, "Document")
}

/// Pure core of the `If-Match` precondition shared by the document,
/// law, and scenario save paths: compare a client-supplied `If-Match`
/// against the ETag of the current content and return that ETag (or
/// `None` when there is no current content). `noun` names the resource
/// in the Dutch 412 messages ("Document", "Wet", "Scenario").
///
/// `if_match = None` is intentionally a no-op — see [`enforce_if_match`]
/// for the backward-compatibility reasoning.
fn check_if_match(
    current: Option<&str>,
    if_match: Option<&str>,
    noun: &'static str,
) -> Result<Option<String>, (StatusCode, String)> {
    let current_etag = current.map(document_etag);
    if let Some(client) = if_match {
        match (client, &current_etag) {
            ("*", Some(_)) => {}
            ("*", None) => {
                return Err((
                    StatusCode::PRECONDITION_FAILED,
                    format!("{noun} bestaat (nog) niet"),
                ))
            }
            (val, Some(etag)) if val == etag.as_str() => {}
            _ => {
                return Err((
                    StatusCode::PRECONDITION_FAILED,
                    format!("{noun} is intussen door iemand anders gewijzigd"),
                ))
            }
        }
    }
    Ok(current_etag)
}

/// GET /api/trajects/{traject_ref}/corpus/documents
///
/// List all documents in the traject's documents folder, recursively.
/// A fresh traject without any documents yet returns an empty list
/// rather than 404 — the editor's sidebar shows "Geen documenten" and
/// offers the create form.
pub async fn list_traject_documents(
    State(state): State<AppState>,
    session: Session,
    Path(traject_ref): Path<String>,
) -> Result<Json<TrajectDocumentList>, (StatusCode, String)> {
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let backend = resolve_traject_documents_writer(&traject).await?;
    let base = traject_documents_base(&traject_ref);
    let entries = backend
        .list_files_recursive(&base, None)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "list_files_recursive on documents failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Kon documenten niet ophalen".to_string(),
            )
        })?;
    // Filter at the API boundary too — the on-disk tree could carry a
    // stray hand-committed file (e.g. an editor's `~` backup or a
    // hidden `.DS_Store`) and the API should not advertise those.
    let documents = entries
        .into_iter()
        .filter(|e| {
            std::path::Path::new(&e.relative_path)
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ALLOWED_DOCUMENT_EXTENSIONS.contains(&ext))
        })
        .map(|e| TrajectDocumentListEntry {
            path: e.relative_path,
        })
        .collect();
    Ok(Json(TrajectDocumentList { documents }))
}

/// Upload formats accepted in fase 1: PDF and Word.
const UPLOAD_DOCUMENT_EXTENSIONS: &[&str] = &["pdf", "doc", "docx"];

#[derive(Debug, Serialize)]
pub struct UploadDocumentResponse {
    /// The `.md` path the converted document will appear at once its
    /// conversion job completes.
    pub target_path: String,
}

#[derive(Debug, Serialize)]
pub struct TrajectJobList {
    pub jobs: Vec<regelrecht_pipeline::document_convert::TrajectJobView>,
}

/// Uniform 500 for the upload/jobs endpoints — logs the real cause, returns a
/// generic Dutch message.
fn upload_internal_error(context: &str, e: impl std::fmt::Display) -> (StatusCode, String) {
    tracing::error!(error = %e, context, "document upload/jobs handler failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Er ging iets mis bij het verwerken van de upload.".to_string(),
    )
}

/// Derive a collision-safe `<name>.md` target path from an uploaded filename,
/// sanitized to the document-path rules (`[a-z0-9._-]`, no hidden/empty
/// segments). Appends `-2`, `-3`, … when a document with the derived name
/// already exists in the traject.
fn derive_markdown_target(filename: &str, existing: &[String]) -> String {
    let raw_stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let mut sanitized = String::with_capacity(raw_stem.len());
    let mut last: Option<char> = None;
    for ch in raw_stem.chars() {
        let c = ch.to_ascii_lowercase();
        // Map to the allowed alphabet: alphanumerics and `_` pass through, any
        // other character becomes a `-`. `.` is kept (extensions read naturally).
        let mapped = if c.is_ascii_alphanumeric() || c == '_' {
            c
        } else if c == '.' {
            '.'
        } else {
            '-'
        };
        // Collapse consecutive separators so a run of spaces or dots (e.g.
        // `rapport..2024`) reduces to a single `-`/`.` rather than repeating.
        if (mapped == '-' || mapped == '.') && last == Some(mapped) {
            continue;
        }
        sanitized.push(mapped);
        last = Some(mapped);
    }
    let trimmed = sanitized.trim_matches(|c| c == '-' || c == '.');
    let stem = if trimmed.is_empty() {
        "document"
    } else {
        trimmed
    };

    let mut candidate = format!("{stem}.md");
    let mut n = 2;
    while existing.iter().any(|p| p == &candidate) {
        candidate = format!("{stem}-{n}.md");
        n += 1;
    }
    candidate
}

/// POST /api/trajects/{traject_ref}/corpus/documents/upload
///
/// Accept a multipart upload of a PDF/Word document, store its bytes in
/// `document_uploads`, and enqueue a `document_convert` job that converts it to
/// a markdown werkdocument. Responds `202 Accepted` with the target `.md` path
/// the converted document will appear at.
pub async fn upload_traject_document(
    State(state): State<AppState>,
    session: Session,
    Path(traject_ref): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadDocumentResponse>), (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))?;
    // Membership guard (also resolves the traject); the id feeds the job payload.
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let traject_id = resolve_traject_ref(pool, &traject_ref).await?;

    // Pull the uploaded file out of the multipart body (the `file` field).
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Ongeldige upload: {e}")))?
    {
        if field.name() == Some("file") {
            filename = field.file_name().map(|s| s.to_string());
            content_type = field.content_type().map(|s| s.to_string());
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Kon het bestand niet lezen: {e}"),
                )
            })?;
            data = Some(bytes.to_vec());
            break;
        }
    }
    let filename = filename.ok_or((
        StatusCode::BAD_REQUEST,
        "Geen bestand in de upload (het 'file'-veld ontbreekt).".to_string(),
    ))?;
    let data = data.filter(|d| !d.is_empty()).ok_or((
        StatusCode::BAD_REQUEST,
        "Het geüploade bestand is leeg.".to_string(),
    ))?;
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    // Only PDF/Word in fase 1.
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !UPLOAD_DOCUMENT_EXTENSIONS.contains(&ext.as_str()) {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Alleen PDF- en Word-documenten (.pdf, .doc, .docx) worden ondersteund.".to_string(),
        ));
    }

    // Derive a collision-safe target markdown path against the existing docs.
    // A failed listing must NOT be swallowed: an empty `existing` set would let
    // the derivation hand back a name that already exists, and the worker's
    // unconditional write would silently overwrite that document. Fail the
    // upload instead.
    let backend = resolve_traject_documents_writer(&traject).await?;

    // Enforcement gate, on the writer resolved above (one lock, not two). The
    // conversion runs in a background worker that can never carry the acting
    // user's cookie-bound token, so its write would always fall back to the
    // backend's configured token. With user-token enforcement on that is
    // exactly the silent service-token fallback `require_user_token` forbids —
    // refuse the upload fail-closed. Deliberately NOT a 428: linking GitHub
    // would not change anything here, so the koppel-flow redirect would loop.
    // Only for a backend that honors the override, though: a local writable-own
    // (preview/local-stack) writes without any token, so there is no service-
    // token fallback to forbid. The cheap backend check runs first; the flag
    // read (a DB round-trip) only fires for GitHub-backed trajects.
    if backend.supports_token_override() {
        if let Some(oauth) = state.config.github_oauth.as_ref() {
            if github_oauth::write_requires_user_token(&state, oauth).await? {
                return Err((
                    StatusCode::FORBIDDEN,
                    "Documenten uploaden is niet beschikbaar wanneer schrijven met je \
                     persoonlijke GitHub-token vereist is: de conversie schrijft op de \
                     achtergrond en kan niet namens jou committen."
                        .to_string(),
                ));
            }
        }
    }

    let base = traject_documents_base(&traject_ref);
    let mut existing: Vec<String> = backend
        .list_files_recursive(&base, None)
        .await
        .map_err(|e| upload_internal_error("list documents for collision check", e))?
        .into_iter()
        .map(|e| e.relative_path)
        .collect();
    // Release the writable-backend lock before the DB work below — the backend
    // is only needed for the listing, and holding its mutex through the
    // transaction would serialize every writable-document op on this traject.
    drop(backend);
    // Also avoid colliding with conversions that are enqueued but haven't
    // committed their `.md` yet — otherwise two same-named uploads both derive
    // e.g. `report.md` and the second conversion overwrites the first. (This
    // narrows but does not fully close a concurrent-upload window: two requests
    // can both read the pending set before either commits its job — an
    // acceptable residual TOCTOU for same-traject same-name simultaneous uploads.)
    existing.extend(
        regelrecht_pipeline::document_convert::pending_target_paths(pool, &traject_ref)
            .await
            .map_err(|e| {
                upload_internal_error("list pending conversions for collision check", e)
            })?,
    );
    let target_path = derive_markdown_target(&filename, &existing);

    // Persist the bytes and enqueue the conversion job in one transaction.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| upload_internal_error("begin tx", e))?;
    let (upload_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO document_uploads (traject_ref, filename, content_type, bytes) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&traject_ref)
    .bind(&filename)
    .bind(&content_type)
    .bind(&data)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| upload_internal_error("insert upload", e))?;

    let payload = regelrecht_pipeline::document_convert::DocumentConvertPayload {
        upload_id,
        traject_id,
        traject_ref: traject_ref.clone(),
        target_path: target_path.clone(),
        provider: None,
        requested_by: None,
    };
    let payload_json = serde_json::to_value(&payload)
        .map_err(|e| upload_internal_error("serialize payload", e))?;
    let req = regelrecht_pipeline::job_queue::CreateJobRequest::new(
        regelrecht_pipeline::JobType::DocumentConvert,
        format!("doc:{traject_ref}/{target_path}"),
    )
    .with_traject_ref(traject_ref.clone())
    // Single attempt: a retry would re-run the (expensive) LLM conversion from
    // scratch, and most failures here are deterministic (bad content, write
    // config). A failed conversion stays visible in the status block for the
    // user to re-upload. (Caching the produced markdown across retries is a
    // possible future refinement.)
    .with_max_attempts(1)
    .with_payload(payload_json);
    regelrecht_pipeline::job_queue::create_job(&mut *tx, req)
        .await
        .map_err(|e| upload_internal_error("create job", e))?;
    tx.commit()
        .await
        .map_err(|e| upload_internal_error("commit tx", e))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(UploadDocumentResponse { target_path }),
    ))
}

/// GET /api/trajects/{traject_ref}/corpus/documents/jobs
///
/// List the traject's document-convert jobs that are still relevant to show
/// (running or failed — completed ones are represented by the actual `.md` in
/// the documents list). Backs the werkdocumenten conversion-status block.
pub async fn list_traject_document_convert_jobs(
    State(state): State<AppState>,
    session: Session,
    Path(traject_ref): Path<String>,
) -> Result<Json<TrajectJobList>, (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database not configured".to_string(),
    ))?;
    // Membership guard.
    require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let jobs =
        regelrecht_pipeline::document_convert::list_traject_document_jobs(pool, &traject_ref)
            .await
            .map_err(|e| upload_internal_error("list document jobs", e))?;
    Ok(Json(TrajectJobList { jobs }))
}

/// GET /api/trajects/{traject_ref}/corpus/documents/{*doc_path}
///
/// Returns the raw markdown/text body, an appropriate `Content-Type`,
/// and an `ETag` header the client echoes back in `If-Match` on the
/// next PUT/DELETE to detect a concurrent edit.
pub async fn get_traject_document(
    State(state): State<AppState>,
    session: Session,
    Path((traject_ref, doc_path)): Path<(String, String)>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::response::IntoResponse;
    validate_document_path(&doc_path)?;
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let backend = resolve_traject_documents_writer(&traject).await?;
    let relative_path = traject_documents_base(&traject_ref).join(&doc_path);
    let content = backend
        .read_file(&relative_path)
        .await
        .map_err(corpus_write_error("document"))?
        .ok_or((StatusCode::NOT_FOUND, "Document niet gevonden".to_string()))?;
    let etag = document_etag(&content);
    let content_type = match std::path::Path::new(&doc_path)
        .extension()
        .and_then(|s| s.to_str())
    {
        Some("md") => "text/markdown; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    };
    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type.to_string()),
            (axum::http::header::ETAG, etag),
        ],
        content,
    )
        .into_response())
}

/// PUT /api/trajects/{traject_ref}/corpus/documents/{*doc_path}
///
/// Create or replace a document. Honors an optional `If-Match` header
/// (the previously returned ETag) for optimistic concurrency, and
/// returns the new ETag both in the response body and the response
/// `ETag` header. New documents return `201 Created`; updates return
/// `200 OK`.
pub async fn save_traject_document(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path((traject_ref, doc_path)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::response::IntoResponse;
    validate_document_path(&doc_path)?;
    let author = Some(require_editor_user(&session).await?);

    // The body is always text in fase 1. A *missing* Content-Type is
    // allowed because browsers occasionally omit it on
    // `fetch(PUT, body: string)`; that gets treated as text/plain by
    // the handler. A *present* Content-Type, however, must pass the
    // allowlist — an empty value is rejected as malformed and a binary
    // type (e.g. someone pointing the document endpoint at a PDF)
    // fails closed (415) instead of silently landing in git as opaque
    // bytes.
    if let Some(ct) = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        if !allowed_document_content_type(ct) {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Alleen text/markdown of text/plain is toegestaan".to_string(),
            ));
        }
    }

    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let backend = resolve_traject_documents_writer(&traject).await?;
    let token_override =
        github_oauth::user_write_token_for_backend(&state, account.id, &headers, &**backend)
            .await?;
    let relative_path = traject_documents_base(&traject_ref).join(&doc_path);

    let if_match = extract_if_match(&headers);
    let existed_before = enforce_if_match(&**backend, &relative_path, if_match.as_deref())
        .await?
        .is_some();

    backend
        .write_file(&relative_path, &body)
        .await
        .map_err(corpus_write_error("document"))?;

    let message = if existed_before {
        format!("Update document {doc_path}")
    } else {
        format!("Add document {doc_path}")
    };
    let outcome = backend
        .persist(&WriteContext {
            message,
            author,
            token_override,
        })
        .await
        .map_err(corpus_write_error("document"))?;

    let new_etag = document_etag(&body);
    let status = if existed_before {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        status,
        [(axum::http::header::ETAG, new_etag.clone())],
        Json(SaveDocumentResponse {
            etag: new_etag,
            pr: outcome.pr.map(|pr| SavePrInfo {
                url: pr.html_url,
                number: pr.number,
            }),
        }),
    )
        .into_response())
}

/// DELETE /api/trajects/{traject_ref}/corpus/documents/{*doc_path}
///
/// Remove a document, optionally guarded by `If-Match` for a
/// conflict-safe delete. A delete against a non-existent file returns
/// `404` rather than a silent success: the editor's confirm-and-delete
/// flow assumes the user just looked at the document, so absence
/// signals real divergence (someone else removed it) worth surfacing.
pub async fn delete_traject_document(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path((traject_ref, doc_path)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SaveResponse>, (StatusCode, String)> {
    validate_document_path(&doc_path)?;
    let author = Some(require_editor_user(&session).await?);

    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let backend = resolve_traject_documents_writer(&traject).await?;
    let token_override =
        github_oauth::user_write_token_for_backend(&state, account.id, &headers, &**backend)
            .await?;
    let relative_path = traject_documents_base(&traject_ref).join(&doc_path);

    let if_match = extract_if_match(&headers);
    let existed = enforce_if_match(&**backend, &relative_path, if_match.as_deref())
        .await?
        .is_some();
    if !existed {
        return Err((StatusCode::NOT_FOUND, "Document niet gevonden".to_string()));
    }

    backend
        .delete_file(&relative_path)
        .await
        .map_err(corpus_write_error("document"))?;

    let outcome = backend
        .persist(&WriteContext {
            message: format!("Delete document {doc_path}"),
            author,
            token_override,
        })
        .await
        .map_err(corpus_write_error("document"))?;

    Ok(Json(save_response_from_traject(outcome)))
}

#[cfg(test)]
mod tests {
    //! Tests for the small, pure helpers in this module. The full
    //! save/delete handlers require an axum harness with sessions +
    //! sqlx + a real source map and live behind separate integration
    //! tests.
    use super::*;

    #[test]
    fn partition_notes_routes_on_visibility_and_strips_the_marker() {
        let notes = vec![
            serde_json::json!({"type": "Annotation", "body": {"value": "publiek impliciet"}}),
            serde_json::json!({
                "type": "Annotation",
                "body": {"value": "publiek expliciet"},
                "regelrecht:visibility": "public"
            }),
            serde_json::json!({
                "type": "Annotation",
                "body": {"value": "prive"},
                "regelrecht:visibility": "personal"
            }),
        ];

        let (public, personal) = partition_notes_by_visibility(notes).unwrap();
        assert_eq!(public.len(), 2);
        assert_eq!(personal.len(), 1);
        // The marker never reaches the sidecar (schema would reject it)
        // and is stripped from personal notes too.
        for note in public.iter().chain(personal.iter()) {
            assert!(note.get(VISIBILITY_KEY).is_none());
        }
    }

    #[test]
    fn partition_notes_rejects_unknown_visibility() {
        // A typo must never silently publish a note meant to be personal.
        for bad in [
            serde_json::json!({"regelrecht:visibility": "privat"}),
            serde_json::json!({"regelrecht:visibility": 1}),
            serde_json::json!({"regelrecht:visibility": null}),
        ] {
            let err = partition_notes_by_visibility(vec![bad]).unwrap_err();
            assert_eq!(err.0, StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn save_response_from_traject_passes_through_pr_when_set() {
        use regelrecht_corpus::backend::PrInfo;
        let out = PersistOutcome {
            pr: Some(PrInfo {
                number: 7,
                html_url: "https://github.com/x/y/pull/7".to_string(),
            }),
        };
        let body = save_response_from_traject(out);
        let pr = body.pr.expect("response must carry pr");
        assert_eq!(pr.number, 7);
        assert_eq!(pr.url, "https://github.com/x/y/pull/7");
    }

    #[test]
    fn save_response_from_traject_returns_none_for_plain_commit() {
        let body = save_response_from_traject(PersistOutcome { pr: None });
        assert!(body.pr.is_none());
        // Law/scenario saves are never a notes no-op.
        assert!(!body.no_change);
        // The etag is filled in by the law/scenario handlers themselves.
        assert!(body.etag.is_none());
    }

    // ---- editor_user_from_session: attribution invariants ----

    use std::sync::Arc;
    use tower_sessions::{MemoryStore, Session};

    fn empty_session() -> Session {
        Session::new(None, Arc::new(MemoryStore::default()), None)
    }

    async fn session_with(
        name: Option<&str>,
        email: Option<&str>,
        verified: Option<bool>,
    ) -> Session {
        let s = empty_session();
        if let Some(n) = name {
            s.insert(SESSION_KEY_NAME, n.to_string()).await.unwrap();
        }
        if let Some(e) = email {
            s.insert(SESSION_KEY_EMAIL, e.to_string()).await.unwrap();
        }
        if let Some(v) = verified {
            s.insert(SESSION_KEY_EMAIL_VERIFIED, v).await.unwrap();
        }
        s
    }

    #[tokio::test]
    async fn editor_user_requires_verified_email() {
        // verified=false must produce None even when name+email are
        // present — otherwise an IdP that doesn't verify emails would
        // let an attacker pick any email and have it land in the commit
        // author field.
        let s = session_with(Some("Alice"), Some("alice@example.com"), Some(false)).await;
        assert!(editor_user_from_session(&s).await.is_none());
    }

    #[tokio::test]
    async fn editor_user_missing_verified_claim_is_rejected() {
        // No verified key at all (older session created before the
        // claim was added, or auth disabled) falls in the same bucket
        // as `verified=false`: fail closed.
        let s = session_with(Some("Alice"), Some("alice@example.com"), None).await;
        assert!(editor_user_from_session(&s).await.is_none());
    }

    #[tokio::test]
    async fn editor_user_happy_path() {
        let s = session_with(Some("Alice"), Some("alice@example.com"), Some(true)).await;
        let user = editor_user_from_session(&s).await.unwrap();
        assert_eq!(user.name, "Alice");
        assert_eq!(user.email, "alice@example.com");
    }

    #[tokio::test]
    async fn require_editor_user_returns_403_when_unverified() {
        let s = session_with(Some("Alice"), Some("alice@example.com"), Some(false)).await;
        let err = require_editor_user(&s).await.expect_err("must refuse");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert!(
            err.1.contains("e-mailadres"),
            "message should mention email verification: {}",
            err.1
        );
    }

    // ---- Spoofing-by-body invariant ----

    /// The handler-input contract guarantees that no save handler exposes
    /// a JSON field named `author` (or a structurally similar one) that
    /// could overwrite the OIDC-derived attribution. This is enforced at
    /// the type system level — `save_scenario` / `save_annotations` /
    /// `save_law` all take `body: String` (raw scenario / raw JSON
    /// notes array / raw YAML), never a `Json<SomeRequest>` struct that
    /// might carry an `author` field.
    ///
    /// If a future refactor switches one of those handlers to take a
    /// structured body, this test fails (the function signature no
    /// longer matches) and the author has to come back and re-examine
    /// the spoofing surface. Document the invariant via a compile-time
    /// signature assertion rather than a runtime probe — the runtime
    /// path is "session in → context out", with no body in between.
    #[test]
    fn save_handler_signatures_take_raw_body_no_author_field() {
        // Compile-time assertions: the function pointer types include
        // `body: String` as the last positional argument. If any handler
        // changes to `Json<X>` for some struct `X`, this code won't
        // compile — forcing a re-review of whether `X` can carry an
        // `author`-shaped field.
        let _: fn(
            axum::extract::State<crate::state::AppState>,
            axum::extract::Extension<crate::accounts::AccountRecord>,
            Session,
            axum::extract::Path<(String, String, String)>,
            axum::http::HeaderMap,
            String,
        ) -> _ = save_scenario;
        let _: fn(
            axum::extract::State<crate::state::AppState>,
            axum::extract::Extension<crate::accounts::AccountRecord>,
            Session,
            axum::extract::Path<(String, String)>,
            axum::http::HeaderMap,
            String,
        ) -> _ = save_annotations;
        let _: fn(
            axum::extract::State<crate::state::AppState>,
            axum::extract::Extension<crate::accounts::AccountRecord>,
            Session,
            axum::extract::Path<(String, String)>,
            axum::http::HeaderMap,
            String,
        ) -> _ = save_law;
        // delete_scenario takes no body at all — even stronger guarantee.
        let _: fn(
            axum::extract::State<crate::state::AppState>,
            axum::extract::Extension<crate::accounts::AccountRecord>,
            Session,
            axum::extract::Path<(String, String, String)>,
            axum::http::HeaderMap,
        ) -> _ = delete_scenario;
    }

    // ---- Document helpers ----

    #[test]
    fn validate_document_path_accepts_simple_md() {
        validate_document_path("notes.md").unwrap();
        validate_document_path("mvt/concept.md").unwrap();
        validate_document_path("a/b/c.txt").unwrap();
        validate_document_path("with-dashes_and.dots.md").unwrap();
    }

    #[test]
    fn validate_document_path_rejects_traversal() {
        // The traject-folder prefix means a `..` would land in another
        // traject, so it must be refused at the validation boundary.
        assert!(validate_document_path("../escape.md").is_err());
        assert!(validate_document_path("mvt/../escape.md").is_err());
        assert!(validate_document_path("/leading.md").is_err());
        assert!(validate_document_path("with\\backslash.md").is_err());
        assert!(validate_document_path("with\0nul.md").is_err());
    }

    #[test]
    fn validate_document_path_rejects_hidden_segments() {
        // Dot-leading segments would let a local-checkout backend touch
        // hidden filesystem entries (`.git`, `.env`); refuse them outright.
        assert!(validate_document_path(".git").is_err());
        assert!(validate_document_path(".env").is_err());
        assert!(validate_document_path("mvt/.git/config").is_err());
        assert!(validate_document_path(".DS_Store.md").is_err());
    }

    #[test]
    fn validate_document_path_rejects_disallowed_extensions() {
        assert!(validate_document_path("notes.pdf").is_err());
        assert!(validate_document_path("notes.html").is_err());
        assert!(validate_document_path("noextension").is_err());
    }

    #[test]
    fn validate_document_path_rejects_uppercase_or_unicode() {
        // Lowercase-only keeps the on-branch tree predictable across
        // case-insensitive filesystems and avoids the "Notes.md" /
        // "notes.md" duplicate-document footgun on macOS.
        assert!(validate_document_path("NOTES.md").is_err());
        assert!(validate_document_path("notités.md").is_err());
    }

    #[test]
    fn validate_document_path_rejects_empty_or_blank() {
        assert!(validate_document_path("").is_err());
        assert!(validate_document_path("/").is_err());
        assert!(validate_document_path("a//b.md").is_err());
    }

    #[test]
    fn derive_markdown_target_sanitizes_and_appends_md() {
        assert_eq!(derive_markdown_target("Report.pdf", &[]), "report.md");
        assert_eq!(
            derive_markdown_target("Mijn Brief.docx", &[]),
            "mijn-brief.md"
        );
        // Every derived target must satisfy the document-path rules.
        assert!(validate_document_path(&derive_markdown_target("Béépje 2024!.doc", &[])).is_ok());
    }

    #[test]
    fn derive_markdown_target_falls_back_when_stem_empty() {
        let target = derive_markdown_target("...pdf", &[]);
        assert!(validate_document_path(&target).is_ok());
    }

    #[test]
    fn derive_markdown_target_resolves_collisions() {
        let existing = vec!["report.md".to_string(), "report-2.md".to_string()];
        assert_eq!(
            derive_markdown_target("report.pdf", &existing),
            "report-3.md"
        );
    }

    #[test]
    fn derive_markdown_target_collapses_consecutive_separators() {
        // Consecutive dots and spaces collapse to a single separator.
        assert_eq!(
            derive_markdown_target("rapport..2024.pdf", &[]),
            "rapport.2024.md"
        );
        assert_eq!(
            derive_markdown_target("mijn   brief.docx", &[]),
            "mijn-brief.md"
        );
        assert!(validate_document_path(&derive_markdown_target("a...b   c.doc", &[])).is_ok());
    }

    #[test]
    fn document_etag_is_quoted_hex() {
        let etag = document_etag("hello world");
        // RFC 7232 strong validator: quoted ASCII.
        assert!(etag.starts_with('"') && etag.ends_with('"'));
        // SHA-256 hex = 64 chars; +2 quotes.
        assert_eq!(etag.len(), 66);
        // Same input → same ETag.
        assert_eq!(document_etag("hello world"), etag);
        // Different input → different ETag.
        assert_ne!(document_etag("hello world!"), etag);
    }

    #[test]
    fn extract_if_match_trims_and_normalises() {
        use axum::http::{HeaderMap, HeaderValue};
        let mut h = HeaderMap::new();
        assert!(extract_if_match(&h).is_none());

        h.insert(axum::http::header::IF_MATCH, HeaderValue::from_static("  "));
        assert!(extract_if_match(&h).is_none());

        h.insert(
            axum::http::header::IF_MATCH,
            HeaderValue::from_static("\"abc\""),
        );
        assert_eq!(extract_if_match(&h).as_deref(), Some("\"abc\""));
    }

    #[test]
    fn allowed_document_content_type_accepts_text_variants() {
        // Mime parameters (charset, boundary) are stripped before
        // matching so a normal `text/markdown; charset=utf-8` passes.
        assert!(allowed_document_content_type("text/markdown"));
        assert!(allowed_document_content_type(
            "text/markdown; charset=utf-8"
        ));
        assert!(allowed_document_content_type("TEXT/PLAIN"));
        assert!(allowed_document_content_type(
            "text/plain; charset=US-ASCII"
        ));
    }

    #[test]
    fn allowed_document_content_type_rejects_binary_and_empty() {
        // An explicit binary type — the protection against someone
        // pointing the document endpoint at a PDF — must fail.
        assert!(!allowed_document_content_type("application/pdf"));
        assert!(!allowed_document_content_type("image/png"));
        assert!(!allowed_document_content_type("application/octet-stream"));
        // An empty Content-Type header is a malformed request; the
        // allowlist refuses it explicitly instead of silently passing.
        assert!(!allowed_document_content_type(""));
        assert!(!allowed_document_content_type("   "));
    }

    // ---- enforce_if_match matrix ----

    use async_trait::async_trait;
    use regelrecht_corpus::backend::{
        FileEntry, PersistOutcome, RepoBackend, WriteContext as CorpusWriteContext,
    };
    use regelrecht_corpus::error::Result as CorpusResult;
    use std::path::Path as StdPath;

    /// Read-only backend stub that pretends the file's body is
    /// `Some(content)` (or `None` when the file is absent). Used to
    /// drive `enforce_if_match` through all of its branches without an
    /// axum harness.
    struct StubBackend {
        body: Option<String>,
    }

    #[async_trait]
    impl RepoBackend for StubBackend {
        async fn read_file(&self, _: &StdPath) -> CorpusResult<Option<String>> {
            Ok(self.body.clone())
        }
        async fn write_file(&self, _: &StdPath, _: &str) -> CorpusResult<()> {
            unreachable!("enforce_if_match never writes")
        }
        async fn delete_file(&self, _: &StdPath) -> CorpusResult<()> {
            unreachable!("enforce_if_match never deletes")
        }
        async fn list_files(&self, _: &StdPath, _: Option<&str>) -> CorpusResult<Vec<FileEntry>> {
            Ok(Vec::new())
        }
        async fn persist(&self, _: &CorpusWriteContext) -> CorpusResult<PersistOutcome> {
            Ok(PersistOutcome::default())
        }
        async fn ensure_ready(&mut self) -> CorpusResult<()> {
            Ok(())
        }
        fn is_writable(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn enforce_if_match_returns_current_etag_when_no_precondition() {
        // No `If-Match` header → no check; the caller still gets the
        // current ETag back so a subsequent write can chain.
        let backend = StubBackend {
            body: Some("hello".to_string()),
        };
        let etag = enforce_if_match(&backend, StdPath::new("x"), None)
            .await
            .unwrap();
        assert_eq!(etag.as_deref(), Some(document_etag("hello").as_str()));
    }

    #[tokio::test]
    async fn enforce_if_match_returns_none_when_file_absent_and_no_precondition() {
        let backend = StubBackend { body: None };
        let etag = enforce_if_match(&backend, StdPath::new("x"), None)
            .await
            .unwrap();
        assert!(etag.is_none());
    }

    #[tokio::test]
    async fn enforce_if_match_412_on_etag_mismatch() {
        let backend = StubBackend {
            body: Some("hello".to_string()),
        };
        let err = enforce_if_match(&backend, StdPath::new("x"), Some("\"stale\""))
            .await
            .expect_err("must refuse stale etag");
        assert_eq!(err.0, StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn enforce_if_match_passes_on_exact_etag() {
        let backend = StubBackend {
            body: Some("hello".to_string()),
        };
        let etag = document_etag("hello");
        let returned = enforce_if_match(&backend, StdPath::new("x"), Some(&etag))
            .await
            .unwrap();
        assert_eq!(returned.as_deref(), Some(etag.as_str()));
    }

    #[tokio::test]
    async fn enforce_if_match_wildcard_412_on_missing_file() {
        // `If-Match: *` semantically means "match any existing version".
        // Against a file that doesn't exist yet, the precondition fails.
        let backend = StubBackend { body: None };
        let err = enforce_if_match(&backend, StdPath::new("x"), Some("*"))
            .await
            .expect_err("must refuse `*` against missing file");
        assert_eq!(err.0, StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn enforce_if_match_wildcard_passes_on_any_existing_file() {
        let backend = StubBackend {
            body: Some("anything".to_string()),
        };
        let returned = enforce_if_match(&backend, StdPath::new("x"), Some("*"))
            .await
            .unwrap();
        assert_eq!(
            returned.as_deref(),
            Some(document_etag("anything").as_str())
        );
    }

    // ---- check_if_match: the law/scenario save precondition ----
    //
    // `save_law` / `save_scenario` resolve the current bytes through the
    // traject's write routing and run this pure check against them; the
    // three cases below pin the contract those handlers rely on.

    #[test]
    fn law_save_absent_if_match_is_permissive() {
        // `check_if_match` with `if_match = None` is a no-op (that path is
        // exercised in production by `enforce_if_match` on the document
        // route); `save_law`/`save_scenario` skip the call entirely when the
        // header is absent, which is what keeps older clients (frontend
        // without etag plumbing, curl) on the blind last-write-wins save.
        let current = "$id: wet\nname: v1\n";
        let etag = check_if_match(Some(current), None, "Wet").unwrap();
        assert_eq!(etag.as_deref(), Some(document_etag(current).as_str()));
    }

    #[test]
    fn law_save_stale_if_match_is_412() {
        // Someone else saved between this client's GET and PUT: the
        // client's ETag no longer matches the stored YAML → 412, and the
        // message names the law (not "Document").
        let current = "$id: wet\nname: v2-van-iemand-anders\n";
        let stale = document_etag("$id: wet\nname: v1\n");
        let err = check_if_match(Some(current), Some(&stale), "Wet")
            .expect_err("stale etag must be refused");
        assert_eq!(err.0, StatusCode::PRECONDITION_FAILED);
        assert!(
            err.1.contains("Wet"),
            "message should name the noun: {}",
            err.1
        );
    }

    #[test]
    fn scenario_save_matching_if_match_passes() {
        let current = "Feature: bestaand scenario\n";
        let etag = document_etag(current);
        let returned = check_if_match(Some(current), Some(&etag), "Scenario").unwrap();
        assert_eq!(returned.as_deref(), Some(etag.as_str()));
    }

    #[test]
    fn scenario_save_if_match_against_missing_file_is_412() {
        // The scenario was deleted between the client's GET and PUT —
        // a precondition can never match absent content.
        let stale = document_etag("Feature: weg\n");
        let err = check_if_match(None, Some(&stale), "Scenario")
            .expect_err("etag against missing file must be refused");
        assert_eq!(err.0, StatusCode::PRECONDITION_FAILED);
    }

    // --- extract_target_law_ids ---

    #[test]
    fn extract_targets_single_when_step() {
        let content = r#"Feature: X
  Scenario: a
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
"#;
        assert_eq!(
            extract_target_law_ids(content),
            vec!["wet_op_de_zorgtoeslag".to_string()]
        );
    }

    #[test]
    fn extract_targets_dedups_and_preserves_order() {
        let content = r#"
    When I evaluate "a" of "law_b"
    When I evaluate "b" of "law_a"
    When I evaluate "c" of "law_b"
"#;
        assert_eq!(extract_target_law_ids(content), vec!["law_b", "law_a"]);
    }

    #[test]
    fn extract_targets_accepts_then_and_but_continuations() {
        // The frontend step matcher strips the Gherkin keyword before
        // matching, so Then/And/But/`*` lines are valid execution steps.
        let content = r#"
    When I evaluate "a" of "law_a"
    Then I evaluate "b" of "law_b"
    And I evaluate "c" of "law_c"
    But I evaluate "d" of "law_d"
    * I evaluate "e" of "law_e"
"#;
        assert_eq!(
            extract_target_law_ids(content),
            vec!["law_a", "law_b", "law_c", "law_d", "law_e"]
        );
    }

    #[test]
    fn extract_targets_ignores_given_law_loaded_dependencies() {
        let content = r#"
    Given law "zorgverzekeringswet" is loaded
    Given law "wet_inkomstenbelasting_2001" is loaded
    When I evaluate "x" of "wet_op_de_zorgtoeslag"
"#;
        assert_eq!(
            extract_target_law_ids(content),
            vec!["wet_op_de_zorgtoeslag"]
        );
    }

    #[test]
    fn extract_targets_empty_for_file_without_execution_step() {
        let content =
            "Feature: WIP\n  Scenario: later\n    Given the calculation date is \"2025-01-01\"\n";
        assert_eq!(extract_target_law_ids(content), Vec::<String>::new());
    }

    #[test]
    fn extract_targets_ignores_malformed_lines() {
        let content = r#"
    When I evaluate "no closing quote of "x
    When I evaluate "a" of ""
    When something else entirely
"#;
        assert_eq!(extract_target_law_ids(content), Vec::<String>::new());
    }
}
