pub mod api;
mod api_state;
pub mod config;
pub mod db;
pub mod document_convert;
pub mod enrich;
pub mod error;
pub mod feature_flags;
pub mod harvest;
pub mod harvest_request;
pub mod health;
pub mod job_queue;
pub mod law_status;
pub mod models;
#[cfg(feature = "test-utils")]
pub mod test_utils;
pub mod untranslatables;
pub mod worker;

pub use api_state::ApiState;

pub use config::{PipelineConfig, WorkerConfig};
pub use db::{create_pool, ensure_schema, MIGRATION_LOCK_KEY};
pub use enrich::{
    progress_file_path, EnrichConfig, EnrichPayload, EnrichResult, EnrichmentMetadata,
    EnrichmentResultEnvelope, LlmProvider, LlmRunner, ProcessLlmRunner, RelatedLegislation,
    ENRICH_PROVIDERS,
};
pub use error::PipelineError;
pub use harvest::{HarvestPayload, HarvestResult, MAX_HARVEST_DEPTH};
// Deliberately no crate-root re-export of the `request_harvest` fn: the axum
// handler `api::harvest::request_harvest` shares that name, so callers import
// it path-qualified via the module.
pub use harvest_request::{HarvestRequestOptions, HarvestRequestOutcome};
pub use models::{
    FeatureFlag, Job, JobStatus, JobType, LawEntry, LawStatusValue, Priority, Untranslatable,
};
