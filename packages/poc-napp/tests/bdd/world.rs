//! Cucumber world for NAPP BDD scenarios.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use cucumber::World;
use regelrecht_engine::{ArticleResult, EngineError, LawExecutionService, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// Law files loaded into every scenario's engine instance.
const LAW_FILES: &[&str] = &[
    "law/wet_op_de_politieke_partijen/2026-01-01.yaml",
    "law/regeling_subsidiebedragen/2026-01-01.yaml",
    "law/besluit_subsidiering_decentrale_politieke_partijen/2026-01-01.yaml",
    "law/algemene_wet_bestuursrecht/1994-01-01.yaml",
    "law/algemene_termijnenwet/1964-04-01.yaml",
    "law/kieswet/1989-09-28.yaml",
];

#[derive(World)]
#[world(init = Self::new)]
pub struct NappWorld {
    pub service: LawExecutionService,
    pub calculation_date: String,
    pub parameters: BTreeMap<String, Value>,
    pub result: Option<ArticleResult>,
    pub error: Option<EngineError>,
}

impl fmt::Debug for NappWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NappWorld")
            .field("calculation_date", &self.calculation_date)
            .field("parameters", &self.parameters)
            .field("result", &self.result)
            .field("error", &self.error.as_ref().map(|e| e.to_string()))
            .finish()
    }
}

impl NappWorld {
    pub fn new() -> Self {
        let mut service = LawExecutionService::new();
        // De wetten staan bij de casus-inhoud, niet naast de crate: vanuit
        // packages/poc-napp is packages/ de ouder en de repo-wortel de opa.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(|p| p.join("corpus-poc").join("napp"))
            .expect("repo-wortel");
        for law_file in LAW_FILES {
            let path = root.join(law_file);
            let yaml = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
            service
                .load_law(&yaml)
                .unwrap_or_else(|e| panic!("Failed to load {}: {e}", path.display()));
        }
        Self {
            service,
            calculation_date: "2026-06-01".to_string(),
            parameters: BTreeMap::new(),
            result: None,
            error: None,
        }
    }

    /// Execute the Wpp for the besluit outputs. Hook outputs (betaalopdracht,
    /// bezwaartermijn, motivering) arrive reactively in the result — never
    /// request them as primary outputs.
    pub fn execute_besluit(&mut self) {
        self.apply_besluit_defaults();
        self.execute(
            "wet_op_de_politieke_partijen",
            &["subsidie_toegekend", "subsidiebedrag"],
        );
    }

    /// Article 15 requires every parameter; scenarios that don't exercise
    /// the ledencomponent or the neveninstellingen may omit those rows.
    pub fn apply_besluit_defaults(&mut self) {
        let defaults: [(&str, Value); 5] = [
            ("totaal_aantal_betalende_leden", Value::Int(0)),
            ("heeft_wetenschappelijk_instituut", Value::Bool(false)),
            ("heeft_jongerenorganisatie", Value::Bool(false)),
            ("aantal_leden_jongerenorganisatie", Value::Int(0)),
            ("heeft_instelling_buitenland", Value::Bool(false)),
        ];
        for (key, value) in defaults {
            self.parameters.entry(key.to_string()).or_insert(value);
        }
    }

    /// Execute an arbitrary law for the given outputs.
    pub fn execute(&mut self, law_id: &str, outputs: &[&str]) {
        match self.service.evaluate_law(
            law_id,
            outputs,
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

    pub fn get_output(&self, name: &str) -> Option<&Value> {
        self.result.as_ref().and_then(|r| r.outputs.get(name))
    }

    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }

    pub fn error_message(&self) -> Option<String> {
        self.error.as_ref().map(|e| e.to_string())
    }
}
