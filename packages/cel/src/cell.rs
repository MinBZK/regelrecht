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

/// A parameter of a decision: its value, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub value: serde_json::Value,
    pub provenance: serde_json::Value,
}

/// A cell. It holds its configuration and its chronicles, not the law: every
/// call gets the service that executes the law, and `now`, the moment of
/// recording in Dutch time (a lexostatus reads the day of receipt from it, so
/// a UTC moment would move a receipt just after midnight to the day before).
pub struct Cell {
    config: CellConfig,
    chronicles: BTreeMap<String, Chronicle>,
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
    /// A cell over `cell_yaml`, with its chronicles under
    /// `data_dir/<cell>/<chronicle>.jsonl`.
    pub fn open(
        cell_yaml: &Path,
        service: &LawExecutionService,
        data_dir: &Path,
        today: NaiveDate,
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
        Self::new(config, chronicles, service, today)
    }

    /// A cell whose chronicles live in memory, starting from `grams` (what
    /// the caller kept of an earlier session), as in the browser.
    pub fn in_memory(
        config: CellConfig,
        grams: Vec<Gram>,
        service: &LawExecutionService,
        today: NaiveDate,
    ) -> Result<Self> {
        let mut chronicles: BTreeMap<String, Chronicle> = config
            .streams
            .iter()
            .map(|s| (s.chronicle.clone(), Chronicle::in_memory(Vec::new())))
            .collect();
        for gram in grams {
            chronicles
                .get_mut(&gram.chronicle)
                .ok_or_else(|| {
                    setup(format!(
                        "no chronicle '{}' for gram '{}'",
                        gram.chronicle, gram.id
                    ))
                })?
                .append(gram)?;
        }
        Self::new(config, chronicles, service, today)
    }

