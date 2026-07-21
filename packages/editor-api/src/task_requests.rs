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
    headers: axum::http::HeaderMap,
) -> Result<(StatusCode, Json<EnrichRequestResponse>), (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database niet geconfigureerd".to_string(),
    ))?;

    // Zelfde guard + resolutie als de document-upload: membership-check en
    // traject-id voor de payload/taak.
    let traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let traject_id = resolve_traject_ref(pool, &traject_ref).await?;

    // Snapshot de wet zoals de gebruiker hem nu ziet (traject-scope),
    // inclusief het per-request leestoken voor de writable-own source —
    // zonder service-token leest de wet-body anders via de seed of 404't.
    let scope = ReadScope::for_traject(&state, account.id, &headers, traject).await;
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
        new_law: None,
    };
    let payload_json = serde_json::to_value(&payload).map_err(internal("payload serialiseren"))?;

    // Input-blob + job in één transactie (patroon document-upload), idempotent
    // op (law_id, provider) via de bestaande unieke actieve-enrich-index.
    let mut tx = pool.begin().await.map_err(internal("begin tx"))?;

    enforce_task_job_cap(&mut tx, account.id).await?;

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

/// Bovengrens op de meegegeven weergavenaam: hij landt in taak-titels en in
/// de job-payload, dus onbegrensde user input hoort daar niet thuis.
const MAX_LAW_NAME_CHARS: usize = 200;

#[derive(serde::Deserialize)]
pub struct TrajectHarvestRequest {
    pub bwb_id: String,
    /// Weergavenaam uit de zoekresultaten (optioneel; alleen voor titels).
    #[serde(default)]
    pub law_name: Option<String>,
}

#[derive(serde::Serialize)]
pub struct TrajectHarvestResponse {
    pub job_id: Uuid,
}

/// Valideer en normaliseer een door de gebruiker aangeleverd BWB-id.
/// Canonieke vorm is hoofdletters (`BWBR0018451`); lowercase input wordt
/// genormaliseerd. De vorm-check is bewust ruim (BWBR/BWBV/…): de harvester
/// zelf is de autoriteit over wat echt bestaat.
fn normalize_bwb_id(raw: &str) -> Option<String> {
    let id = raw.trim().to_ascii_uppercase();
    let ok = id.starts_with("BWB")
        && (6..=20).contains(&id.len())
        && id.chars().all(|c| c.is_ascii_alphanumeric());
    ok.then_some(id)
}

/// POST /api/trajects/{traject_ref}/corpus/harvest
///
/// Start een traject-scoped harvest via het taken-mechanisme: enqueue een
/// `traject_harvest`-job (BWB-download op de harvest-worker) die een
/// taak-flow-enrich ketent (zie `pipeline::traject_harvest`). De aanvraag is
/// direct zichtbaar in het takenpaneel als lopende aanvraag; het resultaat
/// komt terug als `law_create`-review-taak van de aanvrager.
pub async fn request_traject_harvest(
    State(state): State<AppState>,
    session: Session,
    Extension(account): Extension<AccountRecord>,
    Path(traject_ref): Path<String>,
    axum::Json(req): axum::Json<TrajectHarvestRequest>,
) -> Result<(StatusCode, Json<TrajectHarvestResponse>), (StatusCode, String)> {
    let pool = state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "database niet geconfigureerd".to_string(),
    ))?;

    let bwb_id = normalize_bwb_id(&req.bwb_id).ok_or((
        StatusCode::BAD_REQUEST,
        "Geen geldig BWB-id (verwacht bijv. BWBR0018451).".to_string(),
    ))?;
    let law_name = req
        .law_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(MAX_LAW_NAME_CHARS).collect::<String>());

    // Zelfde guard + resolutie als de enrich-aanvraag: membership-check en
    // traject-id voor de payload/taak.
    let _traject = require_traject_corpus_from_ref(&state, &session, &traject_ref).await?;
    let traject_id = resolve_traject_ref(pool, &traject_ref).await?;

    let payload = regelrecht_pipeline::traject_harvest::TrajectHarvestPayload {
        bwb_id: bwb_id.clone(),
        traject_id,
        traject_ref: traject_ref.clone(),
        law_name,
        provider: Some(state.config.task_enrich_provider.clone()),
        requested_by: Some(account.id),
        deliver: Some("task".into()),
    };
    let payload_json = serde_json::to_value(&payload).map_err(internal("payload serialiseren"))?;

    let mut tx = pool.begin().await.map_err(internal("begin tx"))?;

    enforce_task_job_cap(&mut tx, account.id).await?;

    // Serialiseer per (traject, wet) en dedup dan tegen actieve jobs: twee
    // leden die dezelfde wet tegelijk aanvragen zouden anders allebei langs
    // de SELECT komen (zelfde TOCTOU-klasse en remedie als harvest_request).
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('traject_harvest:' || $1 || ':' || $2, 0))",
    )
    .bind(&traject_ref)
    .bind(&bwb_id)
    .execute(&mut *tx)
    .await
    .map_err(internal("harvest-lock nemen"))?;

    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM jobs \
         WHERE job_type = 'traject_harvest' AND law_id = $1 AND traject_ref = $2 \
           AND status IN ('pending', 'processing') \
         LIMIT 1",
    )
    .bind(&bwb_id)
    .bind(&traject_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal("actieve harvest-jobs zoeken"))?;
    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "Er loopt al een ophaal-aanvraag voor deze wet in dit traject.".to_string(),
        ));
    }

    // max_attempts 3: BWB-outages en netwerkfouten zijn transiënt en de
    // harvest is idempotent; deterministische fouten failt de worker
    // terminaal (zie process_next_traject_harvest_job).
    let req = CreateJobRequest::new(JobType::TrajectHarvest, &bwb_id)
        .with_traject_ref(&traject_ref)
        .with_priority(Priority::new(TASK_ENRICH_PRIORITY))
        .with_payload(payload_json)
        .with_max_attempts(3);
    let job = job_queue::create_job(&mut *tx, req)
        .await
        .map_err(internal("enqueue traject-harvest"))?;
    tx.commit().await.map_err(internal("commit"))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(TrajectHarvestResponse { job_id: job.id }),
    ))
}

