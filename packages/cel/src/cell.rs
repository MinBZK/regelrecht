//! A cell: it records an application, the decisions on it and their
//! execution, and reads its chronicle back.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate};
use regelrecht_engine::{LawExecutionService, ParameterType, StageInputs, Value};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::chronicle::{Chronicle, Gram, Period};
use crate::config::{CellConfig, Derivation};
use crate::error::{refused, setup, Result};
use crate::extension::Every;
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
                let shape = shape::derive_for(service, &event.name, &event.establishes, today)
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
            shape::derive_for(service, &e.name, &e.establishes, day)?,
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
            period: None,
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

    /// Read a lexostatus: the cell's chronicle reduced to parameters, as it
    /// holds at `as_of` (the moment the cell reads; a gram that holds only
    /// later does not count).
    pub fn read(
        &self,
        lexostatus: &str,
        inputs: &Map<String, serde_json::Value>,
        as_of: DateTime<FixedOffset>,
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
        lexostatus::read(definition, inputs, chronicle, as_of)
    }

    /// What the lexostatuses `event` reads give for the case `root` at
    /// `as_of`, per parameter with the lexostatus it came from. A parameter
    /// two of them give is ambiguous.
    fn read_case(
        &self,
        event: &str,
        root: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<BTreeMap<String, (serde_json::Value, String)>> {
        let (_, e) = self.config.event(event).ok_or_else(|| {
            refused(format!(
                "cell '{}' records no event '{event}'",
                self.config.id
            ))
        })?;
        if e.reads.is_empty() {
            return Err(refused(format!(
                "event '{event}' reads no lexostatus (`reads` in its stream)"
            )));
        }
        let mut inputs = Map::new();
        inputs.insert("root".into(), serde_json::Value::String(root.into()));
        let mut read: BTreeMap<String, (serde_json::Value, String)> = BTreeMap::new();
        for lexostatus in &e.reads {
            for (name, value) in self.read(lexostatus, &inputs, as_of)? {
                if let Some((_, first)) = read.get(&name) {
                    return Err(setup(format!(
                        "event '{event}' reads '{name}' from both lexostatus '{first}' and '{lexostatus}'"
                    )));
                }
                read.insert(name, (value, lexostatus.clone()));
            }
        }
        Ok(read)
    }

    /// The parameters of the decision `event` on the gram `root`: the
    /// lexostatuses the event `reads`, each read with `{root}` as the case
    /// holds at `as_of`, kept to what
    /// executing the establishing article at the event's stage asks (the
    /// article and the hooks that fire at that stage), each with the
    /// lexostatus it came from. The article is the version in force on the
    /// day of `as_of`, or, if the law says the decision concerns a period, on
    /// the first day of the period read; the parameter that gives the period
    /// is kept too. What [`Cell::decide`] reads; read-only, to look before
    /// deciding.
    pub fn decision_inputs(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<BTreeMap<String, Input>> {
        let read = self.read_case(event, root, as_of)?;
        let day = as_of.date_naive();
        let (shape, _) = self.shape(service, event, day)?;
        let period_parameter = shape.period.as_ref().map(|p| p.parameter.as_str());
        // A period the case does not give leaves the day as it is: `decide`
        // refuses to take the decision without one.
        let day = match period_parameter.and_then(|p| read.get(p)) {
            Some((value, _)) => period(&shape, Some(value))?.map_or(day, |(_, d)| d),
            None => day,
        };
        let at = stage_of(service, &shape, day)?;
        let asked = asked(&at);
        Ok(read
            .into_iter()
            .filter(|(name, _)| {
                asked.contains(&name.as_str()) || Some(name.as_str()) == period_parameter
            })
            .map(|(name, (value, lexostatus))| (name, from_lexostatus(value, &lexostatus)))
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

    /// Take a decision and record it: execute the establishing article and
    /// record its outputs as the fields, referring to the grams in
    /// `refers_to`. The cell reads the parameters itself from the case
    /// `refers_to` names (the lexostatus the event `reads`); `extra_inputs`
    /// may only add parameters that lexostatus does not supply.
    ///
    /// The decision is taken at `now`: that is when it holds and is
    /// recorded. The law it applies is the law of what it concerns: if the
    /// law says the decision concerns a period, the version in force on the
    /// first day of that period, otherwise the version in force on the day
    /// it is taken.
    pub fn decide(
        &mut self,
        service: &LawExecutionService,
        event: &str,
        refers_to: BTreeMap<String, String>,
        extra_inputs: BTreeMap<String, Input>,
        now: DateTime<FixedOffset>,
    ) -> Result<Gram> {
        // Not yet: refusing an application out of time.
        let today = now.date_naive();
        let (shape, chronicle) = self.shape(service, event, today)?;

        let reads = self
            .config
            .event(event)
            .is_some_and(|(_, e)| !e.reads.is_empty());
        let mut inputs = if reads {
            let root = self.case_root(&shape, &chronicle, &refers_to)?;
            self.decision_inputs(service, event, &root, now)?
        } else {
            BTreeMap::new()
        };
        for (name, input) in extra_inputs {
            if inputs.contains_key(&name) {
                return Err(refused(format!(
                    "'{name}' is read from the case; it cannot be given as well"
                )));
            }
            inputs.insert(name, input);
        }

        let period_parameter = shape.period.as_ref().map(|p| p.parameter.clone());
        let period = period(
            &shape,
            period_parameter
                .as_ref()
                .and_then(|p| inputs.get(p))
                .map(|i| &i.value),
        )?;
        let day = period.map_or(today, |(_, d)| d);
        let shape = if day == today {
            shape
        } else {
            self.shape(service, event, day)?.0
        };
        self.check_references(&shape, &chronicle, &refers_to)?;

        // The decision is taken at its stage of the procedure: what that
        // stage requires to be entered and is a date (the dagtekening of the
        // voorschot, the besluitdatum) is the day it is taken.
        let at = stage_of(service, &shape, day)?;
        for required in &at.requires {
            if required.req_type == ParameterType::Date && !inputs.contains_key(&required.name) {
                let input = Input {
                    value: serde_json::Value::String(today.to_string()),
                    provenance: serde_json::json!({
                        "source": "decision",
                        "stage": at.stage,
                    }),
                };
                inputs.insert(required.name.clone(), input);
            }
        }

        // The parameter that gives the period takes part in the decision,
        // but goes to the article only if the stage asks it.
        let asked = asked(&at);
        let parameters: BTreeMap<String, Value> = inputs
            .iter()
            .filter(|(k, _)| asked.contains(&k.as_str()) || Some(*k) != period_parameter.as_ref())
            .map(|(k, i)| (k.clone(), Value::from(&i.value)))
            .collect();
        let output = shape.outputs.first().ok_or_else(|| {
            setup(format!(
                "{}: the article has no output to take the decision by",
                shape.establishes
            ))
        })?;
        let result = service.execute_stage_at(
            &shape.law_id,
            output,
            &at.stage,
            parameters,
            &day.to_string(),
        )?;

        let mut gram = self.gram(&shape, &chronicle, now);
        gram.regulation = Some(shape.law_id.clone());
        gram.regulation_valid_from = result.regulation_valid_from.clone();
        gram.period = period.map(|(p, _)| p);
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

    /// Execute the article that establishes the execution `event` (an
    /// executogram, such as a voorschottermijn that is paid) for the case
    /// `root` on the day `on`, and record a gram only if the law says one
    /// arises. Returns `None` if it does not.
    ///
    /// What the law declares (`extensions.chronolex` of the article) decides
    /// every step; the cell knows no case:
    /// - `refers_to`: each reference is resolved within the case, to the
    ///   latest gram it admits that holds at `now` (by stage, or by the
    ///   article that establishes it). A required one that is not there is
    ///   refused.
    /// - `until`: once the case has a gram of that stage, no gram arises any
    ///   more; the call is refused.
    /// - the parameters are what the lexostatuses the event `reads` give at
    ///   `now`, kept to the parameters of the article, and `on` in the
    ///   parameter of `executed_on`. With a `period`, the law of the period.
    /// - `record_when`: the boolean output that says whether a gram arises.
    /// - `executed_on.once_per`: a second gram for the same references in the
    ///   same month is refused.
    ///
    /// A fact that has yet to happen is not a fact (RFC-044): `on` may not
    /// lie after `now`, nor before the moment of a gram it refers to. The
    /// gram holds from the start of `on` (or from `now`, on the day itself)
    /// and is recorded at `now`.
    pub fn execute(
        &mut self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        on: NaiveDate,
        now: DateTime<FixedOffset>,
    ) -> Result<Option<Gram>> {
        let today = now.date_naive();
        let (shape, chronicle) = self.shape(service, event, today)?;
        let Some(executed_on) = shape.executed_on.clone() else {
            return Err(refused(format!(
                "'{event}' is not an execution: {} declares no executed_on",
                shape.establishes
            )));
        };
        if on > today {
            return Err(refused(format!(
                "'{event}' on {on} has yet to happen on {today}: not a fact"
            )));
        }
        let effective_at = if on == today {
            now
        } else {
            on.and_hms_opt(0, 0, 0)
                .and_then(|t| t.and_local_timezone(*now.offset()).single())
                .ok_or_else(|| setup(format!("no start of the day {on}")))?
        };

        let grams = self
            .chronicles
            .get(&chronicle)
            .ok_or_else(|| setup(format!("no chronicle '{chronicle}'")))?;
        if grams.find(root).is_none() {
            return Err(refused(format!(
                "no gram '{root}' in chronicle '{chronicle}'"
            )));
        }
        let case: Vec<&Gram> = lexostatus::in_force(grams, now)?
            .into_iter()
            .filter(|g| grams.root_of(g) == root)
            .collect();
        if let Some(until) = &shape.until {
            if let Some(ended) = case
                .iter()
                .find(|g| g.stage.as_deref() == Some(until.stage.as_str()))
            {
                return Err(refused(format!(
                    "'{event}' no longer arises in case '{root}': it has a gram of stage {} ('{}')",
                    until.stage, ended.id
                )));
            }
        }
        let mut refers_to = BTreeMap::new();
        for (name, reference) in &shape.refers_to {
            let Some(gram) = case.iter().rev().find(|g| reference.admits(g)) else {
                if reference.required {
                    return Err(refused(format!(
                        "'{event}' refers to a gram of {} as '{name}', and case '{root}' has none",
                        reference.target()
                    )));
                }
                continue;
            };
            if lexostatus::moment(gram, &gram.effective_at)? > effective_at {
                return Err(refused(format!(
                    "'{event}' on {on} lies before '{name}' ('{}', {})",
                    gram.id, gram.effective_at
                )));
            }
            refers_to.insert(name.clone(), gram.id.clone());
        }
        if executed_on.once_per == Some(Every::Month) {
            let month = |d: NaiveDate| (d.year(), d.month());
            for gram in grams.grams().iter().filter(|g| g.name == event) {
                let day = lexostatus::moment(gram, &gram.effective_at)?.date_naive();
                if gram.refers_to == refers_to && month(day) == month(on) {
                    return Err(refused(format!(
                        "'{event}' already arose in {}-{:02} ('{}')",
                        on.year(),
                        on.month(),
                        gram.id
                    )));
                }
            }
        }

        let mut inputs: BTreeMap<String, Input> = self
            .read_case(event, root, now)?
            .into_iter()
            .filter(|(name, _)| shape.parameters.contains(name))
            .map(|(name, (value, lexostatus))| (name, from_lexostatus(value, &lexostatus)))
            .collect();
        if inputs.contains_key(&executed_on.parameter) {
            return Err(setup(format!(
                "'{event}': '{}' is the day it is executed on, and a lexostatus gives it too",
                executed_on.parameter
            )));
        }
        inputs.insert(
            executed_on.parameter.clone(),
            Input {
                value: serde_json::Value::String(on.to_string()),
                provenance: serde_json::json!({"source": "execution"}),
            },
        );
        let period = period(
            &shape,
            shape
                .period
                .as_ref()
                .and_then(|p| inputs.get(&p.parameter))
                .map(|i| &i.value),
        )?;
        let day = period.map_or(on, |(_, d)| d);
        let shape = if day == today {
            shape
        } else {
            self.shape(service, event, day)?.0
        };
        let when = shape.record_when.clone().ok_or_else(|| {
            setup(format!(
                "{}: '{event}' says not when it arises (record_when)",
                shape.establishes
            ))
        })?;
        let mut asked: Vec<&str> = vec![when.as_str()];
        asked.extend(shape.fields.iter().map(|f| f.name.as_str()));
        let result = service.evaluate_law(
            &shape.law_id,
            &asked,
            inputs
                .iter()
                .map(|(k, i)| (k.clone(), Value::from(&i.value)))
                .collect(),
            &day.to_string(),
        )?;
        match result.outputs.get(&when) {
            Some(Value::Bool(true)) => {}
            Some(Value::Bool(false)) => return Ok(None),
            other => {
                return Err(setup(format!(
                    "{}: '{when}' says whether '{event}' arises, and is {other:?}",
                    shape.establishes
                )))
            }
        }

        let mut gram = self.gram(&shape, &chronicle, now);
        gram.effective_at = effective_at.to_rfc3339();
        gram.regulation = Some(shape.law_id.clone());
        gram.regulation_valid_from = result.regulation_valid_from.clone();
        gram.period = period.map(|(p, _)| p);
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
        self.append(&chronicle, gram).map(Some)
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
                        shape.event,
                        reference.target()
                    )));
                }
                continue;
            };
            let gram = grams
                .find(id)
                .ok_or_else(|| refused(format!("no gram '{id}' in chronicle '{chronicle}'")))?;
            if !reference.admits(gram) {
                return Err(refused(format!(
                    "'{name}' must be a gram of {}, '{id}' is one of {}{}",
                    reference.target(),
                    gram.establishes,
                    gram.stage
                        .as_deref()
                        .map(|s| format!(" at stage {s}"))
                        .unwrap_or_default()
                )));
            }
        }
        Ok(())
    }
}

