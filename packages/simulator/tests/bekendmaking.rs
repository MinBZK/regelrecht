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
    regulation_root, FieldOrigin, GramKind, GramSnapshot, Scenario, ScenarioRun, Value,
    BESCHIKKINGEN,
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
    // De dag van de bekendmaking is het moment van haar gram.
    assert_eq!(bekendmaking.op_moment.to_string(), "2024-04-15");
}

/// **Het formulier van de bekendmaking komt uit de procedure.**
///
/// Wat er bij de bekendmaking ingevuld werd — de `requires` van de stage in de
/// procedure van de algemene wet — staat in het gram als input met herkomst
/// `parameter`, net als een parameter van een besluit. Het datumveld draagt de
/// dag van de bekendmaking onder de naam die de wet koos, en het andere veld
/// is het feit waarop art. 3:41 zijn toets doet.
#[test]
fn de_bekendmaking_draagt_haar_formulier_als_inputs() {
    let run = run("bekendmaking.yaml");
    let grams = beschikkingen(&run);
    let bekendmaking = grams[1];
    for (naam, waarde) in [
        ("datum_bekendmaking", "2024-04-15"),
        ("toegezonden_aan_belanghebbende", "true"),
    ] {
        let field = bekendmaking
            .fields
            .get(naam)
            .unwrap_or_else(|| panic!("het gram hoort input '{naam}' te dragen"));
        assert_eq!(field.value.to_string(), waarde);
        let FieldOrigin::BesluitInput { recorded_origin } = &field.origin else {
            panic!("'{naam}' hoort een input te zijn, kreeg {:?}", field.origin);
        };
        let Value::Object(origin) = recorded_origin else {
            panic!("de herkomst is een object");
        };
        assert_eq!(
            origin.get("herkomst").and_then(Value::as_str),
            Some("parameter"),
            "'{naam}' is ingevuld bij de bekendmaking"
        );
    }
    // En de toets die erop leunt, kwam uit de wet.
    assert_eq!(
        veld(bekendmaking, "op_voorgeschreven_wijze_bekendgemaakt"),
        "true"
    );
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

    assert_eq!(
        veld(bekendmaking, "uiterste_betaaldatum_4_87"),
        "2024-05-27"
    );
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
        herkomst("uiterste_betaaldatum_4_87"),
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

/// Speel de wereld van `bekendmaking_na_vervanging.yaml` af met andere handelingen.
///
/// Elke stap is een dag, een actie en de bsn waarop ze gaat (leeg voor de
/// bekendmaking, die het formulier van haar procedure krijgt: de dag en het
/// feit dat het besluit is toegezonden). Het scenariobestand legt één
/// volgorde vast; de tegenproeven hieronder schuiven met die volgorde en met
/// de zaak, en houden verder alles gelijk.
fn speel(stappen: &[(&str, &str, Option<&str>)]) -> regelrecht_simulator::World {
    let path = scenario_path("bekendmaking_na_vervanging.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for (dag, actie, bsn) in stappen {
        let dag = dag
            .parse::<chrono::NaiveDate>()
            .unwrap_or_else(|e| panic!("testdatum '{dag}' moet leesbaar zijn: {e}"));
        world
            .advance(dag)
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
        let form: BTreeMap<String, Value> = match bsn {
            Some(bsn) => BTreeMap::from([("bsn".to_string(), Value::String((*bsn).to_string()))]),
            None => BTreeMap::from([
                (
                    "datum_bekendmaking".to_string(),
                    Value::String(dag.to_string()),
                ),
                (
                    "toegezonden_aan_belanghebbende".to_string(),
                    Value::Bool(true),
                ),
            ]),
        };
        world
            .act(actie, &form)
            .unwrap_or_else(|e| panic!("'{actie}' moet kunnen op {dag}: {e}"));
    }
    world
}

/// Het gram van de laatste bekendmaking in `beschikkingen`, uit het beeld.
fn laatste_bekendmaking(world: &regelrecht_simulator::World) -> GramSnapshot {
    world
        .snapshot()
        .cells
        .iter()
        .filter(|cell| cell.id == "uitvoerder")
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == BESCHIKKINGEN)
        .flat_map(|chronicle| &chronicle.grams)
        .rfind(|gram| veld(gram, "stage") == "BEKENDMAKING")
        .cloned()
        .unwrap_or_else(|| panic!("er hoort een bekendmaking te liggen"))
}

