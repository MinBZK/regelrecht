//! Het journaal: het verhaal dat de wereld bijhoudt.
//!
//! Twee dingen worden hier vastgepind. Ten eerste dat een volledig verhaal een
//! journaal oplevert dat het ook echt vertelt: wie deed wat, welke grammen kwamen
//! eruit, wat veranderde er aan de stand van de zaak, en welke vraag ging
//! daarvoor over een celgrens. Ten tweede — en dat is het scherpe punt — dat de
//! **meting** die zo'n verandering oplevert niet als verkeer meetelt: ze reikt
//! nooit buiten de cel waar ze over gaat, ze levert geen contact over een
//! celgrens op, en de invarianten-gate blijft er groen onder.

use regelrecht_simulator::observation::ObservationLog;
use regelrecht_simulator::{
    journal, regulation_root, JournalActor, JournalKind, Scenario, ScenarioRun,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Het volledige verhaal: aanvraag → besluit → betalingen → tweede besluit.
///
/// Dat scenario en niet een kleiner: een journaal dat alleen een vastlegging
/// dekt, zegt niets over een besluit, een vervallen termijn of een vraag over een
/// celgrens.
fn scenario_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("toeslagen_volledig_verhaal.yaml")
}

fn run() -> ScenarioRun {
    let path = scenario_path();
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());
    run
}

/// Het verhaal staat erin, in de volgorde waarin het gebeurde.
#[test]
fn een_volledig_verhaal_levert_een_journaal_op() {
    let run = run();

    let soorten: Vec<JournalKind> = run.journal.iter().map(|entry| entry.kind).collect();
    assert_eq!(
        soorten,
        [
            JournalKind::Vastlegging,
            JournalKind::Besluit,
            JournalKind::Vraag,
            JournalKind::Betaling,
            JournalKind::Betaling,
            JournalKind::Betaling,
            JournalKind::Besluit,
            JournalKind::Vraag,
            JournalKind::Betaling,
        ],
        "dit verhaal is: een aanvraag, een besluit met zijn vraag, drie termijnen, \
         een tweede besluit met zijn vraag, en de laatste termijn"
    );

    // Oplopend in de tijd, en doorgenummerd vanaf 0: het journaal groeit
    // achteraan, zoals een kroniek.
    let momenten: Vec<_> = run.journal.iter().map(|entry| entry.moment).collect();
    let mut oplopend = momenten.clone();
    oplopend.sort();
    assert_eq!(
        momenten, oplopend,
        "een journaal loopt niet terug in de tijd"
    );
    for (plek, entry) in run.journal.iter().enumerate() {
        assert_eq!(entry.seq, plek, "de plek in het journaal is het volgnummer");
    }
}

/// De eerste regel: de aanvrager doet iets, en twee cellen weten er iets van.
#[test]
fn een_actie_draagt_haar_label_haar_grammen_en_haar_verschil() {
    let run = run();
    let aanvraag = &run.journal[0];

    assert_eq!(
        aanvraag.actor,
        JournalActor::Actor {
            id: "burger".to_string()
        },
        "de actor komt uit het wereldbestand en niet uit de cel waarin het gram landt"
    );
    assert!(
        !aanvraag.description.is_empty(),
        "de omschrijving is het label van de actie, en dat is casusdata"
    );
    assert_eq!(
        aanvraag
            .grams
            .iter()
            .map(|gram| gram.id.as_str())
            .collect::<Vec<_>>(),
        ["burger|aanvragen|0", "toeslagen|aanvragen|0"],
        "een aanvraag die geleverd wordt is één gebeurtenis met twee grammen"
    );
    assert_eq!(
        aanvraag.changes.len(),
        2,
        "beide cellen gaan van 'niets vastgesteld' naar een aanvraag: {:?}",
        aanvraag.changes
    );
    assert!(
        aanvraag
            .changes
            .iter()
            .all(|change| change.voor.is_none() && change.na.is_some()),
        "'niets vastgesteld' is een stand en geen lege waarde: {:?}",
        aanvraag.changes
    );
}

/// Een besluit vertelt wat het van een ander accepteerde, en wat het veranderde.
#[test]
fn een_besluit_draagt_wat_het_accepteerde_en_wat_het_veranderde() {
    let run = run();
    let besluit = run
        .journal
        .iter()
        .find(|entry| entry.kind == JournalKind::Besluit)
        .expect("dit verhaal neemt besluiten");

    let accepted = besluit
        .accepted
        .first()
        .expect("dit besluit accepteert het toetsingsinkomen van een andere cel");
    assert_eq!(accepted.cell, "belastingdienst");
    assert!(
        accepted.lexostatus.is_some() && accepted.op_moment.is_some(),
        "een geaccepteerde waarde draagt haar herkomst: {accepted:?}"
    );
    assert!(
        accepted.describe().contains("accepteerde toetsingsinkomen"),
        "de regel hoort te zeggen dat er geaccepteerd is: {}",
        accepted.describe()
    );

    // De vraag die dit besluit stelde, hangt eronder en staat niet los.
    let vraag = &run.journal[besluit.seq + 1];
    assert_eq!(vraag.kind, JournalKind::Vraag);
    assert_eq!(vraag.parent, Some(besluit.seq));
    let question = vraag
        .question
        .as_ref()
        .expect("een vraag-regel draagt het contact zoals het beeld het geeft");
    assert_eq!(question.answer.cell, "belastingdienst");
}

