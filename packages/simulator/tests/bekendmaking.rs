//! De **bekendmaking**: de tweede stage van de procedure, als eigen gram.
//!
//! RFC-008 kent een beschikking meer dan één stap, en RFC-022 §1.2 zegt wat dat
//! in een kroniek betekent: elke stage legt een **eigen elementair gram** op
//! hetzelfde zaakkenmerk. Dit is de meting van dat gram — wat het draagt, waar
//! die waarden vandaan komen, en wat er pas door de bekendmaking gaat lopen.
//!
//! Het scenario ernaast (`scenarios/bekendmaking.yaml`) speelt het verhaal af en
//! rekent af op wat een reductie ervan terugziet. Wat hier staat, is wat een
//! scenariobestand niet kan zeggen: de **vorm** van het gram, de herkomst per
//! veld, en dat een termijn die op de bekendmaking wachtte er vóór die dag als
//! belofte zonder datum in het besluit ligt.

use regelrecht_simulator::{
    regulation_root, GramKind, GramSnapshot, Scenario, ScenarioRun, Value, BESCHIKKINGEN,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn scenario_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join(name)
}

/// Speel een scenario af; een run die niet slaagt bewijst hier niets.
fn run(name: &str) -> ScenarioRun {
    let path = scenario_path(name);
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());
    run
}

/// De grammen in `beschikkingen` van de uitvoerende cel, op volgorde.
///
/// Uit het **beeld** en niet uit een kroniek rechtstreeks: dat is wat een lezer
/// van deze opstelling te zien krijgt, en dus waar een verschil op hoort te
/// vallen.
fn beschikkingen(run: &ScenarioRun) -> Vec<&GramSnapshot> {
    run.snapshot
        .cells
        .iter()
        .filter(|cell| cell.id == "uitvoerder")
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == BESCHIKKINGEN)
        .flat_map(|chronicle| &chronicle.grams)
        .collect()
}

/// Eén veld van een gram, als tekst; een veld dat er niet is, valt luid om.
fn veld(gram: &GramSnapshot, name: &str) -> String {
    gram.fields
        .get(name)
        .unwrap_or_else(|| panic!("het gram hoort veld '{name}' te dragen"))
        .value
        .to_string()
}

/// De waarde van één veld, zoals het gram haar draagt.
fn waarde<'a>(gram: &'a GramSnapshot, name: &str) -> &'a Value {
    &gram
        .fields
        .get(name)
        .unwrap_or_else(|| panic!("het gram hoort veld '{name}' te dragen"))
        .value
}

/// **Twee stages, twee grammen, één zaak.**
///
/// Het besluit en zijn bekendmaking staan naast elkaar in dezelfde kroniek, op
/// hetzelfde zaakkenmerk en met hetzelfde besluit erin, en ze zijn uit elkaar te
/// houden aan één veld: `stage`. Dat is precies wat RFC-022 §1.2 vraagt — wat
/// tegelijk ontstaat hoort in één gram, wat later gebeurt in een volgend.
#[test]
fn het_besluit_en_de_bekendmaking_zijn_twee_grammen_op_een_zaak() {
    let run = run("bekendmaking.yaml");
    let grams = beschikkingen(&run);
    assert_eq!(grams.len(), 2, "er horen twee grammen te liggen");

    let besluit = grams[0];
    let bekendmaking = grams[1];
    assert_eq!(veld(besluit, "stage"), "BESLUIT");
    assert_eq!(veld(bekendmaking, "stage"), "BEKENDMAKING");
    for gram in [besluit, bekendmaking] {
        assert_eq!(veld(gram, "zaakkenmerk"), "tegemoetkoming/999993653");
        assert_eq!(veld(gram, "besluit"), "toekenning");
        // Allebei een decretogram: de bekendmaking is iets wat de cel zelf deed
        // en niet iets wat haar overkwam.
        assert_eq!(gram.kind, GramKind::Decretogram);
    }
    // De bekendmaking wijst naar het besluit waar ze bij hoort: cel, kroniek en
    // plek zijn de verwijzing waarmee het beeld een gram aanwijst.
    assert_eq!(veld(bekendmaking, "besluit_gram"), "0");
    assert_eq!(veld(bekendmaking, "besluit_op_moment"), "2024-03-01");
    assert_eq!(veld(bekendmaking, "bekendmaking_datum"), "2024-04-15");
}