/// Zegt het gram van deze bekendmaking dat er gewoon iets is gaan lopen?
fn roostert_in(bekendmaking: &GramSnapshot) {
    assert_eq!(
        waarde(bekendmaking, "termijnen_vervallen_door"),
        &Value::Null,
        "er is niets vervangen, dus het gram zwijgt erover"
    );
    let Value::Array(obligations) = waarde(bekendmaking, "obligations") else {
        panic!("de bekendmaking draagt haar schema");
    };
    assert_eq!(obligations.len(), 1, "het ritme is `ineens`");
}

const BSN: &str = "999993653";

/// **De tegenproef: zonder dat vervangende besluit gaat de termijn gewoon lopen.**
///
/// Dezelfde wereld en dezelfde twee handelingen, alleen de intrekking ertussenuit.
/// Dat ene verschil is wat de bekendmaking van een belofte een termijn maakt —
/// zou de bekendmaking hier ook niets inroosteren, dan zat de fout niet in de
/// vervanging maar in de bekendmaking zelf.
#[test]
fn zonder_vervangend_besluit_roostert_diezelfde_bekendmaking_wel_in() {
    let mut world = speel(&[
        ("2024-03-01", "uitvoerder.toekenning", Some(BSN)),
        ("2024-04-15", "uitvoerder.bekendmaking", None),
    ]);
    roostert_in(&laatste_bekendmaking(&world));

    // En de klok komt hem na: op de uiterste betaaldatum wordt er betaald.
    world
        .advance(
            "2024-06-01"
                .parse()
                .unwrap_or_else(|e| panic!("testdatum moet leesbaar zijn: {e}")),
        )
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

/// **Ná het besluit, en niet ergens op de zaak.** Een intrekking die vóór de
/// toekenning ligt, kwam niet in de plaats van die toekenning: de toekenning
/// staat er daarna zelf, en haar bekendmaking roostert gewoon in.
#[test]
fn een_vervanging_van_voor_het_besluit_houdt_de_bekendmaking_niet_tegen() {
    let world = speel(&[
        ("2024-02-01", "uitvoerder.intrekking", Some(BSN)),
        ("2024-03-01", "uitvoerder.toekenning", Some(BSN)),
        ("2024-04-15", "uitvoerder.bekendmaking", None),
    ]);
    roostert_in(&laatste_bekendmaking(&world));
}

/// **Dezelfde zaak, en niet de hele kroniek.** Een intrekking over een andere zaak
/// zegt niets over deze toekenning, ook al ligt ze ertussen in dezelfde kroniek.
#[test]
fn een_vervanging_op_een_andere_zaak_houdt_de_bekendmaking_niet_tegen() {
    let world = speel(&[
        ("2024-03-01", "uitvoerder.toekenning", Some(BSN)),
        ("2024-04-01", "uitvoerder.intrekking", Some("123456782")),
        ("2024-04-15", "uitvoerder.bekendmaking", None),
    ]);
    roostert_in(&laatste_bekendmaking(&world));
}

/// Speel het bekendmakingsscenario tot en met het besluit, en geef de wereld.
fn na_het_besluit() -> regelrecht_simulator::World {
    let path = scenario_path("bekendmaking.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for (dag, actie, form) in [
        (
            "2024-03-01",
            "uitvoerder.toekenning",
            BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))]),
        ),
        ("2024-04-15", "", BTreeMap::new()),
    ] {
        world
            .advance(
                dag.parse()
                    .unwrap_or_else(|e| panic!("testdatum '{dag}' moet leesbaar zijn: {e}")),
            )
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
        if !actie.is_empty() {
            world
                .act(actie, &form)
                .unwrap_or_else(|e| panic!("'{actie}' moet kunnen: {e}"));
        }
    }
    world
}

