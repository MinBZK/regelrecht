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
