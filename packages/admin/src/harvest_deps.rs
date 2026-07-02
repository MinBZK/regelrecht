//! On-demand harvest of a root law's model-dependency closure.
//!
//! The recursive harvester only follows `<extref>` BWB hyperlinks found in a
//! law's source text. Regelingen reached purely through the machine-readable
//! model — a `source.regulation` binding, an `implements` IoC link, or a
//! `legal_basis` anchor — are therefore never auto-harvested. A prime example
//! is `regeling_standaardpremie` (BWBR0050536): `wet_op_de_zorgtoeslag`
//! delegates its `standaardpremie` open term to it via `implements`/`open_terms`
//! and nothing in the source text hyperlinks to it.
//!
//! This module walks that model-reference graph over the curated corpus,
//! resolves each referenced `$id` to a BWB id, and enqueues a harvest for any
//! law that has not already been harvested — reusing the canonical
//! [`request_harvest`] entry point so all dedup/exhausted/bookkeeping semantics
//! stay in one place.

use axum::extract::State;
use axum::Json;
use regelrecht_corpus::source_map::{collect_law_references, extract_bwb_id};
use regelrecht_pipeline::api::harvest::find_bwb_id_by_slug;
use regelrecht_pipeline::harvest_request::{
    request_harvest, HarvestRequestOptions, HarvestRequestOutcome,
};
use regelrecht_pipeline::law_status;
use regelrecht_pipeline::{LawStatusValue, PipelineError, Priority};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::ApiError;
use crate::state::AppState;

/// Priority for dependency-closure harvest jobs. Same band as the admin
/// "harvest now" button (50) — deliberately below interactive editor harvests.
const DEPENDENCY_HARVEST_PRIORITY: i32 = 50;

// Report `source` values — how a referenced `$id` was mapped to a BWB id.
const SOURCE_CURATED: &str = "curated";
const SOURCE_LAW_ENTRIES: &str = "law_entries";
const SOURCE_UNRESOLVED: &str = "unresolved";

// Report `action` values — what happened to the resolved BWB id.
const ACTION_ENQUEUED: &str = "enqueued";
const ACTION_ALREADY_HARVESTED: &str = "already_harvested";
const ACTION_UNRESOLVED: &str = "unresolved";

