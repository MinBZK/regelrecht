//! De invarianten-gate, gemeten aan scenario's die hem moeten laten struikelen.
//!
//! Een gate die nooit rood wordt, is niet van een ontbrekende gate te
//! onderscheiden. Daarom staat er naast de positieve suite een map met
//! **negatieve fixtures**: scenariobestanden die met opzet één invariant
//! schenden. Deze test draait ze en rekent ze af op twee dingen — dat ze falen,
//! en waaróp ze falen. Dat tweede is het eigenlijke werk: een fixture die om de
//! verkeerde reden rood staat, bewijst niets over de invariant die hij zegt te
//! meten.
//!
//! Hier staat ook de assertie die de gate aan het meetinstrument bindt. De gate
//! leest de bewijsstukken van de veiligheidscontext; het observatielog bewaart
//! dezelfde bewijsstukken. Dat die twee hetzelfde graf opleveren is een
//! eigenschap die iemand kan breken, dus hij wordt gemeten en niet aangenomen —
//! het log hoort nergens in `src/` geïmporteerd te worden, dus de gate kan hem
//! niet zelf uitlezen.

use regelrecht_simulator::invariant::{check_invariants, observed_graph, DecisionTraffic, Traffic};
use regelrecht_simulator::observation::ObservationLog;
use regelrecht_simulator::{
    defined_graph, regulation_root, InvariantFailure, QueryEdge, Scenario, ScenarioRun,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Of een faalmelding de melding is waar een fixture over gaat.
type Expected = fn(&InvariantFailure) -> bool;

/// Elke negatieve fixture, met de melding die hij moet opleveren.
///
/// Vier kolommen: het bestand, de invariant waar hij over gaat, welke melding dat
/// is, en **hoeveel** meldingen het bestand in totaal oplevert. Die laatste staat
/// erbij omdat "de bedoelde melding zit erbij" niet uitsluit dat er nog iets
/// anders bij zit: een fixture die er een tweede schending bij krijgt, of een gate
/// die te veel meldt, blijft dan groen. Waar een fixture meer dan één melding
/// heeft, staat in zijn eigen `description` welke en waarom.
///
/// De tabel staat in Rust en niet in de bestanden zelf, en dat is met opzet: een
/// scenario dat zijn eigen falen verwacht, zou `passed()` van binnenuit kunnen
/// omdraaien, en dan is "dit scenario is groen" geen uitspraak meer. Wat er wél
/// afgedwongen wordt, is dat de tabel volledig is — zie
/// [`elke_negatieve_fixture_staat_in_de_tabel`].
const FIXTURES: [(&str, &str, Expected, usize); 6] = [
    (
        "niet_gedeclareerde_call.yaml",
        "I3",
        |failure| matches!(failure, InvariantFailure::UndeclaredCall { .. }),
        1,
    ),
    (
        // Dezelfde schending langs de andere weg naar het besluit-pad: een actie
        // lokt het besluit uit. Zou de gate alleen de rechtstreekse besluiten
        // lezen, dan zou een wereldbestand elke celgrens over kunnen door een
        // actie ervoor te zetten, en blijft deze fixture groen.
        "actie_lokt_niet_gedeclareerde_call_uit.yaml",
        "I3",
        |failure| matches!(failure, InvariantFailure::UndeclaredCall { .. }),
        1,
    ),
    (
        "gedeclareerde_call_bleef_uit.yaml",
        "I3",
        |failure| matches!(failure, InvariantFailure::CallDidNotHappen { .. }),
        1,
    ),
    (
        "cel_bevraagt_cel_buiten_haar_definities.yaml",
        "I3",
        |failure| matches!(failure, InvariantFailure::CallOutsideDefinitions { .. }),
        // Plus de declaratie die uit de pas loopt met de definities: het bestand
        // declareert de vraag die het niet mag stellen.
        2,
    ),
    (
        "declaratie_buiten_de_definities.yaml",
        "I3",
        |failure| {
            matches!(
                failure,
                InvariantFailure::DeclarationOutsideDefinitions { .. }
            )
        },
        // Plus de gedeclareerde vraag die uitbleef: ze wordt hier nooit gesteld.
        2,
    ),
    (
        "reductie_combineert_twee_cellen.yaml",
        "I4",
        |failure| matches!(failure, InvariantFailure::SynthesisOutsideBesluit { .. }),
        1,
    ),
];

fn negatief_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("negatief")
}

fn scenario_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join(name)
}

fn run(path: &Path) -> ScenarioRun {
    let scenario = Scenario::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn elke_negatieve_fixture_staat_in_de_tabel() {
    let dir = negatief_dir();
    let files: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", dir.display()))
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".yaml"))
        .collect();
    let tabel: BTreeSet<String> = FIXTURES
        .iter()
        .map(|(name, ..)| (*name).to_string())
        .collect();

    // Beide kanten. Een fixture zonder tabelregel wordt nooit gedraaid en is dus
    // een bestand dat niets doet; een tabelregel zonder fixture zou een test zijn
    // die op niets afgaat en toch groen meldt.
    assert_eq!(
        files, tabel,
        "elke negatieve fixture hoort in de tabel te staan en omgekeerd"
    );
}