/// **Een ontbrekend veld van het formulier weigert de bekendmaking, en noemt het.**
///
/// Het formulier is wat de procedure vraagt. Een bekendmaking zonder het feit
/// waarop art. 3:41 zijn toets doet, zou een gram opleveren waarin die toets
/// stil ontbreekt; dus gaat ze niet door, en de melding zegt welk veld er mist.
#[test]
fn een_bekendmaking_zonder_verplicht_veld_wordt_geweigerd() {
    let mut world = na_het_besluit();
    let error = world
        .act(
            "uitvoerder.bekendmaking",
            &BTreeMap::from([(
                "datum_bekendmaking".to_string(),
                Value::String("2024-04-15".to_string()),
            )]),
        )
        .expect_err("zonder het feit over de toezending hoort dit te falen");
    assert!(
        error.to_string().contains("toegezonden_aan_belanghebbende"),
        "de weigering hoort het veld te noemen, kreeg: {error}"
    );
    assert_eq!(
        beschikkingen_van(&world).len(),
        1,
        "er is niets vastgelegd: alleen het besluit ligt er"
    );
}

/// **De dag van de bekendmaking is de klok.**
///
/// Het datumveld dat de procedure vraagt, draagt het moment van het gram. Een
/// andere dag zou het gram laten zeggen dat het op een ander moment gebeurde
/// dan waarop het vastgelegd werd.
#[test]
fn een_bekendmaking_op_een_andere_dag_dan_de_klok_wordt_geweigerd() {
    let mut world = na_het_besluit();
    let error = world
        .act(
            "uitvoerder.bekendmaking",
            &BTreeMap::from([
                (
                    "datum_bekendmaking".to_string(),
                    Value::String("2024-04-01".to_string()),
                ),
                (
                    "toegezonden_aan_belanghebbende".to_string(),
                    Value::Bool(true),
                ),
            ]),
        )
        .expect_err("een dag die niet de klok is, hoort te falen");
    let melding = error.to_string();
    assert!(
        melding.contains("datum_bekendmaking") && melding.contains("2024-04-15"),
        "de weigering hoort het veld en de klok te noemen, kreeg: {melding}"
    );
}

/// De grammen in `beschikkingen` van de uitvoerende cel van een wereld.
fn beschikkingen_van(world: &regelrecht_simulator::World) -> Vec<GramSnapshot> {
    world
        .snapshot()
        .cells
        .iter()
        .filter(|cell| cell.id == "uitvoerder")
        .flat_map(|cell| &cell.chronicles)
        .filter(|chronicle| chronicle.stream == BESCHIKKINGEN)
        .flat_map(|chronicle| chronicle.grams.clone())
        .collect()
}

/// De objecten in een lijstveld van een gram.
fn objecten<'a>(
    gram: &'a GramSnapshot,
    name: &str,
) -> Vec<&'a std::collections::BTreeMap<String, Value>> {
    let Value::Array(items) = waarde(gram, name) else {
        panic!("'{name}' hoort een lijst te zijn");
    };
    items
        .iter()
        .map(|item| {
            let Value::Object(fields) = item else {
                panic!("een regel in '{name}' is een object");
            };
            fields
        })
        .collect()
}

