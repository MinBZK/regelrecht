//! The cell of the demo (`corpus/demo/cells/toeslagen`) over the demo corpus:
//! the same aanvraag, voorschot and toekenning as on the main corpus, so the
//! browser shows what the tests prove.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use regelrecht_cel::cell::load_regulations;
use regelrecht_cel::config::CellConfig;
use regelrecht_cel::extension::{PeriodParameter, PeriodUnit};
use regelrecht_cel::register;
use regelrecht_cel::{Cell, Error, Gram, Input, Period};
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::json;

/// The fictional test person of the demo scenarios.
const BSN: &str = "999993653";
/// What the citizen expects to earn over 2025 (Awir 16), in eurocent.
const ESTIMATE: i64 = 2_500_000;

/// The values the lexostatus `name` of `cell` gives at `as_of`.
fn read(
    cell: &Cell,
    service: &LawExecutionService,
    name: &str,
    inputs: &serde_json::Map<String, serde_json::Value>,
    as_of: DateTime<FixedOffset>,
) -> serde_json::Map<String, serde_json::Value> {
    let reading = cell.read_lexostatus(service, name, inputs, as_of);
    let reading = reading.unwrap_or_else(|e| panic!("{e}"));
    reading
        .values
        .into_iter()
        .map(|(k, i)| (k, i.value))
        .collect()
}
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
                parameter: "berekeningsjaar".into(),
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
    transport_at(service, toeslagen, bank, order, now, now)
}

/// [`transport`] of a message that arrived at `arrived` and is recorded at
/// `now`, as the demo delivers after a clock step over several days: the
/// order at its own moment, the answer at the moment of the bank's gram.
fn transport_at(
    service: &LawExecutionService,
    toeslagen: &mut Cell,
    bank: &mut Cell,
    order: &Gram,
    arrived: &str,
    now: &str,
) -> (Gram, Gram) {
    let mut credited =
        to_bank(service, bank, order, arrived, now).unwrap_or_else(|e| panic!("bank: {e}"));
    assert_eq!(credited.len(), 1, "{credited:?}");
    let at_bank = credited.remove(0);
    let mut answered = answer(service, toeslagen, &at_bank, &at_bank.effective_at, now)
        .unwrap_or_else(|e| panic!("toeslagen: {e}"));
    assert_eq!(answered.len(), 1, "{answered:?}");
    (at_bank, answered.remove(0))
}

/// The bank receives the order (the first channel), arrived at `arrived`
/// and recorded at `now`.
fn to_bank(
    service: &LawExecutionService,
    bank: &mut Cell,
    order: &Gram,
    arrived: &str,
    now: &str,
) -> Result<Vec<Gram>, regelrecht_cel::Error> {
    let field = |g: &Gram, name: &str| channel(g.fields[name].clone());
    bank.receive(
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
        at(arrived),
        at(now),
    )
}

/// Toeslagen receives the bank's gram `at_bank` (the second channel).
fn answer(
    service: &LawExecutionService,
    toeslagen: &mut Cell,
    at_bank: &Gram,
    arrived: &str,
    now: &str,
) -> Result<Vec<Gram>, regelrecht_cel::Error> {
    let field = |g: &Gram, name: &str| channel(g.fields[name].clone());
    toeslagen.receive(
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
            ("bijgeschreven".to_string(), field(at_bank, "bijgeschreven")),
            (
                "bijgeschreven_bedrag".to_string(),
                field(at_bank, "bijgeschreven_bedrag"),
            ),
            ("bedrag_opdracht".to_string(), field(at_bank, "bedrag")),
            ("reden_weigering".to_string(), field(at_bank, "reden")),
        ]),
        at(arrived),
        at(now),
    )
}

/// What is in arrears on the application at `moment`, as the policy of
/// Toeslagen reads it back (art. 3).
fn arrears(cell: &Cell, service: &LawExecutionService, root: &str, moment: &str) -> i64 {
    cell.read_case(
        service,
        "betaalopdracht_gegeven",
        root,
        Some(("berekeningsjaar", 2025)),
        at(moment),
    )
    .unwrap_or_else(|e| panic!("{e}"))["achterstallig_bedrag"]
        .value
        .as_i64()
        .unwrap()
}

