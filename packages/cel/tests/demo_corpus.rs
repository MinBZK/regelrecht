//! The cell of the demo (`corpus/demo/cells/toeslagen`) over the demo corpus:
//! the same aanvraag, voorschot and toekenning as on the main corpus, so the
//! browser shows what the tests prove.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use regelrecht_cel::cell::load_regulations;
use regelrecht_cel::config::CellConfig;
use regelrecht_cel::extension::{PeriodParameter, PeriodUnit};
use regelrecht_cel::register;
use regelrecht_cel::{Cell, Gram};
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::json;

/// The fictional test person of the demo scenarios.
const BSN: &str = "999993653";
/// What the citizen expects to earn over 2025 (Awir 16), in eurocent.
const ESTIMATE: i64 = 2_500_000;

fn demo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/demo")
}

/// The demo corpus, with the registers of the demo cell bound, as the page
/// binds them when it starts the cell.
fn regulations() -> LawExecutionService {
    let mut service =
        load_regulations(&demo().join("regulation")).unwrap_or_else(|e| panic!("{e}"));
    let config = CellConfig::load(&demo().join("cells/toeslagen/cell.yaml"))
        .unwrap_or_else(|e| panic!("{e}"));
    register::bind(&mut service, &config).unwrap_or_else(|e| panic!("{e}"));
    service
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

fn at(moment: &str) -> DateTime<chrono::FixedOffset> {
    DateTime::parse_from_rfc3339(moment).unwrap()
}

/// The demo cell with the persona's application received at `received` and
/// the voorschot on it decided at `decided`; the registers know `income`.
fn with_voorschot(
    service: &mut LawExecutionService,
    data: &std::path::Path,
    received: &str,
    decided: &str,
    income: i64,
) -> (Cell, Gram, Gram) {
    let received = at(received);
    let mut cell = Cell::open(
        &demo().join("cells/toeslagen/cell.yaml"),
        service,
        data,
        received.date_naive(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let application = cell
        .record_submission(
            service,
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
        .unwrap_or_else(|e| panic!("{e}"));
    register(service, income);
    let voorschot = cell
        .decide(
            service,
            "voorschot_verleend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            BTreeMap::new(),
            at(decided),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    (cell, application, voorschot)
}

/// Pay the voorschottermijn of the month of the first of `year`-`month`.
fn pay(
    cell: &mut Cell,
    service: &LawExecutionService,
    root: &str,
    year: i32,
    month: u32,
) -> Option<Gram> {
    let now = at(&format!("{year}-{month:02}-01T10:00:00+01:00"));
    cell.execute(
        service,
        "voorschottermijn_betaald",
        root,
        now.date_naive(),
        now,
    )
    .unwrap_or_else(|e| panic!("{year}-{month}: {e}"))
}

fn amount(gram: &Gram, field: &str) -> i64 {
    gram.fields[field]
        .as_i64()
        .unwrap_or_else(|| panic!("{field}: {}", gram.fields[field]))
}

/// The demo pays the voorschot as the main corpus does: twelve termijnen
/// from December when it is granted before the year (Awir 22 lid 1), the
/// passed months at once when it is granted in March (lid 2 and 4).
#[test]
fn the_demo_cell_pays_the_voorschot_in_termijnen() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut paid = Vec::new();
    for (year, month) in std::iter::once((2024, 12)).chain((1..=11).map(|m| (2025, m))) {
        paid.push(pay(&mut cell, &service, &application.id, year, month).unwrap());
    }
    assert_eq!(pay(&mut cell, &service, &application.id, 2025, 12), None);
    let total: i64 = paid.iter().map(|g| amount(g, "termijnbedrag")).sum();
    assert_eq!(total, amount(&voorschot, "voorschotbedrag"));
    assert_eq!(paid[0].refers_to["voorschot"], voorschot.id);

    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2025-02-27T10:15:00+01:00",
        "2025-03-01T09:00:00+01:00",
        79547,
    );
    let paid: Vec<Gram> = (3..=11)
        .map(|m| pay(&mut cell, &service, &application.id, 2025, m).unwrap())
        .collect();
    let total: i64 = paid.iter().map(|g| amount(g, "termijnbedrag")).sum();
    assert_eq!(total, amount(&voorschot, "voorschotbedrag"));
    assert!(amount(&paid[0], "termijnbedrag") > amount(&paid[1], "termijnbedrag"));
}

/// The toekenning on the demo corpus sets off what was paid on the
/// voorschot (Awir 24): a nabetaling when the income is lower than the
/// estimate, a terugvordering when it is higher, nothing to recover up to
/// € 118 (Awir 26a). After it no termijn is paid.
#[test]
fn the_demo_toekenning_sets_off_the_paid_termijnen() {
    // (registered income, nabetaling?, recovered?)
    for (income, pays_out, recovers) in [(79547, true, false), (6_000_000, false, true)] {
        let data = tempfile::tempdir().unwrap();
        let mut service = regulations();
        let (mut cell, application, voorschot) = with_voorschot(
            &mut service,
            data.path(),
            "2025-02-27T10:15:00+01:00",
            "2025-03-01T09:00:00+01:00",
            income,
        );
        let paid: i64 = (3..=6)
            .map(|m| {
                let gram = pay(&mut cell, &service, &application.id, 2025, m).unwrap();
                amount(&gram, "termijnbedrag")
            })
            .sum();
        assert!(paid < amount(&voorschot, "voorschotbedrag"));
        let toekenning = cell
            .decide(
                &service,
                "zorgtoeslag_toegekend",
                BTreeMap::from([("on_application".to_string(), application.id.clone())]),
                BTreeMap::from([(
                    "datum_vaststelling_aanslag".to_string(),
                    regelrecht_cel::Input {
                        value: json!(null),
                        provenance: json!({"source": "dossier"}),
                    },
                )]),
                at("2026-06-01T09:00:00+02:00"),
            )
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(toekenning.inputs["uitbetaalde_voorschotten"]["value"], paid);
        let toegekend = amount(&toekenning, "toegekende_tegemoetkoming");
        let hoogte = hoogte_on(income, "2025-01-01").as_i64().unwrap();
        assert_eq!(toegekend, (hoogte + 50) / 100 * 100);
        let nog = amount(&toekenning, "nog_uit_te_betalen");
        let terug = amount(&toekenning, "terug_te_vorderen");
        assert_eq!(nog > 0, pays_out, "{income}");
        assert_eq!(terug > 0, recovers, "{income}");
        assert_eq!(
            nog - amount(&toekenning, "terug_te_vorderen_na_verrekening"),
            toegekend - paid
        );
        // Awir 19 lid 2: no aanslag, so at the latest 31 December 2026.
        assert_eq!(toekenning.fields["uiterste_toekenningsdatum"], "2026-12-31");
        // The termijnen still open are not paid.
        let e = cell
            .execute(
                &service,
                "voorschottermijn_betaald",
                &application.id,
                "2025-07-01".parse().unwrap(),
                at("2026-06-02T10:00:00+02:00"),
            )
            .unwrap_err();
        assert!(e.to_string().contains("TOEKENNING"), "{e}");
    }
}

/// What the demo shows before the decision: the next termijn and the
/// toekenning as the law would give them, without a gram. The previews
/// record nothing; recording them afterwards gives the same fields.
#[test]
fn the_demo_cell_previews_the_next_termijn_and_the_toekenning_without_recording() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let before = cell.grams().count();
    let now = at("2024-11-20T10:00:00+01:00");
    // November has no termijn (the first is in December, Awir 22 lid 1).
    let november = cell
        .preview_execution(
            &service,
            "voorschottermijn_betaald",
            &application.id,
            "2024-11-20".parse().unwrap(),
            now,
        )
        .unwrap();
    assert_eq!(november, None);
    // December lies after now: no fact, but the law says what it would be.
    let december = cell
        .preview_execution(
            &service,
            "voorschottermijn_betaald",
            &application.id,
            "2024-12-01".parse().unwrap(),
            now,
        )
        .unwrap()
        .expect("a termijn in December");
    assert_eq!(cell.grams().count(), before);
    // Recording it is still refused before its day.
    assert!(cell
        .execute(
            &service,
            "voorschottermijn_betaald",
            &application.id,
            "2024-12-01".parse().unwrap(),
            now,
        )
        .is_err());
    let paid = pay(&mut cell, &service, &application.id, 2024, 12).unwrap();
    assert_eq!(paid.fields, december.fields);

    // The toekenning asks the dossier for the date of the aanslag (Awir 19),
    // which the cell does not read from its chronicle.
    let stage = cell
        .decision_stage(&service, "zorgtoeslag_toegekend", &application.id, now)
        .unwrap();
    assert_eq!(stage.stage, "TOEKENNING");
    let asked: Vec<&str> = stage
        .inputs
        .iter()
        .map(|i| i.parameter.name.as_str())
        .collect();
    assert!(asked.contains(&"datum_vaststelling_aanslag"), "{asked:?}");
    let read = cell
        .decision_inputs(&service, "zorgtoeslag_toegekend", &application.id, now)
        .unwrap();
    assert!(!read.contains_key("datum_vaststelling_aanslag"));

    let aanslag = BTreeMap::from([(
        "datum_vaststelling_aanslag".to_string(),
        regelrecht_cel::Input {
            value: json!("2026-04-15"),
            provenance: json!({"source": "dossier"}),
        },
    )]);
    let preview = cell
        .preview_decision(
            &service,
            "zorgtoeslag_toegekend",
            BTreeMap::from([("on_application".to_string(), application.id.clone())]),
            aanslag.clone(),
            at("2026-04-15T09:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    // Awir 19 lid 1: within six months of the aanslag.
    assert_eq!(preview.fields["uiterste_toekenningsdatum"], "2026-10-15");
    assert_eq!(cell.grams().count(), before + 1);
}
