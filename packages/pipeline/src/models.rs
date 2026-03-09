use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Harvest,
    Enrich,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, strum::EnumIter,
)]
#[sqlx(type_name = "job_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl JobStatus {
    /// Returns the lowercase string representation matching the DB/serde format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, strum::EnumIter,
)]
#[sqlx(type_name = "law_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum LawStatusValue {
    Unknown,
    Queued,
    Harvesting,
    Harvested,
    #[sqlx(rename = "harvest_failed")]
    #[serde(rename = "harvest_failed")]
    HarvestFailed,
    Enriching,
    Enriched,
    #[sqlx(rename = "enrich_failed")]
    #[serde(rename = "enrich_failed")]
    EnrichFailed,
}

impl LawStatusValue {
    /// Returns the lowercase/snake_case string representation matching the DB/serde format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Queued => "queued",
            Self::Harvesting => "harvesting",
            Self::Harvested => "harvested",
            Self::HarvestFailed => "harvest_failed",
            Self::Enriching => "enriching",
            Self::Enriched => "enriched",
            Self::EnrichFailed => "enrich_failed",
        }
    }
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
    pub attempts: i32,
    pub max_attempts: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LawEntry {
    pub law_id: String,
    pub law_name: Option<String>,
    pub status: LawStatusValue,
    pub harvest_job_id: Option<Uuid>,
    pub enrich_job_id: Option<Uuid>,
    pub quality_score: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