/// One row of the closure report: a referenced law, how it resolved, and what
/// was done about it.
#[derive(Debug, Serialize)]
pub struct DependencyReportEntry {
    /// The referenced law's `$id`.
    pub id: String,
    /// The resolved BWB id, or `null` when the reference could not be resolved.
    pub resolved_bwb_id: Option<String>,
    /// How the id was resolved: `curated` | `law_entries` | `unresolved`.
    pub source: String,
    /// What was done: `enqueued` | `already_harvested` | `unresolved`.
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct HarvestDependenciesBody {
    /// The root law: a `$id` slug (e.g. `wet_op_de_zorgtoeslag`) or a BWB/CVDR
    /// id of a law present in the curated corpus.
    pub law_id: String,
    /// Optional consolidation date (`YYYY-MM-DD`) propagated to every enqueued
    /// harvest. `None` lets the harvester resolve the latest consolidation.
    pub date: Option<String>,
}

/// POST /api/harvest-dependencies — harvest the model-dependency closure of a
/// root law.
///
/// Resolves the root to a curated law, walks its model references transitively,
/// and enqueues harvests for any referenced law not already harvested. Returns
/// one report row per referenced law.
pub async fn harvest_dependencies(
    State(state): State<AppState>,
    Json(body): Json<HarvestDependenciesBody>,
) -> Result<Json<Vec<DependencyReportEntry>>, ApiError> {
    let raw = body.law_id.trim().to_string();
    if raw.is_empty() {
        return Err(ApiError::BadRequest("law_id must not be empty".to_string()));
    }

    let (root_id, root_yaml) = resolve_root(&state, &raw).await?;

    let report = harvest_dependency_closure(&state, &root_id, &root_yaml, body.date).await?;

    tracing::info!(
        root = %root_id,
        referenced = report.len(),
        enqueued = report.iter().filter(|e| e.action == ACTION_ENQUEUED).count(),
        "harvested dependency closure"
    );

    Ok(Json(report))
}

/// Resolve the root argument (a `$id` slug or a BWB/CVDR id) to the curated
/// law's `$id` and YAML body. A BWB/CVDR id triggers a scan of the loaded
/// corpus for a matching `bwb_id`.
async fn resolve_root(state: &AppState, raw: &str) -> Result<(String, String), ApiError> {
    let corpus = state.corpus.read().await;

    // Direct hit on a `$id` slug.
    if let Some(law) = corpus.source_map.get_law(raw) {
        if law.is_loaded() {
            return Ok((law.law_id.clone(), law.yaml_content.clone()));
        }
    }

    // BWB/CVDR id → scan the loaded corpus for a matching bwb_id.
    let upper = raw.to_uppercase();
    if upper.starts_with("BWBR") || upper.starts_with("CVDR") {
        for law in corpus.source_map.laws() {
            if law.is_loaded()
                && extract_bwb_id(&law.yaml_content).as_deref() == Some(upper.as_str())
            {
                return Ok((law.law_id.clone(), law.yaml_content.clone()));
            }
        }
    }

    Err(ApiError::NotFound(format!(
        "no curated law found for '{raw}' — the root must be a $id slug or a BWB/CVDR id present in the corpus"
    )))
}

/// Walk the transitive model-reference closure of `root_yaml` and enqueue a
/// harvest for every referenced law not already harvested.
///
/// The closure follows edges in BOTH directions:
/// - **Forward** — the law's own [`collect_law_references`] (`source.regulation`,
///   `legal_basis`, `implements`): the laws it depends on for inputs.
/// - **Reverse `implements`** — the lower regulations that declare they
///   `implements` this law's open terms. This is essential: a delegated regeling
///   (e.g. `regeling_standaardpremie`) is NOT referenced forward by the higher
///   law (`wet_op_de_zorgtoeslag` only declares the `standaardpremie` open term);
///   the link lives on the regeling. Without the reverse edge the delegated
///   regelingen — the whole point of this feature — would be missed.
///
/// BFS, cycle-safe via a visited set of `$id`s. A law that has a curated YAML is
/// recursed into (forward), so the whole model chain is covered. The root law is
/// not itself (re-)harvested — only the laws it reaches.
///
/// Limitation: a yearly regeling is a distinct BWB law per year (2025 =
/// BWBR0050536, 2026 differs). Resolution uses the `bwb_id` of whichever curated
/// version the corpus surfaces, so the closure harvests that version's BWB.
async fn harvest_dependency_closure(
    state: &AppState,
    root_id: &str,
    root_yaml: &str,
    date: Option<String>,
) -> Result<Vec<DependencyReportEntry>, ApiError> {
    // Reverse `implements` index over the curated corpus: higher-law $id -> the
    // $ids of the regelingen that implement its open terms.
    let reverse_impl = build_reverse_implements(state).await;

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut report: Vec<DependencyReportEntry> = Vec::new();

    // The root is the starting point, not a harvest target. Seed with both its
    // forward references and the regelingen that implement it.
    visited.insert(root_id.to_string());
    for reference in collect_law_references(root_yaml) {
        queue.push_back(reference);
    }
    if let Some(implementers) = reverse_impl.get(root_id) {
        for implementer in implementers {
            queue.push_back(implementer.clone());
        }
    }

    while let Some(id) = queue.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }

        // Fetch the referenced law's curated YAML once: it drives both bwb_id
        // resolution and recursion into deeper references.
        let curated = curated_yaml(state, &id).await;

        if let Some(yaml) = &curated {
            for reference in collect_law_references(yaml) {
                if !visited.contains(&reference) {
                    queue.push_back(reference);
                }
            }
        }
        // Reverse: regelingen implementing this law's open terms.
        if let Some(implementers) = reverse_impl.get(&id) {
            for implementer in implementers {
                if !visited.contains(implementer) {
                    queue.push_back(implementer.clone());
                }
            }
        }