#[test]
fn er_zijn_minstens_drie_negatieve_fixtures() {
    assert!(
        FIXTURES.len() >= 3,
        "één gate en één tegenvoorbeeld is te weinig: elke soort schending hoort \
         een eigen fixture te hebben"
    );
}

#[test]
fn elke_negatieve_fixture_faalt_op_de_invariant_die_hij_meet() {
    for (name, invariant, expected, meldingen) in FIXTURES {
        let path = negatief_dir().join(name);
        let run = run(&path);

        assert!(
            !run.passed(),
            "{name}: deze fixture hoort te falen, maar de run is groen:\n{}",
            run.report()
        );
        // Precies de meldingen uit de tabel en geen meldingen erbij. Een fixture
        // die er ongemerkt een tweede schending bij krijgt, meet niet meer wat hij
        // zegt te meten — ook niet als de bedoelde melding er nog bij zit.
        assert_eq!(
            run.invariant_failures.len(),
            meldingen,
            "{name}: verwachtte precies {meldingen} melding(en):\n{}",
            run.report()
        );
        let failure = run
            .invariant_failures
            .iter()
            .find(|failure| expected(failure))
            .unwrap_or_else(|| {
                panic!(
                    "{name}: verwachtte de melding van {invariant} waar deze fixture over \
                     gaat, kreeg:\n{}",
                    run.report()
                )
            });
        assert_eq!(
            failure.invariant(),
            invariant,
            "{name}: de melding hoort bij {invariant} te horen"
        );

        // De melding hoort leesbaar te zijn voor wie dit scenario niet kent:
        // welke invariant, welke actoren, en — waar dat betekenis heeft — welk
        // moment. Het verslag van de run draagt haar, want dat is wat een mens
        // bij een falende test onder ogen krijgt.
        let melding = failure.describe();
        assert!(
            melding.starts_with(invariant),
            "{name}: de melding hoort met de invariant te beginnen, kreeg: {melding}"
        );
        assert!(
            melding.contains("cel") || melding.contains("scenario"),
            "{name}: de melding hoort de actoren te noemen, kreeg: {melding}"
        );
        assert!(
            run.report().contains(&melding),
            "{name}: het verslag hoort de melding te dragen:\n{}",
            run.report()
        );
    }
}

/// De negatieve fixtures falen op een invariant, niet op een gewone verwachting.
///
/// Zonder deze test zou een fixture waarin per ongeluk een bedrag niet uitkomt
/// even rood zijn, en dan zou de suite groen blijven terwijl de gate niets deed.
#[test]
fn de_negatieve_fixtures_halen_hun_gewone_verwachtingen_wel() {
    for (name, ..) in FIXTURES {
        let path = negatief_dir().join(name);
        let run = run(&path);

        let gewone_fouten: Vec<String> = run
            .acts
            .iter()
            .map(|outcome| outcome.failures.len())
            .chain(run.decisions.iter().map(|outcome| outcome.failures.len()))
            .chain(run.outcomes.iter().map(|outcome| outcome.failures.len()))
            .chain(
                run.transport_outcomes
                    .iter()
                    .map(|outcome| outcome.failures.len()),
            )
            .chain(std::iter::once(run.failures.len()))
            .filter(|count| *count > 0)
            .map(|count| count.to_string())
            .collect();

        assert!(
            gewone_fouten.is_empty(),
            "{name}: deze fixture hoort uitsluitend op een invariant te falen, niet op \
             een verwachting bij een actie, een besluit, een vraag of de run als \
             geheel:\n{}",
            run.report()
        );
    }
}

/// Het graf dat de gate leest, is het graf dat het observatielog laat zien.
///
/// De gate kan het log niet zelf uitlezen — geen productiepad mag ernaar
/// verwijzen — dus dat ze hetzelfde zien is een eigenschap die iemand kan breken.
/// Hier wordt ze gemeten: de contacten van de run gaan in een echt log, het graf
/// komt uit de entries van dat log, en dat moet gelijk zijn aan het graf dat de
/// gate over dezelfde run afleidt.
#[test]
fn het_graf_van_de_gate_is_het_graf_van_het_observatielog() {
    for name in [
        "transport_toeslagen_brp.yaml",
        "toeslagen_accepteert_toetsingsinkomen.yaml",
        "toeslagen_accepteert_via_de_wet.yaml",
    ] {
        let path = scenario_path(name);
        let run = run(&path);
        assert!(run.passed(), "{name}:\n{}", run.report());

        let mut log = ObservationLog::new();
        for crossing in run.crossings() {
            log.record(crossing);
        }
        assert!(
            !log.is_empty(),
            "{name}: dit scenario hoort verkeer over een celgrens te hebben, anders \
             vergelijkt deze test twee lege verzamelingen"
        );

        assert_eq!(
            observed_graph(log.entries()),
            observed_graph(run.crossings()),
            "{name}: de gate en het meetinstrument horen hetzelfde graf te zien"
        );
    }
}