/// **Wat de bekendmaking toevoegt, komt uit de wet — en zegt uit welke.**
///
/// De uiterste betaaldatum, de bezwaartermijn en de rechtsmiddelenclausule staan
/// niet in de uitvoerende regeling maar in de algemene wet, en ze komen in het
/// gram langs de hooks op deze stage. Dat het gram per veld opschrijft welk
/// artikel het zei, is het verschil tussen "de wet rekent het uit" en "de
/// uitvoerder heeft het ingevuld".
#[test]
fn de_bekendmaking_draagt_de_uitkomsten_van_de_hooks_met_hun_artikel() {
    let run = run("bekendmaking.yaml");
    let grams = beschikkingen(&run);
    let bekendmaking = grams[1];

    assert_eq!(veld(bekendmaking, "uiterste_betaaldatum"), "2024-05-27");
    assert_eq!(veld(bekendmaking, "bezwaartermijn_einddatum"), "2024-05-27");
    assert_eq!(veld(bekendmaking, "bezwaar_bij"), "Uitvoerder");
    assert_eq!(veld(bekendmaking, "bezwaar_termijn_weken"), "6");

    let Value::Array(hooks) = waarde(bekendmaking, "hooks") else {
        panic!("de bekendmaking hoort de herkomst van haar uitkomsten te dragen");
    };
    let herkomst = |veld_naam: &str| -> String {
        hooks
            .iter()
            .find_map(|hook| {
                let Value::Object(entry) = hook else {
                    return None;
                };
                if entry.get("veld")?.as_str()? != veld_naam {
                    return None;
                }
                let Value::Object(lexogram) = entry.get("lexogram")? else {
                    return None;
                };
                Some(format!(
                    "{} artikel {}",
                    lexogram.get("regulation")?.as_str()?,
                    lexogram.get("artikel")?.as_str()?
                ))
            })
            .unwrap_or_else(|| panic!("veld '{veld_naam}' hoort een herkomst te hebben"))
    };
    assert_eq!(
        herkomst("uiterste_betaaldatum"),
        "test_awb_procedure artikel 4:87"
    );
    assert_eq!(
        herkomst("bezwaartermijn_einddatum"),
        "test_awb_procedure artikel 6:8"
    );
    assert_eq!(herkomst("bezwaar_bij"), "test_awb_procedure artikel 3:45");

    // En de uitkomsten van het besluit staan er **niet** nog eens in: die staan
    // in het gram van het besluit, en twee keer hetzelfde opschrijven is precies
    // hoe twee kopieën uiteen gaan lopen.
    assert!(
        !bekendmaking.fields.contains_key("hoogte_tegemoetkoming"),
        "een stage-gram herhaalt de uitkomsten van het besluit niet"
    );
}