    fn new(
        config: CellConfig,
        chronicles: BTreeMap<String, Chronicle>,
        service: &LawExecutionService,
        today: NaiveDate,
    ) -> Result<Self> {
        let cell = Self { config, chronicles };
        // Every event must take its shape from the law now, not at the first
        // application; and every field a lexostatus reads must be a field of
        // a gram in its chronicle, or a typo reads as a fact nobody has.
        let mut fields: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for stream in &cell.config.streams {
            for event in &stream.events {
                let shape = shape::derive(service, &event.name, &event.establishes, today)
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

    pub fn config(&self) -> &CellConfig {
        &self.config
    }

    /// Every gram, per chronicle in recording order.
    pub fn grams(&self) -> impl Iterator<Item = &Gram> {
        self.chronicles.values().flat_map(|c| c.grams())
    }

    /// The shape of an event on `day`, with the chronicle it goes to.
    pub fn shape(
        &self,
        service: &LawExecutionService,
        event: &str,
        day: NaiveDate,
    ) -> Result<(Shape, String)> {
        let (stream, e) = self.config.event(event).ok_or_else(|| {
            refused(format!(
                "cell '{}' records no event '{event}'",
                self.config.id
            ))
        })?;
        Ok((
            shape::derive(service, &e.name, &e.establishes, day)?,
            stream.chronicle.clone(),
        ))
    }

    fn gram(&self, shape: &Shape, chronicle: &str, now: DateTime<FixedOffset>) -> Gram {
        Gram {
            id: uuid::Uuid::now_v7().to_string(),
            type_: shape.type_.clone(),
            subtype: shape.subtype.clone(),
            stage: shape.stage.clone(),
            name: shape.event.clone(),
            chronicle: chronicle.to_string(),
            recording_actor: self.config.recording_actor.clone(),
            establishes: shape.establishes.clone(),
            legal_basis: shape.legal_basis(),
            legal_character: shape.legal_character.clone(),
            decision_type: shape.decision_type.clone(),
            regulation: None,
            regulation_valid_from: None,
            effective_at: now.to_rfc3339(),
            effective_at_legal_basis: shape
                .effective_at
                .as_ref()
                .map(|e| e.legal_basis.clone())
                .unwrap_or_default(),
            recorded_at: now.to_rfc3339(),
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
        service: &LawExecutionService,
        event: &str,
        submitted: &Map<String, serde_json::Value>,
        now: DateTime<FixedOffset>,
    ) -> Result<Gram> {
        let (shape, chronicle) = self.shape(service, event, now.date_naive())?;
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
        // The moment of recording is the moment that counts; the provisions
        // it rests on are the law's `effective_at` entries (none if the law
        // names none).
        let mut gram = self.gram(&shape, &chronicle, now);
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

    /// The parameters of the decision `event` on the gram `root`: the
    /// lexostatus the event `reads`, read with `{root}`, kept to the
    /// parameters the establishing article declares on `day`, each with
    /// where it came from. What [`Cell::decide`] reads; read-only, to look
    /// before deciding.
    pub fn decision_inputs(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        day: NaiveDate,
    ) -> Result<BTreeMap<String, Input>> {
        let (_, e) = self.config.event(event).ok_or_else(|| {
            refused(format!(
                "cell '{}' records no event '{event}'",
                self.config.id
            ))
        })?;
        let lexostatus = e.reads.as_deref().ok_or_else(|| {
            refused(format!(
                "event '{event}' reads no lexostatus (`reads` in its stream)"
            ))
        })?;
        let mut inputs = Map::new();
        inputs.insert("root".into(), serde_json::Value::String(root.into()));
        let read = self.read(lexostatus, &inputs)?;
        let asked = shape::parameter_names(service, &e.establishes, day)?;
        Ok(read
            .into_iter()
            .filter(|(name, _)| asked.contains(name))
            .map(|(name, value)| {
                let input = Input {
                    value,
                    provenance: serde_json::json!({
                        "source": "lexostatus",
                        "lexostatus": lexostatus,
                    }),
                };
                (name, input)
            })
            .collect())
    }

    /// The root of the case a decision is taken on: the root of the gram
    /// its one required reference names.
    fn case_root(
        &self,
        shape: &Shape,
        chronicle: &str,
        refers_to: &BTreeMap<String, String>,
    ) -> Result<String> {
        let required: Vec<&String> = shape
            .refers_to
            .iter()
            .filter(|(_, r)| r.required)
            .map(|(name, _)| name)
            .collect();
        let [name] = required.as_slice() else {
            return Err(setup(format!(
                "{}: '{}' reads its case, but has {} required references, not one",
                shape.establishes,
                shape.event,
                required.len()
            )));
        };
        let id = refers_to.get(*name).ok_or_else(|| {
            refused(format!(
                "'{}' must refer to a gram as '{name}'",
                shape.event
            ))
        })?;
        let grams = self
            .chronicles
            .get(chronicle)
            .ok_or_else(|| setup(format!("no chronicle '{chronicle}'")))?;
        let gram = grams
            .find(id)
            .ok_or_else(|| refused(format!("no gram '{id}' in chronicle '{chronicle}'")))?;
        Ok(grams.root_of(gram).to_string())
    }

    /// Take a decision and record it: execute the establishing article on the
    /// day it is taken, and record its outputs as the fields, referring to
    /// the grams in `refers_to`. The cell reads the parameters itself from
    /// the case `refers_to` names (the lexostatus the event `reads`);
    /// `extra_inputs` may only add parameters that lexostatus does not
    /// supply.
    pub fn decide(
        &mut self,
        service: &LawExecutionService,
        event: &str,
        refers_to: BTreeMap<String, String>,
        extra_inputs: BTreeMap<String, Input>,
        now: DateTime<FixedOffset>,
    ) -> Result<Gram> {
        // The decision is taken, and the law applied, on the day it is
        // recorded. Not yet: the law of the berekeningsjaar applied for
        // (Awir 15 lid 1 lets a 2025 application be decided in 2026), and
        // refusing an application out of time.
        let day = now.date_naive();
        let (shape, chronicle) = self.shape(service, event, day)?;
        self.check_references(&shape, &chronicle, &refers_to)?;

        let reads = self.config.event(event).and_then(|(_, e)| e.reads.clone());
        let mut inputs = match reads {
            Some(_) => {
                let root = self.case_root(&shape, &chronicle, &refers_to)?;
                self.decision_inputs(service, event, &root, day)?
            }
            None => BTreeMap::new(),
        };
        for (name, input) in extra_inputs {
            if inputs.contains_key(&name) {
                return Err(refused(format!(
                    "'{name}' is read from the case; it cannot be given as well"
                )));
            }
            inputs.insert(name, input);
        }

        let parameters: BTreeMap<String, Value> = inputs
            .iter()
            .map(|(k, i)| (k.clone(), Value::from(&i.value)))
            .collect();
        let names: Vec<&str> = shape.fields.iter().map(|f| f.name.as_str()).collect();
        let result = service.evaluate_law(&shape.law_id, &names, parameters, &day.to_string())?;

        let mut gram = self.gram(&shape, &chronicle, now);
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
            if gram.establishes != reference.to {
                return Err(refused(format!(
                    "'{name}' must be a gram of {}, '{id}' is one of {}",
                    reference.to, gram.establishes
                )));
            }
        }
        Ok(())
    }
}
