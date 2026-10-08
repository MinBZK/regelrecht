//! A cell: it records an application, the decisions on it and their
//! execution, and reads its chronicle back.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate};
use regelrecht_engine::{LawExecutionService, OutputProvenance, StageInputs, Value};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::chronicle::{Chronicle, Gram, Period};
use crate::config::{
    CellConfig, Derivation, Event, LexostatusDefinition, Read, Reduction, Register, Stream,
};
use crate::error::{refused, setup, Result};
use crate::extension::{Every, ExecutedOn};
use crate::lexostatus;
use crate::register;
use crate::shape::{self, Shape};

/// A parameter of a decision: its value, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub value: serde_json::Value,
    pub provenance: serde_json::Value,
}

/// An event of the cell that reads a lexostatus for its case (`reads` in its
/// stream): the event, its stream, and the stage of the decision it records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadBy {
    pub event: String,
    pub stream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
}

/// An article of a policy of the holder, with the outputs it gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyArticle {
    pub number: String,
    pub outputs: Vec<String>,
}

/// A lexostatus of the cell as it reduces its chronicle: in the cell
/// configuration (`lexostatuses.yaml`), or in an article in the policy of the
/// holder that reads a chronicle of the cell as a register (`registers:` in
/// `cell.yaml`). Either is read with [`Cell::read_lexostatus`] by its `name`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LexostatusDescription {
    /// A reduction in the cell configuration: filter, pick, derivations.
    Configuration {
        name: String,
        inputs: Vec<String>,
        reduction: Reduction,
        read_by: Vec<ReadBy>,
    },
    /// A policy of the holder: `name` is the policy, `register` the name the
    /// binding gives the register, `register_input` the input without a
    /// source (`source: {}`) the chronicle is given as.
    Policy {
        name: String,
        register: String,
        chronicle: String,
        register_input: String,
        inputs: Vec<String>,
        articles: Vec<PolicyArticle>,
        read_by: Vec<ReadBy>,
    },
}

/// A reading of a lexostatus: per parameter its value and where it came
/// from, and the grams it was read from, by id, in the order they hold.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reading {
    pub values: BTreeMap<String, Input>,
    pub grams: Vec<String>,
}

