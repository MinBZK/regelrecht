//! World struct for Cucumber BDD tests
//!
//! Contains the test state that persists across steps in a scenario.

#![allow(unused_imports)]

use cucumber::World;
use regelrecht_engine::{ArticleResult, EngineError, LawExecutionService, Value};
use std::collections::HashMap;
use std::fmt;

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
    pub parameters: HashMap<String, Value>,
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
    pub rvig_personal: HashMap<String, HashMap<String, Value>>,
    /// RVIG relationship_data
    pub rvig_relationship: HashMap<String, HashMap<String, Value>>,
    /// RVZ insurance data
    pub rvz_insurance: HashMap<String, HashMap<String, Value>>,
    /// Belastingdienst box1 data
    pub bd_box1: HashMap<String, HashMap<String, Value>>,
    /// Belastingdienst box2 data
    pub bd_box2: HashMap<String, HashMap<String, Value>>,
    /// Belastingdienst box3 data
    pub bd_box3: HashMap<String, HashMap<String, Value>>,
    /// DJI detenties data
    pub dji_detenties: HashMap<String, HashMap<String, Value>>,
    /// DUO inschrijvingen data
    pub duo_inschrijvingen: HashMap<String, HashMap<String, Value>>,
    /// DUO studiefinanciering data
    pub duo_studiefinanciering: HashMap<String, HashMap<String, Value>>,
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
            parameters: HashMap::new(),
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

    /// Execute a law and store the result or error
    pub fn execute_law(&mut self, law_id: &str, output_name: &str) {
        match self.service.evaluate_law_output(
            law_id,
            output_name,
            self.parameters.clone(),
            &self.calculation_date,
        ) {
            Ok(result) => {
                self.result = Some(result);
                self.error = None;
            }
            Err(e) => {
                self.result = None;
                self.error = Some(e);
            }
        }
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