/// **Een uitkomst van de eigen regeling komt bij het ene besluit wel en bij het
/// andere van hetzelfde soort niet in het stage-gram.**
///
/// Verlening en vaststelling zijn allebei TOEKENNING, dus een hook zou op beide
/// vuren. `stage_uitkomsten` staat op het artikel van de verlening, en daarom
/// draagt alleen háár bekendmaking `voorschot_uiterlijk_op` — met het artikel van
/// de regeling dat haar uitrekende, en niet als hook.
#[test]
fn een_stage_uitkomst_hoort_bij_het_besluit_van_het_eigen_artikel() {
    let run = run("bekendmaking_stage_uitkomsten.yaml");
    let grams = beschikkingen(&run);
    let bekendmakingen: Vec<&GramSnapshot> = grams
        .iter()
        .copied()
        .filter(|gram| veld(gram, "stage") == "BEKENDMAKING")
        .collect();
    assert_eq!(bekendmakingen.len(), 2);
    let (verlening, vaststelling) = (bekendmakingen[0], bekendmakingen[1]);
    assert_eq!(veld(verlening, "besluit"), "verlening");
    assert_eq!(veld(vaststelling, "besluit"), "vaststelling");

    assert_eq!(veld(verlening, "voorschot_uiterlijk_op"), "2024-04-29");
    let herkomst = objecten(verlening, "stage_uitkomsten");
    assert_eq!(herkomst.len(), 1);
    assert_eq!(
        herkomst[0].get("veld").and_then(Value::as_str),
        Some("voorschot_uiterlijk_op")
    );
    let Some(Value::Object(lexogram)) = herkomst[0].get("lexogram") else {
        panic!("een stage-uitkomst hoort haar lexogram te dragen");
    };
    assert_eq!(
        lexogram.get("regulation").and_then(Value::as_str),
        Some("test_stage_uitkomsten")
    );
    assert_eq!(lexogram.get("artikel").and_then(Value::as_str), Some("3"));
    assert!(
        objecten(verlening, "hooks")
            .iter()
            .all(|hook| hook.get("veld").and_then(Value::as_str) != Some("voorschot_uiterlijk_op")),
        "een uitkomst van de eigen regeling is geen hook"
    );

    assert!(
        !vaststelling.fields.contains_key("voorschot_uiterlijk_op"),
        "de vaststelling declareert geen stage-uitkomst"
    );
    assert!(objecten(vaststelling, "stage_uitkomsten").is_empty());
    // Het veld van de gaten staat er altijd, ook leeg: bij een bekendmaking die
    // alles leverde, en bij een die niets declareerde.
    assert!(objecten(verlening, "stage_uitkomst_niet_geleverd").is_empty());
    assert!(objecten(vaststelling, "stage_uitkomst_niet_geleverd").is_empty());
    // De hooks van de algemene wet vuurden er wel.
    assert_eq!(
        veld(vaststelling, "uiterste_betaaldatum_4_87"),
        "2024-10-22"
    );

    // En het besluit-gram van de verlening is niet aangeraakt: de uitkomst
    // staat in het gram van de stage en niet in dat van het besluit.
    assert!(!grams[0].fields.contains_key("voorschot_uiterlijk_op"));
}

/// **Een hook zonder zijn input laat het besluit staan en zegt dat hij niet
/// draaide.**
///
/// In het gram van het besluit en in dat van de bekendmaking, elk met het artikel
/// en de input die er niet was; in het journaal als regel onder de gebeurtenis;
/// en bij het optuigen al als waarschuwing.
#[test]
fn een_hook_zonder_input_staat_in_het_gram_en_in_het_journaal() {
    let run = run("hook_zonder_input.yaml");
    let grams = beschikkingen(&run);
    assert_eq!(grams.len(), 2, "het besluit en de bekendmaking liggen er");

    for (gram, artikel, input) in [
        (grams[0], "1", "motivering_gegeven"),
        (grams[1], "2", "adres_opgegeven"),
    ] {
        let niet = objecten(gram, "hook_niet_uitgevoerd");
        assert_eq!(niet.len(), 1, "precies één hook draaide niet");
        assert_eq!(
            niet[0].get("ontbrekende_input").and_then(Value::as_str),
            Some(input)
        );
        let Some(Value::Object(lexogram)) = niet[0].get("lexogram") else {
            panic!("de hook hoort zijn artikel te noemen");
        };
        assert_eq!(
            lexogram.get("regulation").and_then(Value::as_str),
            Some("test_hook_zonder_input")
        );
        assert_eq!(
            lexogram.get("artikel").and_then(Value::as_str),
            Some(artikel)
        );
    }

    let regels: Vec<_> = run
        .journal
        .iter()
        .filter(|entry| entry.kind == regelrecht_simulator::JournalKind::HookNietUitgevoerd)
        .collect();
    assert_eq!(regels.len(), 2, "één regel per hook die niet draaide");
    for regel in regels {
        assert!(
            regel.parent.is_some(),
            "de regel hangt onder het besluit of de bekendmaking"
        );
        assert!(regel.description.contains("test_hook_zonder_input"));
    }
    assert_eq!(run.warnings.len(), 2, "het optuigen waarschuwde voor beide");
}

