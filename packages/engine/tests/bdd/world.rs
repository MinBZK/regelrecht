//! World struct for Cucumber BDD tests
//!
//! Contains the test state that persists across steps in a scenario.

#![allow(unused_imports)]

use cucumber::World;
use regelrecht_engine::{ArticleResult, EngineError, LawExecutionService, Value};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global scenario counter for unique trace file names.
static SCENARIO_COUNTER: AtomicUsize = AtomicUsize::new(0);

use crate::helpers::regulation_loader::load_all_regulations;

/// Test world that holds state across steps in a Cucumber scenario.
#[derive(World)]
#[world(init = Self::new)]
pub struct RegelrechtWorld {
    /// Law execution service with all regulations loaded
    pub service: LawExecutionService,
    /// Calculation date for the current scenario
    pub calculation_date: String,
    /// Parameters for law execution
    pub parameters: BTreeMap<String, Value>,
    /// Last execution result (if successful)
    pub result: Option<ArticleResult>,
    /// Last error (if execution failed)
    pub error: Option<EngineError>,
    /// External data sources for zorgtoeslag scenarios
    pub external_data: ExternalData,
}

impl fmt::Debug for RegelrechtWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegelrechtWorld")
            .field("calculation_date", &self.calculation_date)
            .field("parameters", &self.parameters)
            .field("result", &self.result)
            .field("error", &self.error.as_ref().map(|e| e.to_string()))
            .field("external_data", &self.external_data)
            .field(
                "service",
                &format!("<{} laws loaded>", self.service.law_count()),
            )
            .finish()
    }
}

/// External data sources (mocked for testing)
#[derive(Debug, Default, Clone)]
pub struct ExternalData {
    /// RVIG personal_data
    pub rvig_personal: HashMap<String, BTreeMap<String, Value>>,
    /// RVIG relationship_data
    pub rvig_relationship: HashMap<String, BTreeMap<String, Value>>,
    /// RVZ insurance data
    pub rvz_insurance: HashMap<String, BTreeMap<String, Value>>,
    /// Belastingdienst box1 data
    pub bd_box1: HashMap<String, BTreeMap<String, Value>>,
    /// Belastingdienst box2 data
    pub bd_box2: HashMap<String, BTreeMap<String, Value>>,
    /// Belastingdienst box3 data
    pub bd_box3: HashMap<String, BTreeMap<String, Value>>,
    /// DJI detenties data
    pub dji_detenties: HashMap<String, BTreeMap<String, Value>>,
    /// DUO inschrijvingen data
    pub duo_inschrijvingen: HashMap<String, BTreeMap<String, Value>>,
    /// DUO studiefinanciering data
    pub duo_studiefinanciering: HashMap<String, BTreeMap<String, Value>>,
}

impl Default for RegelrechtWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl RegelrechtWorld {
    /// Create a new world with all regulations loaded.
    pub fn new() -> Self {
        let mut service = LawExecutionService::new();

        // Load all regulations from the regulation directory
        if let Err(e) = load_all_regulations(&mut service) {
            panic!("Failed to load regulations: {}", e);
        }

        Self {
            service,
            calculation_date: "2024-01-01".to_string(),
            parameters: BTreeMap::new(),
            result: None,
            error: None,
            external_data: ExternalData::default(),
        }
    }

    /// Clear state between scenarios (but keep service loaded)
    #[allow(dead_code)]
    pub fn reset_scenario_state(&mut self) {
        self.calculation_date = "2024-01-01".to_string();
        self.parameters.clear();
        self.result = None;
        self.error = None;
        self.external_data = ExternalData::default();
    }

    /// Returns true if trace output is enabled via the `TRACE` env var.
    fn trace_enabled() -> bool {
        std::env::var("TRACE").is_ok_and(|v| !v.is_empty() && v != "0")
    }

    /// Execute a law and store the result or error.
    ///
    /// When the `TRACE` environment variable is set (e.g. `TRACE=1`), execution
    /// uses `evaluate_law_output_with_trace` and writes a JSON receipt file per
    /// scenario to the `trace_output/` directory under the engine package.
    pub fn execute_law(&mut self, law_id: &str, output_name: &str) {
        let trace = Self::trace_enabled();

        let outcome = if trace {
            self.service.evaluate_law_output_with_trace(
                law_id,
                output_name,
                self.parameters.clone(),
                &self.calculation_date,
            )
        } else {
            self.service.evaluate_law_output(
                law_id,
                output_name,
                self.parameters.clone(),
                &self.calculation_date,
            )
        };

        match outcome {
            Ok(result) => {
                if trace {
                    self.write_trace(&result, law_id, output_name);
                }
                self.result = Some(result);
                self.error = None;
            }
            Err(e) => {
                self.result = None;
                self.error = Some(e);
            }
        }
    }

    /// Write the box-drawing trace for a successful traced execution.
    fn write_trace(&self, result: &ArticleResult, law_id: &str, output_name: &str) {
        let trace = match &result.trace {
            Some(t) => t,
            None => return,
        };

        let dir = trace_output_dir();
        std::fs::create_dir_all(&dir).expect("Failed to create trace output directory");

        let seq = SCENARIO_COUNTER.fetch_add(1, Ordering::SeqCst);
        let filename = format!(
            "{:03}_{}_{}_{}.txt",
            seq, law_id, output_name, self.calculation_date
        );
        let path = dir.join(&filename);

        std::fs::write(&path, trace.render_box_drawing()).expect("Failed to write trace file");

        eprintln!("  trace → {}", path.display());
    }

    /// Get an output value from the last result
    pub fn get_output(&self, name: &str) -> Option<&Value> {
        self.result.as_ref()?.outputs.get(name)
    }

    /// Check if the last execution was successful
    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }

    /// Get error message if execution failed
    pub fn error_message(&self) -> Option<String> {
        self.error.as_ref().map(|e| e.to_string())
    }
}

/// Directory where trace JSON files are written.
fn trace_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("trace_output")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::RegelrechtWorld;

    #[test]
    fn test_world_initialization() {
        let world = RegelrechtWorld::new();
        // Should have laws loaded
        assert!(
            world.service.law_count() > 0,
            "Expected at least one law to be loaded"
        );
    }
}
