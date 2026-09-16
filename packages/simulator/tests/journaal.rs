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
use regelrecht_simulator::Value;
use regelrecht_simulator::{
    journal, regulation_root, JournalActor, JournalEntry, JournalKind, Scenario, ScenarioRun,
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

/// De eerste besluitregel van dit verhaal.
fn eerste_besluit(run: &ScenarioRun) -> &JournalEntry {
    run.journal
        .iter()
        .find(|entry| entry.kind == JournalKind::Besluit)
        .expect("dit verhaal neemt besluiten")
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

/// Het besluit vertelt wát het uitvoerde: het recht, de inputs en de uitkomsten.
///
/// Dit is waar het om begonnen was. De regel noemde de actie, het gram en de
/// geaccepteerde waarden; wat er níet in stond, was welke regelingen er gedraaid
/// hebben, waarop ze rekenden en wat eruit kwam. Het journaal is de
/// hoofdweergave, dus daar hoort het verhaal van de uitvoering te staan.
#[test]
fn een_besluit_noemt_de_uitgevoerde_regelingen_met_hun_inputs_en_uitkomsten() {
    let run = run();
    let executed = eerste_besluit(&run)
        .executed
        .as_ref()
        .expect("een besluit uit het besluit-pad draagt zijn uitvoering");

    // Meer dan de regeling waarop het besluit gaat: die haalde haar
    // standaardpremie bij een uitvoeringsregeling en haar partnerbegrip bij de
    // kaderwet, en zonder die twee is niet te zien onder welk recht dit bedrag
    // tot stand kwam.
    assert_eq!(
        executed
            .regulations
            .iter()
            .map(|uitgevoerd| uitgevoerd.regulation.as_str())
            .collect::<Vec<_>>(),
        [
            "wet_op_de_zorgtoeslag",
            "algemene_wet_inkomensafhankelijke_regelingen",
            "regeling_standaardpremie",
        ],
        "de regeling van het besluit vooraan, daarachter wat zij aanriep"
    );
    assert_eq!(
        executed.regulations[0].valid_from.as_deref(),
        Some("2024-01-01"),
        "de versie die op het moment van het besluit gold, en niet de nieuwste: {:?}",
        executed.regulations[0]
    );
    assert_eq!(
        executed.describe_regulations(),
        "wet_op_de_zorgtoeslag 2024-01-01, algemene_wet_inkomensafhankelijke_regelingen, \
         regeling_standaardpremie 2024-01-01",
        "een regeling zonder versiedatum staat er kaal en niet met een verzonnen datum"
    );

    // De inputs, elk met de herkomst zoals het gram haar opschreef. De
    // geaccepteerde waarde is de scherpe: die hoort niet op een berekende te
    // lijken (invariant I5).
    let namen: Vec<&str> = executed
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect();
    assert_eq!(namen, ["bsn", "is_verzekerde", "toetsingsinkomen"]);
    let herkomsten: Vec<&str> = executed
        .inputs
        .iter()
        .filter_map(|input| input.herkomst.as_object()?.get("herkomst")?.as_str())
        .collect();
    assert_eq!(
        herkomsten,
        ["parameter", "eigen_kroniek", "geaccepteerd"],
        "elke input draagt haar herkomst in de woorden van het gram: {:?}",
        executed.inputs
    );
    let geaccepteerd = &executed.inputs[2];
    assert_eq!(
        geaccepteerd
            .herkomst
            .as_object()
            .and_then(|herkomst| herkomst.get("cell"))
            .and_then(Value::as_str),
        Some("belastingdienst"),
        "de geaccepteerde input noemt de cel die haar vaststelde: {geaccepteerd:?}"
    );

    // En wat eruit kwam: beide uitkomsten die dit besluit vastlegt.
    assert_eq!(
        executed
            .outputs
            .iter()
            .map(|output| output.name.as_str())
            .collect::<Vec<_>>(),
        ["heeft_recht_op_zorgtoeslag", "hoogte_zorgtoeslag"],
    );
    assert!(
        executed
            .outputs
            .iter()
            .any(|output| output.value != Value::Null),
        "de uitkomsten dragen hun waarde: {:?}",
        executed.outputs
    );
}

/// De uitvoering staat alleen bij een besluit, en komt uit het gram.
///
/// Twee beweringen in één, want ze houden elkaar overeind: een vastlegging voert
/// niets uit (en een lege uitvoering zou beweren dat er een regeling gedraaid
/// heeft die niets deed), en wat er bij een besluit staat, staat ook in het
/// decretogram waarnaar de regel wijst. Er is geen tweede administratie.
#[test]
fn de_uitvoering_hangt_aan_het_gram_en_aan_niets_anders() {
    let run = run();
    for entry in &run.journal {
        assert_eq!(
            entry.executed.is_some(),
            entry.kind == JournalKind::Besluit,
            "regel {} ({:?}) hoort {} uitvoering te dragen",
            entry.seq,
            entry.kind,
            if entry.kind == JournalKind::Besluit {
                "een"
            } else {
                "geen"
            }
        );
    }

    let besluit = eerste_besluit(&run);
    let executed = besluit.executed.as_ref().expect("een besluit voert uit");
    // Het gram waar deze regel naar wijst, opgezocht langs de verwijzing die ze
    // draagt: `<cel>|<kroniek>|<plek>`.
    let verwijzing = besluit
        .grams
        .first()
        .expect("een besluit legt een gram vast");
    let plek: usize = verwijzing
        .id
        .rsplit('|')
        .next()
        .and_then(|plek| plek.parse().ok())
        .expect("de verwijzing eindigt op de plek in de kroniek");
    let decretogram = run
        .snapshot
        .cells
        .iter()
        .find(|cell| cell.id == verwijzing.cell)
        .and_then(|cell| {
            cell.chronicles
                .iter()
                .find(|chronicle| chronicle.stream == verwijzing.chronicle)
        })
        .and_then(|chronicle| chronicle.grams.get(plek))
        .expect("de verwijzing wijst een gram aan dat in het beeld staat");

    for output in &executed.outputs {
        assert_eq!(
            decretogram
                .fields
                .get(&output.name)
                .map(|field| &field.value),
            Some(&output.value),
            "uitkomst '{}' hoort in het gram te staan met dezelfde waarde",
            output.name
        );
    }
    for input in &executed.inputs {
        assert_eq!(
            decretogram
                .fields
                .get(&input.name)
                .map(|field| &field.value),
            Some(&input.value),
            "input '{}' hoort in het gram te staan met dezelfde waarde",
            input.name
        );
    }
}

/// De vraag over de celgrens draagt het antwoord dat de andere cel gaf.
///
/// Een vraag zonder antwoord is een half verhaal: wie de regel leest, hoort te
/// zien wat eruit kwam en waarop dat berust, zonder het observatielog ernaast te
/// leggen — dat staat buiten de opstelling.
#[test]
fn een_vraag_draagt_het_antwoord_met_zijn_uitleg() {
    let run = run();
    let besluit = eerste_besluit(&run);
    let vraag = &run.journal[besluit.seq + 1];
    assert_eq!(vraag.kind, JournalKind::Vraag);

    let question = vraag
        .question
        .as_ref()
        .expect("een vraag draagt haar contact");
    let answer = &question.answer;
    assert_eq!(answer.cell, "belastingdienst");
    assert_eq!(
        answer
            .values()
            .and_then(|values| values.get("toetsingsinkomen")),
        Some(&Value::Int(81000)),
        "het antwoord draagt de waarde zoals de andere cel haar gaf: {answer:?}"
    );
    assert_eq!(
        answer.op_moment, vraag.moment,
        "en het moment waarop het geldt"
    );

    let reductie = answer
        .reductie
        .as_ref()
        .expect("een antwoord uit een reductie draagt zijn uitleg");
    assert!(
        reductie.genoemde_cellen().all(|cell| cell == answer.cell),
        "de uitleg gaat over de bevraagde cel en over geen andere: {reductie:?}"
    );

    // En het verslag schrijft het op, onder de vraag.
    let verslag = vraag.describe(8);
    assert!(
        verslag.contains("antwoord: toetsingsinkomen van belastingdienst"),
        "de vraag-regel hoort haar antwoord te noemen:\n{verslag}"
    );
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