/// **Een gedeclareerde stage-uitkomst die niet ontstaat, staat in het gram en in
/// het journaal; een die `null` is, is gewoon geleverd.**
///
/// Twee verleningen onder hetzelfde artikel, dat drie uitkomsten voor de
/// bekendmaking declareert. Met voorschriften blijft de controle onbekend: die
/// staat niet als waarde in het gram en niet onder `stage_uitkomsten`, maar wel
/// onder `stage_uitkomst_niet_geleverd` — met het artikel en de feiten die
/// ontbraken — en er hangt een journaalregel onder de bekendmaking. Zonder
/// voorschriften zegt de regeling "er is geen dag": `null` is afwezigheid, een
/// waarde (RFC-036), en staat als waarde in het gram.
#[test]
fn een_stage_uitkomst_die_niet_ontstaat_valt_niet_stil_weg() {
    let run = run("bekendmaking_stage_uitkomst_niet_geleverd.yaml");
    let grams = beschikkingen(&run);
    let bekendmakingen: Vec<&GramSnapshot> = grams
        .iter()
        .copied()
        .filter(|gram| veld(gram, "stage") == "BEKENDMAKING")
        .collect();
    assert_eq!(bekendmakingen.len(), 2);
    let (met, zonder) = (bekendmakingen[0], bekendmakingen[1]);
    let velden = |gram: &GramSnapshot| -> Vec<String> {
        objecten(gram, "stage_uitkomsten")
            .iter()
            .filter_map(|herkomst| herkomst.get("veld").and_then(Value::as_str))
            .map(str::to_string)
            .collect()
    };

    // Met voorschriften: wat er kwam, staat er gewoon.
    assert_eq!(veld(met, "vergunning_geldig_vanaf"), "2024-03-04");
    assert_eq!(veld(met, "nakoming_gemeld_uiterlijk_op"), "2024-04-01");
    assert_eq!(
        velden(met),
        ["vergunning_geldig_vanaf", "nakoming_gemeld_uiterlijk_op"]
    );

    // De controle staat er niet als waarde, maar wel als gat.
    assert!(!met.fields.contains_key("eerste_controle_uiterlijk_op"));
    let niet = objecten(met, "stage_uitkomst_niet_geleverd");
    assert_eq!(niet.len(), 1, "precies één uitkomst kwam niet");
    assert_eq!(
        niet[0].get("uitkomst").and_then(Value::as_str),
        Some("eerste_controle_uiterlijk_op")
    );
    let Some(Value::Object(lexogram)) = niet[0].get("lexogram") else {
        panic!("de uitkomst hoort haar artikel te noemen");
    };
    assert_eq!(
        lexogram.get("regulation").and_then(Value::as_str),
        Some("test_stage_uitkomst_niet_geleverd")
    );
    assert_eq!(
        lexogram
            .get("regulation_valid_from")
            .and_then(Value::as_str),
        Some("2024-01-01")
    );
    assert_eq!(lexogram.get("artikel").and_then(Value::as_str), Some("4"));
    assert_eq!(
        niet[0].get("reden").and_then(Value::as_str),
        Some(
            "onbekend, want deze feiten ontbraken: datum_controleverzoek \
             (test_stage_uitkomst_niet_geleverd)"
        ),
        "de engine miste een feit, en dat is de reden"
    );

    // Een vast veld van de bekendmaking, en dus geen uitkomst van de regeling.
    assert!(matches!(
        met.fields["stage_uitkomst_niet_geleverd"].origin,
        FieldOrigin::Besluit
    ));

    // Zonder voorschriften: `null` is een geleverde waarde. Beide uitkomsten
    // staan als `null` in het gram, met hun herkomst, en het veld van de gaten
    // staat er — leeg.
    assert_eq!(veld(zonder, "vergunning_geldig_vanaf"), "2024-03-06");
    assert_eq!(waarde(zonder, "nakoming_gemeld_uiterlijk_op"), &Value::Null);
    assert_eq!(waarde(zonder, "eerste_controle_uiterlijk_op"), &Value::Null);
    assert_eq!(
        velden(zonder),
        [
            "vergunning_geldig_vanaf",
            "nakoming_gemeld_uiterlijk_op",
            "eerste_controle_uiterlijk_op"
        ]
    );
    assert!(objecten(zonder, "stage_uitkomst_niet_geleverd").is_empty());

    let regels: Vec<_> = run
        .journal
        .iter()
        .filter(|entry| entry.kind == regelrecht_simulator::JournalKind::StageUitkomstNietGeleverd)
        .collect();
    assert_eq!(
        regels.len(),
        1,
        "één regel, bij de bekendmaking met voorschriften"
    );
    let ouder = regels[0]
        .parent
        .expect("de regel hangt onder de bekendmaking");
    assert!(run.journal.iter().any(|entry| entry.seq == ouder
        && entry.kind == regelrecht_simulator::JournalKind::Bekendmaking
        && entry.moment.to_string() == "2024-03-04"));
    assert!(regels[0]
        .description
        .contains("eerste_controle_uiterlijk_op"));
    assert!(regels[0]
        .description
        .contains("test_stage_uitkomst_niet_geleverd artikel 4"));
}

