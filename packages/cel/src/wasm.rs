//! The cell in the browser: `WasmCell`, next to the `WasmEngine` it executes
//! the law with. The chronicle lives in memory; the page keeps it between
//! sessions by passing back what `grams()` gave.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset, NaiveDate};
use regelrecht_engine::wasm::WasmEngine;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

use crate::cell::{Cell, Input};
use crate::chronicle::Gram;
use crate::config::CellConfig;

fn error(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// An error of the cell as a JS `Error` whose `name` is the kind of error
/// ([`crate::Error::code`]: `refused`, `ended`, ...), so the page can tell
/// "nothing more to come" from a failure without reading the message.
fn cell_error(e: crate::Error) -> JsValue {
    let js = js_sys::Error::new(&e.to_string());
    js.set_name(e.code());
    js.into()
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&Serializer::json_compatible())
        .map_err(error)
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(error)
}

fn moment(now: &str) -> Result<DateTime<FixedOffset>, JsValue> {
    DateTime::parse_from_rfc3339(now).map_err(|e| error(format!("'{now}': {e}")))
}

fn day(day: &str) -> Result<NaiveDate, JsValue> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|e| error(format!("'{day}': {e}")))
}

/// `{name: {value, provenance}}` from the page, or nothing.
fn extra(extra_inputs: JsValue) -> Result<BTreeMap<String, Input>, JsValue> {
    if extra_inputs.is_null() || extra_inputs.is_undefined() {
        Ok(BTreeMap::new())
    } else {
        from_js(extra_inputs)
    }
}

#[wasm_bindgen]
pub struct WasmCell {
    cell: Cell,
}

#[wasm_bindgen]
impl WasmCell {
    /// A cell from its configuration texts (`cell.yaml`, its streams and its
    /// lexostatuses), with the grams the page kept, checked against the law
    /// the engine has loaded as it applies on `today` (`YYYY-MM-DD`). Its
    /// registers are bound with the engine as data sources (see
    /// `bindRegisters`).
    #[wasm_bindgen(constructor)]
    pub fn new(
        engine: &mut WasmEngine,
        cell_yaml: &str,
        streams: Vec<String>,
        lexostatuses: Option<String>,
        grams: JsValue,
        today: &str,
    ) -> Result<WasmCell, JsValue> {
        let streams: Vec<&str> = streams.iter().map(String::as_str).collect();
        let config = CellConfig::from_yaml(cell_yaml, &streams, lexostatuses.as_deref())
            .map_err(cell_error)?;
        let grams: Vec<Gram> = if grams.is_null() || grams.is_undefined() {
            Vec::new()
        } else {
            from_js(grams)?
        };
        crate::register::bind(engine.service_mut(), &config).map_err(cell_error)?;
        let cell =
            Cell::in_memory(config, grams, engine.service(), day(today)?).map_err(cell_error)?;
        Ok(WasmCell { cell })
    }

    /// Bind the registers of the cell with the engine again: after the page
    /// cleared the engine's data sources (`clearDataSources`), a policy that
    /// reads a chronicle of the cell would otherwise read nothing.
    #[wasm_bindgen(js_name = bindRegisters)]
    pub fn bind_registers(&self, engine: &mut WasmEngine) -> Result<(), JsValue> {
        crate::register::bind(engine.service_mut(), self.cell.config()).map_err(cell_error)
    }

    /// What a gram of `event` holds on `day`: per field its name, type,
    /// legal basis, the article that asks it, and a value the cell fills in
    /// itself. The fields of an application form.
    pub fn shape(&self, engine: &WasmEngine, event: &str, on: &str) -> Result<JsValue, JsValue> {
        let (shape, _) = self
            .cell
            .shape(engine.service(), event, day(on)?)
            .map_err(cell_error)?;
        to_js(&shape)
    }

    /// Record an application as the applicant made it, received at `now`
    /// (RFC 3339, Dutch time).
    #[wasm_bindgen(js_name = recordSubmission)]
    pub fn record_submission(
        &mut self,
        engine: &WasmEngine,
        event: &str,
        submitted: JsValue,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let submitted: serde_json::Map<String, serde_json::Value> = from_js(submitted)?;
        let gram = self
            .cell
            .record_submission(engine.service(), event, &submitted, moment(now)?)
            .map_err(cell_error)?;
        to_js(&gram)
    }

    /// Read a lexostatus: the chronicle reduced to parameters, as it holds
    /// at `asOf` (RFC 3339): a gram that holds only later does not count.
    pub fn read(&self, lexostatus: &str, inputs: JsValue, as_of: &str) -> Result<JsValue, JsValue> {
        let inputs: serde_json::Map<String, serde_json::Value> = from_js(inputs)?;
        to_js(
            &self
                .cell
                .read(lexostatus, &inputs, moment(as_of)?)
                .map_err(cell_error)?,
        )
    }

    /// The parameters of the decision `event` on the application `root`, as
    /// the cell reads them from its chronicle at `now` (RFC 3339):
    /// `{name: {value, provenance}}`, ready for `decide`.
    #[wasm_bindgen(js_name = inputsFor)]
    pub fn inputs_for(
        &self,
        engine: &WasmEngine,
        event: &str,
        root: &str,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let inputs = self
            .cell
            .decision_inputs(engine.service(), event, root, moment(now)?)
            .map_err(cell_error)?;
        to_js(&inputs)
    }

    /// What taking the decision `event` on the application `root` at `now`
    /// asks: the stage with what it requires, the articles taking part and
    /// every parameter they declare with its origin. What the cell does not
    /// read from its chronicle (`inputsFor`), the page may give as
    /// `extraInputs`.
    #[wasm_bindgen(js_name = decisionStage)]
    pub fn decision_stage(
        &self,
        engine: &WasmEngine,
        event: &str,
        root: &str,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let stage = self
            .cell
            .decision_stage(engine.service(), event, root, moment(now)?)
            .map_err(cell_error)?;
        to_js(&stage)
    }

