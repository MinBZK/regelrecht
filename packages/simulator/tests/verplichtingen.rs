//! De **richting** van een verplichting: wie aan wie, en wat een negatief bedrag
//! betekent.
//!
//! Een verplichting is een rechtsverhouding tussen twee partijen: een
//! schuldenaar en een schuldeiser, allebei een naam uit het recht. Wat hier
//! staat, is wat geen scenariobestand kan dragen — een run die afbreekt, en de
//! termijn die wél ingeroosterd wordt en níet nagekomen omdat de wereld de
//! schuldenaar niet als cel kent.
//!
//! De positieve gevallen staan in `scenarios/toeslagen_terugvordering.yaml`,
//! want daar horen ze: een besluit dat slaagt, draagt zijn eigen verwachting.

use regelrecht_simulator::{
    regulation_root, JournalKind, ObligationKind, Scenario, SimulatorError, Value,
};
use std::path::{Path, PathBuf};

fn scenario_path(dir: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join(dir)
        .join(name)
}

fn scenario(path: &Path) -> Scenario {
    Scenario::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **Een negatief bedrag is een fout, en de melding wijst de weg.**
///
/// Een negatieve betaling bestaat niet. Wat de wet ervoor in de plaats stelt —
/// een verplichting de andere kant op — is een keuze die in het lexogram hoort en
/// die het platform niet mag verzinnen. De melding is daarom het halve werk: wie
/// hem leest, hoort te weten welke declaratie eronder had moeten staan.
#[test]
fn een_negatief_bedrag_zonder_declaratie_laat_het_besluit_omvallen() {
    let path = scenario_path("geweigerd", "negatief_bedrag_zonder_omkeren.yaml");
    let error = scenario(&path)
        .run(&regulation_root())
        .err()
        .unwrap_or_else(|| panic!("{}: deze fixture hoort af te breken", path.display()));

    assert!(
        matches!(error, SimulatorError::NegativeObligationAmount { .. }),
        "verwachtte NegativeObligationAmount, kreeg {error}"
    );
    let melding = error.to_string();
    assert!(
        melding.contains("richting_bij_negatief: omkeren"),
        "de melding hoort de declaratie te noemen die eronder hoort, kreeg: {melding}"
    );
    assert!(
        melding.contains("test_terugvordering artikel 2"),
        "en het artikel waar die declaratie dan moet staan, kreeg: {melding}"
    );
}

/// **Een omgekeerde verplichting draagt de twee partijen omgewisseld.**
///
/// Het gram is waar dit in terechtkomt, dus daar wordt het gemeten: de
/// terugvordering staat op naam van de partij, de betaling op naam van het gezag,
/// en het bedrag is in beide gevallen positief. Zonder deze test zou een
/// omkering die alleen het bedrag draait er in de sommen precies zo uitzien.
#[test]
fn een_omkering_wisselt_de_partijen_en_maakt_het_bedrag_positief() {
    let path = scenario_path("", "toeslagen_terugvordering.yaml");
    let run = scenario(&path)
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());

    let voorschot = &run.decisions[0].decretogram.obligations[0];
    assert_eq!(voorschot.soort, ObligationKind::Betaling);
    assert_eq!(voorschot.schuldenaar, "Dienst Toeslagen");
    assert_eq!(voorschot.schuldeiser, "999993653");
    assert_eq!(voorschot.betaler.as_deref(), Some("belastingdienst"));

    let terug = &run.decisions[1].decretogram.obligations[0];
    assert_eq!(terug.soort, ObligationKind::Terugvordering);
    assert_eq!(
        terug.schuldenaar, "999993653",
        "bij een omkering is de partij de schuldenaar"
    );
    assert_eq!(terug.schuldeiser, "Dienst Toeslagen");
    assert_eq!(
        terug.betaler.as_deref(),
        Some("aanvrager"),
        "en de cel die die naam draagt, komt haar na"
    );
    assert!(
        terug
            .bedrag
            .as_decimal()
            .is_some_and(|bedrag| bedrag.is_sign_positive()),
        "een verplichting kent geen minteken, alleen een richting: {}",
        terug.bedrag
    );
}