/// **Een vervaldag die op een niet-geleverde uitkomst leunt, valt hard om.**
///
/// Een gat in het gram is goed genoeg voor een uitkomst die alleen gelezen
/// wordt, maar niet voor een termijn die erop ingeroosterd moet worden. Of de
/// uitkomst onbekend bleef of `null` was: de bekendmaking wordt geweigerd, met
/// de uitkomst in de melding, en er wordt niets vastgelegd.
#[test]
fn een_vervaldag_op_een_niet_geleverde_uitkomst_weigert_de_bekendmaking() {
    let yaml = r#"
name: een vervaldag die op een niet-geleverde uitkomst leunt
clock:
  start: 2024-01-01
cells:
  - id: uitvoerder
    identity: Uitvoerder
    laws:
      - test_stage_uitkomst_niet_geleverd
      - test_awb_procedure
    komt_na:
      - Uitvoerder
    chronicles:
      - stream: betalingen
        key: zaakkenmerk
        gebeurtenissen:
          - name: betaling_gedaan
            intake: betaling
            grondslag: Algemene wet bestuursrecht, art. 4:89
            fields: &betalingsvelden
              - name: zaakkenmerk
                type: string
              - name: bedrag
                type: amount
              - name: volgnummer
                type: number
              - name: besluit
                type: string
              - name: schuldenaar
                type: string
              - name: schuldeiser
                type: string
          - name: betaling_gemeld
            intake: levering
            grondslag: Algemene wet bestuursrecht, art. 4:89
            fields: *betalingsvelden
    besluit_definitions:
      - name: vergoeding
        doc: een vergoeding die op de dag van de controle vervalt
        regulation: test_stage_uitkomst_niet_geleverd
        output: vergoeding_toegekend
        outputs:
          - hoogte_vergoeding
        zaakkenmerk: vergoeding/{bsn}
        params:
          - name: bsn
            type: string
          - name: met_voorschriften
            type: boolean
        inputs:
          bsn:
            param: bsn
          met_voorschriften:
            param: met_voorschriften
actions:
  - id: uitvoerder.vergoeding
    actor: uitvoerder
    label: Ken de vergoeding toe
    decides:
      cell: uitvoerder
      besluit: vergoeding
  - id: uitvoerder.bekendmaking
    actor: uitvoerder
    label: Maak de vergoeding bekend
    publishes:
      cell: uitvoerder
      besluit: vergoeding
act: []
"#;
    // Met voorschriften blijft de uitkomst onbekend; zonder is ze `null`.
    for met_voorschriften in [true, false] {
        let scenario =
            Scenario::from_yaml(yaml).unwrap_or_else(|e| panic!("testscenario moet laden: {e}"));
        let mut world = scenario
            .world(&regulation_root())
            .unwrap_or_else(|e| panic!("testwereld moet op te tuigen zijn: {e}"));
        world
            .advance("2024-03-01".parse().expect("testdatum"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
        world
            .act(
                "uitvoerder.vergoeding",
                &BTreeMap::from([
                    ("bsn".to_string(), Value::String(BSN.to_string())),
                    (
                        "met_voorschriften".to_string(),
                        Value::Bool(met_voorschriften),
                    ),
                ]),
            )
            .unwrap_or_else(|e| panic!("het besluit moet kunnen: {e}"));
        world
            .advance("2024-03-04".parse().expect("testdatum"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
        let error = world
            .act(
                "uitvoerder.bekendmaking",
                &BTreeMap::from([
                    (
                        "datum_bekendmaking".to_string(),
                        Value::String("2024-03-04".to_string()),
                    ),
                    (
                        "toegezonden_aan_belanghebbende".to_string(),
                        Value::Bool(true),
                    ),
                ]),
            )
            .expect_err("een termijn zonder vervaldag hoort de bekendmaking te weigeren");
        assert!(
            matches!(
                &error,
                regelrecht_simulator::SimulatorError::BekendmakingZonderBetaaldatum { veld, .. }
                    if veld == "eerste_controle_uiterlijk_op"
            ),
            "met_voorschriften={met_voorschriften}: verwachtte een weigering op de \
             vervaldag, kreeg: {error}"
        );
        assert_eq!(
            beschikkingen_van(&world).len(),
            1,
            "er is niets vastgelegd: alleen het besluit ligt er"
        );
    }
}

/// **Een vervaldag uit de eigen regeling die al voorbij is, wordt ingehaald.**
///
/// De verplichting noemt haar vervaldatum zelf, en die kan achter de
/// bekendmaking liggen. Dan vervalt de termijn op de dag van de bekendmaking, en
/// draagt het gram de dag uit de wet als `oorspronkelijke_vervaldatum`.
#[test]
fn een_vervaldatum_van_voor_de_bekendmaking_wordt_op_die_dag_ingehaald() {
    let run = run("bekendmaking_inhalen.yaml");
    let grams = beschikkingen(&run);
    let bekendmaking = grams[1];
    assert_eq!(veld(bekendmaking, "aanvulling_uiterlijk_op"), "2024-02-29");
    let termijnen = objecten(bekendmaking, "obligations");
    assert_eq!(termijnen.len(), 1, "het ritme is `ineens`");
    assert_eq!(
        termijnen[0].get("vervaldatum").and_then(Value::as_str),
        Some("2024-04-15"),
        "de termijn vervalt op de dag van de bekendmaking"
    );
    assert_eq!(
        termijnen[0]
            .get("oorspronkelijke_vervaldatum")
            .and_then(Value::as_str),
        Some("2024-02-29"),
        "en draagt de dag die de wet noemde"
    );
}

/// De herkomst van één input van een gram, als object.
fn herkomst<'a>(gram: &'a GramSnapshot, naam: &str) -> &'a BTreeMap<String, Value> {
    let field = gram
        .fields
        .get(naam)
        .unwrap_or_else(|| panic!("het gram hoort input '{naam}' te dragen"));
    let FieldOrigin::BesluitInput { recorded_origin } = &field.origin else {
        panic!("'{naam}' hoort een input te zijn, kreeg {:?}", field.origin);
    };
    let Value::Object(origin) = recorded_origin else {
        panic!("de herkomst van '{naam}' is een object");
    };
    origin
}

/// **Een hook bij de bekendmaking leest wat het besluit vastlegde, en het gram
/// zegt waar elke waarde vandaan kwam.**
///
/// De hook rekent op drie waarden: de dag uit het formulier, een uitkomst van
/// het besluit en een input van het besluit. Het stage-gram draagt ze alle drie
/// als input, elk met haar herkomst, en die van het besluit met de plek van zijn
/// gram. Wat het besluit verder draagt maar geen hook las, staat er niet nog
/// eens in. Een expliciete `null` uit het besluit gaat mee als waarde.
#[test]
fn een_hook_bij_de_bekendmaking_leest_uitkomsten_en_inputs_van_het_besluit() {
    let run = run("bekendmaking_besluitcontext.yaml");
    let grams = beschikkingen(&run);
    assert_eq!(grams.len(), 4, "twee besluiten en hun bekendmakingen");

    for (plek, bekendmaking, later, uiterlijk) in [
        (
            0,
            grams[1],
            Value::String("2024-07-15".to_string()),
            "2024-07-15",
        ),
        (2, grams[3], Value::Null, "2024-05-07"),
    ] {
        assert_eq!(veld(bekendmaking, "stage"), "BEKENDMAKING");
        assert_eq!(veld(bekendmaking, "uiterlijk_uitgevoerd_op"), uiterlijk);

        let formulier = herkomst(bekendmaking, "datum_bekendmaking");
        assert_eq!(
            formulier.get("herkomst").and_then(Value::as_str),
            Some("parameter")
        );

        for (naam, soort, waarde_in_gram) in [
            ("later_tijdstip_in_besluit", "besluit_uitkomst", later),
            ("uitvoeringstermijn_weken", "besluit_input", Value::Int(8)),
        ] {
            assert_eq!(
                waarde(bekendmaking, naam),
                &waarde_in_gram,
                "'{naam}' gaat mee zoals het besluit haar vastlegde"
            );
            let origin = herkomst(bekendmaking, naam);
            assert_eq!(origin.get("herkomst").and_then(Value::as_str), Some(soort));
            assert_eq!(
                origin.get("besluit").and_then(Value::as_str),
                Some("toekenning")
            );
            assert_eq!(origin.get("besluit_gram"), Some(&Value::Int(plek)));
            assert_eq!(origin.get("field").and_then(Value::as_str), Some(naam));
        }

        // Wat het besluit draagt maar geen hook las, staat in het besluit-gram
        // en niet nog eens hier.
        for naam in ["vermeldt_later_tijdstip", "aanvraag_ontvangen_op", "bsn"] {
            assert!(
                !bekendmaking.fields.contains_key(naam),
                "'{naam}' las geen hook, en hoort niet in het stage-gram"
            );
        }
    }
}

/// **Wat het besluit niet draagt, blijft onbekend.**
///
/// Dezelfde wereld, maar het besluit legt geen termijn vast: zijn definitie
/// vraagt er niet om, en de regeling kan zonder. Dan krijgt de hook die parameter
/// niet, en is ze onbekend (RFC-036) — ook als het besluit wél zegt dat er geen
/// later tijdstip is.
#[test]
fn een_input_die_het_besluit_niet_vastlegt_blijft_onbekend() {
    let path = scenario_path("bekendmaking_besluitcontext.yaml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    let zonder = [
        (
            "          - name: uitvoeringstermijn_weken\n            type: number\n",
            "",
        ),
        (
            "          uitvoeringstermijn_weken:\n            param: uitvoeringstermijn_weken\n",
            "",
        ),
    ]
    .into_iter()
    .fold(text, |text, (oud, nieuw)| {
        assert_eq!(
            text.matches(oud).count(),
            1,
            "het scenario hoort '{oud}' precies één keer te bevatten"
        );
        text.replace(oud, nieuw)
    });
    let scenario = Scenario::from_yaml(&zonder)
        .unwrap_or_else(|e| panic!("het ingekorte scenario hoort te lezen: {e}"));
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("het ingekorte scenario hoort op te tuigen: {e}"));

    world
        .advance("2024-03-01".parse().expect("testdatum"))
        .expect("de klok moet vooruit kunnen");
    world
        .act(
            "uitvoerder.toekenning",
            &BTreeMap::from([
                ("bsn".to_string(), Value::String("999993653".to_string())),
                (
                    "aanvraag_ontvangen_op".to_string(),
                    Value::String("2024-01-15".to_string()),
                ),
                ("vermeldt_later_tijdstip".to_string(), Value::Bool(false)),
            ]),
        )
        .expect("het besluit hoort door te gaan");
    world
        .advance("2024-03-04".parse().expect("testdatum"))
        .expect("de klok moet vooruit kunnen");
    world
        .act(
            "uitvoerder.bekendmaking",
            &BTreeMap::from([
                (
                    "datum_bekendmaking".to_string(),
                    Value::String("2024-03-04".to_string()),
                ),
                (
                    "toegezonden_aan_belanghebbende".to_string(),
                    Value::Bool(true),
                ),
            ]),
        )
        .expect("de bekendmaking hoort door te gaan");
    let bekendmaking = laatste_bekendmaking(&world);

    let Value::Unknown(missing) = waarde(&bekendmaking, "uiterlijk_uitgevoerd_op") else {
        panic!(
            "zonder termijn hoort de datum onbekend te zijn, kreeg {}",
            waarde(&bekendmaking, "uiterlijk_uitgevoerd_op")
        );
    };
    assert!(
        missing
            .iter()
            .any(|fact| fact.name == "uitvoeringstermijn_weken"),
        "het onbekende hoort de ontbrekende parameter te noemen: {missing:?}"
    );
    assert!(
        !bekendmaking.fields.contains_key("uitvoeringstermijn_weken"),
        "wat het besluit niet draagt, staat ook niet als input in het stage-gram"
    );
    // De `null` van het besluit ging wél mee.
    assert_eq!(
        waarde(&bekendmaking, "later_tijdstip_in_besluit"),
        &Value::Null
    );
}