/// **Een termijn die op de bekendmaking wacht, staat in het besluit zonder dag.**
///
/// Het besluit belooft het bedrag — met partijen, ritme en grondslag — maar het
/// kan er geen vervaldag bij zetten: die volgt uit een gebeurtenis die nog moet
/// plaatsvinden (Awb 3:40 jo. 4:87). Blijft de bekendmaking uit, dan blijft die
/// belofte staan en gaat de klok er niet overheen.
#[test]
fn een_wachtende_termijn_staat_in_het_besluit_zonder_vervaldatum() {
    let run = run("bekendmaking_blijft_uit.yaml");
    let grams = beschikkingen(&run);
    assert_eq!(
        grams.len(),
        1,
        "zonder bekendmaking ligt er alleen het besluit"
    );
    let besluit = grams[0];

    let Value::Array(wachtend) = waarde(besluit, "wacht_op_bekendmaking") else {
        panic!("het besluit hoort zijn wachtende verplichting te dragen");
    };
    assert_eq!(wachtend.len(), 1);
    let Value::Object(verplichting) = &wachtend[0] else {
        panic!("een wachtende verplichting is een object");
    };
    assert_eq!(
        verplichting.get("bedrag").map(ToString::to_string),
        Some("42000".to_string())
    );
    assert_eq!(
        verplichting.get("schuldenaar").and_then(Value::as_str),
        Some("Uitvoerder")
    );
    assert!(
        !verplichting.contains_key("vervaldatum"),
        "een wachtende verplichting heeft nog geen vervaldag; die zou verzonnen zijn"
    );

    // En er is niets ingeroosterd: `obligations` blijft leeg, dus de klok komt
    // niets na.
    let Value::Array(obligations) = waarde(besluit, "obligations") else {
        panic!("elk besluit draagt zijn schema, ook als het leeg is");
    };
    assert!(
        obligations.is_empty(),
        "zolang er niet bekendgemaakt is, vervalt er geen termijn"
    );
}

/// **De bekendmaking roostert de termijn in, op de dag die de wet noemt.**
///
/// En de termijn wijst terug naar het **besluit** waar ze uit volgt en niet naar
/// de bekendmaking: de verplichting komt uit het besluit, en een betaling hoort
/// daar naartoe te wijzen.
#[test]
fn de_bekendmaking_roostert_de_wachtende_termijn_in() {
    let run = run("bekendmaking.yaml");
    let grams = beschikkingen(&run);
    let Value::Array(obligations) = waarde(grams[1], "obligations") else {
        panic!("de bekendmaking hoort de termijnen te dragen die ze laat lopen");
    };
    assert_eq!(obligations.len(), 1, "het ritme is `ineens`");
    let Value::Object(termijn) = &obligations[0] else {
        panic!("een termijn is een object");
    };
    assert_eq!(
        termijn.get("vervaldatum").and_then(Value::as_str),
        Some("2024-05-27"),
        "de vervaldag is de uiterste betaaldatum uit de hook en niet een dag van hier"
    );
    assert_eq!(
        termijn.get("betaler").and_then(Value::as_str),
        Some("uitvoerder")
    );
}

