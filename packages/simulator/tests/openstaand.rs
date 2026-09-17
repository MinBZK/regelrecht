//! De openstaandvorm: wat er verwacht werd, wat er gebeurde, en het verschil.
//!
//! Drie dingen worden hier vastgepind. Ten eerste dat de **uitleg** bij het
//! antwoord beide stromen noemt en elk gelezen gram met zijn bijdrage — zonder
//! dat is een bedrag niet na te rekenen, en dan is het verschil met een saldo
//! alleen nog een belofte. Ten tweede dat een termijn die niet nagekomen wordt
//! een journaalregel oplevert zónder gram, en dat hij daarna ook niet stil
//! alsnog voldaan raakt. Ten derde dat een definitie die deze vorm verkeerd
//! opschrijft bij het **optuigen** strandt en niet bij de eerste vraag.
//!
//! Het scenario staat in `scenarios/` en niet hier: het is een verhaal dat
//! iemand kan navertellen, en deze meting leest het na. Wat hier wél in het
//! bestand staat, zijn de wereldjes die met opzet niet deugen — die horen niet
//! tussen de scenario's die draaien.

use regelrecht_simulator::{
    regulation_root, JournalKind, Lexostatus, LexostatusOutcome, Reductie, ReductieVorm, Scenario,
    ScenarioRun, Value, World, WorldDefinition, BESCHIKKINGEN, BETALINGEN,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// De BSN uit de publieke wereld: een testnummer uit de officiële reeks.
const BSN: &str = "999993653";

/// Het scenario waarin de tweede termijn niet nagekomen wordt.
fn scenario_path() -> PathBuf {
    scenario("toeslagen_openstaande_termijnen.yaml")
}

fn scenario(naam: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join(naam)
}

fn run() -> ScenarioRun {
    run_path(&scenario_path())
}

fn run_path(path: &Path) -> ScenarioRun {
    let scenario = Scenario::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());
    run
}

/// Het antwoord op de vraag over dit moment.
fn antwoord(run: &ScenarioRun, op_moment: &str) -> Lexostatus {
    run.outcomes
        .iter()
        .find(|outcome| {
            outcome.lexostatus == "openstaande_termijnen"
                && outcome.lexostatus_value.op_moment.to_string() == op_moment
        })
        .map(|outcome| outcome.lexostatus_value.clone())
        .unwrap_or_else(|| panic!("dit scenario vraagt naar openstaand op {op_moment}"))
}

/// De uitleg bij een antwoord. Ze hoort er altijd te zijn.
fn uitleg(answer: &Lexostatus) -> &Reductie {
    answer.reductie.as_ref().unwrap_or_else(|| {
        panic!("elk antwoord van een reductie hoort te zeggen hoe het tot stand kwam")
    })
}

/// Eén gepubliceerde uitkomst van een antwoord.
fn waarde(answer: &Lexostatus, name: &str) -> Value {
    match &answer.outcome {
        LexostatusOutcome::Established(values) => values
            .get(name)
            .cloned()
            .unwrap_or_else(|| panic!("dit antwoord hoort '{name}' te publiceren")),
        LexostatusOutcome::NotEstablished { reason } => {
            panic!("verwachtte een antwoord, en kreeg: {reason}")
        }
    }
}

/// De termijnen uit een antwoord, als `<volgnummer>:<status>`.
fn standen(answer: &Lexostatus) -> Vec<String> {
    termijnen(answer)
        .iter()
        .map(|termijn| {
            format!(
                "{}:{}",
                veld(termijn, "volgnummer"),
                veld(termijn, "status")
            )
        })
        .collect()
}