/// Een geslaagd scenario declareert precies wat er gebeurde.
///
/// De positieve suite dekt dit al — de gate draait bij elke run — maar niet
/// zichtbaar: daar staat alleen dat de run groen is. Hier staat waaróm dat iets
/// betekent, en dat het toegestane graf uit de celconfiguraties volgt en niet uit
/// de declaratie.
#[test]
fn het_gedeclareerde_graf_van_een_geslaagd_scenario_is_het_feitelijke() {
    let path = scenario_path("toeslagen_accepteert_toetsingsinkomen.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let verwacht = BTreeSet::from([QueryEdge {
        from: "toeslagen".to_string(),
        to: "belastingdienst".to_string(),
        lexostatus: "toetsingsinkomen".to_string(),
    }]);
    assert_eq!(
        observed_graph(run.crossings()),
        verwacht,
        "één besluit dat accepteert is één tak, en het besluit ernaast vraagt niemand \
         iets:\n{}",
        run.report()
    );
    assert_eq!(
        defined_graph(&scenario.cells),
        verwacht,
        "en dat is ook wat de `accept_from`-input van haar besluit-definitie toestaat"
    );
}

/// Een gram dat zegt te hebben geaccepteerd zonder vastgelegd contact, valt op.
///
/// Dit is de I2-kant van de naad, en hij is met opzet niet als fixture te
/// schrijven: in de opstelling is het onmogelijk, want de enige weg naar een
/// geaccepteerde waarde loopt langs de veiligheidscontext, die het bewijsstuk
/// teruggeeft. Wat hier gebouwd wordt is dus geen wereld maar een meting: een
/// echt decretogram, en de contacten weggelaten. Zou de gate dit laten passeren,
/// dan zou een incompleet log niet van een volledig te onderscheiden zijn — en
/// dat is precies de kant waar het gevaarlijk is.
#[test]
fn een_geaccepteerde_waarde_zonder_contact_faalt_op_i2() {
    let path = scenario_path("toeslagen_accepteert_toetsingsinkomen.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = run(&path);
    let accepterend = &run.decisions[0];

    let zonder_contact = Traffic {
        decisions: vec![DecisionTraffic {
            decretogram: &accepterend.decretogram,
            crossings: &[],
        }],
        probes: Vec::new(),
    };
    let failures = check_invariants(&scenario.query_graph, &scenario.cells, &zonder_contact);

    let melding = failures
        .iter()
        .find(|failure| matches!(failure, InvariantFailure::AcceptedWithoutCrossing { .. }))
        .map(InvariantFailure::describe)
        .unwrap_or_else(|| panic!("verwachtte een I2-melding, kreeg {failures:?}"));
    assert!(
        melding.starts_with("I2")
            && melding.contains("belastingdienst")
            && melding.contains("zorgtoeslag/999993653"),
        "de melding hoort de bron en de zaak te noemen, kreeg: {melding}"
    );
}

/// Een contact dat het decretogram niet laat zien, valt op (I4).
///
/// De andere kant van dezelfde naad, en ook deze is in de opstelling onmogelijk:
/// wat een besluit accepteert, staat in zijn gram. De meting zet daarom het
/// contact van het accepterende besluit naast het gram van het besluit dat
/// nárekende — twee echte artefacten, verkeerd aan elkaar geknoopt. Zonder deze
/// toets zou een cel die over de grens reikt en het niet vastlegt, alleen aan een
/// declaratie op te merken zijn.
#[test]
fn een_contact_dat_het_gram_niet_laat_zien_faalt_op_i4() {
    let path = scenario_path("toeslagen_accepteert_toetsingsinkomen.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = run(&path);
    let accepterend = &run.decisions[0];
    let narekenend = &run.decisions[1];

    let verkeerd_geknoopt = Traffic {
        decisions: vec![DecisionTraffic {
            decretogram: &narekenend.decretogram,
            crossings: &accepterend.crossings,
        }],
        probes: Vec::new(),
    };
    let failures = check_invariants(&scenario.query_graph, &scenario.cells, &verkeerd_geknoopt);

    let melding = failures
        .iter()
        .find(|failure| matches!(failure, InvariantFailure::CrossingNotInDecretogram { .. }))
        .map(InvariantFailure::describe)
        .unwrap_or_else(|| panic!("verwachtte een I4-melding, kreeg {failures:?}"));
    assert!(
        melding.starts_with("I4")
            && melding.contains("belastingdienst")
            && melding.contains("zorgtoeslag-eigen/999993653")
            && melding.contains("2024-06-01"),
        "de melding hoort de actoren, de zaak en het moment te noemen, kreeg: {melding}"
    );
}
