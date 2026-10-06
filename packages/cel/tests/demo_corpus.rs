//! The cell of the demo (`corpus/demo/cells/toeslagen`) over the demo corpus:
//! the same aanvraag as on the main corpus, so the browser shows what the
//! tests prove.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use chrono::DateTime;
use regelrecht_cel::cell::load_regulations;
use regelrecht_cel::extension::{PeriodParameter, PeriodUnit};
use regelrecht_cel::Cell;
use serde_json::json;

fn demo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/demo")
}

#[test]
fn the_demo_cell_records_an_application_for_zorgtoeslag() {
    let service = load_regulations(&demo().join("regulation")).unwrap_or_else(|e| panic!("{e}"));
    let received = DateTime::parse_from_rfc3339("2025-03-04T10:15:00+01:00").unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut cell = Cell::open(
        &demo().join("cells/toeslagen/cell.yaml"),
        &service,
        data.path(),
        received.date_naive(),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let (shape, _) = cell
        .shape(&service, "aanvraag_ontvangen", received.date_naive())
        .unwrap();
    let names: Vec<&str> = shape.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "bsn",
            "aangevraagd_berekeningsjaar",
            "ondertekening_partner",
            "naam_aanvrager",
            "adres_aanvrager",
            "dagtekening",
            "gevraagde_beschikking",
            "ondertekening",
        ]
    );

    let gram = cell
        .record_submission(
            &service,
            "aanvraag_ontvangen",
            json!({"bsn": "999100001", "aangevraagd_berekeningsjaar": 2025})
                .as_object()
                .unwrap(),
            received,
        )
        .unwrap();
    assert_eq!(gram.fields["gevraagde_beschikking"], "zorgtoeslagwet#2");

    // The decision concerns the berekeningsjaar applied for.
    let (decision, _) = cell
        .shape(&service, "zorgtoeslag_toegekend", received.date_naive())
        .unwrap();
    assert_eq!(
        decision.period,
        Some(PeriodParameter {
            parameter: "aangevraagd_berekeningsjaar".into(),
            unit: PeriodUnit::Year,
        })
    );
}
