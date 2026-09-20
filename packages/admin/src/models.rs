use serde::Serialize;

// Row types come straight from the pipeline crate — admin reads the same
// tables, so re-declaring them here only created drift (every migration had
// to be mirrored by hand). Note the pipeline types carry two fields the old
// admin copies dropped (`LawEntry.slug`, `Job.scheduled_at`); they now appear
// in API responses, which is additive — the Vue frontend ignores unknown
// fields.
pub use regelrecht_pipeline::{Job, LawEntry};

#[derive(Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// A marking as the API returns it (schema v0.7.0). Like [`Untranslatable`],
/// this carries the joined `law_name` from `law_entries` for display, so it is
/// declared here rather than re-exported from the pipeline crate.
///
/// The successor of [`Untranslatable`]: both are served, because a law pinned
/// to schema v0.5.x still carries the old field and the engine still reads it.
/// The view merges the two rather than making a reader work out which schema
/// version a law happens to be on.
#[derive(Serialize, sqlx::FromRow)]
pub struct Marking {
    pub id: uuid::Uuid,
    pub law_id: String,
    pub law_name: Option<String>,
    pub enrich_job_id: uuid::Uuid,
    pub provider: String,
    pub article: String,
    pub about: String,
    pub resolution: String,
    pub resolved_by: Option<String>,
    pub target: Vec<String>,
    pub legal_text_excerpt: String,
    pub accepted: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// One cluster of markings that name the same change, for the backlog view.
///
/// The schema says it outright: "Grouping markings by this field is how the
/// backlog is read off the corpus." One marking is an observation; the same
/// `resolved_by` on four articles across two laws is a pattern, and the
/// pattern is what decides whether this is worth building.
///
/// `providers` carries the triage signal. A change that only one provider ever
/// asks for is evidence the agent did not see what another one modelled, which
/// is work on the enricher rather than on the format.
#[derive(Serialize, sqlx::FromRow)]
pub struct MarkingCluster {
    pub resolution: String,
    pub resolved_by: Option<String>,
    /// Markings in this cluster.
    pub markings: i64,
    /// Distinct laws they sit in. A change wanted by five articles in one law
    /// is a weaker signal than one wanted by three articles across three.
    pub laws: i64,
    /// Distinct articles blocked.
    pub articles: i64,
    /// Which providers asked for this change.
    pub providers: Vec<String>,
    /// Whether every marking in the cluster has been accepted by a human.
    pub all_accepted: bool,
}

/// A captured untranslatable (RFC-012) as returned by the harvester API. Unlike
/// the pipeline row type, this carries the joined `law_name` (from `law_entries`)
/// for display; the join is a LEFT JOIN, so `law_name` may be `None`.
#[derive(Serialize, sqlx::FromRow)]
pub struct Untranslatable {
    pub id: uuid::Uuid,
    pub law_id: String,
    pub law_name: Option<String>,
    pub enrich_job_id: uuid::Uuid,
    pub provider: String,
    pub article: String,
    pub construct: String,
    pub reason: String,
    pub suggestion: Option<String>,
    pub legal_text_excerpt: Option<String>,
    pub accepted: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
