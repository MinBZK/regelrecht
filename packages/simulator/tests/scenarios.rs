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
            !run.outcomes.is_empty(),
            "{}: een scenario zonder vragen bewijst niets",
            path.display()
        );
    }
}