    /// The gram `decide` would record at `now`, without recording it: what
    /// the law decides, to look before deciding (or ahead, to a moment that
    /// has yet to come).
    #[wasm_bindgen(js_name = previewDecision)]
    pub fn preview_decision(
        &self,
        engine: &WasmEngine,
        event: &str,
        refers_to: JsValue,
        now: &str,
        extra_inputs: JsValue,
    ) -> Result<JsValue, JsValue> {
        let refers_to: BTreeMap<String, String> = from_js(refers_to)?;
        let gram = self
            .cell
            .preview_decision(
                engine.service(),
                event,
                refers_to,
                extra(extra_inputs)?,
                moment(now)?,
            )
            .map_err(cell_error)?;
        to_js(&gram)
    }

    /// Take a decision at `now` and record it, referring to `refersTo`
    /// (`{on_application: <id>}`). The cell reads the parameters from that
    /// case itself; `extraInputs` (`{name: {value, provenance}}`, optional)
    /// may only add what it does not read.
    pub fn decide(
        &mut self,
        engine: &WasmEngine,
        event: &str,
        refers_to: JsValue,
        now: &str,
        extra_inputs: JsValue,
    ) -> Result<JsValue, JsValue> {
        let refers_to: BTreeMap<String, String> = from_js(refers_to)?;
        let gram = self
            .cell
            .decide(
                engine.service(),
                event,
                refers_to,
                extra(extra_inputs)?,
                moment(now)?,
            )
            .map_err(cell_error)?;
        to_js(&gram)
    }

    /// Execute the execution `event` (an executogram) for the case `root` on
    /// `on` (`YYYY-MM-DD`), recorded at `now` (RFC 3339): the gram if the
    /// law says one arises, otherwise `null`. See `Cell::execute`.
    pub fn execute(
        &mut self,
        engine: &WasmEngine,
        event: &str,
        root: &str,
        on: &str,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let gram = self
            .cell
            .execute(engine.service(), event, root, day(on)?, moment(now)?)
            .map_err(cell_error)?;
        match gram {
            Some(gram) => to_js(&gram),
            None => Ok(JsValue::NULL),
        }
    }

    /// The gram `execute` would record for the case `root` on `on`
    /// (`YYYY-MM-DD`), reading the chronicle as it holds at `now`, without
    /// recording it; `null` if the law says none arises then. `on` may lie
    /// after `now`: what the law gives as the next instalment.
    #[wasm_bindgen(js_name = previewExecution)]
    pub fn preview_execution(
        &self,
        engine: &WasmEngine,
        event: &str,
        root: &str,
        on: &str,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let gram = self
            .cell
            .preview_execution(engine.service(), event, root, day(on)?, moment(now)?)
            .map_err(cell_error)?;
        match gram {
            Some(gram) => to_js(&gram),
            None => Ok(JsValue::NULL),
        }
    }

    /// Record what arises on receipt of a message from another party:
    /// execute `article` (`<regulation>#<article>`) with `inputs`
    /// (`{name: {value, provenance}}`) at `now` (RFC 3339), referring to the
    /// grams of this cell in `refersTo` (`{name: id}`). Returns the grams
    /// recorded (none, one, or more). See `Cell::receive`.
    pub fn receive(
        &mut self,
        engine: &WasmEngine,
        article: &str,
        refers_to: JsValue,
        inputs: JsValue,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let refers_to: BTreeMap<String, String> = if refers_to.is_null() || refers_to.is_undefined()
        {
            BTreeMap::new()
        } else {
            from_js(refers_to)?
        };
        let grams = self
            .cell
            .receive(
                engine.service(),
                article,
                refers_to,
                extra(inputs)?,
                moment(now)?,
            )
            .map_err(cell_error)?;
        to_js(&grams)
    }

    /// The days up to `through` (`YYYY-MM-DD`) on which the execution
    /// `event` is executed for the case `root`, after `after` (a day, or
    /// `null`), as the case holds at `now` (RFC 3339): `["YYYY-MM-DD", ...]`.
    /// See `Cell::due_executions`. The page executes or previews each day;
    /// it does not work out the days itself.
    #[wasm_bindgen(js_name = dueExecutions)]
    pub fn due_executions(
        &self,
        engine: &WasmEngine,
        event: &str,
        root: &str,
        after: Option<String>,
        through: &str,
        now: &str,
    ) -> Result<JsValue, JsValue> {
        let after = after.as_deref().map(day).transpose()?;
        let days = self
            .cell
            .due_executions(
                engine.service(),
                event,
                root,
                after,
                day(through)?,
                moment(now)?,
            )
            .map_err(cell_error)?;
        to_js(&days.iter().map(ToString::to_string).collect::<Vec<_>>())
    }

    /// Per parameter a lexostatus gives, the field of a gram it reads, as
    /// the law declares that field on `on` (name, type, legal basis, the
    /// article): how to show what `read` returns. A parameter that reads no
    /// field (a moment, a period) is left out.
    #[wasm_bindgen(js_name = lexostatusFields)]
    pub fn lexostatus_fields(
        &self,
        engine: &WasmEngine,
        lexostatus: &str,
        on: &str,
    ) -> Result<JsValue, JsValue> {
        let fields = self
            .cell
            .lexostatus_fields(engine.service(), lexostatus, day(on)?)
            .map_err(cell_error)?;
        to_js(&fields)
    }

    /// Every gram, to keep between sessions and to show.
    pub fn grams(&self) -> Result<JsValue, JsValue> {
        to_js(&self.cell.grams().collect::<Vec<_>>())
    }
}