/// **Een besluit dat vervangen is voordat het bekendgemaakt werd, roostert
/// niets meer in.**
///
/// De twee mechanismen komen hier samen: de belofte wacht in het gram van haar
/// eigen besluit (`wacht_op_bekendmaking`) en niet in de wachtrij, dus het
/// vervangende besluit komt haar bij zijn eigen afhandeling niet tegen. Ze gaat
/// pas werken bij de bekendmaking (Awb 3:40), en dáár blijkt dat er over
/// dezelfde zaak een beschikking ligt die in de plaats van deze kwam. Het gram
/// zegt welke en wanneer, en het journaal zegt het in woorden — anders is een
/// bekendmaking zonder termijnen niet te onderscheiden van een besluit dat
/// niets beloofde.
#[test]
fn een_vervangen_besluit_roostert_bij_zijn_bekendmaking_niets_meer_in() {
    let run = run("bekendmaking_na_vervanging.yaml");
    let grams = beschikkingen(&run);
    assert_eq!(
        grams.len(),
        3,
        "de toekenning, de intrekking en de bekendmaking van de toekenning"
    );
    let bekendmaking = grams[2];
    assert_eq!(veld(bekendmaking, "stage"), "BEKENDMAKING");

    let Value::Array(obligations) = waarde(bekendmaking, "obligations") else {
        panic!("de bekendmaking draagt haar schema, ook als het leeg is");
    };
    assert!(
        obligations.is_empty(),
        "er valt niets meer te beloven, dus er wordt niets ingeroosterd"
    );

    let Value::Object(vervallen) = waarde(bekendmaking, "termijnen_vervallen_door") else {
        panic!("de bekendmaking hoort te zeggen waardoor er niets gaat lopen");
    };
    assert_eq!(
        vervallen.get("besluit").and_then(Value::as_str),
        Some("intrekking")
    );
    assert_eq!(
        vervallen.get("op_moment").and_then(Value::as_str),
        Some("2024-04-01"),
        "het moment van het besluit dat ervoor in de plaats kwam"
    );
    assert!(
        vervallen
            .get("grondslag")
            .and_then(Value::as_str)
            .is_some_and(|grondslag| grondslag.contains("art. 2")),
        "de grondslag komt uit het lexogram van het vervangende artikel"
    );

    let regels: Vec<&str> = run
        .journal
        .iter()
        .map(|entry| entry.description.as_str())
        .filter(|description| description.contains("worden niet ingeroosterd"))
        .collect();
    assert_eq!(
        regels.len(),
        1,
        "één regel, bij de bekendmaking: {regels:?}"
    );
    for deel in [
        "besluit 'toekenning'",
        "tegemoetkoming/999993653",
        "besluit 'intrekking' van 2024-04-01",
        "art. 2",
    ] {
        assert!(
            regels[0].contains(deel),
            "de journaalregel hoort '{deel}' te noemen, kreeg: {}",
            regels[0]
        );
    }
}

/// **De tegenproef: zonder dat vervangende besluit gaat de termijn gewoon lopen.**
///
/// Dezelfde wereld en dezelfde twee handelingen, alleen de intrekking ertussenuit.
/// Dat ene verschil is wat de bekendmaking van een belofte een termijn maakt —
/// zou de bekendmaking hier ook niets inroosteren, dan zat de fout niet in de
/// vervanging maar in de bekendmaking zelf.
#[test]
fn zonder_vervangend_besluit_roostert_diezelfde_bekendmaking_wel_in() {
    let path = scenario_path("bekendmaking_na_vervanging.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let dag = |text: &str| {
        text.parse::<chrono::NaiveDate>()
            .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
    };
    let bsn = BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))]);
    world
        .advance(dag("2024-03-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    world
        .act("uitvoerder.toekenning", &bsn)
        .unwrap_or_else(|e| panic!("de toekenning moet kunnen: {e}"));
    world
        .advance(dag("2024-04-15"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    world
        .act("uitvoerder.bekendmaking", &BTreeMap::new())
        .unwrap_or_else(|e| panic!("de bekendmaking moet kunnen: {e}"));

    let snapshot = world.snapshot();
    let grams: Vec<&GramSnapshot> = snapshot
        .cells
        .iter()
        .filter(|cell| cell.id == "uitvoerder")
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == BESCHIKKINGEN)
        .flat_map(|chronicle| &chronicle.grams)
        .collect();
    assert_eq!(grams.len(), 2, "de toekenning en haar bekendmaking");
    let bekendmaking = grams[1];
    assert_eq!(
        waarde(bekendmaking, "termijnen_vervallen_door"),
        &Value::Null,
        "er is niets vervangen, dus het gram zwijgt erover"
    );
    let Value::Array(obligations) = waarde(bekendmaking, "obligations") else {
        panic!("de bekendmaking draagt haar schema");
    };
    assert_eq!(obligations.len(), 1, "het ritme is `ineens`");

    // En de klok komt hem na: op de uiterste betaaldatum wordt er betaald.
    world
        .advance(dag("2024-06-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    let betalingen = world
        .snapshot()
        .cells
        .iter()
        .filter(|cell| cell.id == "uitvoerder")
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == "betalingen")
        .map(|chronicle| chronicle.grams.len())
        .sum::<usize>();
    assert_eq!(betalingen, 1, "de termijn van de toekenning is nagekomen");
}