/// **Kent de wereld de schuldenaar niet, dan staat de termijn open.**
///
/// Een terugvordering op iemand die in deze wereld niet als cel meedoet, is een
/// echte verplichting: de klok roostert haar in, het gram draagt haar, en er
/// gebeurt verder niets. Geen fout — wat de wet oplegt hangt niet af van wie er
/// in deze wereld een systeem heeft — en ook geen stille betaling door een
/// willekeurige andere cel.
#[test]
fn een_schuldenaar_zonder_cel_laat_de_termijn_openstaan() {
    let path = scenario_path("", "toeslagen_terugvordering.yaml");
    let yaml = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        // De aanvrager doet niet meer mee als partij: haar cel geeft zich
        // voortaan uit als zichzelf, en dan is er niemand die de naam uit de
        // verplichting draagt.
        .replace("    identity: '999993653'\n", "");

    let mut scenario =
        Scenario::from_yaml(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // De vragen van het bestand gaan over de betaling die nu niet meer gebeurt;
    // wat deze test meet, staat in het gram.
    scenario.queries.clear();

    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("een schuldenaar zonder cel hoort geen fout te zijn: {e}"));

    let terug = &run.decisions[1].decretogram.obligations[0];
    assert_eq!(
        terug.schuldenaar, "999993653",
        "de wet wijst nog dezelfde partij aan"
    );
    assert_eq!(
        terug.betaler, None,
        "maar deze wereld kent geen cel die zo heet"
    );
    assert!(
        run.snapshot
            .cells
            .iter()
            .filter(|cell| cell.id == "aanvrager")
            .flat_map(|cell| &cell.chronicles)
            .all(|chronicle| chronicle.grams.is_empty()),
        "en dan legt die cel ook niets vast"
    );
}

/// **Een verplichting van nul levert geen termijn en geen betaling.**
///
/// Een vaststelling die precies uitkomt op wat er al betaald is, laat een
/// slotbedrag van nul over. Dat is geen betaling van nul euro: er valt niets te
/// betalen, dus er hoort geen vervaldag en geen executogram bij. Maar het artikel
/// legde de verplichting wél op, dus het gram noemt haar — met `bedrag: 0` en
/// zonder termijnen — en het journaal zegt dat er niets te betalen is.
#[test]
fn een_verplichting_van_nul_levert_geen_termijn_op() {
    let path = scenario_path("", "toeslagen_terugvordering.yaml");
    let yaml = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        // Definitief precies wat er in twee voorschottermijnen betaald is.
        .replace("definitief_bedrag: 50000", "definitief_bedrag: 98602")
        .replace(
            "nog_te_betalen: -48602\n    expect_accepted",
            "nog_te_betalen: 0\n    expect_accepted",
        );
    let mut scenario =
        Scenario::from_yaml(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // De vragen van het bestand gaan over de terugvordering die er nu niet is;
    // wat deze test meet, staat in het gram en het journaal.
    scenario.queries.clear();

    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("een slotbedrag van nul hoort geen fout te zijn: {e}"));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());

    let vaststelling = &run.decisions[1].decretogram;
    assert!(
        vaststelling.obligations.is_empty(),
        "nul levert geen termijn: {:?}",
        vaststelling.obligations
    );
    let [niets] = vaststelling.niets_te_betalen.as_slice() else {
        panic!(
            "het gram hoort de verplichting te noemen: {:?}",
            vaststelling.niets_te_betalen
        );
    };
    assert_eq!(niets.soort, ObligationKind::Betaling, "nul keert niets om");

    // In het gram zoals het in de kroniek ligt: met bedrag nul.
    let gram = run
        .snapshot
        .cells
        .iter()
        .filter(|cell| cell.id == "toeslagen")
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == "beschikkingen")
        .flat_map(|chronicle| &chronicle.grams)
        .find(|gram| gram.name == "zorgtoeslag_vaststelling")
        .unwrap_or_else(|| panic!("de vaststelling hoort in de kroniek te liggen"));
    let Some(Value::Array(genoemd)) = gram.fields.get("niets_te_betalen").map(|f| &f.value) else {
        panic!("het gram hoort 'niets_te_betalen' als lijst te dragen");
    };
    let [Value::Object(velden)] = genoemd.as_slice() else {
        panic!("één verplichting zonder termijnen, kreeg {genoemd:?}");
    };
    assert_eq!(velden.get("bedrag"), Some(&Value::Int(0)));

    // Geen executogram van nul, bij geen enkele cel.
    let nul = run
        .snapshot
        .cells
        .iter()
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == "betalingen")
        .flat_map(|chronicle| &chronicle.grams)
        .filter(|gram| {
            gram.fields
                .get("bedrag")
                .and_then(|field| field.value.as_decimal())
                .is_some_and(|bedrag| bedrag.is_zero())
        })
        .count();
    assert_eq!(nul, 0, "een betaling van nul is een vastlegging van niets");

    // En het journaal zegt het, onder het besluit.
    assert!(
        run.snapshot.journal.iter().any(|entry| {
            entry.kind == JournalKind::Termijn
                && entry.parent.is_some()
                && entry.description.contains("niets te betalen")
        }),
        "het journaal hoort te zeggen dat er niets te betalen is"
    );
}