/// De uitleg noemt beide stromen, en elk gram met wat het bijdroeg.
#[test]
fn de_uitleg_noemt_beide_stromen_met_hun_bijdrage() {
    let run = run();
    let answer = antwoord(&run, "2024-06-01");
    let reductie = uitleg(&answer);

    let ReductieVorm::Openstaand(vorm) = &reductie.vorm else {
        panic!("verwachtte de openstaandvorm");
    };
    assert_eq!(vorm.beschikkingen, BESCHIKKINGEN);
    assert_eq!(vorm.betalingen, BETALINGEN);
    assert_eq!(vorm.key, "zaakkenmerk");
    assert_eq!(
        vorm.key_value,
        Value::String("kwartaal/999993653".to_string())
    );

    // Het decretogram met wat eruit verwacht werd, en de betaling met wat ze
    // daarvan dekte. Zo staan de twee bedragen van het antwoord er allebei
    // uitgesplitst onder.
    let gelezen: Vec<(String, String, String)> = reductie
        .grammen
        .iter()
        .map(|gram| {
            (
                gram.gram.chronicle.clone(),
                gram.gram.name.clone(),
                gram.bijdrage
                    .as_ref()
                    .map_or_else(|| "geen".to_string(), ToString::to_string),
            )
        })
        .collect();
    assert_eq!(
        gelezen,
        [
            (
                BESCHIKKINGEN.to_string(),
                "toekenning_per_kwartaal".to_string(),
                "60000".to_string()
            ),
            (
                BETALINGEN.to_string(),
                "betaling_gemeld".to_string(),
                "30000".to_string()
            ),
        ],
        "de uitleg hoort het decretogram met het verwachte bedrag te noemen en de \
         betaling met wat ze dekte"
    );
}

/// Een termijn die niet nagekomen wordt, is een regel zonder gram.
#[test]
fn een_termijn_die_niet_nagekomen_wordt_levert_een_journaalregel_op() {
    let run = run();
    let regels: Vec<_> = run
        .journal
        .iter()
        .filter(|entry| entry.kind == JournalKind::NietNagekomen)
        .collect();

    assert_eq!(
        regels.len(),
        1,
        "precies één termijn hoort hier niet nagekomen te zijn"
    );
    let regel = regels[0];
    assert_eq!(regel.moment.to_string(), "2024-05-01");
    assert!(
        regel.description.starts_with("termijn niet nagekomen"),
        "de regel hoort te zeggen wat er niet gebeurde: {}",
        regel.description
    );
    assert!(
        regel.grams.is_empty(),
        "er is niets vastgelegd, dus er hoort geen gram aan te hangen"
    );
    assert!(
        regel.changes.is_empty(),
        "zonder vastlegging verandert er niets aan de stand van de zaak"
    );
}

/// Wat openbleef, blijft open: de klok komt er niet later alsnog op terug.
#[test]
fn een_opengebleven_termijn_wordt_niet_stil_alsnog_betaald() {
    let run = run();
    let laat = antwoord(&run, "2024-06-01");

    assert_eq!(standen(&laat), ["1:betaald", "2:te_laat"]);
    assert_eq!(waarde(&laat, "verwacht"), Value::Int(60000));
    assert_eq!(waarde(&laat, "betaald"), Value::Int(30000));
    assert_eq!(waarde(&laat, "openstaand"), Value::Int(30000));

    // De klok stond aan het eind van de run op 2024-06-01 en is dus langs de
    // tweede vervaldatum gekomen. Er ligt één betaling, niet twee.
    let betalingen = run
        .journal
        .iter()
        .filter(|entry| entry.kind == JournalKind::Betaling)
        .count();
    assert_eq!(betalingen, 1, "alleen de eerste termijn is nagekomen");
}

/// Dezelfde vraag over een eerder moment blijft hetzelfde antwoord geven.
#[test]
fn het_antwoord_over_een_eerder_moment_verandert_niet_mee() {
    let run = run();
    let vroeg = antwoord(&run, "2024-04-30");

    assert_eq!(standen(&vroeg), ["1:betaald"]);
    assert_eq!(waarde(&vroeg, "openstaand"), Value::Int(0));
}

/// Zonder decretogram over de zaak is er niets vastgesteld, met wat er wél lag.
#[test]
fn zonder_besluit_is_er_niets_vastgesteld() {
    let run = run();
    let leeg = run
        .outcomes
        .iter()
        .find(|outcome| {
            outcome.lexostatus == "openstaande_termijnen"
                && outcome.lexostatus_value.not_established().is_some()
        })
        .expect("dit scenario vraagt ook naar een zaak die er niet is");

    let reductie = uitleg(&leeg.lexostatus_value);
    let gemist = reductie
        .gemist
        .expect("niets vastgesteld hoort te zeggen wat er wél lag");
    assert_eq!(gemist.in_de_stroom, 1);
    assert_eq!(gemist.andere_sleutel, 1);
    assert!(
        reductie.grammen.is_empty(),
        "er is geen gram gelezen, dus er hoort er geen genoemd te worden"
    );
}