/// The order of `year`-`month`, executed at `now` (a later moment: the
/// clock passed that month in one step).
fn order_late(
    cell: &mut Cell,
    service: &LawExecutionService,
    root: &str,
    year: i32,
    month: u32,
    now: &str,
) -> Gram {
    cell.execute(
        service,
        "betaalopdracht_gegeven",
        root,
        format!("{year}-{month:02}-01").parse().unwrap(),
        at(now),
    )
    .unwrap_or_else(|e| panic!("{year}-{month}: {e}"))
    .unwrap_or_else(|| panic!("{year}-{month}: no order"))
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
    let root = json!({"root": application.id, "berekeningsjaar": 2025});
    let paid = |cell: &Cell, moment: &str| {
        read(
            cell,
            &service,
            "uitbetaald",
            root.as_object().unwrap(),
            at(moment),
        )["uitbetaalde_voorschotten"]
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
    let root = json!({"root": application.id, "berekeningsjaar": 2025});
    let paid = |cell: &Cell, service: &LawExecutionService, moment: &str| {
        read(
            cell,
            service,
            "uitbetaald",
            root.as_object().unwrap(),
            at(moment),
        )["uitbetaalde_voorschotten"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(paid(&cell, &service, "2024-12-15T12:00:00+01:00"), 0);
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
        paid(&cell, &service, "2025-01-01T12:00:00+01:00"),
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
            at("2025-01-01T10:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(grams.len(), 1);
    assert_eq!(grams[0].name, "overboeking_geweigerd");
    assert_eq!(grams[0].fields["reden"], "rekening onbekend");
}

/// The bank's answer delivered twice (the sender did not hear the first
/// delivery land): the second is refused as answered and records nothing,
/// so a refusal does not double what is in arrears and a credit does not
/// double what is paid.
#[test]
fn a_second_answer_to_the_same_order_is_refused_as_answered() {
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
    let (at_bank, first) = transport(
        &service,
        &mut cell,
        &mut bank_cell,
        &december,
        "2024-12-01T11:00:00+01:00",
    );
    assert_eq!(first.name, "betaling_mislukt");
    let before = cell.grams().count();
    let e = answer(
        &service,
        &mut cell,
        &at_bank,
        "2024-12-02T09:00:00+01:00",
        "2024-12-02T09:00:00+01:00",
    )
    .unwrap_err();
    assert!(matches!(e, regelrecht_cel::Error::Answered(_)), "{e}");
    assert_eq!(e.code(), "answered");
    assert_eq!(cell.grams().count(), before);
    assert_eq!(
        arrears(
            &cell,
            &service,
            &application.id,
            "2024-12-15T12:00:00+01:00"
        ),
        amount(&december, "bedrag")
    );
}

/// The order delivered to the bank twice (the sender did not hear the first
/// delivery land): the bank identifies a transfer by its betaalkenmerk
/// (`identified_by`), so the second is refused as answered and nothing is
/// credited twice, although the order is no gram of the bank's own. Another
/// order is credited as usual.
#[test]
fn a_second_transfer_with_the_same_reference_is_refused_by_the_bank() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, false);
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &application.id, 2024, 12).unwrap();
    let first = to_bank(
        &service,
        &mut bank_cell,
        &december,
        "2024-12-01T11:00:00+01:00",
        "2024-12-01T11:00:00+01:00",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(first[0].name, "overboeking_bijgeschreven");
    let before = bank_cell.grams().count();
    let e = to_bank(
        &service,
        &mut bank_cell,
        &december,
        "2024-12-02T09:00:00+01:00",
        "2024-12-02T09:00:00+01:00",
    )
    .unwrap_err();
    assert!(matches!(e, regelrecht_cel::Error::Answered(_)), "{e}");
    assert_eq!(bank_cell.grams().count(), before);

    let january = order(&mut cell, &service, &application.id, 2025, 1).unwrap();
    let next = to_bank(
        &service,
        &mut bank_cell,
        &january,
        "2025-01-01T11:00:00+01:00",
        "2025-01-01T11:00:00+01:00",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(next[0].name, "overboeking_bijgeschreven");
}

/// A transfer without its betaalkenmerk could not be told from another one
/// delivered again: the bank refuses it rather than take it as new.
#[test]
fn a_transfer_without_its_identifying_value_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, false);
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &application.id, 2024, 12).unwrap();
    let field = |name: &str| channel(december.fields[name].clone());
    let e = bank_cell
        .receive(
            &service,
            BANK_TRANSFER,
            BTreeMap::new(),
            BTreeMap::from([
                (
                    "rekeningnummer".to_string(),
                    field("rekeningnummer_begunstigde"),
                ),
                ("bedrag".to_string(), field("bedrag")),
                ("uitvoerdatum".to_string(), field("uitvoerdatum")),
            ]),
            at("2024-12-01T11:00:00+01:00"),
            at("2024-12-01T11:00:00+01:00"),
        )
        .unwrap_err();
    assert_eq!(e.code(), "refused", "{e}");
    assert!(e.to_string().contains("'betaalkenmerk'"), "{e}");
    assert_eq!(bank_cell.grams().count(), 0);
}

