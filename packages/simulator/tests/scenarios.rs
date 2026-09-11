//! Draait elk scenariobestand in `scenarios/`.
//!
//! De assertie staat in het scenario, niet hier: deze test controleert alleen
//! dat elk bestand draait en dat geen enkele verwachting mist. Een nieuw
//! testgeval is dus een nieuw YAML-bestand, geen nieuwe Rust.

use regelrecht_simulator::{regulation_root, Scenario};
use std::path::{Path, PathBuf};

fn scenario_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    files.sort();
    files
}

#[test]
fn er_is_minstens_een_scenario() {
    assert!(
        !scenario_files().is_empty(),
        "zonder scenario bewijst deze suite niets"
    );
}

#[test]
fn alle_scenarios_voldoen_aan_hun_eigen_verwachtingen() {
    for path in scenario_files() {
        let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let run = scenario
            .run(&regulation_root())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        assert!(run.passed(), "{}:\n{}", path.display(), run.report());
        assert!(
            run.proved_something(),
            "{}: een scenario zonder vragen bewijst niets",
            path.display()
        );
    }
}

/// Twee runs van hetzelfde bestand geven hetzelfde verslag.
///
/// De klok van een wereld is logisch en de tijdlijn staat in het bestand, dus
/// een run mag nergens van de wandklok of van een willekeurige volgorde
/// afhangen. Dat is niet per scenario te beweren — het is een eigenschap van de
/// opstelling — dus staat het hier en niet in een YAML-verwachting.
#[test]
fn twee_runs_geven_hetzelfde_verslag() {
    for path in scenario_files() {
        let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let first = scenario
            .run(&regulation_root())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let second = scenario
            .run(&regulation_root())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        assert_eq!(
            first.report(),
            second.report(),
            "{}: twee runs horen identiek te zijn",
            path.display()
        );
        assert_eq!(
            first.clock,
            second.clock,
            "{}: de klok hoort na elke run op hetzelfde moment te staan",
            path.display()
        );
    }
}
