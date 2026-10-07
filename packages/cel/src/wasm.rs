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

#[wasm_bindgen]
pub struct WasmCell {
    cell: Cell,
}

#[wasm_bindgen]
impl WasmCell {
    /// A cell from its configuration texts (`cell.yaml`, its streams and its
    /// lexostatuses), with the grams the page kept, checked against the law
    /// the engine has loaded as it applies on `today` (`YYYY-MM-DD`).
    #[wasm_bindgen(constructor)]
    pub fn new(
        engine: &WasmEngine,
        cell_yaml: &str,
        streams: Vec<String>,
        lexostatuses: Option<String>,
        grams: JsValue,
        today: &str,
    ) -> Result<WasmCell, JsValue> {
        let streams: Vec<&str> = streams.iter().map(String::as_str).collect();
        let config =
            CellConfig::from_yaml(cell_yaml, &streams, lexostatuses.as_deref()).map_err(error)?;
        let grams: Vec<Gram> = if grams.is_null() || grams.is_undefined() {
            Vec::new()
        } else {
            from_js(grams)?
        };
        let cell = Cell::in_memory(config, grams, engine.service(), day(today)?).map_err(error)?;
        Ok(WasmCell { cell })
    }

    /// What a gram of `event` holds on `day`: per field its name, type,
    /// legal basis, the article that asks it, and a value the cell fills in
    /// itself. The fields of an application form.
    pub fn shape(&self, engine: &WasmEngine, event: &str, on: &str) -> Result<JsValue, JsValue> {
        let (shape, _) = self
            .cell
            .shape(engine.service(), event, day(on)?)
            .map_err(error)?;
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
            .map_err(error)?;
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
                .map_err(error)?,
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
            .map_err(error)?;
        to_js(&inputs)
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
        let extra_inputs: BTreeMap<String, Input> =
            if extra_inputs.is_null() || extra_inputs.is_undefined() {
                BTreeMap::new()
            } else {
                from_js(extra_inputs)?
            };
        let gram = self
            .cell
            .decide(
                engine.service(),
                event,
                refers_to,
                extra_inputs,
                moment(now)?,
            )
            .map_err(error)?;
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
            .map_err(error)?;
        match gram {
            Some(gram) => to_js(&gram),
            None => Ok(JsValue::NULL),
        }
    }

    /// Every gram, to keep between sessions and to show.
    pub fn grams(&self) -> Result<JsValue, JsValue> {
        to_js(&self.cell.grams().collect::<Vec<_>>())
    }
}