/// Een verplichting die op de bekendmaking wacht, staat er zonder vervaldag.
///
/// Opgelegd en toch niet verwacht. Dat onderscheid is de hele reden dat ze in de
/// lijst staat: "er staat niets open" zou anders hetzelfde antwoord zijn als
/// "hier loopt niets", en dat is precies het verschil dat Awb 3:40 maakt.
#[test]
fn wat_op_de_bekendmaking_wacht_heeft_geen_vervaldag() {
    let pad = scenario("bekendmaking_blijft_uit.yaml");
    let run = run_path(&pad);
    let answer = antwoord(&run, "2024-12-01");

    assert_eq!(standen(&answer), ["1:wacht_op_bekendmaking"]);
    // Geen vervaldag, dus niets verwacht en niets te laat — ook niet negen
    // maanden later. Een nul die hier wél zou meetellen, zou een termijn
    // opleveren die te laat is op een dag die de wet nooit genoemd heeft.
    assert_eq!(waarde(&answer, "verwacht"), Value::Int(0));
    assert_eq!(waarde(&answer, "openstaand"), Value::Int(0));
}

/// Zodra de bekendmaking er is, staan de termijnen in háár gram.
#[test]
fn de_bekendmaking_geeft_de_wachtende_verplichting_haar_termijnen() {
    let pad = scenario("bekendmaking.yaml");
    let run = run_path(&pad);

    // Vóór de bekendmaking: één regel, zonder vervaldag.
    assert_eq!(
        standen(&antwoord(&run, "2024-03-15")),
        ["1:wacht_op_bekendmaking"]
    );

    // Op de dag van de bekendmaking heeft ze een vervaldag, en die ligt ná dit
    // moment: dan staat ze niet in de lijst en is er niets verwacht.
    let bekend = antwoord(&run, "2024-04-15");
    assert!(standen(&bekend).is_empty());
    assert_eq!(waarde(&bekend, "verwacht"), Value::Int(0));

    // En na de uiterste betaaldatum is ze vervallen en nagekomen.
    let later = antwoord(&run, "2024-06-01");
    assert_eq!(standen(&later), ["1:betaald"]);
    assert_eq!(waarde(&later, "verwacht"), Value::Int(42000));

    // Twee grammen gelezen in `beschikkingen`: het besluit en zijn bekendmaking.
    // Alleen de tweede droeg de termijn, en dat is aan de bijdragen te zien.
    let uit_de_beschikkingen: Vec<String> = uitleg(&later)
        .grammen
        .iter()
        .filter(|gram| gram.gram.chronicle == BESCHIKKINGEN)
        .map(|gram| {
            format!(
                "{}={}",
                gram.gram.name,
                gram.bijdrage
                    .as_ref()
                    .map_or_else(|| "geen".to_string(), ToString::to_string)
            )
        })
        .collect();
    assert_eq!(
        uit_de_beschikkingen,
        ["toekenning=0", "toekenning_bekendmaking=42000"]
    );
}

/// Een termijn die door een latere vaststelling vervalt, staat er als `vervallen`.
///
/// De publieke wereld, met de vaststelling **vóór** de laatste voorschottermijn —
/// hetzelfde geval als in `tests/vervanging.rs`, van de andere kant bekeken: daar
/// wordt gemeten dat de klok haar niet meer nakomt, hier dat de vraag "wat staat
/// er nog open" haar niet als schuld opvoert. Zonder dat zou deze reductie een
/// bedrag noemen dat de wet heeft laten vervallen.
#[test]
fn een_termijn_die_door_een_vaststelling_vervalt_telt_niet_mee() {
    let pad = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml");
    let definition =
        WorldDefinition::load(&pad).unwrap_or_else(|e| panic!("{}: {e}", pad.display()));
    let mut world = World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", pad.display()));
    let bsn = BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))]);
    let zaak = BTreeMap::from([(
        "zaakkenmerk".to_string(),
        Value::String(format!("zorgtoeslag/{BSN}")),
    )]);

    // De toekenning valt in april, dus de vierde kwartaaltermijn vervalt pas in
    // januari; de vaststelling komt daar met opzet vóór.
    for (moment, besluit) in [
        ("2024-04-01", Some("zorgtoeslag_toekenning")),
        ("2024-12-15", Some("zorgtoeslag_vaststelling")),
        ("2025-02-01", None),
    ] {
        let dag = dag(moment);
        world
            .advance(dag)
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
        if let Some(besluit) = besluit {
            world
                .decide("toeslagen", besluit, &bsn, dag)
                .unwrap_or_else(|e| panic!("{besluit} moet kunnen: {e}"));
        }
    }

    let answer = world
        .reduce(
            "toeslagen",
            "openstaande_termijnen",
            &zaak,
            dag("2025-02-01"),
        )
        .unwrap_or_else(|e| panic!("de vraag hoort beantwoord te worden: {e}"));

    // De vierde voorschottermijn: haar vervaldag (1 januari) is gepasseerd en er
    // is niet op betaald, en toch is ze niet te laat.
    let vervallen: Vec<String> = termijnen(&answer)
        .iter()
        .filter(|termijn| veld(termijn, "status") == "vervallen")
        .map(|termijn| veld(termijn, "volgnummer"))
        .collect();
    assert_eq!(vervallen, ["4"], "standen: {:?}", standen(&answer));

    // En ze telt niet mee: wat er verwacht werd is wat er betaald is, dus er
    // staat niets open. Een `te_laat` zou hier 49296,01 als schuld opvoeren.
    assert_eq!(waarde(&answer, "openstaand"), Value::Int(0));
    assert_eq!(
        waarde(&answer, "verwacht"),
        waarde(&answer, "betaald"),
        "standen: {:?}",
        standen(&answer)
    );
}

