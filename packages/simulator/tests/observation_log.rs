//! Het observatielog, en de grenzen die het op zijn plek houden.
//!
//! Twee soorten assertie staan hier naast elkaar, en dat is geen toeval. De
//! eerste meet wat er over een celgrens ging; de tweede bewaakt dat het
//! meetinstrument geen onderdeel van de opstelling wordt. Zonder de tweede is
//! de eerste op termijn een leugen: een observator die ergens in een
//! productiepad wordt aangeroepen, kent het totaalbeeld dat nergens hoort te
//! bestaan.

use regelrecht_engine::Value;
use regelrecht_simulator::observation::ObservationLog;
use regelrecht_simulator::security::SIMULATED_SIGNATURE_PREFIX;
use regelrecht_simulator::{regulation_root, Scenario};
use std::path::{Path, PathBuf};

/// Het scenario met twee cellen: `toeslagen` vraagt bij `brp`.
fn scenario_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("transport_toeslagen_brp.yaml")
}

/// Alle Rust-bestanden onder `src/`, met hun pad relatief aan de crate.
fn source_files() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|entry| {
            let path = entry.path();
            let relative = path
                .strip_prefix(&root)
                .unwrap_or_else(|e| panic!("{} valt buiten src/: {e}", path.display()))
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
            (relative, text)
        })
        .collect()
}

/// Regels die niet alleen commentaar zijn: alleen die verwijzen echt.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.is_empty()
        })
}

/// Komt `needle` in `line` voor als heel woord, en niet als deel van een langere
/// naam? `CellTransport` is dus geen voorkomen van `Cell`.
fn mentions_word(line: &str, needle: &str) -> bool {
    let part_of_identifier = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(needle).any(|(start, _)| {
        let before = line[..start].chars().next_back();
        let after = line[start + needle.len()..].chars().next();
        !before.is_some_and(part_of_identifier) && !after.is_some_and(part_of_identifier)
    })
}

#[test]
fn een_vraag_over_de_celgrens_levert_exact_een_regel_in_het_log() {
    let path = scenario_path();
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}", run.report());

    // Het log is een recorder: hij krijgt aangereikt wat er over de grens ging en
    // beslist niets. Precies daarom staat hij buiten de opstelling.
    let mut log = ObservationLog::new();
    for outcome in &run.transport_outcomes {
        log.record(&outcome.signed);
    }

    assert_eq!(
        log.len(),
        1,
        "één vraag over de grens hoort één regel te zijn:\n{}",
        log.report()
    );

    let entry = &log.entries()[0];
    assert_eq!(entry.asked_by.cell(), "toeslagen", "de vrager");
    assert_eq!(entry.answer.cell, "brp", "de bevraagde cel");
    assert_eq!(entry.answer.name, "partnerschap", "de lexostatus");
    assert_eq!(
        entry.params.get("bsn"),
        Some(&Value::String("999993653".to_string())),
        "de parameters waarmee gevraagd is"
    );
    assert_eq!(
        entry.answer.op_moment.to_string(),
        "2024-01-01",
        "het moment waarop de vraag gold"
    );
    assert!(
        entry.answer.values().is_some(),
        "en wat er terugkwam: {}",
        log.report()
    );

    // De ondertekening staat in het log, én dat ze gesimuleerd is. Wie dat laatste
    // weglaat, leest later een placeholder als bewijs.
    assert!(
        entry.signature.is_simulated(),
        "de ondertekening hoort herkenbaar nep te zijn: {}",
        entry.signature
    );
    assert_eq!(
        entry.signature.signer().cell(),
        "toeslagen",
        "en ze hoort de identiteit te noemen die ondertekende"
    );
    assert!(
        log.report().contains(SIMULATED_SIGNATURE_PREFIX),
        "het verslag hoort te laten zien dat er ondertekend is, en hoe:\n{}",
        log.report()
    );
}

#[test]
fn geen_productiepad_verwijst_naar_het_observatielog() {
    // De poort die de moduledocs van `observation` belooft. Hij is er niet voor de
    // sierlijkheid: het log ziet elk cross-cel-contact, dus zodra iets in de
    // opstelling hem aanroept, bestaat het totaalbeeld wél ergens.
    let declaration = "pub mod observation;";
    let mut offenders = Vec::new();

    for (file, text) in source_files() {
        if file == "observation.rs" {
            continue;
        }
        for (number, line) in code_lines(&text) {
            if file == "lib.rs" && line.trim() == declaration {
                continue;
            }
            if line.contains("observation") || line.contains("Observation") {
                offenders.push(format!("src/{file}:{number}: {}", line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "het observatielog is een meetinstrument buiten de band; deze regels in \
         `src/` verwijzen ernaar en horen dat niet te doen:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn het_observatielog_kan_geen_cel_aanraken() {
    let (_, text) = source_files()
        .into_iter()
        .find(|(file, _)| file == "observation.rs")
        .unwrap_or_else(|| panic!("src/observation.rs hoort te bestaan"));

    let offenders: Vec<String> = code_lines(&text)
        .filter(|(_, line)| mentions_word(line, "Cell") || mentions_word(line, "ChronicleStore"))
        .map(|(number, line)| format!("src/observation.rs:{number}: {}", line.trim()))
        .collect();

    assert!(
        offenders.is_empty(),
        "een recorder leest wat een vraag opleverde en mag geen cel of kroniek \
         kunnen bereiken:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn een_cel_heeft_geen_transport_en_geen_veiligheidscontext() {
    // Dat een cel niet over haar grens kan reiken, hoort een compileerfout te zijn
    // en geen afspraak (RFC-022 §2). Deze poort zegt het nog een keer op de plek
    // waar iemand het per ongeluk zou weghalen: zodra `src/cell/` een transport of
    // een veiligheidscontext noemt, is de weg terug naar proza ingezet.
    let mut offenders = Vec::new();

    for (file, text) in source_files() {
        if !file.starts_with("cell/") {
            continue;
        }
        for (number, line) in code_lines(&text) {
            for forbidden in ["CellTransport", "SecurityContext", "SignedAnswer"] {
                if line.contains(forbidden) {
                    offenders.push(format!("src/{file}:{number}: {}", line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "een cel houdt geen transport en geen veiligheidscontext; deze regels \
         zeggen iets anders:\n{}",
        offenders.join("\n")
    );
}