/// Per-account-cap op actieve taak-flow-jobs, binnen de transactie van de
/// aanroeper. Serialiseert eerst per account (xact-scoped advisory lock):
/// de COUNT-then-INSERT is anders een TOCTOU-race (zelfde klasse en remedie
/// als harvest_request). Telt naast enrich- ook law_convert- en
/// traject_harvest-jobs mee: die ketenen immers elk een enrich-job na.
/// Gedeeld door `request_enrich`, `request_traject_harvest` en de law-upload
/// in `corpus_handlers`.
pub(crate) async fn enforce_task_job_cap(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: Uuid,
) -> Result<(), (StatusCode, String)> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('task_enrich:' || $1::text, 0))")
        .bind(account_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(internal("account-lock nemen"))?;

    let (active_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM jobs \
         WHERE job_type IN ('enrich', 'law_convert', 'traject_harvest') \
           AND status IN ('pending', 'processing') \
           AND payload->>'requested_by' = $1",
    )
    .bind(account_id.to_string())
    .fetch_one(&mut **tx)
    .await
    .map_err(internal("actieve taak-jobs tellen"))?;
    if active_count >= MAX_ACTIVE_TASK_JOBS_PER_ACCOUNT {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Je hebt te veel verrijkingen tegelijk lopen; wacht tot er een paar klaar zijn."
                .to_string(),
        ));
    }
    Ok(())
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

    #[test]
    fn normalize_bwb_id_accepts_and_uppercases_valid_ids() {
        assert_eq!(
            normalize_bwb_id("BWBR0018451").as_deref(),
            Some("BWBR0018451")
        );
        assert_eq!(
            normalize_bwb_id("  bwbr0018451 ").as_deref(),
            Some("BWBR0018451")
        );
        // Verdragen (BWBV) vallen ook onder BWB.
        assert_eq!(
            normalize_bwb_id("BWBV0001000").as_deref(),
            Some("BWBV0001000")
        );
    }

    #[test]
    fn normalize_bwb_id_rejects_garbage() {
        assert!(normalize_bwb_id("").is_none());
        assert!(normalize_bwb_id("BWB").is_none());
        assert!(normalize_bwb_id("CVDR681386").is_none());
        assert!(normalize_bwb_id("BWBR00184-1").is_none());
        assert!(normalize_bwb_id("BWBR001845100000000000").is_none());
        assert!(normalize_bwb_id("wet_op_de_zorgtoeslag").is_none());
    }
}
