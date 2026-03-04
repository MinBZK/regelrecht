use regelrecht_pipeline::harvest::{HarvestPayload, HarvestResult};

#[test]
fn test_harvest_payload_from_json_full() {
    let json = serde_json::json!({
        "bwb_id": "BWBR0018451",
        "date": "2025-01-01",
        "max_size_mb": 100
    });

    let payload: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(payload.bwb_id, "BWBR0018451");
    assert_eq!(payload.date.as_deref(), Some("2025-01-01"));
    assert_eq!(payload.max_size_mb, Some(100));
}

#[test]
fn test_harvest_payload_from_json_minimal() {
    let json = serde_json::json!({ "bwb_id": "BWBR0018451" });

    let payload: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(payload.bwb_id, "BWBR0018451");
    assert!(payload.date.is_none());
    assert!(payload.max_size_mb.is_none());
}

#[test]
fn test_harvest_payload_roundtrip() {
    let payload = HarvestPayload {
        bwb_id: "BWBR0018451".to_string(),
        date: Some("2025-01-01".to_string()),
        max_size_mb: None,
    };

    let json = serde_json::to_value(&payload).unwrap();
    let back: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(back.bwb_id, payload.bwb_id);
    assert_eq!(back.date, payload.date);
    assert_eq!(back.max_size_mb, payload.max_size_mb);
}

#[test]
fn test_harvest_payload_skip_none_fields() {
    let payload = HarvestPayload {
        bwb_id: "BWBR0018451".to_string(),
        date: None,
        max_size_mb: None,
    };

    let json = serde_json::to_string(&payload).unwrap();
    assert!(!json.contains("date"));
    assert!(!json.contains("max_size_mb"));
}

#[test]
fn test_harvest_result_serialization() {
    let result = HarvestResult {
        law_name: "Zorgtoeslagwet".to_string(),
        slug: "zorgtoeslagwet".to_string(),
        layer: "WET".to_string(),
        file_path: "/tmp/regulation/nl/wet/zorgtoeslagwet/2025-01-01.yaml".to_string(),
        article_count: 15,
        warning_count: 3,
        warnings: vec![
            "warning 1".to_string(),
            "warning 2".to_string(),
            "warning 3".to_string(),
        ],
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["law_name"], "Zorgtoeslagwet");
    assert_eq!(json["slug"], "zorgtoeslagwet");
    assert_eq!(json["layer"], "WET");
    assert_eq!(json["article_count"], 15);
    assert_eq!(json["warning_count"], 3);
    assert_eq!(json["warnings"].as_array().unwrap().len(), 3);
}

/// Integration test: download and harvest a real law.
/// Requires network access to wetten.overheid.nl.
#[tokio::test]
#[ignore]
async fn test_execute_harvest_real_law() {
    use regelrecht_pipeline::harvest::execute_harvest;
    use tempfile::tempdir;

    let tmp = tempdir().unwrap();
    let repo_path = tmp.path();

    let payload = HarvestPayload {
        bwb_id: "BWBR0018451".to_string(),
        date: Some("2025-01-01".to_string()),
        max_size_mb: Some(50),
    };

    let (result, written_files) = execute_harvest(&payload, repo_path, "regulation/nl")
        .await
        .unwrap();

    assert!(!result.law_name.is_empty());
    assert!(!result.slug.is_empty());
    assert!(result.article_count > 0);
    assert_eq!(written_files.len(), 2);
    for f in &written_files {
        assert!(f.exists(), "expected file to exist: {}", f.display());
    }
}
