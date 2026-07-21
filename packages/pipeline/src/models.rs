use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Harvest,
    Enrich,
    /// Convert an uploaded document (PDF/Word) to a markdown werkdocument via
    /// the LLM agent. Scoped to a traject (see [`Job::traject_ref`]).
    #[sqlx(rename = "document_convert")]
    #[serde(rename = "document_convert")]
    DocumentConvert,
    /// Convert an uploaded document (PDF/Word) to a harvested base-law YAML
    /// and chain a task-flow enrich job on it. Scoped to a traject.
    #[sqlx(rename = "law_convert")]
    #[serde(rename = "law_convert")]
    LawConvert,
    /// Harvest a law from BWB for one traject (task flow): download + parse
    /// the base-law YAML and chain a task-flow enrich job on it, exactly like
    /// [`JobType::LawConvert`] does after its conversion. Unlike
    /// [`JobType::Harvest`] this never touches the central corpus repo — the
    /// result travels as job blobs and lands via the review task's approve.
    #[sqlx(rename = "traject_harvest")]
    #[serde(rename = "traject_harvest")]
    TrajectHarvest,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    sqlx::Type,
    strum::EnumIter,
    strum::Display,
)]
#[sqlx(type_name = "job_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    sqlx::Type,
    strum::EnumIter,
    strum::Display,
)]
#[sqlx(type_name = "law_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "snake_case")]
pub enum LawStatusValue {
    Unknown,
    Queued,
    Harvesting,
    Harvested,
    #[sqlx(rename = "harvest_failed")]
    #[serde(rename = "harvest_failed")]
    HarvestFailed,
    #[sqlx(rename = "harvest_exhausted")]
    #[serde(rename = "harvest_exhausted")]
    HarvestExhausted,
    Enriching,
    Enriched,
    #[sqlx(rename = "enrich_failed")]
    #[serde(rename = "enrich_failed")]
    EnrichFailed,
    #[sqlx(rename = "enrich_exhausted")]
    #[serde(rename = "enrich_exhausted")]
    EnrichExhausted,
    /// No consolidated text is available to harvest (the work is withdrawn, not
    /// yet in force, or only announced). Terminal — the precise reason and date
    /// are recorded in the harvest job's result. Future laws can be re-harvested
    /// manually once their text appears.
    #[sqlx(rename = "not_harvestable")]
    #[serde(rename = "not_harvestable")]
    NotHarvestable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(i32);

impl Priority {
    pub fn new(value: i32) -> Self {
        Self(value.clamp(0, 100))
    }

    pub fn value(self) -> i32 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self(50)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub job_type: JobType,
    pub law_id: String,
    pub status: JobStatus,
    pub priority: i32,
    pub payload: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub progress: Option<serde_json::Value>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Earliest moment the job may be claimed. `None` means claimable
    /// immediately; set by the retry-backoff logic in `fail_job`.
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LawEntry {
    pub law_id: String,
    pub law_name: Option<String>,
    pub slug: Option<String>,
    pub status: LawStatusValue,
    pub harvest_job_id: Option<Uuid>,
    pub enrich_job_id: Option<Uuid>,
    /// Completeness metric: fraction of articles that received a
    /// `machine_readable` section during enrichment. This measures COVERAGE,
    /// not CORRECTNESS — a score of 1.0 means every article was modelled, not
    /// that the modelling is legally faithful. Correctness is checked elsewhere
    /// (schema/cross-law gates, BDD, and the methodological drift/desk-review).
    pub coverage_score: Option<f64>,
    pub harvest_fail_count: i32,
    pub enrich_fail_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single untranslatable construct captured during enrichment (RFC-012),
/// one row per (law, provider, article, construct). Mirrors the
/// `untranslatables` table; refreshed per (law_id, provider) on each enrich.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Untranslatable {
    pub id: Uuid,
    pub law_id: String,
    pub enrich_job_id: Uuid,
    pub provider: String,
    pub article: String,
    pub construct: String,
    pub reason: String,
    pub suggestion: Option<String>,
    pub legal_text_excerpt: Option<String>,
    pub accepted: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FeatureFlag {
    pub key: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
