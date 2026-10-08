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
            // The (fictitious) policy of Toeslagen asks the account.
            "rekeningnummer",
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
                "rekeningnummer": ACCOUNT,
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
                "rekeningnummer": ACCOUNT,
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

/// Pay the voorschottermijn of the month of the first of `year`-`month`: the
/// betaalopdracht, and the bank's answer that it credited it. Returns the
/// order (`None` if the law gives none that month).
fn pay(
    cell: &mut Cell,
    service: &LawExecutionService,
    root: &str,
    year: i32,
    month: u32,
) -> Option<Gram> {
    let order = order(cell, service, root, year, month)?;
    let bedrag = order.fields["bedrag"].clone();
    let answer = BTreeMap::from([
        ("bijgeschreven".to_string(), channel(json!(true))),
        ("bijgeschreven_bedrag".to_string(), channel(bedrag.clone())),
        ("bedrag_opdracht".to_string(), channel(bedrag)),
        ("reden_weigering".to_string(), channel(json!(""))),
    ]);
    cell.receive(
        service,
        TOESLAGEN_ANSWER,
        BTreeMap::from([("betaalopdracht".to_string(), order.id.clone())]),
        answer,
        at(&format!("{year}-{month:02}-01T11:00:00+01:00")),
    )
    .unwrap_or_else(|e| panic!("{year}-{month}: {e}"));
    Some(order)
}

/// Only the betaalopdracht of the month of the first of `year`-`month`.
fn order(
    cell: &mut Cell,
    service: &LawExecutionService,
    root: &str,
    year: i32,
    month: u32,
) -> Option<Gram> {
    let now = at(&format!("{year}-{month:02}-01T10:00:00+01:00"));
    cell.execute(
        service,
        "betaalopdracht_gegeven",
        root,
        now.date_naive(),
        now,
    )
    .unwrap_or_else(|e| panic!("{year}-{month}: {e}"))
}

/// The article of Toeslagen that receives the bank's answer.
const TOESLAGEN_ANSWER: &str = "fictief_beleid_termijnbedrag_voorschot#2";
/// The article of the bank that receives a transfer.
const BANK_TRANSFER: &str = "fictieve_bankvoorwaarden#1";
/// The persona's account (a clearly fictitious format).
const ACCOUNT: &str = "NL00TEST0123456789";

/// A value as a channel delivers it.
fn channel(value: serde_json::Value) -> regelrecht_cel::Input {
    regelrecht_cel::Input {
        value,
        provenance: json!({"source": "kanaal"}),
    }
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
                "betaalopdracht_gegeven",
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
            "betaalopdracht_gegeven",
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
            "betaalopdracht_gegeven",
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
            "betaalopdracht_gegeven",
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
    // The order of December and the bank's answer to it.
    assert_eq!(cell.grams().count(), before + 2);
}

