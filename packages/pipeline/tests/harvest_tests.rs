use regelrecht_pipeline::harvest::{HarvestPayload, HarvestResult};

#[test]
fn test_harvest_payload_from_json_full() {
    let json = serde_json::json!({
        "bwb_id": "BWBR0018451",
        "date": "2025-01-01",
        "max_size_mb": 100
    });

    let payload: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(payload.bwb_id.as_deref(), Some("BWBR0018451"));
    assert!(payload.cvdr_id.is_none());
    assert_eq!(payload.date.as_deref(), Some("2025-01-01"));
    assert_eq!(payload.max_size_mb, Some(100));
    assert!(payload.depth.is_none());
}

#[test]
fn test_harvest_payload_from_json_minimal() {
    let json = serde_json::json!({ "bwb_id": "BWBR0018451" });

    let payload: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(payload.bwb_id.as_deref(), Some("BWBR0018451"));
    assert!(payload.cvdr_id.is_none());
    assert!(payload.date.is_none());
    assert!(payload.max_size_mb.is_none());
    assert!(payload.depth.is_none());
}

#[test]
fn test_harvest_payload_cvdr() {
    let json = serde_json::json!({
        "cvdr_id": "CVDR681386",
        "date": "2025-06-01"
    });

    let payload: HarvestPayload = serde_json::from_value(json).unwrap();
    assert!(payload.bwb_id.is_none());
    assert_eq!(payload.cvdr_id.as_deref(), Some("CVDR681386"));
    assert_eq!(payload.date.as_deref(), Some("2025-06-01"));
}

#[test]
fn test_harvest_payload_with_depth() {
    let json = serde_json::json!({
        "bwb_id": "BWBR0018451",
        "date": "2025-01-01",
        "depth": 2
    });

    let payload: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(payload.depth, Some(2));
}

#[test]
fn test_harvest_payload_roundtrip() {
    let payload = HarvestPayload {
        bwb_id: Some("BWBR0018451".to_string()),
        cvdr_id: None,
        date: Some("2025-01-01".to_string()),
        max_size_mb: None,
        depth: Some(1),
    };

    let json = serde_json::to_value(&payload).unwrap();
    let back: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(back.bwb_id, payload.bwb_id);
    assert_eq!(back.cvdr_id, payload.cvdr_id);
    assert_eq!(back.date, payload.date);
    assert_eq!(back.max_size_mb, payload.max_size_mb);
    assert_eq!(back.depth, payload.depth);
}

#[test]
fn test_harvest_payload_roundtrip_cvdr() {
    let payload = HarvestPayload {
        bwb_id: None,
        cvdr_id: Some("CVDR681386".to_string()),
        date: Some("2025-06-01".to_string()),
        max_size_mb: None,
        depth: None,
    };

    let json = serde_json::to_value(&payload).unwrap();
    let back: HarvestPayload = serde_json::from_value(json).unwrap();
    assert_eq!(back.bwb_id, None);
    assert_eq!(back.cvdr_id.as_deref(), Some("CVDR681386"));
}

#[test]
fn test_harvest_payload_skip_none_fields() {
    let payload = HarvestPayload {
        bwb_id: Some("BWBR0018451".to_string()),
        cvdr_id: None,
        date: None,
        max_size_mb: None,
        depth: None,
    };

    let json = serde_json::to_string(&payload).unwrap();
    assert!(!json.contains("cvdr_id"));
    assert!(!json.contains("date"));
    assert!(!json.contains("max_size_mb"));
    assert!(!json.contains("depth"));
}

#[test]
fn test_harvest_payload_law_id_helper() {
    let bwb = HarvestPayload {
        bwb_id: Some("BWBR0018451".to_string()),
        cvdr_id: None,
        date: None,
        max_size_mb: None,
        depth: None,
    };
    assert_eq!(bwb.law_id(), Some("BWBR0018451"));

    let cvdr = HarvestPayload {
        bwb_id: None,
        cvdr_id: Some("CVDR681386".to_string()),
        date: None,
        max_size_mb: None,
        depth: None,
    };
    assert_eq!(cvdr.law_id(), Some("CVDR681386"));
}

/// Backward compatibility: payloads with `bwb_id` as a plain String
/// (from existing queued jobs) must still deserialize.
#[test]
fn test_harvest_payload_backward_compat() {
    let json = r#"{"bwb_id":"BWBR0018451","date":"2025-01-01"}"#;
    let payload: HarvestPayload = serde_json::from_str(json).unwrap();
    assert_eq!(payload.bwb_id.as_deref(), Some("BWBR0018451"));
    assert!(payload.cvdr_id.is_none());
}

/// Empty JSON object should deserialize with bwb_id defaulting to None.
#[test]
fn test_harvest_payload_empty_json() {
    let json = r#"{}"#;
    let payload: HarvestPayload = serde_json::from_str(json).unwrap();
    assert!(payload.bwb_id.is_none());
    assert!(payload.cvdr_id.is_none());
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
        referenced_bwb_ids: vec!["BWBR0002629".to_string()],
        harvest_date: "2025-01-01".to_string(),
        source_type: "bwb".to_string(),
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["law_name"], "Zorgtoeslagwet");
    assert_eq!(json["slug"], "zorgtoeslagwet");
    assert_eq!(json["layer"], "WET");
    assert_eq!(json["article_count"], 15);
    assert_eq!(json["warning_count"], 3);
    assert_eq!(json["warnings"].as_array().unwrap().len(), 3);
    assert_eq!(json["harvest_date"], "2025-01-01");
    assert_eq!(json["source_type"], "bwb");
    assert_eq!(json["referenced_bwb_ids"].as_array().unwrap().len(), 1);
}

#[test]
fn test_harvest_result_cvdr_source_type() {
    let result = HarvestResult {
        law_name: "Verordening test".to_string(),
        slug: "verordening_test".to_string(),
        layer: "VERORDENING".to_string(),
        file_path: "/tmp/regulation/nl/verordening/verordening_test/2025-01-01.yaml".to_string(),
        article_count: 5,
        warning_count: 0,
        warnings: vec![],
        referenced_bwb_ids: vec![],
        harvest_date: "2025-01-01".to_string(),
        source_type: "cvdr".to_string(),
    };

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["source_type"], "cvdr");
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
        bwb_id: Some("BWBR0018451".to_string()),
        cvdr_id: None,
        date: Some("2025-01-01".to_string()),
        max_size_mb: Some(50),
        depth: None,
    };

    let client = regelrecht_harvester::http::create_client().unwrap();
    let (result, written_files) = execute_harvest(&payload, repo_path, "regulation/nl", &client)
        .await
        .unwrap();

    assert!(!result.law_name.is_empty());
    assert!(!result.slug.is_empty());
    assert!(result.article_count > 0);
    assert_eq!(result.source_type, "bwb");
    assert_eq!(written_files.len(), 2);
    for f in &written_files {
        assert!(f.exists(), "expected file to exist: {}", f.display());
    }
}