/// Eén veld van een termijn uit het antwoord, als tekst.
fn veld(termijn: &Value, name: &str) -> String {
    let Value::Object(velden) = termijn else {
        panic!("elke termijn hoort een object te zijn");
    };
    velden[name].to_string()
}

/// De termijnen uit een antwoord, ongelezen.
fn termijnen(answer: &Lexostatus) -> Vec<Value> {
    let Value::Array(termijnen) = waarde(answer, "termijnen") else {
        panic!("'termijnen' hoort een lijst te zijn");
    };
    termijnen
}

fn dag(text: &str) -> chrono::NaiveDate {
    text.parse()
        .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
}

/// Een wereldbestand dat niet hoort op te tuigen, met de melding die erbij hoort.
fn weigert(cel: &str, verwacht: &str) {
    let yaml = format!(
        r"
clock:
  start: 2024-01-01

cells:
{cel}
"
    );
    let definition = WorldDefinition::from_yaml(&yaml);
    let melding = match definition {
        Err(error) => error.to_string(),
        Ok(definition) => match World::from_definition(&definition, &regulation_root()) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("dit wereldbestand hoort niet op te tuigen"),
        },
    };
    assert!(
        melding.contains(verwacht),
        "de melding hoort '{verwacht}' te noemen, en was: {melding}"
    );
}

/// De openstaandvorm publiceert haar eigen uitkomsten; `outputs` erbij is een fout.
#[test]
fn openstaand_met_outputs_wordt_geweigerd() {
    weigert(
        r"  - id: toeslagen
    identity: Dienst Toeslagen
    laws:
      - test_openstaande_termijnen
    besluit_definitions:
      - name: toekenning
        regulation: test_openstaande_termijnen
        output: toegekende_termijnen
        zaakkenmerk: zaak/{bsn}
        params:
          - name: bsn
            type: string
        inputs:
          bsn:
            param: bsn
    chronicles:
      - stream: betalingen
        key: zaakkenmerk
        gebeurtenissen:
          - name: betaling_gemeld
            intake: levering
            grondslag: test
            fields:
              - name: zaakkenmerk
                type: string
              - name: bedrag
                type: amount
              - name: volgnummer
                type: number
              - name: besluit
                type: string
    lexostatus_definitions:
      - name: openstaand
        inputs:
          - name: zaakkenmerk
            type: string
        outputs:
          - openstaand
        reduction:
          openstaand: true
          key: zaakkenmerk
",
        "laat `outputs` weg",
    );
}

/// `openstaand: false` is geen derde schrijfwijze maar een typfout.
#[test]
fn openstaand_false_wordt_geweigerd() {
    weigert(
        r"  - id: bron
    laws: []
    lexostatus_definitions:
      - name: openstaand
        inputs:
          - name: zaakkenmerk
            type: string
        reduction:
          openstaand: false
          key: zaakkenmerk
",
        "`openstaand: false` is geen reductie",
    );
}

/// Een opschorting die geen `true` en geen datum is, strandt bij het lezen.
#[test]
fn een_onleesbare_opschorting_wordt_geweigerd() {
    weigert(
        r"  - id: bron
    laws: []
    betalingen_opgeschort: morgen
",
        "is geen opschorting; schrijf `true`",
    );
}

