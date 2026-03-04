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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
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