        // Resolve $id → BWB id: curated YAML first, then the law_entries slug
        // mapping.
        let (resolved_bwb_id, source) = match curated.as_deref().and_then(extract_bwb_id) {
            Some(bwb_id) => (Some(bwb_id), SOURCE_CURATED),
            None => match find_bwb_id_by_slug(&state.pool, &id).await {
                Ok(Some(bwb_id)) => (Some(bwb_id), SOURCE_LAW_ENTRIES),
                Ok(None) => (None, SOURCE_UNRESOLVED),
                Err(e) => {
                    tracing::warn!(id = %id, error = %e, "failed to look up slug in law_entries");
                    (None, SOURCE_UNRESOLVED)
                }
            },
        };

        let action = match &resolved_bwb_id {
            Some(bwb_id) => enqueue_if_needed(state, bwb_id, &id, date.clone()).await?,
            None => ACTION_UNRESOLVED,
        };

        report.push(DependencyReportEntry {
            id,
            resolved_bwb_id,
            source: source.to_string(),
            action: action.to_string(),
        });
    }

    Ok(report)
}

/// Build a reverse `implements` index over the curated corpus: a higher law's
/// `$id` → the `$id`s of the lower regulations that declare they `implements`
/// it (fill one of its open terms).
///
/// This is how delegated regelingen are discovered: the higher law does not
/// reference them forward, so we must find the laws pointing back at it.
///
/// Uses each law's `implements` list, which the corpus precomputes at load time
/// (`LoadedLaw.implements`) — no per-request YAML re-parse and no body clones.
/// The read lock is held only for this synchronous pass. O(corpus); a build-time
/// index is the eventual optimization (see the known implementor-scan cost).
async fn build_reverse_implements(state: &AppState) -> HashMap<String, Vec<String>> {
    let corpus = state.corpus.read().await;
    let mut index: HashMap<String, Vec<String>> = HashMap::new();
    for law in corpus.source_map.laws() {
        if law.is_loaded() {
            add_reverse_implements(&mut index, &law.law_id, &law.implements);
        }
    }
    index
}

/// Add one law's `implements` edges (`implementer → higher law`) to the reverse
/// index (`higher law → [implementer]`). Pure, so the inversion is unit-testable
/// without corpus state.
fn add_reverse_implements(
    index: &mut HashMap<String, Vec<String>>,
    id: &str,
    implements: &[String],
) {
    for higher in implements {
        let implementers = index.entry(higher.clone()).or_default();
        if !implementers.iter().any(|existing| existing == id) {
            implementers.push(id.to_string());
        }
    }
}

/// Read a law's curated YAML body by `$id` from the in-memory corpus. Returns
/// `None` for laws not in the corpus or whose body is a metadata-only (unloaded)
/// index entry. This is the admin↔corpus integration point.
async fn curated_yaml(state: &AppState, id: &str) -> Option<String> {
    let corpus = state.corpus.read().await;
    corpus
        .source_map
        .get_law(id)
        .filter(|law| law.is_loaded())
        .map(|law| law.yaml_content.clone())
}

/// Enqueue a harvest for `bwb_id` unless the law has already been harvested
/// (or is mid-flight / exhausted / not harvestable). Reuses [`request_harvest`]
/// so dedup, exhausted and bookkeeping semantics are not re-implemented.
async fn enqueue_if_needed(
    state: &AppState,
    bwb_id: &str,
    slug: &str,
    date: Option<String>,
) -> Result<&'static str, ApiError> {
    // Skip laws that already have (or are actively getting) content. A missing
    // entry (LawNotFound) or a plain harvest_failed is fair game to enqueue.
    match law_status::get_law(&state.pool, bwb_id).await {
        Ok(law) if is_already_harvested(law.status) => return Ok(ACTION_ALREADY_HARVESTED),
        Ok(_) | Err(PipelineError::LawNotFound(_)) => {}
        Err(e) => {
            tracing::error!(bwb_id = %bwb_id, error = %e, "failed to read law status");
            return Err(ApiError::Internal("failed to read law status".to_string()));
        }
    }

    let opts = HarvestRequestOptions {
        priority: Priority::new(DEPENDENCY_HARVEST_PRIORITY),
        date,
        law_name: None,
        slug: Some(slug.to_string()),
    };

    match request_harvest(&state.pool, bwb_id, opts).await {
        Ok(HarvestRequestOutcome::Created(job)) => {
            tracing::info!(job_id = %job.id, bwb_id = %bwb_id, slug = %slug, "enqueued dependency harvest");
            Ok(ACTION_ENQUEUED)
        }
        // An active job already exists, or the law is exhausted — either way
        // there is nothing to enqueue.
        Ok(HarvestRequestOutcome::AlreadyQueued { .. }) | Ok(HarvestRequestOutcome::Exhausted) => {
            Ok(ACTION_ALREADY_HARVESTED)
        }
        Ok(HarvestRequestOutcome::InvalidDate { reason }) => {
            Err(ApiError::BadRequest(format!("invalid date: {reason}")))
        }
        Err(e) => {
            tracing::error!(bwb_id = %bwb_id, error = %e, "failed to enqueue dependency harvest");
            Err(ApiError::Internal(
                "failed to enqueue dependency harvest".to_string(),
            ))
        }
    }
}