/// The bank cell over the demo corpus, with its chronicle in `data`.
fn bank(service: &LawExecutionService, data: &std::path::Path, day: &str) -> Cell {
    Cell::open(
        &demo().join("cells/bank/cell.yaml"),
        service,
        data,
        day.parse().unwrap(),
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

/// What the bank knows of the persona's account (its BANK data in
/// profiles.yaml): whether it is blocked. A second account the bank has no
/// record of is null, as the binding says (`absent: null`).
fn register_bank(service: &mut LawExecutionService, blocked: bool) {
    let record = |account: &str, blocked: Value| {
        BTreeMap::from([
            ("rekeningnummer".to_string(), Value::String(account.into())),
            ("rekening_geblokkeerd".to_string(), blocked),
        ])
    };
    // Replace what the bank knew before (the account was unblocked).
    service.remove_data_source("BANK");
    service
        .register_dict_source_for_law(
            "fictieve_bankvoorwaarden",
            "BANK",
            "rekeningnummer",
            vec![
                record(ACCOUNT, Value::Bool(blocked)),
                record("NL00TEST0000000000", Value::Null),
            ],
            10,
        )
        .unwrap();
}

/// The transport the demo does between the two cells (`channels` in
/// demo-config.yaml): the order to the bank, the bank's gram back to
/// Toeslagen. Returns the bank's gram and Toeslagen's.
fn transport(
    service: &LawExecutionService,
    toeslagen: &mut Cell,
    bank: &mut Cell,
    order: &Gram,
    now: &str,
) -> (Gram, Gram) {
    let field = |g: &Gram, name: &str| channel(g.fields[name].clone());
    let mut credited = bank
        .receive(
            service,
            BANK_TRANSFER,
            BTreeMap::new(),
            BTreeMap::from([
                ("betaalkenmerk".to_string(), channel(json!(order.id))),
                (
                    "rekeningnummer".to_string(),
                    field(order, "rekeningnummer_begunstigde"),
                ),
                ("bedrag".to_string(), field(order, "bedrag")),
                ("uitvoerdatum".to_string(), field(order, "uitvoerdatum")),
            ]),
            at(now),
        )
        .unwrap_or_else(|e| panic!("bank: {e}"));
    assert_eq!(credited.len(), 1, "{credited:?}");
    let at_bank = credited.remove(0);
    let mut answered = toeslagen
        .receive(
            service,
            TOESLAGEN_ANSWER,
            BTreeMap::from([(
                "betaalopdracht".to_string(),
                at_bank.fields["betaalkenmerk"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            )]),
            BTreeMap::from([
                (
                    "bijgeschreven".to_string(),
                    field(&at_bank, "bijgeschreven"),
                ),
                (
                    "bijgeschreven_bedrag".to_string(),
                    field(&at_bank, "bijgeschreven_bedrag"),
                ),
                ("bedrag_opdracht".to_string(), field(&at_bank, "bedrag")),
                ("reden_weigering".to_string(), field(&at_bank, "reden")),
            ]),
            at(now),
        )
        .unwrap_or_else(|e| panic!("toeslagen: {e}"));
    assert_eq!(answered.len(), 1, "{answered:?}");
    (at_bank, answered.remove(0))
}

/// A betaalopdracht goes to the bank, the bank credits it to the persona's
/// account, and Toeslagen records the termijn as paid, referring to the
/// order. Only what the bank credited counts as paid (Awir 24 lid 2).
#[test]
fn the_bank_credits_the_order_and_toeslagen_records_it_paid() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, false);
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut bank = bank(&service, data.path(), "2024-12-01");
    let order = order(&mut cell, &service, &application.id, 2024, 12).unwrap();
    assert_eq!(order.fields["rekeningnummer_begunstigde"], ACCOUNT);
    assert_eq!(order.fields["uitvoerdatum"], "2024-12-01");
    assert_eq!(order.fields["bedrag"], order.fields["termijnbedrag"]);
    assert_eq!(order.inputs["rekeningnummer"]["value"], ACCOUNT);
    assert_eq!(
        order.inputs["rekeningnummer"]["provenance"]["article"],
        "fictief_beleid_kroniek_toeslagen#2"
    );

    // Nothing is paid before the bank says so.
    let root = json!({"root": application.id});
    let paid = |cell: &Cell, moment: &str| {
        cell.read("uitbetaald", root.as_object().unwrap(), at(moment))
            .unwrap()["uitbetaalde_voorschotten"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(paid(&cell, "2024-12-01T10:30:00+01:00"), 0);

    let (at_bank, answer) = transport(
        &service,
        &mut cell,
        &mut bank,
        &order,
        "2024-12-01T11:00:00+01:00",
    );
    assert_eq!(at_bank.name, "overboeking_bijgeschreven");
    assert_eq!(at_bank.chronicle, "rekeningen");
    assert_eq!(at_bank.recording_actor, "fictieve_bank");
    assert_eq!(at_bank.fields["betaalkenmerk"], order.id);
    assert_eq!(at_bank.fields["rekeningnummer"], ACCOUNT);
    assert_eq!(
        at_bank.fields["bijgeschreven_bedrag"],
        order.fields["bedrag"]
    );
    assert_eq!(answer.name, "voorschottermijn_betaald");
    assert_eq!(answer.refers_to["betaalopdracht"], order.id);
    assert_eq!(answer.fields["betaald_bedrag"], order.fields["bedrag"]);
    assert_eq!(
        paid(&cell, "2024-12-01T12:00:00+01:00"),
        amount(&order, "bedrag")
    );
    assert!(amount(&order, "bedrag") < amount(&voorschot, "voorschotbedrag"));
}

/// A blocked account: the bank refuses the transfer, Toeslagen records the
/// payment as failed, nothing counts as paid, and the amount goes along
/// with the next month's order once the account is unblocked.
#[test]
fn a_blocked_account_fails_the_payment_and_the_next_order_retries_it() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, true);
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &application.id, 2024, 12).unwrap();
    let (at_bank, answer) = transport(
        &service,
        &mut cell,
        &mut bank_cell,
        &december,
        "2024-12-01T11:00:00+01:00",
    );
    assert_eq!(at_bank.name, "overboeking_geweigerd");
    assert_eq!(at_bank.fields["reden"], "rekening geblokkeerd");
    assert_eq!(at_bank.fields["bijgeschreven_bedrag"], 0);
    assert_eq!(answer.name, "betaling_mislukt");
    assert_eq!(answer.refers_to["betaalopdracht"], december.id);
    assert_eq!(answer.fields["mislukt_bedrag"], december.fields["bedrag"]);
    let root = json!({"root": application.id});
    let paid = |cell: &Cell, moment: &str| {
        cell.read("uitbetaald", root.as_object().unwrap(), at(moment))
            .unwrap()["uitbetaalde_voorschotten"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(paid(&cell, "2024-12-15T12:00:00+01:00"), 0);
    // Once per month: no second order in December (decided: the retry goes
    // with the next execution day, see the fictitious policy, art. 1).
    assert!(cell
        .execute(
            &service,
            "betaalopdracht_gegeven",
            &application.id,
            "2024-12-15".parse().unwrap(),
            at("2024-12-15T10:00:00+01:00"),
        )
        .is_err());

    // Unblocked: January's order carries December's termijn as well.
    register_bank(&mut service, false);
    let january = order(&mut cell, &service, &application.id, 2025, 1).unwrap();
    assert_eq!(
        january.fields["meegenomen_achterstand"],
        december.fields["bedrag"]
    );
    assert_eq!(
        amount(&january, "bedrag"),
        amount(&january, "termijnbedrag") + amount(&december, "bedrag")
    );
    let (at_bank, answer) = transport(
        &service,
        &mut cell,
        &mut bank_cell,
        &january,
        "2025-01-01T11:00:00+01:00",
    );
    assert_eq!(at_bank.name, "overboeking_bijgeschreven");
    assert_eq!(answer.name, "voorschottermijn_betaald");
    assert_eq!(
        paid(&cell, "2025-01-01T12:00:00+01:00"),
        amount(&january, "bedrag")
    );
    // Nothing is in arrears any more: February pays its own termijn.
    let february = order(&mut cell, &service, &application.id, 2025, 2).unwrap();
    assert_eq!(february.fields["meegenomen_achterstand"], 0);
}

/// An account the bank does not know: the transfer is refused as unknown.
#[test]
fn the_bank_refuses_an_unknown_account() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, false);
    let mut bank = bank(&service, data.path(), "2025-01-01");
    let grams = bank
        .receive(
            &service,
            BANK_TRANSFER,
            BTreeMap::new(),
            BTreeMap::from([
                ("betaalkenmerk".to_string(), channel(json!("kenmerk-1"))),
                (
                    "rekeningnummer".to_string(),
                    channel(json!("NL00TEST0000000000")),
                ),
                ("bedrag".to_string(), channel(json!(1000))),
                ("uitvoerdatum".to_string(), channel(json!("2025-01-01"))),
            ]),
            at("2025-01-01T10:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(grams.len(), 1);
    assert_eq!(grams[0].name, "overboeking_geweigerd");
    assert_eq!(grams[0].fields["reden"], "rekening onbekend");
}