/// A cell. It holds its configuration and its chronicles, not the law: every
/// call gets the service that executes the law, and `now`, the moment of
/// recording in Dutch time (a lexostatus reads the day of receipt from it, so
/// a UTC moment would move a receipt just after midnight to the day before).
pub struct Cell {
    config: CellConfig,
    chronicles: BTreeMap<String, Chronicle>,
    /// Per event, the fields the law gives its grams (on the day the cell
    /// started): a register row carries each, null where a gram has none.
    event_fields: BTreeMap<String, Vec<String>>,
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
        // A stage that says it `is` a stage no procedure has would make the
        // hooks on that stage silently not fire: refuse to start instead.
        let unknown = service.resolver().unknown_stage_aliases();
        if !unknown.is_empty() {
            return Err(setup(format!(
                "the law names stages no procedure has: {}",
                unknown.join("; ")
            )));
        }
        // A policy that reads a register must be bound with the engine, or
        // its input reads as unknown at the first decision.
        for r in &config.registers {
            register::check_bound(service, &config.id, r)?;
        }
        let mut cell = Self {
            config,
            chronicles,
            event_fields: BTreeMap::new(),
        };
        // Every event must take its shape from the law now, not at the first
        // application; and every field a lexostatus reads must be a field of
        // a gram in its chronicle, or a typo reads as a fact nobody has.
        let mut fields: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut event_fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for stream in &cell.config.streams {
            for event in &stream.events {
                let shape = shape::derive_for(service, event, today).map_err(|e| {
                    setup(format!(
                        "stream '{}', event '{}': {e}",
                        stream.id, event.name
                    ))
                })?;
                let names: Vec<String> = shape.fields.into_iter().map(|f| f.name).collect();
                fields
                    .entry(stream.chronicle.as_str())
                    .or_default()
                    .extend(names.iter().cloned());
                event_fields.insert(event.name.clone(), names);
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
        cell.event_fields = event_fields;
        Ok(cell)
    }

    pub fn config(&self) -> &CellConfig {
        &self.config
    }

    fn chronicle(&self, name: &str) -> Result<&Chronicle> {
        self.chronicles
            .get(name)
            .ok_or_else(|| setup(format!("no chronicle '{name}'")))
    }

    fn event(&self, name: &str) -> Result<(&Stream, &Event)> {
        self.config.event(name).ok_or_else(|| {
            refused(format!(
                "cell '{}' records no event '{name}'",
                self.config.id
            ))
        })
    }

    /// The register policy `policy` reads, bound with the engine, and its
    /// chronicle; `None` if the policy reads no register of the cell.
    fn register(
        &self,
        service: &LawExecutionService,
        policy: &str,
    ) -> Result<Option<(&Register, &Chronicle)>> {
        let Some(register) = self.config.registers.iter().find(|r| r.policy == policy) else {
            return Ok(None);
        };
        register::check_bound(service, &self.config.id, register)?;
        Ok(Some((register, self.chronicle(&register.chronicle)?)))
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
        let (stream, e) = self.event(event)?;
        Ok((
            shape::derive_for(service, e, day)?,
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

    /// The lexostatus `name` in the cell configuration with the chronicle it
    /// reads; `None` if the configuration has none of that name.
    fn configured(&self, name: &str) -> Result<Option<(&LexostatusDefinition, &Chronicle)>> {
        let Some(definition) = self.config.lexostatuses.iter().find(|l| l.name == name) else {
            return Ok(None);
        };
        Ok(Some((
            definition,
            self.chronicle(&definition.reduction.chronicle)?,
        )))
    }

    /// Every lexostatus of the cell and how it reduces the chronicle: first
    /// those in the cell configuration, then each policy of the holder that
    /// reads a register of the cell, with its articles as they hold on `day`.
    /// Per lexostatus the events that read it for their case.
    pub fn lexostatuses(
        &self,
        service: &LawExecutionService,
        day: NaiveDate,
    ) -> Result<Vec<LexostatusDescription>> {
        let read_by = |read: &dyn Fn(&Read) -> bool| -> Vec<ReadBy> {
            self.config
                .streams
                .iter()
                .flat_map(|s| s.events.iter().map(move |e| (s, e)))
                .filter(|(_, e)| e.reads.iter().any(read))
                .map(|(s, e)| ReadBy {
                    event: e.name.clone(),
                    stream: s.id.clone(),
                    stage: e.stage.clone(),
                })
                .collect()
        };
        let mut out = Vec::new();
        for l in &self.config.lexostatuses {
            out.push(LexostatusDescription::Configuration {
                name: l.name.clone(),
                inputs: l.inputs.clone(),
                reduction: l.reduction.clone(),
                read_by: read_by(&|r| matches!(r, Read::Lexostatus(n) if *n == l.name)),
            });
        }
        for register in &self.config.registers {
            let policy = &register.policy;
            let law = service
                .resolver()
                .get_law_for_date(policy, Some(day))
                .ok_or_else(|| setup(format!("policy '{policy}' has no version on {day}")))?;
            let mut inputs: Vec<String> = Vec::new();
            let mut articles = Vec::new();
            for article in &law.articles {
                let Some(execution) = article.get_execution_spec() else {
                    continue;
                };
                for p in execution.parameters.iter().flatten() {
                    if !inputs.contains(&p.name) {
                        inputs.push(p.name.clone());
                    }
                }
                articles.push(PolicyArticle {
                    number: article.number.clone(),
                    outputs: execution
                        .output
                        .iter()
                        .flatten()
                        .map(|o| o.name.clone())
                        .collect(),
                });
            }
            out.push(LexostatusDescription::Policy {
                name: policy.clone(),
                register: register.name.clone(),
                chronicle: register.chronicle.clone(),
                register_input: register::register_input(service, policy)?,
                inputs,
                articles,
                read_by: read_by(
                    &|r| matches!(r, Read::Regulation { regulation } if regulation == policy),
                ),
            });
        }
        Ok(out)
    }

    /// Read the lexostatus `name` as it holds at `as_of`, with where each
    /// parameter came from and the grams it was read from. `name` is a
    /// lexostatus in the cell configuration, or a policy of the holder that
    /// reads a register of the cell (see [`Self::lexostatuses`]); a policy
    /// is executed for the case `root` of `inputs`, as [`Self::read_case`]
    /// executes it, and its grams are those of that case in the register.
    /// An output the policy leaves empty is no parameter.
    pub fn read_lexostatus(
        &self,
        service: &LawExecutionService,
        name: &str,
        inputs: &Map<String, serde_json::Value>,
        as_of: DateTime<FixedOffset>,
    ) -> Result<Reading> {
        if let Some((definition, chronicle)) = self.configured(name)? {
            let (values, grams) = lexostatus::reduce(definition, inputs, chronicle, as_of)?;
            return Ok(Reading {
                values: values
                    .into_iter()
                    .map(|(n, v)| (n, from_lexostatus(v, name)))
                    .collect(),
                grams: grams.iter().map(|g| g.id.clone()).collect(),
            });
        }
        let Some((_, chronicle)) = self.register(service, name)? else {
            return Err(refused(format!("no lexostatus '{name}'")));
        };
        let root = inputs
            .get("root")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| refused(format!("policy '{name}' needs input 'root'")))?;
        let values = self
            .read_policy(service, name, root, as_of)?
            .into_iter()
            .collect();
        let grams = lexostatus::in_force(chronicle, as_of)?
            .into_iter()
            .filter(|g| chronicle.root_of(g) == root)
            .map(|g| g.id.clone())
            .collect();
        Ok(Reading { values, grams })
    }

    /// Per parameter the lexostatus `lexostatus` gives, the field of a gram
    /// it reads (a `field`, a `sum`, whether one is `filled`), as the law
    /// declares that field on `day`: how to read what [`Self::read`]
    /// returns. The field is looked up in the events of the chronicle the
    /// lexostatus reads, the event its filter names first. A parameter that
    /// reads no field (a moment, a period) is left out.
    pub fn lexostatus_fields(
        &self,
        service: &LawExecutionService,
        lexostatus: &str,
        day: NaiveDate,
    ) -> Result<BTreeMap<String, shape::FieldDef>> {
        let definition = self
            .config
            .lexostatuses
            .iter()
            .find(|l| l.name == lexostatus)
            .ok_or_else(|| refused(format!("no lexostatus '{lexostatus}'")))?;
        let reduction = &definition.reduction;
        let mut events: Vec<&str> = self
            .config
            .streams
            .iter()
            .filter(|s| s.chronicle == reduction.chronicle)
            .flat_map(|s| s.events.iter().map(|e| e.name.as_str()))
            .collect();
        if let Some(named) = reduction.filter.event.as_deref() {
            events.sort_by_key(|e| *e != named);
        }
        // Shapes are derived as needed, the filter's event first, and a
        // field is taken from the first event that has it: an event that
        // has nothing to do with the reading cannot break it.
        let mut shapes: Vec<Option<Result<Shape>>> = events.iter().map(|_| None).collect();
        let mut out = BTreeMap::new();
        for (name, derivation) in &reduction.derivations {
            let Some(field) = derivation.field() else {
                continue;
            };
            let mut found = None;
            let mut first_error = None;
            for (i, event) in events.iter().enumerate() {
                let shape = shapes[i]
                    .get_or_insert_with(|| self.shape(service, event, day).map(|(s, _)| s));
                match shape {
                    Ok(shape) => {
                        if let Some(def) = shape.field(field) {
                            found = Some(def.clone());
                            break;
                        }
                    }
                    Err(e) => {
                        first_error.get_or_insert_with(|| e.to_string());
                    }
                }
            }
            let def = found.ok_or_else(|| {
                setup(format!(
                    "lexostatus '{lexostatus}' reads field '{field}', which no event of chronicle '{}' has on {day}{}",
                    reduction.chronicle,
                    first_error.map(|e| format!(" ({e})")).unwrap_or_default()
                ))
            })?;
            // Whether a field is filled in is a yes or a no, whatever the
            // field holds.
            let def = match derivation {
                Derivation::Filled { legal_basis, .. } => shape::FieldDef {
                    name: field.to_string(),
                    type_: Some(regelrecht_law_model::ParameterType::Boolean),
                    legal_basis: if legal_basis.is_empty() {
                        def.legal_basis
                    } else {
                        legal_basis.clone()
                    },
                    declared_by: def.declared_by,
                    fixed: None,
                },
                _ => def,
            };
            out.insert(name.clone(), def);
        }
        Ok(out)
    }

    /// What the reads of `event` give for the case `root` at `as_of`, per
    /// parameter with where it came from: each lexostatus it names, and each
    /// policy of the holder it names, executed on the day of `as_of` with
    /// `{root}` over the register of the cell as it holds then. A parameter
    /// two of them give is ambiguous; an output a policy leaves empty (null)
    /// is no parameter. Read-only: what a decision or an execution of
    /// `event` reads, before it keeps what its article asks.
    pub fn read_case(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<BTreeMap<String, Input>> {
        let (_, e) = self.event(event)?;
        if e.reads.is_empty() {
            return Err(refused(format!(
                "event '{event}' reads nothing for its case (`reads` in its stream)"
            )));
        }
        let mut inputs = Map::new();
        inputs.insert("root".into(), serde_json::Value::String(root.into()));
        let mut read: BTreeMap<String, Input> = BTreeMap::new();
        let mut from: BTreeMap<String, String> = BTreeMap::new();
        for source in &e.reads {
            let values: Vec<(String, Input)> = match source {
                Read::Lexostatus(lexostatus) => {
                    let (definition, chronicle) = self
                        .configured(lexostatus)?
                        .ok_or_else(|| refused(format!("no lexostatus '{lexostatus}'")))?;
                    lexostatus::reduce(definition, &inputs, chronicle, as_of)?
                        .0
                        .into_iter()
                        .map(|(name, value)| (name, from_lexostatus(value, lexostatus)))
                        .collect()
                }
                Read::Regulation { regulation } => {
                    self.read_policy(service, regulation, root, as_of)?
                }
            };
            for (name, input) in values {
                if let Some(first) = from.get(&name) {
                    return Err(setup(format!(
                        "event '{event}' reads '{name}' from both {first} and {source}"
                    )));
                }
                from.insert(name.clone(), source.to_string());
                read.insert(name, input);
            }
        }
        Ok(read)
    }

    /// Execute the policy `policy`, which reads a register of the cell, for
    /// the case `root`: every output it has, on the day of `as_of`, over the
    /// grams of the register that hold at `as_of`. Per output its value and
    /// where it came from (the register and the article, as the engine says).
    fn read_policy(
        &self,
        service: &LawExecutionService,
        policy: &str,
        root: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<Vec<(String, Input)>> {
        let (register, chronicle) = self
            .register(service, policy)?
            .ok_or_else(|| setup(format!("policy '{policy}' reads no register of the cell")))?;
        let outputs = service
            .get_law_info(policy)
            .map(|i| i.outputs)
            .unwrap_or_default();
        let asked: Vec<&str> = outputs.iter().map(String::as_str).collect();
        if asked.is_empty() {
            return Err(setup(format!("policy '{policy}' has no outputs")));
        }
        let rows = register::rows(chronicle, as_of, &self.event_fields)?;
        let result = register::with_rows(
            &register::source_name(&self.config.id, register),
            rows,
            || {
                service.evaluate_law(
                    policy,
                    &asked,
                    BTreeMap::from([("root".to_string(), Value::String(root.to_string()))]),
                    &as_of.date_naive().to_string(),
                )
            },
        )?;
        let mut out = Vec::new();
        for (name, value) in result.outputs {
            if value.is_null() {
                continue;
            }
            let article = match result.output_provenance.get(&name) {
                Some(
                    OutputProvenance::Direct { law_id, article }
                    | OutputProvenance::Reactive {
                        law_id, article, ..
                    }
                    | OutputProvenance::Override { law_id, article }
                    | OutputProvenance::Voided {
                        law_id, article, ..
                    },
                ) => format!("{law_id}#{article}"),
                None => format!("{}#{}", result.law_id, result.article_number),
            };
            out.push((
                name,
                Input {
                    value: shape::to_json(&value),
                    provenance: serde_json::json!({
                        "source": "lexostatus",
                        "lexostatus": register.key(),
                        "article": article,
                    }),
                },
            ));
        }
        Ok(out)
    }

    /// The parameters of the decision `event` on the gram `root`: the
    /// lexostatuses the event `reads`, each read with `{root}` as the case
    /// holds at `as_of`, kept to what executing the establishing article at
    /// the event's stage asks (the article and the hooks that fire at that
    /// stage), each with the lexostatus it came from. The article is the
    /// version in force on the day of `as_of`, or, if the law says the
    /// decision concerns a period, on the first day of the period read; the
    /// parameter that gives the period is kept too. What [`Cell::decide`]
    /// reads; read-only, to look before deciding.
    pub fn decision_inputs(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<BTreeMap<String, Input>> {
        Ok(self
            .prepare(service, event, Some(root), BTreeMap::new(), as_of)?
            .inputs)
    }

    /// What taking the decision `event` on the case `root` at `as_of` asks:
    /// the stage of its procedure with what it requires, the article and the
    /// hooks that fire at that stage, and every parameter they declare, in
    /// the law of the period the decision concerns (see
    /// [`Self::decision_inputs`]). A caller that holds what the cell does not
    /// read from its chronicle (a date from a dossier) finds here what to
    /// give as `extra_inputs`; read-only.
    pub fn decision_stage(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<StageInputs> {
        Ok(self
            .prepare(service, event, Some(root), BTreeMap::new(), as_of)?
            .at)
    }

    /// Everything a decision `event` takes, worked out once: what the
    /// lexostatuses it `reads` give for the case `root` at `now` (kept to
    /// what its stage asks, and the parameter that gives its period),
    /// together with `extra_inputs`; the period those merged inputs give, the
    /// day whose law applies (the first day of that period, or the day of
    /// `now`), the shape in that law, and its stage there. Filtering and
    /// executing both use this, so they cannot apply different laws.
    fn prepare(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: Option<&str>,
        extra_inputs: BTreeMap<String, Input>,
        now: DateTime<FixedOffset>,
    ) -> Result<Prepared> {
        let today = now.date_naive();
        let (shape, chronicle) = self.shape(service, event, today)?;
        let read: BTreeMap<String, Input> = match root {
            Some(root) => self.read_case(service, event, root, now)?,
            None => BTreeMap::new(),
        };
        let mut merged = read.clone();
        for (name, input) in &extra_inputs {
            if merged.contains_key(name) {
                return Err(refused(format!(
                    "'{name}' is read from the case; it cannot be given as well"
                )));
            }
            merged.insert(name.clone(), input.clone());
        }

        let period_parameter = shape.period.as_ref().map(|p| p.parameter.clone());
        let period = period(&shape, &merged)?;
        let day = period.map_or(today, |(_, d)| d);
        let shape = if day == today {
            shape
        } else {
            self.shape(service, event, day)?.0
        };
        let at = stage_of(service, &shape, day)?;

        // What the cell reads goes to the decision only if its stage asks
        // it, or if it gives the period; what the caller gives goes as is.
        let asked = asked(&at);
        let mut inputs: BTreeMap<String, Input> = read
            .into_iter()
            .filter(|(name, _)| {
                asked.contains(&name.as_str()) || Some(name) == period_parameter.as_ref()
            })
            .collect();
        inputs.extend(extra_inputs);
        Ok(Prepared {
            inputs,
            shape,
            chronicle,
            day,
            period: period.map(|(p, _)| p),
            at,
        })
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
        let grams = self.chronicle(chronicle)?;
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
        let (gram, chronicle) = self.take(service, event, refers_to, extra_inputs, now)?;
        self.append(&chronicle, gram)
    }

    /// The gram [`Self::decide`] would record at `now`, without recording
    /// it: what the law decides, to look before deciding. Nothing in the
    /// chronicle changes, so a moment that has yet to come may be asked too.
    pub fn preview_decision(
        &self,
        service: &LawExecutionService,
        event: &str,
        refers_to: BTreeMap<String, String>,
        extra_inputs: BTreeMap<String, Input>,
        now: DateTime<FixedOffset>,
    ) -> Result<Gram> {
        Ok(self.take(service, event, refers_to, extra_inputs, now)?.0)
    }

    /// Take a decision: the gram and the chronicle it goes to, not recorded.
    fn take(
        &self,
        service: &LawExecutionService,
        event: &str,
        refers_to: BTreeMap<String, String>,
        extra_inputs: BTreeMap<String, Input>,
        now: DateTime<FixedOffset>,
    ) -> Result<(Gram, String)> {
        // Not yet: refusing an application out of time.
        let today = now.date_naive();
        let (shape, chronicle) = self.shape(service, event, today)?;
        let reads = self
            .config
            .event(event)
            .is_some_and(|(_, e)| !e.reads.is_empty());
        let root = if reads {
            Some(self.case_root(&shape, &chronicle, &refers_to)?)
        } else {
            None
        };
        let Prepared {
            mut inputs,
            shape,
            chronicle,
            day,
            period,
            at,
        } = self.prepare(service, event, root.as_deref(), extra_inputs, now)?;
        self.check_references(&shape, &chronicle, &refers_to)?;

        // The decision is taken at its stage of the procedure. The date the
        // decision bears is the one parameter the law names for it
        // (`dated_by`): the day it is taken. Whatever else the stage requires
        // is not the cell's to make up; it must be given.
        for required in &at.requires {
            if inputs.contains_key(&required.name) {
                continue;
            }
            if shape.dated_by.as_deref() != Some(required.name.as_str()) {
                return Err(refused(format!(
                    "stage {} of '{event}' requires '{}', and nothing gives it ({} names {} as the date of the decision)",
                    at.stage,
                    required.name,
                    shape.establishes,
                    shape
                        .dated_by
                        .as_deref()
                        .map_or("nothing".to_string(), |d| format!("'{d}'")),
                )));
            }
            inputs.insert(
                required.name.clone(),
                Input {
                    value: serde_json::Value::String(today.to_string()),
                    provenance: serde_json::json!({
                        "source": "decision",
                        "stage": at.stage,
                    }),
                },
            );
        }

        // The parameter that gives the period takes part in the decision,
        // but goes to the article only if the stage asks it.
        let period_parameter = shape.period.as_ref().map(|p| p.parameter.clone());
        let asked = asked(&at);
        let parameters =
            values(inputs.iter().filter(|(k, _)| {
                asked.contains(&k.as_str()) || Some(*k) != period_parameter.as_ref()
            }));
        let (_, article_number) = shape::split_reference(&shape.establishes)?;
        let result = service.execute_stage_at(
            &shape.law_id,
            article_number,
            &at.stage,
            parameters,
            &day.to_string(),
        )?;

        let mut gram = self.gram(&shape, &chronicle, now);
        gram.regulation = Some(shape.law_id.clone());
        gram.regulation_valid_from = result.regulation_valid_from.clone();
        gram.period = period;
        gram.refers_to = refers_to;
        gram.fields = fields_of(&shape, &result.outputs, &inputs)?;
        gram.inputs = recorded_inputs(inputs);
        Ok((gram, chronicle))
    }

    /// Execute the article that establishes the execution `event` (an
    /// executogram, such as a payment that executes a decision) for the
    /// case `root` on the day `on`, and record a gram only if the law says
    /// one arises. Returns `None` if it does not. Which days to ask is
    /// [`Self::due_executions`].
    ///
    /// What the law declares (`extensions.chronolex` of the article) decides
    /// every step; the cell knows no case:
    /// - `refers_to`: each reference is resolved within the case, to the
    ///   latest gram it admits that holds at `now` (by stage, or by the
    ///   article that establishes it). A required one that is not there is
    ///   refused.
    /// - `until`: once the case has a gram of that stage, no gram arises any
    ///   more; the call fails with [`crate::Error::Ended`].
    /// - the parameters are what the lexostatuses the event `reads` give at
    ///   `now`, kept to the parameters of the article, and `on` in the
    ///   parameter of `executed_on`. With a `period`, the law of the period.
    /// - `record_when`: the boolean output that says whether a gram arises.
    /// - `executed_on.once_per`: a second gram of the event in the same case
    ///   in the same month is refused, whatever gram it refers to.
    ///
    /// A fact that has yet to happen is not a fact (RFC-044): `on` may not
    /// lie after `now`, nor before the day of a gram it refers to. The gram
    /// holds from the start of `on` (or from `now`, on the day itself), but
    /// not before the moment of a gram it refers to; it is recorded at `now`.
    pub fn execute(
        &mut self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        on: NaiveDate,
        now: DateTime<FixedOffset>,
    ) -> Result<Option<Gram>> {
        if on > now.date_naive() {
            return Err(refused(format!(
                "'{event}' on {on} has yet to happen on {}: not a fact",
                now.date_naive()
            )));
        }
        match self.execution(service, event, root, on, now)? {
            Some((gram, chronicle)) => self.append(&chronicle, gram).map(Some),
            None => Ok(None),
        }
    }

    /// The gram [`Self::execute`] would record for the case `root` on `on`,
    /// reading the chronicle as it holds at `now`, without recording it:
    /// whether the law says one arises then, and with what. `on` may lie
    /// after `now`; what has yet to happen is no fact, but the law can say
    /// what it would be, such as the execution of next month.
    pub fn preview_execution(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        on: NaiveDate,
        now: DateTime<FixedOffset>,
    ) -> Result<Option<Gram>> {
        Ok(self
            .execution(service, event, root, on, now)?
            .map(|(gram, _)| gram))
    }

    /// Record what arises on receipt of what another party sends (a channel,
    /// RFC-022): execute `article` once with `inputs`, and record a gram of
    /// every event of the cell that establishes it on receipt (an execution
    /// with `record_when` and no `executed_on`) whose `record_when` is true.
    /// `refers_to` names the grams of this cell the received message is
    /// about (a payment order this cell gave); each event takes the
    /// references its law declares, a required one must be there, and a name
    /// no event declares is refused. Which article receives what, is for the
    /// caller (the transport between cells) to say; what arises, says the law.
    ///
    /// The message arrived at `at` and is recorded at `now`: the grams hold
    /// from `at`, under the law of that day. The
    /// transport delivers the bank's answer to an order at the moment of
    /// that order, so the next order (which reads the case as of its own
    /// moment) sees it; a message delivered late holds from when it arrived.
    /// `at` after `now` is refused (a future fact is never recorded), and so
    /// is `at` before a gram the message refers to.
    ///
    /// One answer per message: a message about a gram that already has a
    /// gram of `article` referring to it under the same required name, or
    /// whose identifying value (`identified_by`) a gram of `article` already
    /// holds, is refused as [`Error::Answered`](crate::Error::Answered), so a
    /// sender that delivers again (it did not hear the first delivery land)
    /// records nothing twice.
    pub fn receive(
        &mut self,
        service: &LawExecutionService,
        article: &str,
        refers_to: BTreeMap<String, String>,
        inputs: BTreeMap<String, Input>,
        at: DateTime<FixedOffset>,
        now: DateTime<FixedOffset>,
    ) -> Result<Vec<Gram>> {
        let grams = self.receipt(service, article, &refers_to, &inputs, at, now)?;
        grams
            .into_iter()
            .map(|(gram, chronicle)| self.append(&chronicle, gram))
            .collect()
    }

    /// Execute a receipt of a message that arrived at `at`: the grams that
    /// arise, with their chronicle, recorded at `now`.
    fn receipt(
        &self,
        service: &LawExecutionService,
        article: &str,
        refers_to: &BTreeMap<String, String>,
        inputs: &BTreeMap<String, Input>,
        at: DateTime<FixedOffset>,
        now: DateTime<FixedOffset>,
    ) -> Result<Vec<(Gram, String)>> {
        if at > now {
            return Err(refused(format!(
                "{article}: a message that arrives at {at} is not yet received at {now}"
            )));
        }
        let today = at.date_naive();
        let referred: Vec<&Gram> = refers_to
            .values()
            .filter_map(|id| self.chronicles.values().find_map(|c| c.find(id)))
            .collect();
        for gram in &referred {
            if lexostatus::moment(gram, &gram.effective_at)? > at {
                return Err(refused(format!(
                    "{article}: a message at {at} about '{}', which holds from {}",
                    gram.id, gram.effective_at
                )));
            }
        }
        // What the message is about decides the law: the receipt of an
        // answer to a gram that concerns a period is judged under the law of
        // that period, as that gram was; otherwise under the law of the day
        // it arrived. The shapes and the execution both use that day.
        let period = referred.iter().find_map(|g| g.period);
        let day = period.and_then(|p| p.first_day()).unwrap_or(today);
        let mut shapes: Vec<(Shape, String)> = Vec::new();
        for stream in &self.config.streams {
            for event in stream.events.iter().filter(|e| e.establishes == article) {
                let (shape, chronicle) = self.shape(service, &event.name, day)?;
                if shape.is_receipt() {
                    shapes.push((shape, chronicle));
                }
            }
        }
        let Some((first, _)) = shapes.first() else {
            return Err(refused(format!(
                "cell '{}' records nothing on receipt for {article}",
                self.config.id
            )));
        };
        let law_id = first.law_id.clone();
        for name in refers_to.keys() {
            if !shapes.iter().any(|(s, _)| s.refers_to.contains_key(name)) {
                return Err(refused(format!(
                    "{article} refers to nothing as '{name}' in cell '{}'",
                    self.config.id
                )));
            }
        }
        self.check_answered(article, &shapes, refers_to, inputs)?;
        // One execution of the article: every output that says whether a
        // gram arises, and every output a gram holds.
        let mut asked: Vec<&str> = Vec::new();
        for (shape, _) in &shapes {
            for name in shape.record_when.iter().chain(
                shape
                    .fields
                    .iter()
                    .map(|f| &f.name)
                    .filter(|n| shape.outputs.contains(n)),
            ) {
                if !asked.contains(&name.as_str()) {
                    asked.push(name);
                }
            }
        }
        let result =
            service.evaluate_law(&law_id, &asked, values(inputs.iter()), &day.to_string())?;
        let mut out = Vec::new();
        for (shape, chronicle) in shapes {
            if !arises(
                &shape,
                &result.outputs,
                shape.record_when.as_deref().unwrap_or_default(),
            )? {
                continue;
            }
            let mine: BTreeMap<String, String> = refers_to
                .iter()
                .filter(|(name, _)| shape.refers_to.contains_key(*name))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            self.check_references(&shape, &chronicle, &mine)?;
            let mut gram = self.gram(&shape, &chronicle, now);
            gram.effective_at = at.to_rfc3339();
            gram.regulation = Some(shape.law_id.clone());
            gram.regulation_valid_from = result.regulation_valid_from.clone();
            gram.period = period;
            gram.refers_to = mine;
            gram.fields = fields_of(&shape, &result.outputs, inputs)?;
            gram.inputs = recorded_inputs(inputs.clone());
            out.push((gram, chronicle));
        }
        Ok(out)
    }

    /// Whether the message already has its answer: a gram of `article` (one
    /// of the receipt events in `shapes`) that refers to the same gram under
    /// the same required name, or that holds the same value of the
    /// parameter that identifies the message (`identified_by`). Refused as
    /// [`Error::Answered`](crate::Error::Answered) if so; a message without
    /// that identifying value is refused outright.
    fn check_answered(
        &self,
        article: &str,
        shapes: &[(Shape, String)],
        refers_to: &BTreeMap<String, String>,
        inputs: &BTreeMap<String, Input>,
    ) -> Result<()> {
        for (shape, chronicle) in shapes {
            // A message without its identifying value could not be told
            // from another: refused, not taken as new.
            if let Some(key) = &shape.identified_by {
                if !inputs.contains_key(key) {
                    return Err(refused(format!(
                        "{article}: the message has no '{key}', which identifies it"
                    )));
                }
            }
            let Some(grams) = self.chronicles.get(chronicle) else {
                continue;
            };
            if let Some(key) = &shape.identified_by {
                let Some(value) = inputs.get(key).map(|i| &i.value) else {
                    continue;
                };
                let answered = grams.grams().iter().find(|g| {
                    g.establishes == article
                        && shapes.iter().any(|(s, _)| s.event == g.name)
                        && g.fields.get(key) == Some(value)
                });
                if let Some(answer) = answered {
                    return Err(crate::Error::Answered(format!(
                        "{key} {value} already has its answer under {article} ('{}', {})",
                        answer.id, answer.name
                    )));
                }
            }
            for (name, _) in shape.refers_to.iter().filter(|(_, r)| r.required) {
                let Some(id) = refers_to.get(name) else {
                    continue;
                };
                let answered = grams.grams().iter().find(|g| {
                    g.establishes == article
                        && shapes.iter().any(|(s, _)| s.event == g.name)
                        && g.refers_to.get(name) == Some(id)
                });
                if let Some(answer) = answered {
                    return Err(crate::Error::Answered(format!(
                        "'{id}' already has its answer under {article} as '{name}' ('{}', {})",
                        answer.id, answer.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// The days up to `through` on which the execution `event` is executed
    /// for the case `root`, as the case holds at `now`: per period of
    /// `executed_on.once_per`, the day the law (or the policy executing it)
    /// gives as `executed_on.day`, or the first day the execution may arise
    /// if that is later; only days after `after` (what the caller already
    /// asked), and no period in which the case already has a gram of the
    /// event. Whether a gram arises on such a day is for [`Self::execute`]
    /// to say.
    ///
    /// The first day an execution may arise is the moment of the latest
    /// gram each of its references admits (it may not lie before one); none
    /// while a required reference has no gram, and none once the case has a
    /// gram of the stage that ends it (`until`). `through` may lie after
    /// `now`: these are the days the law executes on, not facts.
    pub fn due_executions(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        after: Option<NaiveDate>,
        through: NaiveDate,
        now: DateTime<FixedOffset>,
    ) -> Result<Vec<NaiveDate>> {
        let (shape, chronicle) = self.shape(service, event, now.date_naive())?;
        let executed_on = executed_on(&shape)?;
        let (Some(Every::Month), Some(day_of_month)) = (executed_on.once_per, executed_on.day)
        else {
            return Err(setup(format!(
                "{}: '{event}' says not on which day of which period it is executed (executed_on: once_per and day)",
                shape.establishes
            )));
        };
        let (grams, case) = self.case_at(&chronicle, root, now)?;
        if ended(&shape, &case, root).is_err() {
            return Ok(Vec::new());
        }
        let Some(references) = references(&shape, &case) else {
            return Ok(Vec::new());
        };
        // From the root, or the latest gram a reference names.
        let mut start = grams
            .find(root)
            .map(|g| lexostatus::moment(g, &g.effective_at))
            .transpose()?
            .ok_or_else(|| refused(format!("no gram '{root}' in chronicle '{chronicle}'")))?;
        for gram in references.values() {
            start = start.max(lexostatus::moment(gram, &gram.effective_at)?);
        }
        let done: Vec<(i32, u32)> = case
            .iter()
            .filter(|g| g.name == event)
            .map(|g| lexostatus::moment(g, &g.effective_at).map(|m| month_of(m.date_naive())))
            .collect::<Result<_>>()?;

        let mut days = Vec::new();
        let mut month = month_of(start.date_naive());
        while let Some(first) = NaiveDate::from_ymd_opt(month.0, month.1, 1) {
            if first > through {
                break;
            }
            let last = last_day_of_month(first);
            // The day does not depend on when the cell is asked: on the day
            // the referred gram holds, the execution holds from that gram's
            // moment (see `execute`), not from the next day.
            let day = first
                .with_day(day_of_month.min(last.day()))
                .unwrap_or(last)
                .max(start.date_naive());
            if month_of(day) == month
                && day <= through
                && after.is_none_or(|a| day > a)
                && !done.contains(&month)
            {
                days.push(day);
            }
            month = if month.1 == 12 {
                (month.0 + 1, 1)
            } else {
                (month.0, month.1 + 1)
            };
        }
        Ok(days)
    }

    /// The chronicle `chronicle` and the grams of the case `root` in it that
    /// hold at `now`, in the order they hold. A root the chronicle does not
    /// have is refused.
    fn case_at(
        &self,
        chronicle: &str,
        root: &str,
        now: DateTime<FixedOffset>,
    ) -> Result<(&Chronicle, Vec<&Gram>)> {
        let grams = self.chronicle(chronicle)?;
        if grams.find(root).is_none() {
            return Err(refused(format!(
                "no gram '{root}' in chronicle '{chronicle}'"
            )));
        }
        let case = lexostatus::in_force(grams, now)?
            .into_iter()
            .filter(|g| grams.root_of(g) == root)
            .collect();
        Ok((grams, case))
    }

    /// Execute an execution: the gram and its chronicle if one arises, not
    /// recorded.
    fn execution(
        &self,
        service: &LawExecutionService,
        event: &str,
        root: &str,
        on: NaiveDate,
        now: DateTime<FixedOffset>,
    ) -> Result<Option<(Gram, String)>> {
        let today = now.date_naive();
        let (shape, chronicle) = self.shape(service, event, today)?;
        let executed_on = executed_on(&shape)?;
        let (_, case) = self.case_at(&chronicle, root, now)?;
        ended(&shape, &case, root)?;
        let Some(references) = references(&shape, &case) else {
            let (name, reference) = shape
                .refers_to
                .iter()
                .find(|(_, r)| r.required && !case.iter().any(|g| r.admits(g)))
                .ok_or_else(|| setup("a required reference without a gram, and none missing"))?;
            return Err(refused(format!(
                "'{event}' refers to a gram of {} as '{name}', and case '{root}' has none",
                reference.target()
            )));
        };
        // It holds from `now` on the day itself, otherwise from the start of
        // `on`; but never before a gram it refers to: on the day that gram
        // holds, from that gram's moment. A day before it is refused.
        let mut effective_at = if on == today {
            now
        } else {
            start_of(on, &now)?
        };
        let mut refers_to = BTreeMap::new();
        for (name, gram) in &references {
            let moment = lexostatus::moment(gram, &gram.effective_at)?;
            if moment.date_naive() > on {
                return Err(refused(format!(
                    "'{event}' on {on} lies before '{name}' ('{}', {})",
                    gram.id, gram.effective_at
                )));
            }
            effective_at = effective_at.max(moment);
            refers_to.insert(name.clone(), gram.id.clone());
        }

        // Once per period for the case: a second decision referred to in the
        // same case (a revision) does not make a second one arise.
        if executed_on.once_per == Some(Every::Month) {
            for gram in case.iter().filter(|g| g.name == event) {
                let day = lexostatus::moment(gram, &gram.effective_at)?.date_naive();
                if month_of(day) == month_of(on) {
                    return Err(refused(format!(
                        "'{event}' already arose in case '{root}' in {}-{:02} ('{}')",
                        on.year(),
                        on.month(),
                        gram.id
                    )));
                }
            }
        }

        // The case as it holds at the moment of this execution, not at
        // `now`: an execution of a day the clock passed (several missed
        // months in one step) sees only what held before it, such as the
        // answer to the order of the month before, never a later one.
        let mut inputs: BTreeMap<String, Input> = self
            .read_case(service, event, root, effective_at)?
            .into_iter()
            .filter(|(name, _)| shape.parameters.contains(name))
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
        let period = period(&shape, &inputs)?;
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
            values(inputs.iter()),
            &day.to_string(),
        )?;
        if !arises(&shape, &result.outputs, &when)? {
            return Ok(None);
        }

        let mut gram = self.gram(&shape, &chronicle, now);
        gram.effective_at = effective_at.to_rfc3339();
        gram.regulation = Some(shape.law_id.clone());
        gram.regulation_valid_from = result.regulation_valid_from.clone();
        gram.period = period.map(|(p, _)| p);
        gram.refers_to = refers_to;
        gram.fields = fields_of(&shape, &result.outputs, &inputs)?;
        gram.inputs = recorded_inputs(inputs);
        Ok(Some((gram, chronicle)))
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
        let grams = self.chronicle(chronicle)?;
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

/// What [`Cell::prepare`] works out for a decision.
struct Prepared {
    inputs: BTreeMap<String, Input>,
    shape: Shape,
    chronicle: String,
    day: NaiveDate,
    period: Option<Period>,
    at: StageInputs,
}

/// The fields of a gram of `shape`: each from the outputs of executing the
/// law, or, for a field the article declares as a parameter (and not as an
/// output), the value it was given. An output the law does not give is
/// refused, not recorded as nothing: a gram holds what the law decided, and
/// a null is no decision.
fn fields_of(
    shape: &Shape,
    outputs: &BTreeMap<String, Value>,
    inputs: &BTreeMap<String, Input>,
) -> Result<Map<String, serde_json::Value>> {
    shape
        .fields
        .iter()
        .map(|f| {
            let given = || {
                (shape.parameters.contains(&f.name) && !shape.outputs.contains(&f.name))
                    .then(|| inputs.get(&f.name).map(|i| i.value.clone()))
                    .flatten()
            };
            outputs
                .get(&f.name)
                .map(shape::to_json)
                .or_else(given)
                .map(|v| (f.name.clone(), v))
                .ok_or_else(|| {
                    refused(format!(
                        "'{}': the law gives no '{}' ({})",
                        shape.event, f.name, f.declared_by
                    ))
                })
        })
        .collect()
}

/// The values of `inputs`, as the engine takes them.
fn values<'a>(inputs: impl Iterator<Item = (&'a String, &'a Input)>) -> BTreeMap<String, Value> {
    inputs
        .map(|(k, i)| (k.clone(), Value::from(&i.value)))
        .collect()
}

/// Whether the law says a gram of `shape` arises: its boolean output `when`.
fn arises(shape: &Shape, outputs: &BTreeMap<String, Value>, when: &str) -> Result<bool> {
    match outputs.get(when) {
        Some(Value::Bool(b)) => Ok(*b),
        other => Err(setup(format!(
            "{}: '{when}' says whether '{}' arises, and is {other:?}",
            shape.establishes, shape.event
        ))),
    }
}

/// The inputs as a gram records them: per parameter its value and where it
/// came from.
fn recorded_inputs(inputs: BTreeMap<String, Input>) -> BTreeMap<String, serde_json::Value> {
    inputs
        .into_iter()
        .map(|(k, i)| {
            let v = serde_json::to_value(i).unwrap_or(serde_json::Value::Null);
            (k, v)
        })
        .collect()
}

/// The `executed_on` of an execution; refused for an event that is none.
fn executed_on(shape: &Shape) -> Result<ExecutedOn> {
    shape.executed_on.clone().ok_or_else(|| {
        refused(format!(
            "'{}' is not an execution: {} declares no executed_on",
            shape.event, shape.establishes
        ))
    })
}

/// [`crate::Error::Ended`] if the case has a gram of the stage that ends an
/// execution of `shape` (`until`).
fn ended(shape: &Shape, case: &[&Gram], root: &str) -> Result<()> {
    let Some(until) = &shape.until else {
        return Ok(());
    };
    match case
        .iter()
        .find(|g| g.stage.as_deref() == Some(until.stage.as_str()))
    {
        Some(gram) => Err(crate::Error::Ended(format!(
            "'{}' no longer arises in case '{root}': it has a gram of stage {} ('{}')",
            shape.event, until.stage, gram.id
        ))),
        None => Ok(()),
    }
}

/// Per reference of `shape`, the latest gram of `case` it admits; `None` if
/// a required one has none. An optional one without a gram is left out.
fn references<'a>(shape: &Shape, case: &[&'a Gram]) -> Option<BTreeMap<String, &'a Gram>> {
    let mut out = BTreeMap::new();
    for (name, reference) in &shape.refers_to {
        match case.iter().rev().find(|g| reference.admits(g)) {
            Some(gram) => {
                out.insert(name.clone(), *gram);
            }
            None if reference.required => return None,
            None => {}
        }
    }
    Some(out)
}

/// The calendar month of a day.
fn month_of(day: NaiveDate) -> (i32, u32) {
    (day.year(), day.month())
}

/// The last day of the month of `first`.
fn last_day_of_month(first: NaiveDate) -> NaiveDate {
    first
        .checked_add_months(chrono::Months::new(1))
        .and_then(|next| next.pred_opt())
        .unwrap_or(first)
}

/// The start of `day` in the offset of `now`.
fn start_of(day: NaiveDate, now: &DateTime<FixedOffset>) -> Result<DateTime<FixedOffset>> {
    day.and_hms_opt(0, 0, 0)
        .and_then(|t| t.and_local_timezone(*now.offset()).single())
        .ok_or_else(|| setup(format!("no start of the day {day}")))
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

/// The period a decision of `shape` concerns, from the parameter in `inputs`
/// that gives it, with the day the law of that period is the law on; `None`
/// if the law names no period.
fn period(shape: &Shape, inputs: &BTreeMap<String, Input>) -> Result<Option<(Period, NaiveDate)>> {
    let Some(declared) = &shape.period else {
        return Ok(None);
    };
    let value = inputs
        .get(&declared.parameter)
        .map(|i| &i.value)
        .ok_or_else(|| {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn an_output_the_law_does_not_give_is_refused() {
        let mut service = LawExecutionService::new();
        service
            .load_law(
                "$id: testwet
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Test.
    machine_readable:
      execution:
        produces:
          extensions:
            chronolex:
              establishes:
                - {event: gebeurd, type: decretogram, fields: outputs}
        output: [{name: y, type: number}]
        actions: [{output: y, value: 1}]
",
            )
            .unwrap();
        let event = crate::config::Event {
            name: "gebeurd".into(),
            establishes: "testwet#1".into(),
            stage: None,
            reads: Vec::new(),
        };
        let shape = shape::derive(&service, &event, "2024-06-01".parse().unwrap()).unwrap();
        let e = fields_of(&shape, &BTreeMap::new(), &BTreeMap::new()).unwrap_err();
        assert!(matches!(e, crate::Error::Refused(_)), "{e}");
        assert!(e.to_string().contains("'y'"), "{e}");
        let fields = fields_of(
            &shape,
            &BTreeMap::from([("y".to_string(), Value::Int(1))]),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(fields["y"], 1);
    }
}