/// Whether a law's status means it has already been harvested and should not be
/// re-enqueued. Only `Unknown` and `HarvestFailed` (and a missing entry) are
/// treated as harvestable.
fn is_already_harvested(status: LawStatusValue) -> bool {
    // Exhaustive match (not a `matches!` whitelist) so a newly-added status can't
    // silently default to "harvestable" and spawn jobs — adding a variant forces
    // an explicit decision here at compile time.
    match status {
        // Harvestable: nothing yet, or a retryable harvest failure.
        LawStatusValue::Unknown | LawStatusValue::HarvestFailed => false,
        // Already harvested, in-flight, or terminal — leave it alone.
        LawStatusValue::Queued
        | LawStatusValue::Harvesting
        | LawStatusValue::Harvested
        | LawStatusValue::HarvestExhausted
        | LawStatusValue::Enriching
        | LawStatusValue::Enriched
        | LawStatusValue::EnrichFailed
        | LawStatusValue::EnrichExhausted
        | LawStatusValue::NotHarvestable => true,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn already_harvested_matches_expected_statuses() {
        // Harvestable: nothing yet, or a retryable failure.
        assert!(!is_already_harvested(LawStatusValue::Unknown));
        assert!(!is_already_harvested(LawStatusValue::HarvestFailed));
        // Everything else means "leave it alone".
        assert!(is_already_harvested(LawStatusValue::Queued));
        assert!(is_already_harvested(LawStatusValue::Harvesting));
        assert!(is_already_harvested(LawStatusValue::Harvested));
        assert!(is_already_harvested(LawStatusValue::HarvestExhausted));
        assert!(is_already_harvested(LawStatusValue::Enriching));
        assert!(is_already_harvested(LawStatusValue::Enriched));
        assert!(is_already_harvested(LawStatusValue::EnrichFailed));
        assert!(is_already_harvested(LawStatusValue::EnrichExhausted));
        assert!(is_already_harvested(LawStatusValue::NotHarvestable));
    }

    #[test]
    fn reverse_implements_discovers_delegated_regeling() {
        // The regeling implements the higher law's open term (the reverse edge a
        // forward-only walk would miss); the higher law implements nothing.
        let mut index = HashMap::new();
        add_reverse_implements(&mut index, "wet_op_de_zorgtoeslag", &[]);
        add_reverse_implements(
            &mut index,
            "regeling_standaardpremie",
            &["wet_op_de_zorgtoeslag".to_string()],
        );
        assert_eq!(
            index.get("wet_op_de_zorgtoeslag").map(Vec::as_slice),
            Some(["regeling_standaardpremie".to_string()].as_slice()),
            "the delegated regeling must be discovered via the reverse implements edge"
        );
        // The higher law implements nothing, so it has no reverse entry.
        assert!(!index.contains_key("regeling_standaardpremie"));
    }

    #[test]
    fn body_deserializes_with_and_without_date() {
        let body: HarvestDependenciesBody =
            serde_json::from_str(r#"{"law_id":"wet_op_de_zorgtoeslag","date":"2025-01-01"}"#)
                .unwrap();
        assert_eq!(body.law_id, "wet_op_de_zorgtoeslag");
        assert_eq!(body.date.as_deref(), Some("2025-01-01"));

        let body: HarvestDependenciesBody =
            serde_json::from_str(r#"{"law_id":"BWBR0050536"}"#).unwrap();
        assert_eq!(body.law_id, "BWBR0050536");
        assert!(body.date.is_none());
    }
}