/// Een vervanging die met terugwerkende kracht genomen wordt, laat een termijn
/// die de klok al nakwam niet alsnog vervallen.
///
/// De klok staat al voorbij de vierde voorschottermijn als de vaststelling op
/// een eerder moment genomen wordt. Die termijn stond toen niet meer in de
/// wachtrij — ze ís betaald — en dus hoort ze als `betaald` in de lijst te
/// staan, met haar betaling meegeteld, en niet als `vervallen`.
#[test]
fn een_betaalde_termijn_vervalt_niet_door_een_latere_vaststelling() {
    let pad = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml");
    let definition =
        WorldDefinition::load(&pad).unwrap_or_else(|e| panic!("{}: {e}", pad.display()));
    let mut world = World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", pad.display()));
    let bsn = BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))]);
    let zaak = BTreeMap::from([(
        "zaakkenmerk".to_string(),
        Value::String(format!("zorgtoeslag/{BSN}")),
    )]);

    world
        .advance(dag("2024-04-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    world
        .decide(
            "toeslagen",
            "zorgtoeslag_toekenning",
            &bsn,
            dag("2024-04-01"),
        )
        .unwrap_or_else(|e| panic!("de toekenning moet kunnen: {e}"));
    world
        .advance(dag("2025-02-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));
    world
        .decide(
            "toeslagen",
            "zorgtoeslag_vaststelling",
            &bsn,
            dag("2024-12-15"),
        )
        .unwrap_or_else(|e| panic!("de vaststelling moet kunnen: {e}"));

    let answer = world
        .reduce(
            "toeslagen",
            "openstaande_termijnen",
            &zaak,
            dag("2025-02-01"),
        )
        .unwrap_or_else(|e| panic!("de vraag hoort beantwoord te worden: {e}"));

    assert!(
        standen(&answer).iter().any(|stand| stand == "4:betaald"),
        "standen: {:?}",
        standen(&answer)
    );
    assert_eq!(waarde(&answer, "openstaand"), Value::Int(0));
    assert_eq!(waarde(&answer, "verwacht"), waarde(&answer, "betaald"));

    // En de uitleg telt op tot hetzelfde: de betalingen dragen samen precies
    // `betaald` bij, ook die van de termijn die zonder betaling vervallen was.
    let bijdragen: Vec<Value> = uitleg(&answer)
        .grammen
        .iter()
        .filter(|gram| gram.gram.chronicle == BETALINGEN)
        .filter_map(|gram| gram.bijdrage.clone())
        .collect();
    let som = bijdragen
        .iter()
        .filter_map(Value::as_decimal)
        .sum::<rust_decimal::Decimal>();
    assert_eq!(
        waarde(&answer, "betaald").as_decimal(),
        Some(som),
        "bijdragen: {bijdragen:?}"
    );
}

/// Een verplichting die op de bekendmaking wachtte, vervalt door een besluit dat
/// vóór die bekendmaking in de plaats kwam — en blijft vervallen nadat de
/// bekendmaking er alsnog is.
///
/// Een besluit werkt pas door zijn bekendmaking (Awb 3:40). Ligt er over dezelfde
/// zaak dan al een besluit dat in de plaats van dit besluit kwam, dan gaat er
/// niets meer lopen, en is er niets dat nog op de bekendmaking wacht. Zonder dat
/// zou de lijst een belofte blijven tonen die nooit meer wordt ingelost — of haar
/// na de bekendmaking stil laten verdwijnen.
#[test]
fn een_wachtende_verplichting_vervalt_door_een_intrekking_voor_de_bekendmaking() {
    let run = run_path(&scenario("bekendmaking_na_vervanging.yaml"));

    let voor_de_intrekking = antwoord(&run, "2024-03-15");
    assert_eq!(standen(&voor_de_intrekking), ["1:wacht_op_bekendmaking"]);

    for moment in ["2024-04-10", "2025-04-15"] {
        let answer = antwoord(&run, moment);
        assert_eq!(standen(&answer), ["1:vervallen"], "op {moment}");
        assert_eq!(waarde(&answer, "verwacht"), Value::Int(0), "op {moment}");
        assert_eq!(waarde(&answer, "openstaand"), Value::Int(0), "op {moment}");
    }
}
