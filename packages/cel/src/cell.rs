//! A cell: it records an application and the decision on it, and reads its
//! chronicle back.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset, NaiveDate};
use regelrecht_engine::{LawExecutionService, Value};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::chronicle::{Chronicle, Gram};
use crate::config::{CellConfig, Derivation};
use crate::error::{refused, setup, Result};
use crate::lexostatus;
use crate::shape::{self, Shape};

/// The cell's clock: the moment of recording, in Dutch time. A lexostatus
/// reads the day of receipt from it, so a UTC clock would move a receipt
/// just after midnight to the day before.
pub type Clock = Box<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>;

/// A parameter of a decision: its value, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub value: serde_json::Value,
    pub provenance: serde_json::Value,
}

pub struct Cell {
    config: CellConfig,
    service: LawExecutionService,
    chronicles: BTreeMap<String, Chronicle>,
    clock: Clock,
}

/// Load every regulation under `dir` into a new service. A file the engine
/// refuses is an error: a cell that misses a law would record a different
/// fact.
pub fn load_regulations(dir: &Path) -> Result<LawExecutionService> {
    let mut service = LawExecutionService::new();
    let mut errors = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry.map_err(|e| setup(e.to_string()))?;
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        if let Err(e) = service.load_law(&text) {
            errors.push(format!("{}: {e}", path.display()));
        }
    }
    if errors.is_empty() {
        Ok(service)
    } else {
        Err(setup(errors.join("\n")))
    }
}

impl Cell {
    /// A cell over `cell_yaml`, executing the law in `service`, with its
    /// chronicles under `data_dir/<cell>/`.
    pub fn new(
        cell_yaml: &Path,
        service: LawExecutionService,
        data_dir: &Path,
        clock: Clock,
    ) -> Result<Self> {
        let config = CellConfig::load(cell_yaml)?;
        let mut chronicles = BTreeMap::new();
        for stream in &config.streams {
            if !chronicles.contains_key(&stream.chronicle) {
                let path = data_dir
                    .join(&config.id)
                    .join(format!("{}.jsonl", stream.chronicle));
                chronicles.insert(stream.chronicle.clone(), Chronicle::open(path)?);
            }
        }
        let cell = Self {
            config,
            service,
            chronicles,
            clock,
        };
        // Every event must take its shape from the law now, not at the first
        // application; and every field a lexostatus reads must be a field of
        // a gram in its chronicle, or a typo reads as a fact nobody has.
        let today = cell.now().date_naive();
        let mut fields: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for stream in &cell.config.streams {
            for event in &stream.events {
                let shape = shape::derive(&cell.service, &event.name, &event.establishes, today)
                    .map_err(|e| {
                        setup(format!(
                            "stream '{}', event '{}': {e}",
                            stream.id, event.name
                        ))
                    })?;
                fields
                    .entry(stream.chronicle.as_str())
                    .or_default()
                    .extend(shape.fields.into_iter().map(|f| f.name));
            }
        }
        for l in &cell.config.lexostatuses {
            let known = fields.get(l.reduction.chronicle.as_str());
            for read in l
                .reduction
                .derivations
                .values()
                .filter_map(Derivation::field)
            {
                if !known.is_some_and(|k| k.iter().any(|f| f == read)) {
                    return Err(setup(format!(
                        "lexostatus '{}' reads field '{read}', which no event of chronicle '{}' has",
                        l.name, l.reduction.chronicle
                    )));
                }
            }
        }
        Ok(cell)
    }

    pub fn service(&self) -> &LawExecutionService {
        &self.service
    }

    /// For registering data sources a decision reads.
    pub fn service_mut(&mut self) -> &mut LawExecutionService {
        &mut self.service
    }

    fn now(&self) -> DateTime<FixedOffset> {
        (self.clock)()
    }

    /// The shape of an event on `day`, with the chronicle it goes to.
    pub fn shape(&self, event: &str, day: NaiveDate) -> Result<(Shape, String)> {
        let (stream, e) = self.config.event(event).ok_or_else(|| {
            refused(format!(
                "cell '{}' records no event '{event}'",
                self.config.id
            ))
        })?;
        Ok((
            shape::derive(&self.service, &e.name, &e.establishes, day)?,
            stream.chronicle.clone(),
        ))
    }

    fn gram(&self, shape: &Shape, chronicle: &str, effective_at: String) -> Gram {
        Gram {
            id: uuid::Uuid::now_v7().to_string(),
            type_: shape.type_.clone(),
            subtype: shape.subtype.clone(),
            stage: shape.stage.clone(),
            name: shape.event.clone(),
            chronicle: chronicle.to_string(),
            recording_actor: self.config.recording_actor.clone(),
            legal_basis: shape.legal_basis(),
            legal_character: shape.legal_character.clone(),
            decision_type: shape.decision_type.clone(),
            regulation: None,
            regulation_valid_from: None,
            effective_at,
            effective_at_legal_basis: shape
                .effective_at
                .as_ref()
                .map(|e| e.legal_basis.clone())
                .unwrap_or_default(),
            recorded_at: self.now().to_rfc3339(),
            refers_to: BTreeMap::new(),
            fields: Map::new(),
            inputs: BTreeMap::new(),
        }
    }

    fn append(&mut self, chronicle: &str, gram: Gram) -> Result<Gram> {
        let c = self
            .chronicles
            .get_mut(chronicle)
            .ok_or_else(|| setup(format!("no chronicle '{chronicle}'")))?;
        Ok(c.append(gram)?.clone())
    }