/// A message is never recorded before it arrived: one that arrives after
/// the moment of recording is a future fact, and is refused.
#[test]
fn a_message_that_arrives_after_now_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, false);
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &application.id, 2024, 12).unwrap();
    let e = to_bank(
        &service,
        &mut bank_cell,
        &december,
        "2024-12-02T11:00:00+01:00",
        "2024-12-01T11:00:00+01:00",
    )
    .unwrap_err();
    assert_eq!(e.code(), "refused", "{e}");
    assert!(e.to_string().contains("not yet received"), "{e}");
    assert_eq!(bank_cell.grams().count(), 0);
}

/// An answer about an order cannot hold before that order: a message that
/// arrived before the gram it refers to is refused, and nothing is
/// recorded.
#[test]
fn a_message_that_arrives_before_the_gram_it_refers_to_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    register_bank(&mut service, false);
    let (mut cell, application, _) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &application.id, 2024, 12).unwrap();
    let at_bank = to_bank(
        &service,
        &mut bank_cell,
        &december,
        "2024-12-01T11:00:00+01:00",
        "2024-12-01T11:00:00+01:00",
    )
    .unwrap_or_else(|e| panic!("{e}"))
    .remove(0);
    let before = cell.grams().count();
    let e = answer(
        &service,
        &mut cell,
        &at_bank,
        "2024-11-30T09:00:00+01:00",
        "2024-12-01T12:00:00+01:00",
    )
    .unwrap_err();
    assert_eq!(e.code(), "refused", "{e}");
    assert!(e.to_string().contains(&december.id), "{e}");
    assert_eq!(cell.grams().count(), before);
}

/// The clock passes December to March in one step, and the bank refuses
/// December. Each order is read as of its own moment and each answer holds
/// from the moment of its order: January carries December, February
/// carries nothing, and what is in arrears is never negative in between.
#[test]
fn missed_months_in_one_step_carry_a_refusal_into_the_next_order_only() {
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
    let now = "2025-03-01T10:00:00+01:00";
    let mut bank_cell = bank(&service, data.path(), "2025-03-01");
    let root = application.id.clone();

    let december = order_late(&mut cell, &service, &root, 2024, 12, now);
    assert_eq!(december.effective_at, "2024-12-01T00:00:00+01:00");
    let (_, answer) = transport_at(
        &service,
        &mut cell,
        &mut bank_cell,
        &december,
        &december.effective_at,
        now,
    );
    assert_eq!(answer.name, "betaling_mislukt");
    assert_eq!(answer.effective_at, december.effective_at);
    assert_eq!(answer.recorded_at, "2025-03-01T10:00:00+01:00");

    register_bank(&mut service, false);
    let january = order_late(&mut cell, &service, &root, 2025, 1, now);
    assert_eq!(
        january.fields["meegenomen_achterstand"],
        december.fields["bedrag"]
    );
    let (_, paid) = transport_at(
        &service,
        &mut cell,
        &mut bank_cell,
        &january,
        &january.effective_at,
        now,
    );
    assert_eq!(paid.name, "voorschottermijn_betaald");
    let february = order_late(&mut cell, &service, &root, 2025, 2, now);
    assert_eq!(february.fields["meegenomen_achterstand"], 0);
    for moment in [
        "2024-12-15T12:00:00+01:00",
        "2025-01-15T12:00:00+01:00",
        "2025-02-15T12:00:00+01:00",
    ] {
        assert!(arrears(&cell, &service, &root, moment) >= 0, "{moment}");
    }
    assert_eq!(
        arrears(&cell, &service, &root, "2025-01-15T12:00:00+01:00"),
        0
    );
}

