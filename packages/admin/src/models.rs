use chrono::{DateTime, Utc};
use serde::Serialize;

pub use regelrecht_pipeline::{JobStatus, JobType, LawStatusValue};

#[derive(Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct LawEntry {
    pub law_id: String,
    pub law_name: Option<String>,
    pub slug: Option<String>,
    pub status: LawStatusValue,
    pub coverage_score: Option<f64>,
    pub harvest_job_id: Option<sqlx::types::Uuid>,
    pub enrich_job_id: Option<sqlx::types::Uuid>,
    pub harvest_fail_count: i32,
    pub enrich_fail_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Job {
    pub id: sqlx::types::Uuid,
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
}
