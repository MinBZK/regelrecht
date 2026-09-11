//! Accepteren in plaats van narekenen, en wat er níet mag gebeuren.
//!
//! De scenariobestanden dragen zelf hun verwachting, dus wat hier staat zijn de
//! asserties die geen scenario kán dragen: een run die omvalt, een gram dat er
//! juist niet is, en een verwachting die fout is en dus rood hoort te worden.
//! Alle drie zijn eigenschappen van de opstelling en niet van een wereld.

use regelrecht_engine::Value;
use regelrecht_simulator::{regulation_root, Scenario, SimulatorError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Het tier-3-scenario: `toeslagen` voert een regeling uit die `brp` aanwijst.
fn tier_drie() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("toeslagen_accepteert_via_de_wet.yaml")
}

/// Het accepteer-scenario: `toeslagen` accepteert een waarde van de
/// belastingdienst en rekent dezelfde waarde daarnaast zelf na.
fn accepteren() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("toeslagen_accepteert_toetsingsinkomen.yaml")
}

fn scenario(path: &Path) -> Scenario {
    Scenario::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn bsn() -> BTreeMap<String, Value> {
    BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
}

fn date(text: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
}

/// **De cel reikt niet buiten zichzelf vanuit een reductie.**
///
/// Dezelfde regeling, dezelfde cel, dezelfde parameters — alleen langs de andere
/// ingang. `Cell::decide` krijgt een resolver en komt bij `brp` uit;
/// `Cell::reduce` krijgt er geen, dus de `source.regulation` naar die cel is voor
/// die engine onoplosbaar. Dat is geen afspraak maar een ontbrekende capability:
/// er is geen vlag om aan te zetten en geen pad omheen.
#[test]
fn een_reductie_komt_niet_bij_de_cel_tier() {
    let path = tier_drie();
    let scenario = scenario(&path);
    let world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let err = world
        .reduce("toeslagen", "partnerschap_nu", &bsn(), date("2024-01-01"))
        .expect_err("een reductie hoort niet buiten haar cel te reiken");

    assert!(
        matches!(
            &err,
            SimulatorError::ReductionReachesOutsideCell { peer, .. } if peer == "brp"
        ),
        "verwachtte ReductionReachesOutsideCell over 'brp', kreeg {err}"
    );
    // De melding hoort de lezer te vertellen wat er aan de hand is en niet alleen
    // dat er een naam onbekend was: voor de reduce-engine is `brp` een regeling
    // die ze niet kent, en die letterlijke waarheid verbergt het geval.
    let melding = err.to_string();
    assert!(
        melding.contains("reikt niet buiten de eigen cel") && melding.contains("besluit"),
        "de melding hoort te zeggen waarom dit niet kan, kreeg: {melding}"
    );
}

/// **Stelt de bron niets vast, dan valt het besluit om — en legt niets vast.**
///
/// Het moment ligt vóór de aanslag van de belastingdienst, dus die cel had toen
/// niets vastgesteld. Dat is haar goed recht en geen defect. Wat er niet mag
/// gebeuren is doorrekenen met een gat: er komt geen gram, en de melding zegt
/// welke input van welke cel ontbrak.
#[test]
fn niets_vastgesteld_bij_de_bron_laat_het_besluit_omvallen_zonder_gram() {
    let path = accepteren();
    let scenario = scenario(&path);
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // De aanslag van de belastingdienst stamt van 2024-03-01; hier besluit de
    // cel een maand eerder, met de wetsversie die dan al geldt.
    world
        .advance(date("2024-02-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

    let err = world
        .decide(
            "toeslagen",
            "zorgtoeslag_vaststelling",
            &bsn(),
            date("2024-02-01"),
        )
        .expect_err("zonder feit bij de bron hoort het besluit om te vallen");

    let melding = err.to_string();
    for deel in [
        "toetsingsinkomen",
        "belastingdienst",
        "zorgtoeslag_vaststelling",
    ] {
        assert!(
            melding.contains(deel),
            "de melding hoort '{deel}' te noemen, kreeg: {melding}"
        );
    }

    // En er ligt niets. Een half besluit is erger dan geen besluit: wie het later
    // terugleest, kan niet zien dat er een feit ontbrak.
    let terugkijk = world
        .reduce(
            "toeslagen",
            "zorgtoeslagbeschikking",
            &BTreeMap::from([(
                "zaakkenmerk".to_string(),
                Value::String("zorgtoeslag/999993653".to_string()),
            )]),
            date("2024-02-01"),
        )
        .unwrap_or_else(|e| panic!("de reductie hoort te slagen: {e}"));
    assert!(
        terugkijk.not_established().is_some(),
        "een omgevallen besluit hoort niets vast te leggen, maar er lag: {:?}",
        terugkijk.outcome
    );

    // En de wereld is heel. Een besluit haalt de besluitende cel uit de map en
    // geeft de rest aan de brug; valt het onderweg om, dan horen ze allemaal terug
    // te staan. Zou er één blijven liggen, dan zou een mislukt besluit een cel
    // laten verdwijnen — en dat zou pas bij de volgende vraag blijken, als een
    // onbegrijpelijke "onbekende cel".
    let bron = world
        .reduce(
            "belastingdienst",
            "toetsingsinkomen",
            &bsn(),
            date("2024-02-01"),
        )
        .unwrap_or_else(|e| panic!("de bron-cel hoort er nog te staan: {e}"));
    assert_eq!(bron.cell, "belastingdienst");
}

/// **Een geaccepteerde waarde wordt niet onthouden.**
///
/// Er is geen schaduwboekhouding: de waarde ging het decretogram in en nergens
/// anders — niet in een kroniek, niet in het databronregister. Een tweede besluit
/// gaat daarom opnieuw naar de bron, en dat is precies wat het vraaggraf laat
/// zien: twee besluiten, twee contacten.
#[test]
fn een_nieuw_besluit_haalt_de_waarde_opnieuw_op() {
    let path = tier_drie();
    let scenario = scenario(&path);
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    world
        .advance(date("2024-06-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

    for ronde in 1..=2 {
        let record = world
            .decide(
                "toeslagen",
                "partnerschapstoets",
                &bsn(),
                date("2024-06-01"),
            )
            .unwrap_or_else(|e| panic!("besluit {ronde} hoort te slagen: {e}"));
        assert_eq!(
            record.crossings.len(),
            1,
            "besluit {ronde} hoort de bron opnieuw te vragen, niet te onthouden"
        );
        assert_eq!(record.crossings[0].answer.cell, "brp");
        assert_eq!(
            record.decretogram.accepted_values().get("partnerschap"),
            Some(&"brp"),
            "en de waarde hoort met haar bron in het gram te staan"
        );
    }
}

/// **De invarianten-gate bijt.**
///
/// Een scenario dat zegt dat een waarde geaccepteerd is terwijl de cel haar zelf
/// vaststelde, hoort rood te worden. Zonder deze test bewijst een groene
/// `expect_accepted` alleen dat er een veld gelezen is.
#[test]
fn een_verkeerde_herkomstverwachting_wordt_rood() {
    let path = accepteren();
    let yaml = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        // Het tweede besluit rekent het toetsingsinkomen zelf na. Wie beweert dat
        // het geaccepteerd is, hoort dat niet groen te krijgen.
        .replace(
            "    expect_computed:\n      - toetsingsinkomen",
            "    expect_accepted:\n      toetsingsinkomen: belastingdienst",
        );
    assert!(
        yaml.matches("expect_accepted").count() == 2,
        "de vervanging hoort precies het tweede besluit te raken"
    );

    let scenario = Scenario::from_yaml(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    assert!(
        !run.passed(),
        "een waarde uit de eigen kroniek is niet geaccepteerd, en dat hoort te \
         blijken:\n{}",
        run.report()
    );
    assert!(
        run.report().contains("herkomst van 'toetsingsinkomen'"),
        "het verslag hoort te zeggen wat er over de herkomst mis is:\n{}",
        run.report()
    );
}

/// **Een tier-3-verwijzing zonder afspraak zegt dat de afspraak mist.**
///
/// Alleen gedeclareerde cel-ids bereiken de resolver, dus een wet die een cel
/// noemt die niet in `accepts_from` staat, komt bij de engine uit als een
/// regeling die niemand kent. Dat is letterlijk waar en het verkeerde spoor: wie
/// het leest gaat een corpusbestand zoeken dat er niet hoort te zijn, terwijl de
/// cel een afspraak mist. De melding hoort te zeggen welke van de twee het kan
/// zijn, en de naam te noemen.
#[test]
fn een_tier_3_verwijzing_zonder_afspraak_wijst_naar_de_afspraak() {
    let path = tier_drie();
    let yaml = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // Het hele `accepts_from`-blok eruit, tot aan de volgende sleutel van de cel.
    let start = yaml
        .find("    accepts_from:")
        .unwrap_or_else(|| panic!("{} hoort een accepts_from-blok te hebben", path.display()));
    let end = yaml
        .find("    besluit_definitions:")
        .unwrap_or_else(|| panic!("{} hoort besluit_definitions te hebben", path.display()));
    assert!(start < end, "het blok hoort vóór de besluiten te staan");
    let zonder_afspraak = format!("{}{}", &yaml[..start], &yaml[end..]);

    let scenario =
        Scenario::from_yaml(&zonder_afspraak).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    world
        .advance(date("2024-06-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

    let err = world
        .decide(
            "toeslagen",
            "partnerschapstoets",
            &bsn(),
            date("2024-06-01"),
        )
        .expect_err("zonder afspraak bereikt de engine de cel niet");

    assert!(
        matches!(
            &err,
            SimulatorError::UndeclaredCellSource { name, .. } if name == "brp"
        ),
        "verwachtte UndeclaredCellSource over 'brp', kreeg {err}"
    );
    let melding = err.to_string();
    assert!(
        melding.contains("accepts_from") && melding.contains("laadt geen regeling"),
        "de melding hoort beide uitwegen te noemen, kreeg: {melding}"
    );
}

/// **Een besluit dat niet kan doorgaan, valt een andere organisatie niet lastig.**
///
/// De orde is niet vrij: eerst vaststellen dat er een zaak is om over te
/// besluiten, dan pas vragen. Een zaakkenmerk dat niet eenduidig in te vullen is,
/// levert geen besluit op — en dan hoort er ook geen ondertekende vraag de
/// celgrens over te gaan. Die vraag is bij de bevraagde organisatie een
/// gebeurtenis: ze ziet wie er iets over wie kwam opvragen, en dat hoort niet te
/// gebeuren voor een besluit dat toch al niet genomen kan worden.
///
/// De test meet de orde en niet de melding: bij dit moment had de bron nog niets
/// vastgesteld, dus als er eerst gevraagd wordt, valt het besluit om over de
/// ontbrekende input in plaats van over het kenmerk.
#[test]
fn een_besluit_zonder_eenduidig_zaakkenmerk_vraagt_de_bron_niets() {
    let path = accepteren();
    let yaml = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        // Twee verwijzingen, dus een scheidingsteken dat in geen waarde mag
        // voorkomen — en hieronder komt het er wel in.
        .replace(
            "        zaakkenmerk: zorgtoeslag/{bsn}\n        params:\n          - name: bsn\n            type: string\n",
            "        zaakkenmerk: zorgtoeslag/{bsn}/{jaar}\n        params:\n          - name: bsn\n            type: string\n          - name: jaar\n            type: string\n",
        );

    let scenario = Scenario::from_yaml(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = scenario
        .world(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // Vóór de aanslag van de belastingdienst (2024-03-01): had het besluit hier
    // wél gevraagd, dan was dat de fout geworden die we terugkrijgen.
    world
        .advance(date("2024-02-01"))
        .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

    let mut params = bsn();
    params.insert("jaar".to_string(), Value::String("20/24".to_string()));

    let err = world
        .decide(
            "toeslagen",
            "zorgtoeslag_vaststelling",
            &params,
            date("2024-02-01"),
        )
        .expect_err("een kenmerk dat niet eenduidig is hoort het besluit te weigeren");

    assert!(
        matches!(err, SimulatorError::ZaakkenmerkSeparatorInValue { .. }),
        "het besluit hoort te vallen op het kenmerk en niet op een input die het \
         daarna alsnog is gaan ophalen, kreeg {err}"
    );
}

/// **Een peer die niet bestaat, blijkt bij het optuigen.**
///
/// Een cel kent geen andere cel, dus zij kan dit niet weten; de wereld kent ze
/// allemaal. Zonder deze toets zou een typfout pas tijdens een besluit opduiken,
/// als een melding over het transport in plaats van over het bestand.
#[test]
fn accepteren_van_een_onbekende_cel_wordt_bij_het_optuigen_geweigerd() {
    let path = accepteren();
    let yaml = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace(
            "accept_from: belastingdienst",
            "accept_from: belastingdienstt",
        );

    let scenario = Scenario::from_yaml(&yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let err = scenario
        .world(&regulation_root())
        .expect_err("een peer die niet bestaat hoort te falen");

    assert!(
        matches!(err, SimulatorError::UnknownAcceptedCell { .. }),
        "verwachtte UnknownAcceptedCell, kreeg {err}"
    );
}