/// An answer delivered late holds from when it arrived: the orders of the
/// days the clock passed before it do not carry it (they did not know),
/// the first order after it does. A message from the future is refused.
#[test]
fn a_late_answer_is_carried_by_the_first_order_after_it() {
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
    let root = application.id.clone();
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &root, 2024, 12).unwrap();
    let mut refused = bank_cell
        .receive(
            &service,
            BANK_TRANSFER,
            BTreeMap::new(),
            BTreeMap::from([
                ("betaalkenmerk".to_string(), channel(json!(december.id))),
                (
                    "rekeningnummer".to_string(),
                    channel(december.fields["rekeningnummer_begunstigde"].clone()),
                ),
                (
                    "bedrag".to_string(),
                    channel(december.fields["bedrag"].clone()),
                ),
                (
                    "uitvoerdatum".to_string(),
                    channel(december.fields["uitvoerdatum"].clone()),
                ),
            ]),
            at("2024-12-01T11:00:00+01:00"),
            at("2024-12-01T11:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let at_bank = refused.remove(0);
    // The answer is lost on the way, and arrives on 1 March.
    let now = "2025-03-01T10:00:00+01:00";
    let e = answer(
        &service,
        &mut cell,
        &at_bank,
        "2025-03-02T10:00:00+01:00",
        now,
    )
    .unwrap_err();
    assert!(matches!(e, regelrecht_cel::Error::Refused(_)), "{e}");
    register_bank(&mut service, false);
    let january = order_late(
        &mut cell,
        &service,
        &root,
        2025,
        1,
        "2025-03-01T09:00:00+01:00",
    );
    assert_eq!(january.fields["meegenomen_achterstand"], 0);
    answer(&service, &mut cell, &at_bank, now, now).unwrap_or_else(|e| panic!("{e}"));
    let february = order_late(&mut cell, &service, &root, 2025, 2, now);
    assert_eq!(february.fields["meegenomen_achterstand"], 0);
    let march = order_late(
        &mut cell,
        &service,
        &root,
        2025,
        3,
        "2025-03-01T10:30:00+01:00",
    );
    assert_eq!(
        march.fields["meegenomen_achterstand"],
        december.fields["bedrag"]
    );
}

/// The amount carried forward fails again: what is in arrears is the
/// termijn of the failed order plus what it carried, and the next order
/// carries all of it.
#[test]
fn a_carried_amount_that_fails_again_stays_in_arrears() {
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
    let root = application.id.clone();
    let mut bank_cell = bank(&service, data.path(), "2024-12-01");
    let december = order(&mut cell, &service, &root, 2024, 12).unwrap();
    transport(
        &service,
        &mut cell,
        &mut bank_cell,
        &december,
        "2024-12-01T11:00:00+01:00",
    );
    let january = order(&mut cell, &service, &root, 2025, 1).unwrap();
    assert_eq!(
        january.fields["meegenomen_achterstand"],
        december.fields["bedrag"]
    );
    let (_, failed) = transport(
        &service,
        &mut cell,
        &mut bank_cell,
        &january,
        "2025-01-01T11:00:00+01:00",
    );
    assert_eq!(failed.name, "betaling_mislukt");
    assert_eq!(failed.fields["mislukt_bedrag"], january.fields["bedrag"]);
    assert_eq!(
        arrears(&cell, &service, &root, "2025-01-15T12:00:00+01:00"),
        amount(&january, "bedrag")
    );
    register_bank(&mut service, false);
    let february = order(&mut cell, &service, &root, 2025, 2).unwrap();
    assert_eq!(
        february.fields["meegenomen_achterstand"],
        january.fields["bedrag"]
    );
    assert_eq!(
        amount(&february, "bedrag"),
        amount(&february, "termijnbedrag") + amount(&january, "bedrag")
    );
}

/// The cell describes every lexostatus it reads its chronicle back with, in
/// its configuration or in a policy of the holder, and reads each one with
/// the grams it came from: the application, the paid termijnen, and the
/// grams of the case in the register the policy reads.
#[test]
fn the_demo_cell_describes_and_reads_its_lexostatuses_with_their_grams() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2024-11-04T10:15:00+01:00",
        "2024-11-20T09:00:00+01:00",
        79547,
    );
    let order = pay(&mut cell, &service, &application.id, 2024, 12).unwrap();
    let paid: Vec<String> = cell
        .grams()
        .filter(|g| g.name == "voorschottermijn_betaald")
        .map(|g| g.id.clone())
        .collect();
    assert_eq!(paid.len(), 1);

    let described = serde_json::to_value(
        cell.lexostatuses(&service, "2024-12-01".parse().unwrap())
            .unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap();
    let by_name = |name: &str| {
        described
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["name"] == name)
            .unwrap_or_else(|| panic!("no '{name}' in {described}"))
            .clone()
    };
    let aanvraag = by_name("aanvraag");
    assert_eq!(aanvraag["kind"], "configuration");
    assert_eq!(aanvraag["inputs"], json!(["root"]));
    assert_eq!(aanvraag["reduction"]["pick"], "latest");
    assert_eq!(aanvraag["reduction"]["filter"]["root"], "$root");
    assert_eq!(
        aanvraag["reduction"]["derivations"]["datum_ontvangst"]["moment"],
        "effective_at"
    );
    let readers: Vec<&str> = aanvraag["read_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["event"].as_str().unwrap())
        .collect();
    assert_eq!(readers, ["voorschot_verleend", "zorgtoeslag_toegekend"]);
    assert_eq!(aanvraag["read_by"][0]["stage"], "VOORSCHOT");
    let uitbetaald = by_name("uitbetaald");
    assert_eq!(
        uitbetaald["reduction"]["derivations"]["uitbetaalde_voorschotten"]["sum"],
        "betaald_bedrag"
    );
    // Read per berekeningsjaar: the application holds for the years after
    // it as well (Awir 15 lid 5).
    assert_eq!(uitbetaald["inputs"], json!(["root", "berekeningsjaar"]));
    assert_eq!(uitbetaald["period"], "berekeningsjaar");
    assert!(aanvraag.get("period").is_none(), "{aanvraag}");
    let policy = by_name("fictief_beleid_kroniek_toeslagen");
    assert_eq!(policy["kind"], "policy");
    assert_eq!(policy["register"], "kroniek");
    assert_eq!(policy["chronicle"], "toeslagen");
    assert_eq!(policy["register_input"], "grams");
    assert_eq!(policy["inputs"], json!(["root", "berekeningsjaar"]));
    assert_eq!(policy["period"], "berekeningsjaar");
    assert_eq!(policy["articles"][0]["number"], "1");
    assert!(policy["articles"][0]["outputs"]
        .as_array()
        .unwrap()
        .contains(&json!("voorschotbedrag")));
    // The voorschot reads one article of it (the estimate), the payment
    // order all of it.
    let readers: Vec<&str> = policy["read_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["event"].as_str().unwrap())
        .collect();
    assert_eq!(readers, ["voorschot_verleend", "betaalopdracht_gegeven"]);

    let root = json!({"root": application.id, "berekeningsjaar": 2025});
    let read = |name: &str| {
        cell.read_lexostatus(
            &service,
            name,
            root.as_object().unwrap(),
            at("2024-12-02T09:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let aanvraag = read("aanvraag");
    assert_eq!(aanvraag.grams, std::slice::from_ref(&application.id));
    assert_eq!(aanvraag.values["bsn"].value, BSN);
    assert_eq!(aanvraag.values["datum_ontvangst"].value, "2024-11-04");
    assert_eq!(
        aanvraag.values["bsn"].provenance,
        json!({"source": "lexostatus", "lexostatus": "aanvraag"})
    );
    let uitbetaald = read("uitbetaald");
    assert_eq!(uitbetaald.grams, paid);
    assert_eq!(
        uitbetaald.values["uitbetaalde_voorschotten"].value,
        order.fields["bedrag"]
    );
    let policy = read("fictief_beleid_kroniek_toeslagen");
    assert_eq!(
        policy.values["voorschotbedrag"].value,
        voorschot.fields["voorschotbedrag"]
    );
    assert_eq!(
        policy.values["voorschotbedrag"].provenance["article"],
        "fictief_beleid_kroniek_toeslagen#1"
    );
    assert!(policy.grams.contains(&application.id));
    assert!(policy.grams.contains(&voorschot.id));
    assert!(policy.grams.contains(&order.id));

    // Before the application holds, there is nothing to read it from.
    assert!(cell
        .read_lexostatus(
            &service,
            "aanvraag",
            root.as_object().unwrap(),
            at("2024-11-01T09:00:00+01:00"),
        )
        .is_err());
    assert!(cell
        .read_lexostatus(
            &service,
            "fictief_beleid_kroniek_toeslagen",
            &serde_json::Map::new(),
            at("2024-12-02T09:00:00+01:00"),
        )
        .is_err());
    assert!(cell
        .read_lexostatus(
            &service,
            "onbekend",
            root.as_object().unwrap(),
            at("2024-12-02T09:00:00+01:00"),
        )
        .is_err());
}

fn year(value: i32) -> Period {
    Period {
        unit: PeriodUnit::Year,
        value,
    }
}

/// Every betaalopdracht the cell gives on the application `root` on the days
/// it says are due after `after` through `through` (`YYYY-MM-DD`), each for
/// its period, at `now`; the bank credits each at once. The orders, in the
/// order the cell gave them.
fn pay_due(
    cell: &mut Cell,
    service: &LawExecutionService,
    root: &str,
    after: Option<&str>,
    through: &str,
    now: &str,
) -> Vec<Gram> {
    let due = cell
        .due_executions(
            service,
            "betaalopdracht_gegeven",
            root,
            after.map(|a| a.parse().unwrap()),
            through.parse().unwrap(),
            at(now),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let mut orders = Vec::new();
    for d in due {
        let order = cell
            .execute_in(
                service,
                "betaalopdracht_gegeven",
                root,
                d.day,
                d.period.map(|p| p.value),
                at(now),
            )
            .unwrap_or_else(|e| panic!("{} {:?}: {e}", d.day, d.period));
        let Some(order) = order else { continue };
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
            at(&order.effective_at),
            at(now),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", d.day));
        orders.push(order);
    }
    orders
}

fn of_year(orders: &[Gram], value: i32) -> Vec<&Gram> {
    orders
        .iter()
        .filter(|o| o.period == Some(year(value)))
        .collect()
}

/// Awir 15 lid 5: an application counts for the berekeningsjaren after it
/// too. On the same application Toeslagen grants the voorschot for the next
/// year on 1 November before it (fictitious policy art. 4), its termijnen
/// start in December (Awir 22 lid 1) while the year before is still being
/// paid, and the toekenning over the first year sets off only what was paid
/// on that year and ends only its termijnen.
#[test]
fn an_application_holds_for_the_next_berekeningsjaar() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2025-01-02T10:00:00+01:00",
        "2025-01-02T10:30:00+01:00",
        79547,
    );
    let root = application.id.clone();
    let on_application = || BTreeMap::from([("on_application".to_string(), root.clone())]);
    assert_eq!(voorschot.period, Some(year(2025)));

    // The next voorschot is that of 2026, on 1 November 2025.
    let due = cell
        .due_decision(
            &service,
            "voorschot_verleend",
            &root,
            at("2025-01-02T11:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(due.period, Some(year(2026)));
    assert_eq!(due.day, Some("2025-11-01".parse().unwrap()));
    // Not before that day.
    let e = cell
        .decide(
            &service,
            "voorschot_verleend",
            on_application(),
            BTreeMap::new(),
            at("2025-10-31T09:00:00+01:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
    assert!(e.to_string().contains("2025-11-01"), "{e}");

    // The termijnen of 2025 through October: one per month.
    let mut orders = pay_due(
        &mut cell,
        &service,
        &root,
        None,
        "2025-10-31",
        "2025-10-31T12:00:00+01:00",
    );
    assert_eq!(of_year(&orders, 2025).len(), 10);

    // On 1 November 2025 the voorschot for 2026, on the same application.
    let next = cell
        .decide(
            &service,
            "voorschot_verleend",
            on_application(),
            BTreeMap::new(),
            at("2025-11-01T09:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(next.period, Some(year(2026)));
    assert_eq!(next.refers_to["on_application"], root);
    assert_eq!(
        next.inputs["berekeningsjaar"],
        json!({"value": 2026, "provenance": {"source": "period"}})
    );
    // The estimate in the application holds for 2026 too (an aanname in the
    // policy of Toeslagen, art. 4).
    assert_eq!(
        next.inputs["vermoedelijk_toetsingsinkomen"]["value"],
        ESTIMATE
    );
    assert_eq!(
        next.inputs["vermoedelijk_toetsingsinkomen"]["provenance"]["article"],
        "fictief_beleid_kroniek_toeslagen#4"
    );
    // The demo corpus has no Zorgtoeslagwet of 2026: the version in force on
    // 1 January 2026 is that of 2025, so 2026 computes as 2025 did.
    assert_eq!(next.regulation_valid_from.as_deref(), Some("2025-01-01"));
    assert_eq!(
        next.fields["voorschotbedrag"],
        voorschot.fields["voorschotbedrag"]
    );
    // And the one after that: 2027, on 1 November 2026.
    let due = cell
        .due_decision(
            &service,
            "voorschot_verleend",
            &root,
            at("2025-11-01T10:00:00+01:00"),
        )
        .unwrap();
    assert_eq!(due.period, Some(year(2027)));
    assert_eq!(due.day, Some("2026-11-01".parse().unwrap()));

    // November: the last termijn of 2025. December: the first of the twelve
    // of 2026 (Awir 22 lid 1); 2025 has none left.
    let days: Vec<(String, Option<i32>)> = cell
        .due_executions(
            &service,
            "betaalopdracht_gegeven",
            &root,
            Some("2025-10-31".parse().unwrap()),
            "2025-12-31".parse().unwrap(),
            at("2025-12-01T12:00:00+01:00"),
        )
        .unwrap()
        .into_iter()
        .map(|d| (d.day.to_string(), d.period.map(|p| p.value)))
        .collect();
    let day = |d: &str, y: i32| (d.to_string(), Some(y));
    assert_eq!(
        days,
        [
            day("2025-11-01", 2025),
            day("2025-11-01", 2026),
            day("2025-12-01", 2025),
            day("2025-12-01", 2026),
        ]
    );
    let late = pay_due(
        &mut cell,
        &service,
        &root,
        Some("2025-10-31"),
        "2025-12-31",
        "2025-12-01T12:00:00+01:00",
    );
    let given: Vec<(String, Option<i32>)> = late
        .iter()
        .map(|o| (o.effective_at[..10].to_string(), o.period.map(|p| p.value)))
        .collect();
    assert_eq!(given, [day("2025-11-01", 2025), day("2025-12-01", 2026)]);
    assert_eq!(late[1].refers_to["voorschot"], next.id);
    orders.extend(late);
    let paid_2025: i64 = of_year(&orders, 2025)
        .iter()
        .map(|o| amount(o, "termijnbedrag"))
        .sum();
    assert_eq!(paid_2025, amount(&voorschot, "voorschotbedrag"));

    // January to April 2026: the termijnen of 2026 run on.
    let spring = pay_due(
        &mut cell,
        &service,
        &root,
        Some("2025-12-31"),
        "2026-04-14",
        "2026-04-14T12:00:00+02:00",
    );
    assert_eq!(of_year(&spring, 2026).len(), 4);
    assert!(of_year(&spring, 2025).is_empty());
    orders.extend(spring);

    // What was paid, per berekeningsjaar.
    let uitbetaald = |value: i32| {
        read(
            &cell,
            &service,
            "uitbetaald",
            json!({"root": root, "berekeningsjaar": value})
                .as_object()
                .unwrap(),
            at("2026-04-14T13:00:00+02:00"),
        )["uitbetaalde_voorschotten"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(uitbetaald(2025), paid_2025);
    let paid_2026: i64 = of_year(&orders, 2026)
        .iter()
        .map(|o| amount(o, "termijnbedrag"))
        .sum();
    assert_eq!(uitbetaald(2026), paid_2026);

    // The toekenning over 2025, on the aanslag of 15 April 2026: it sets
    // off what was paid on 2025, not the termijnen of 2026.
    let toekenning = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            on_application(),
            BTreeMap::from([(
                "datum_vaststelling_aanslag".to_string(),
                Input {
                    value: json!("2026-04-15"),
                    provenance: json!({"source": "dossier"}),
                },
            )]),
            at("2026-04-15T09:00:00+02:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(toekenning.period, Some(year(2025)));
    assert_eq!(
        toekenning.inputs["uitbetaalde_voorschotten"]["value"],
        paid_2025
    );

    // It ends the termijnen of 2025 only: May has the termijn of 2026.
    let may = pay_due(
        &mut cell,
        &service,
        &root,
        Some("2026-04-14"),
        "2026-05-31",
        "2026-05-01T12:00:00+02:00",
    );
    assert_eq!(may.len(), 1);
    assert_eq!(may[0].period, Some(year(2026)));
    let e = cell
        .execute_in(
            &service,
            "betaalopdracht_gegeven",
            &root,
            "2026-05-01".parse().unwrap(),
            Some(2025),
            at("2026-05-01T13:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Ended(_)), "{e}");
    // With two years open the cell does not guess which one is meant.
    let e = cell
        .execute(
            &service,
            "betaalopdracht_gegeven",
            &root,
            "2026-05-01".parse().unwrap(),
            at("2026-05-01T13:00:00+02:00"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("say which"), "{e}");

    // The next toekenning concerns 2026; its day is the aanslag over 2026,
    // which the dossier gives, not the policy.
    let due = cell
        .due_decision(
            &service,
            "zorgtoeslag_toegekend",
            &root,
            at("2026-05-01T13:00:00+02:00"),
        )
        .unwrap();
    assert_eq!(due.period, Some(year(2026)));
    assert_eq!(due.day, None);
    // A decision on a year before the one applied for is refused.
    let e = cell
        .decide(
            &service,
            "zorgtoeslag_toegekend",
            on_application(),
            BTreeMap::from([(
                "berekeningsjaar".to_string(),
                Input {
                    value: json!(2024),
                    provenance: json!({"source": "caller"}),
                },
            )]),
            at("2026-05-01T14:00:00+02:00"),
        )
        .unwrap_err();
    assert!(matches!(e, Error::Refused(_)), "{e}");
}

/// The voorschot over a year granted in December of that year is paid in
/// one amount that month (Awir 22 lid 5); the voorschot for the next year,
/// granted the same day, starts its twelve termijnen in that same December
/// (lid 1). Once a month is per berekeningsjaar: both are paid, and neither
/// twice.
#[test]
fn two_voorschotten_are_both_paid_in_december() {
    let data = tempfile::tempdir().unwrap();
    let mut service = regulations();
    let (mut cell, application, voorschot) = with_voorschot(
        &mut service,
        data.path(),
        "2025-12-10T10:00:00+01:00",
        "2025-12-10T10:30:00+01:00",
        79547,
    );
    let root = application.id.clone();
    // 1 November 2025 has passed: the voorschot for 2026 is due at once.
    let due = cell
        .due_decision(
            &service,
            "voorschot_verleend",
            &root,
            at("2025-12-10T10:45:00+01:00"),
        )
        .unwrap();
    assert_eq!(due.period, Some(year(2026)));
    assert_eq!(due.day, Some("2025-11-01".parse().unwrap()));
    let next = cell
        .decide(
            &service,
            "voorschot_verleend",
            BTreeMap::from([("on_application".to_string(), root.clone())]),
            BTreeMap::new(),
            at("2025-12-10T11:00:00+01:00"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(next.period, Some(year(2026)));

    let orders = pay_due(
        &mut cell,
        &service,
        &root,
        None,
        "2025-12-31",
        "2025-12-10T12:00:00+01:00",
    );
    assert_eq!(orders.len(), 2, "{orders:?}");
    assert_eq!(orders[0].period, Some(year(2025)));
    assert_eq!(
        amount(&orders[0], "termijnbedrag"),
        amount(&voorschot, "voorschotbedrag")
    );
    assert_eq!(orders[1].period, Some(year(2026)));
    assert_eq!(orders[1].refers_to["voorschot"], next.id);
    assert!(amount(&orders[1], "termijnbedrag") < amount(&next, "voorschotbedrag"));
    for value in [2025, 2026] {
        let e = cell
            .execute_in(
                &service,
                "betaalopdracht_gegeven",
                &root,
                "2025-12-20".parse().unwrap(),
                Some(value),
                at("2025-12-20T12:00:00+01:00"),
            )
            .unwrap_err();
        assert!(e.to_string().contains("already"), "{value}: {e}");
    }
}
