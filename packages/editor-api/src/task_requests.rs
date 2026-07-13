//! Enrich-op-aanvraag: een gebruiker laat een wet uit zijn traject verrijken.
//!
//! Taak-flow (spec: taken-mechanisme): dit endpoint snapshot de wet-YAML als
//! input-blob in Postgres en enqueue't een enrich-job met `deliver: "task"`.
//! De worker raakt GitHub nooit aan; het resultaat komt terug als review-taak
//! van de aanvrager, en de goedkeuring loopt via het gewone law-save-pad.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::{JobType, Priority};
use regelrecht_pipeline::tasks::{self, BlobKind};

use crate::accounts::AccountRecord;
use crate::corpus_handlers::{
    document_etag, read_law_yaml, require_traject_corpus_from_ref, ReadScope,
};
use crate::state::AppState;
use crate::trajects::resolve_traject_ref;

/// Bovengrens op de gesnapshotte wet-YAML. Ruim boven elke echte wet, klein
/// genoeg dat job_blobs geen blob-opslag wordt (zelfde motivatie als de
/// user_notes-caps).
const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Prioriteit boven de corpus-brede bulk (default 50): een mens wacht erop.
const TASK_ENRICH_PRIORITY: i32 = 80;

/// Maximum aantal gelijktijdig actieve (pending/processing) taak-flow-jobs
/// per account. Beschermt de prio-80-queue en het LLM-uurbudget tegen een
/// scripted flood; ruim boven normaal menselijk gebruik.
const MAX_ACTIVE_TASK_JOBS_PER_ACCOUNT: i64 = 20;

#[derive(serde::Serialize)]
pub struct EnrichRequestResponse {
    pub job_id: Uuid,
}

/// POST /api/trajects/{traject_ref}/corpus/laws/{law_id}/enrich
pub async fn request_enrich(
    State(state): State<AppState>,
    session: Session,
    Extension(account): Extension<AccountRecord>,
    Path((traject_ref, law_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<EnrichRequestResponse>), (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database niet geconfigureerd".to_string(),
    ))?;

    // Zelfde guard + resolutie als de document-upload: membership-check en
    // traject-id voor de payload/taak.
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let traject_id = resolve_traject_ref(pool, &traject_ref).await?;

    // Snapshot de wet zoals de gebruiker hem nu ziet (traject-scope).
    let scope = ReadScope::Traject(traject);
    let yaml = read_law_yaml(&scope, &law_id).await?;
    if yaml.len() > MAX_INPUT_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Deze wet is te groot om via een taak te verrijken.".to_string(),
        ));
    }
    let source_etag = document_etag(&yaml);

    // Synthetisch repo-relatief pad: de parent-directorynaam fungeert als
    // slug voor de feature-file-detectie in execute_enrich.
    let yaml_path = format!("laws/{law_id}/law.yaml");

    let payload = regelrecht_pipeline::enrich::EnrichPayload {
        law_id: law_id.clone(),
        yaml_path: yaml_path.clone(),
        provider: Some(state.config.task_enrich_provider.clone()),
        depth: None,
        requested_by: Some(account.id),
        deliver: Some("task".into()),
        traject_id: Some(traject_id),
        traject_ref: Some(traject_ref.clone()),
        source_etag: Some(source_etag),
    };
    let payload_json = serde_json::to_value(&payload).map_err(internal("payload serialiseren"))?;

    // Input-blob + job in één transactie (patroon document-upload), idempotent
    // op (law_id, provider) via de bestaande unieke actieve-enrich-index.
    let mut tx = pool.begin().await.map_err(internal("begin tx"))?;

    // Per-account-cap: telt actieve taak-flow-jobs van dit account vóórdat we
    // er nog een bij enqueuen, zodat een scripted flood niet de prio-80-queue
    // of het LLM-uurbudget kan opsouperen.
    let (active_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs \
         WHERE job_type = 'enrich' AND status IN ('pending', 'processing') \
           AND payload->>'requested_by' = $1",
    )
    .bind(account.id.to_string())
    .fetch_one(&mut *tx)
    .await
    .map_err(internal("actieve taak-jobs tellen"))?;
    if active_count >= MAX_ACTIVE_TASK_JOBS_PER_ACCOUNT {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Je hebt te veel verrijkingen tegelijk lopen; wacht tot er een paar klaar zijn."
                .to_string(),
        ));
    }

    // max_attempts 3 (default expliciet): transiënte fouten (fork-exhaustion,
    // provider-hiccups) zijn retryable; de input-blob overleeft tot definitief
    // falen, dus een retry her-materialiseert gewoon.
    let req = CreateJobRequest::new(JobType::Enrich, &law_id)
        .with_traject_ref(&traject_ref)
        .with_priority(Priority::new(TASK_ENRICH_PRIORITY))
        .with_payload(payload_json)
        .with_max_attempts(3);
    let job = job_queue::create_enrich_job_if_not_exists(&mut *tx, req)
        .await
        .map_err(internal("enqueue enrich"))?;
    let Some(job) = job else {
        return Err((
            StatusCode::CONFLICT,
            "Er loopt al een verrijking voor deze wet.".to_string(),
        ));
    };
    tasks::insert_blob(&mut *tx, job.id, BlobKind::Input, &yaml_path, &yaml)
        .await
        .map_err(internal("input-blob opslaan"))?;
    tx.commit().await.map_err(internal("commit"))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(EnrichRequestResponse { job_id: job.id }),
    ))
}

fn internal<E: std::fmt::Display>(what: &'static str) -> impl FnOnce(E) -> (StatusCode, String) {
    move |e| {
        tracing::error!(error = %e, what, "enrich-aanvraag mislukt");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Er ging iets mis bij het aanvragen van de verrijking.".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// De per-account-cap query vergelijkt `payload->>'requested_by'` (een
    /// JSON-string) met `account.id.to_string()`. Dat is alleen correct als
    /// serde een `Uuid` als dezelfde lowercase-hyphenated string serialiseert
    /// als `Uuid::to_string()` — anders telt de query altijd 0 en is de cap
    /// een no-op. Pin dat aannames hier vast.
    #[test]
    fn uuid_serializes_to_same_string_as_to_string() {
        let id = Uuid::new_v4();
        let json = serde_json::to_value(id).expect("uuid serialiseert");
        assert_eq!(json, serde_json::Value::String(id.to_string()));
    }
}
