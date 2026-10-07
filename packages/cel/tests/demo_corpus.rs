//! The cell of the demo (`corpus/demo/cells/toeslagen`) over the demo corpus:
//! the same aanvraag, voorschot and toekenning as on the main corpus, so the
//! browser shows what the tests prove.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use regelrecht_cel::cell::load_regulations;
use regelrecht_cel::extension::{PeriodParameter, PeriodUnit};
use regelrecht_cel::Cell;
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::json;

/// The fictional test person of the demo scenarios.
const BSN: &str = "999993653";
/// What the citizen expects to earn over 2025 (Awir 16), in eurocent.
const ESTIMATE: i64 = 2_500_000;

fn demo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/demo")
}

fn regulations() -> LawExecutionService {
    load_regulations(&demo().join("regulation")).unwrap_or_else(|e| panic!("{e}"))
}

/// What the registers know of the citizen, as the demo scenario "Persoon
/// boven 18 heeft recht op zorgtoeslag" (Zorgtoeslagwet 2025) registers it,
/// with `income` as the wages over the year.
fn register(service: &mut LawExecutionService, income: i64) {
    let row = |pairs: Vec<(&str, Value)>| -> BTreeMap<String, Value> {
        let mut row: BTreeMap<String, Value> =
            pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        row.insert("bsn".into(), Value::String(BSN.into()));
        row
    };
    let s = |v: &str| Value::String(v.into());
    let mut belastingdienst = row(vec![("loon_uit_dienstbetrekking", Value::Int(income))]);
    for field in [
        "uitkeringen_en_pensioenen",
        "winst_uit_onderneming",
        "resultaat_overige_werkzaamheden",
        "eigen_woning",
        "reguliere_voordelen",
        "vervreemdingsvoordelen",
        "spaargeld",
        "beleggingen",
        "onroerend_goed",
        "schulden",
        "persoonsgebonden_aftrek",
        "partner_loon_uit_dienstbetrekking",
        "partner_uitkeringen_en_pensioenen",
        "partner_winst_uit_onderneming",
        "partner_resultaat_overige_werkzaamheden",
        "partner_eigen_woning",
        "partner_reguliere_voordelen",
        "partner_vervreemdingsvoordelen",
        "partner_spaargeld",
        "partner_beleggingen",
        "partner_onroerend_goed",
        "partner_schulden",
        "partner_buitenlands_inkomen",
        "buitenlands_inkomen",
    ] {
        belastingdienst.insert(field.into(), Value::Int(0));
    }
    let sources = [
        (
            "penitentiaire_beginselenwet",
            "DJI",
            row(vec![
                ("status", Value::Null),
                ("inrichting_type", Value::Null),
            ]),
        ),
        (
            "wet_brp",
            "RvIG",
            row(vec![
                ("geboortedatum", s("2005-01-01")),
                ("partnerschap_type", s("GEEN")),
                ("partner_bsn", Value::Null),
                ("kinderen_gegevens", Value::Array(Vec::new())),
                ("ouder_adressen", Value::Array(Vec::new())),
                ("land_verblijf", s("NEDERLAND")),
                ("nationaliteit", Value::Null),
                ("adres", Value::Null),
                ("medebewoners", Value::Array(Vec::new())),
                ("partner_geboortedatum", Value::Null),
            ]),
        ),
        ("wet_inkomstenbelasting", "BELASTINGDIENST", belastingdienst),
        (
            "zvw",
            "RVZ",
            row(vec![
                ("polis_status", s("ACTIEF")),
                ("registratie", Value::Null),
            ]),
        ),
    ];
    for (law, name, record) in sources {
        service
            .register_dict_source_for_law(law, name, "bsn", vec![record], 10)
            .unwrap();
    }
}

/// The hoogte Zorgtoeslagwet art. 2 gives on `day` for the citizen with
/// `income`, without any cell.
fn hoogte_on(income: i64, day: &str) -> serde_json::Value {
    let mut direct = regulations();
    register(&mut direct, income);
    let result = direct
        .evaluate_law_output(
            "zorgtoeslagwet",
            "hoogte_toeslag",
            BTreeMap::from([("bsn".to_string(), Value::String(BSN.into()))]),
            day,
        )
        .unwrap();
    serde_json::to_value(&result.outputs["hoogte_toeslag"]).unwrap()
}

#[test]
fn the_demo_cell_records_an_application_for_zorgtoeslag() {
    let service = regulations();
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
            // Awir 16 hooks on the application of Awir 15.
            "vermoedelijk_toetsingsinkomen",
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

    // Both decisions concern the berekeningsjaar applied for, each at its
    // own stage of the procedure of the Awir.
    for (event, stage) in [
        ("voorschot_verleend", "VOORSCHOT"),
        ("zorgtoeslag_toegekend", "TOEKENNING"),
    ] {
        let (decision, _) = cell.shape(&service, event, received.date_naive()).unwrap();
        assert_eq!(decision.stage.as_deref(), Some(stage));
        assert_eq!(
            decision.period,
            Some(PeriodParameter {
                parameter: "aangevraagd_berekeningsjaar".into(),
                unit: PeriodUnit::Year,
            })
        );
        let has = |name: &str| decision.field(name).is_some();
        assert!(has("hoogte_toeslag") && has("tegemoetkoming"), "{event}");
        assert!(
            has("bezwaartermijn_weken") && has("motivering_vereist"),
            "{event}"
        );
        assert_eq!(has("voorschotbedrag"), stage == "VOORSCHOT", "{event}");
    }
}

/// The persona of the demo applies with an estimate: the voorschot rests on
/// it, the toekenning on the income the registers know.
#[test]
fn the_demo_cell_grants_a_voorschot_on_the_estimate() {
    let mut service = regulations();
    let received = DateTime::parse_from_rfc3339("2025-03-04T10:15:00+01:00").unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut cell = Cell::open(
        &demo().join("cells/toeslagen/cell.yaml"),
        &service,
        data.path(),
        received.date_naive(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let application = cell
        .record_submission(
            &service,
            "aanvraag_ontvangen",
            json!({
                "bsn": BSN,
                "aangevraagd_berekeningsjaar": 2025,
                "vermoedelijk_toetsingsinkomen": ESTIMATE,
            })
            .as_object()
            .unwrap(),
            received,
        )
        .unwrap();
    register(&mut service, 79547);
    let refers_to = BTreeMap::from([("on_application".to_string(), application.id.clone())]);

    let decided = DateTime::parse_from_rfc3339("2025-04-15T09:00:00+02:00").unwrap();
    let voorschot = cell
        .decide(
            &service,
            "voorschot_verleend",
            refers_to.clone(),
            BTreeMap::new(),
            decided,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let on_estimate = hoogte_on(ESTIMATE, "2025-01-01");
    assert_eq!(voorschot.fields["toetsingsinkomen"], ESTIMATE);
    assert_eq!(voorschot.fields["hoogte_toeslag"], on_estimate);
    let hoogte = on_estimate.as_i64().unwrap();
    assert_eq!(
        voorschot.fields["voorschotbedrag"],
        json!((hoogte + 50) / 100 * 100)
    );
    assert_eq!(voorschot.fields["bezwaartermijn_weken"], json!(6));

    let toegekend = DateTime::parse_from_rfc3339("2026-06-01T09:00:00+02:00").unwrap();
    let toekenning = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            refers_to,
            BTreeMap::new(),
            toegekend,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        toekenning.fields["hoogte_toeslag"],
        hoogte_on(79547, "2025-01-01")
    );
    assert_ne!(toekenning.fields["hoogte_toeslag"], on_estimate);
}