    /// Record a submission (an application) as the applicant made it.
    /// `submitted` holds the fields the applicant filled in; a field the law
    /// does not ask is refused, a field it asks may be left out.
    pub fn record_submission(
        &mut self,
        event: &str,
        submitted: &Map<String, serde_json::Value>,
    ) -> Result<Gram> {
        let now = self.now();
        let (shape, chronicle) = self.shape(event, now.date_naive())?;
        if shape.subtype.is_none() {
            return Err(refused(format!(
                "'{event}' is not a submission: {} establishes no produces.submission",
                shape.establishes
            )));
        }
        for (name, value) in submitted {
            let field = shape
                .field(name)
                .ok_or_else(|| refused(format!("the law does not ask '{name}' in '{event}'")))?;
            if field.fixed.is_some() {
                return Err(refused(format!(
                    "'{name}' is not for the applicant to fill in: {} fixes it",
                    field.declared_by
                )));
            }
            if !shape::fits(field.type_, value) {
                return Err(refused(format!(
                    "'{name}' is not a {:?} ({})",
                    field.type_, field.declared_by
                )));
            }
        }
        // The moment that counts is the receipt (Awb 4:13 lid 1).
        let mut gram = self.gram(&shape, &chronicle, now.to_rfc3339());
        for f in &shape.fields {
            match (&f.fixed, submitted.get(&f.name)) {
                (Some(v), _) | (None, Some(v)) => {
                    gram.fields.insert(f.name.clone(), v.clone());
                }
                (None, None) => {}
            }
        }
        self.append(&chronicle, gram)
    }

    /// Read a lexostatus: the cell's chronicle reduced to parameters.
    pub fn read(
        &self,
        lexostatus: &str,
        inputs: &Map<String, serde_json::Value>,
    ) -> Result<Map<String, serde_json::Value>> {
        let definition = self
            .config
            .lexostatuses
            .iter()
            .find(|l| l.name == lexostatus)
            .ok_or_else(|| refused(format!("no lexostatus '{lexostatus}'")))?;
        let chronicle = self
            .chronicles
            .get(&definition.reduction.chronicle)
            .ok_or_else(|| {
                setup(format!(
                    "lexostatus '{lexostatus}' reads chronicle '{}', which the cell does not keep",
                    definition.reduction.chronicle
                ))
            })?;
        lexostatus::read(definition, inputs, chronicle)
    }

    /// Take a decision and record it: execute the establishing article with
    /// `inputs` on the day it is taken, and record its outputs as the fields,
    /// referring to the grams in `refers_to`.
    pub fn decide(
        &mut self,
        event: &str,
        refers_to: BTreeMap<String, String>,
        inputs: BTreeMap<String, Input>,
    ) -> Result<Gram> {
        // The decision is taken, and the law applied, on the day it is
        // recorded. Not yet: the law of the berekeningsjaar applied for
        // (Awir 15 lid 1 lets a 2025 application be decided in 2026), and
        // refusing an application out of time.
        let now = self.now();
        let day = now.date_naive();
        let (shape, chronicle) = self.shape(event, day)?;
        self.check_references(&shape, &chronicle, &refers_to)?;

        let parameters: BTreeMap<String, Value> = inputs
            .iter()
            .map(|(k, i)| (k.clone(), Value::from(&i.value)))
            .collect();
        let names: Vec<&str> = shape.fields.iter().map(|f| f.name.as_str()).collect();
        let result =
            self.service
                .evaluate_law(&shape.law_id, &names, parameters, &day.to_string())?;

        let mut gram = self.gram(&shape, &chronicle, now.to_rfc3339());
        gram.regulation = Some(shape.law_id.clone());
        gram.regulation_valid_from = result.regulation_valid_from.clone();
        gram.refers_to = refers_to;
        for f in &shape.fields {
            let value = result
                .outputs
                .get(&f.name)
                .map(shape::to_json)
                .unwrap_or(serde_json::Value::Null);
            gram.fields.insert(f.name.clone(), value);
        }
        gram.inputs = inputs
            .into_iter()
            .map(|(k, i)| {
                let v = serde_json::to_value(i).unwrap_or(serde_json::Value::Null);
                (k, v)
            })
            .collect();
        self.append(&chronicle, gram)
    }

    /// Every reference the law names is to a gram of the article it names;
    /// a required one is there; no other name is used.
    fn check_references(
        &self,
        shape: &Shape,
        chronicle: &str,
        refers_to: &BTreeMap<String, String>,
    ) -> Result<()> {
        for name in refers_to.keys() {
            if !shape.refers_to.contains_key(name) {
                return Err(refused(format!(
                    "'{}' refers to nothing as '{name}' ({})",
                    shape.event, shape.establishes
                )));
            }
        }
        let grams = self
            .chronicles
            .get(chronicle)
            .ok_or_else(|| setup(format!("no chronicle '{chronicle}'")))?;
        for (name, reference) in &shape.refers_to {
            let Some(id) = refers_to.get(name) else {
                if reference.required {
                    return Err(refused(format!(
                        "'{}' must refer to a gram of {} as '{name}'",
                        shape.event, reference.to
                    )));
                }
                continue;
            };
            let gram = grams
                .find(id)
                .ok_or_else(|| refused(format!("no gram '{id}' in chronicle '{chronicle}'")))?;
            if gram.legal_basis.first() != Some(&reference.to) {
                return Err(refused(format!(
                    "'{name}' must be a gram of {}, '{id}' is one of {}",
                    reference.to,
                    gram.legal_basis.first().map_or("nothing", String::as_str)
                )));
            }
        }
        Ok(())
    }
}