/// Een termijn die vervalt is een gebeurtenis van de klok, en geen actie.
#[test]
fn een_vervallen_termijn_staat_op_naam_van_de_klok() {
    let run = run();
    let betaling = run
        .journal
        .iter()
        .find(|entry| entry.kind == JournalKind::Betaling)
        .expect("dit verhaal betaalt vier termijnen");

    assert_eq!(betaling.actor, JournalActor::Klok);
    assert_eq!(
        betaling.grams.len(),
        2,
        "de betalende cel legt vast dat zij betaalde, de besluitende dat het haar \
         gemeld is"
    );
    assert!(
        betaling
            .changes
            .iter()
            .any(|change| change.cell == "belastingdienst"),
        "wat er betaald is, is een reductie en hoort als verschil te verschijnen: {:?}",
        betaling.changes
    );
}

/// Een verschil gaat over de cel waar de gebeurtenis landde, en nooit over een
/// andere.
///
/// Dit is de assertie waar het om begonnen was. De vóór/ná-reductie is een
/// meting van de opstelling, dus ze mag alleen de eigen kroniek van de gemeten
/// cel lezen. Zou ze buiten die cel reiken, dan zou hier een verandering staan
/// bij een cel die deze gebeurtenis niet raakte — en dan zou het journaal een
/// weg over een celgrens zijn in plaats van een verslag ervan.
#[test]
fn een_reductie_voor_een_verschil_raakt_nooit_een_andere_cel() {
    let run = run();
    for entry in &run.journal {
        let geraakt: BTreeSet<&str> = entry.grams.iter().map(|gram| gram.cell.as_str()).collect();
        for change in &entry.changes {
            assert!(
                geraakt.contains(change.cell.as_str()),
                "regel {} verandert de stand van cel '{}', maar daar landde geen gram \
                 ({geraakt:?})",
                entry.seq,
                change.cell
            );
        }
    }
}

/// De metingen leveren geen verkeer op, en de gate blijft ze niet zien.
///
/// Het journaal reduceert bij elke gebeurtenis, dus als zo'n reductie langs de
/// veiligheidscontext zou lopen, zou het aantal contacten meegroeien met het
/// aantal gebeurtenissen. Het staat op het aantal geaccepteerde waarden, en dat
/// is precies wat het recht vraagt.
#[test]
fn de_metingen_van_het_journaal_zijn_geen_contact_over_een_celgrens() {
    let run = run();

    assert!(
        run.journal.iter().any(|entry| !entry.changes.is_empty()),
        "zonder gemeten verschillen bewijst deze test niets"
    );
    assert_eq!(
        run.crossings().len(),
        2,
        "twee besluiten die elk één waarde accepteren, is twee contacten — en de \
         metingen komen daar niet bij: {}",
        run.report()
    );
    assert_eq!(
        run.snapshot.crossings.len(),
        run.crossings().len(),
        "het beeld en de gate horen dezelfde contacten te zien"
    );

    // Het meetinstrument telt hetzelfde. Zou een meting stiekem over een grens
    // gaan, dan stond ze hier wél en in de gate niet, of andersom.
    let mut log = ObservationLog::new();
    for contact in run.crossings() {
        log.record(contact);
    }
    assert_eq!(log.len(), 2);
    assert!(
        run.invariant_failures.is_empty(),
        "de invarianten-gate hoort groen te blijven: {:?}",
        run.invariant_failures
    );
}

/// Elke gramverwijzing wijst een gram aan dat er ook echt ligt.
#[test]
fn elke_verwijzing_wijst_een_gram_aan_dat_in_het_beeld_staat() {
    let run = run();
    let mut bekend: BTreeSet<String> = BTreeSet::new();
    for cell in &run.snapshot.cells {
        for chronicle in &cell.chronicles {
            for (index, _) in chronicle.grams.iter().enumerate() {
                bekend.insert(format!("{}|{}|{index}", cell.id, chronicle.stream));
            }
        }
    }

    let verwijzingen: Vec<&str> = run
        .journal
        .iter()
        .flat_map(|entry| &entry.grams)
        .map(|gram| gram.id.as_str())
        .collect();
    assert!(
        !verwijzingen.is_empty(),
        "dit verhaal legt grammen vast, dus er zijn verwijzingen"
    );
    for id in verwijzingen {
        assert!(bekend.contains(id), "'{id}' staat niet in het beeld");
    }
}

/// Het beeld draagt hetzelfde journaal als de run, en het verslag schrijft het op.
#[test]
fn het_beeld_en_het_verslag_lezen_hetzelfde_journaal() {
    let run = run();

    assert_eq!(
        run.snapshot
            .journal
            .iter()
            .map(|entry| entry.seq)
            .collect::<Vec<_>>(),
        run.journal
            .iter()
            .map(|entry| entry.seq)
            .collect::<Vec<_>>(),
        "het beeld en de run horen één journaal te delen en niet twee te maken"
    );

    let report = run.report();
    assert!(
        report.contains("journaal:"),
        "het verslag hoort het verhaal te vertellen:\n{report}"
    );
    for regel in journal::describe(&run.journal).lines() {
        assert!(
            report.contains(regel.trim_end()),
            "deze journaalregel hoort in het verslag te staan: {regel}\n{report}"
        );
    }
}