/// A parameter read from a lexostatus.
fn from_lexostatus(value: serde_json::Value, lexostatus: &str) -> Input {
    Input {
        value,
        provenance: serde_json::json!({
            "source": "lexostatus",
            "lexostatus": lexostatus,
        }),
    }
}

/// What executing the decision `shape` at its stage asks: the article and
/// every hook that fires at that stage, and what the stage requires.
fn stage_of(service: &LawExecutionService, shape: &Shape, day: NaiveDate) -> Result<StageInputs> {
    let stage = shape.stage.as_deref().ok_or_else(|| {
        setup(format!(
            "{}: '{}' names no stage of its procedure to be taken at",
            shape.establishes, shape.event
        ))
    })?;
    shape::stage_inputs(service, shape, stage, day)
}

/// The names of the parameters a stage asks.
fn asked(at: &StageInputs) -> Vec<&str> {
    at.inputs
        .iter()
        .map(|i| i.parameter.name.as_str())
        .collect()
}

/// The period a decision of `shape` concerns, from `value` (the value of the
/// parameter that gives it), with the day the law of that period is the law
/// on; `None` if the law names no period.
fn period(shape: &Shape, value: Option<&serde_json::Value>) -> Result<Option<(Period, NaiveDate)>> {
    let Some(declared) = &shape.period else {
        return Ok(None);
    };
    let value = value.ok_or_else(|| {
        refused(format!(
            "'{}' concerns the period '{}' gives ({}), and nothing gives it",
            shape.event, declared.parameter, shape.establishes
        ))
    })?;
    value
        .as_i64()
        .and_then(|v| i32::try_from(v).ok())
        .map(|value| Period {
            unit: declared.unit,
            value,
        })
        .and_then(|p| Some((p, p.first_day()?)))
        .map(Some)
        .ok_or_else(|| {
            refused(format!(
                "'{}' gives the period '{}' concerns as {value}, not as a {:?}",
                declared.parameter, shape.event, declared.unit
            ))
        })
}
