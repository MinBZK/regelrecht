//! Draai één scenariobestand en druk het verslag af.
//!
//! ```text
//! cargo run -p regelrecht-simulator --bin run-scenario -- \
//!     packages/simulator/scenarios/toeslagen_zorgtoeslag.yaml
//! ```
//!
//! Exitcode 0 als alle verwachtingen uitkwamen, 1 als er één niet uitkwam of
//! als het scenario niet gedraaid kon worden.

use regelrecht_simulator::{regulation_root, Scenario};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("gebruik: run-scenario <scenario.yaml>");
        return ExitCode::FAILURE;
    };

    let scenario = match Scenario::load(&path) {
        Ok(scenario) => scenario,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    match scenario.run(&regulation_root()) {
        Ok(run) => {
            print!("{}", run.report());
            if run.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
