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
