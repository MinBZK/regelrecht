//! De celconfiguratie: welke wetten een cel laadt, welke feiten ze houdt en
//! welke lexostatussen ze publiceert.
//!
//! Lexostatus-definities zijn **data**, geen Rust. Ze staan in de configuratie
//! van de cel, worden gelezen door de loader en zijn verder onveranderlijk. Een
//! consument kan dus geen eigen reductie injecteren; hij kan alleen een
//! gepubliceerde naam opvragen met gedocumenteerde parameters (RFC-022 §4.1).

use crate::cell::chronicle::ChronicleStream;
use crate::error::{Result, SimulatorError};
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Alles wat nodig is om één cel op te tuigen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellConfig {
    /// Het cel-id, bijvoorbeeld `toeslagen`.
    pub id: String,
    /// De regelingen die deze cel zelf laadt, bij `$id`.
    pub laws: Vec<String>,
    /// De kroniekstromen met de eigen feiten van de cel.
    #[serde(default)]
    pub chronicles: Vec<ChronicleStream>,
    /// De lexostatussen die de cel naar buiten publiceert.
    #[serde(default)]
    pub lexostatus_definitions: Vec<LexostatusDefinition>,
}

/// Eén gepubliceerde lexostatus met haar gedocumenteerde parameters en reductie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexostatusDefinition {
    /// De naam waarmee een consument deze lexostatus opvraagt.
    pub name: String,
    /// Vrije toelichting; verschijnt niet in het antwoord, wel in de config.
    #[serde(default)]
    pub doc: Option<String>,
    /// De gedocumenteerde parameters. Een vraag die hiervan afwijkt, faalt.
    #[serde(default)]
    pub inputs: Vec<LexostatusInput>,
    /// Hoe de cel over haar eigen feiten reduceert.
    pub reduction: Reduction,
}

/// Eén gedocumenteerde parameter van een lexostatus.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexostatusInput {
    /// Parameternaam, waarnaar de reductie met `$naam` verwijst.
    pub name: String,
    /// Het verwachte type van de meegegeven waarde.
    #[serde(rename = "type")]
    pub value_type: ParameterType,
}

/// De typen die een lexostatus-parameter kan hebben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    /// Tekst, bijvoorbeeld een BSN.
    String,
    /// Geheel of decimaal getal.
    Number,
    /// Waar of niet waar.
    Boolean,
}

impl ParameterType {
    /// De naam zoals die in foutmeldingen verschijnt.
    fn label(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }

    /// Past deze waarde bij het gedocumenteerde type?
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String => matches!(value, Value::String(_)),
            Self::Number => matches!(value, Value::Int(_) | Value::Decimal(_)),
            Self::Boolean => matches!(value, Value::Bool(_)),
        }
    }
}

/// De chronolexoreductie: welke uitkomst van welke eigen regeling de cel over
/// haar eigen feiten berekent, en met welke parameters.
///
/// In deze eerste versie is de reductie precies één uitkomst van één eigen
/// regeling. Rijkere vormen (filteren en aggregeren over meerdere kronieken)
/// passen in dezelfde plek in de configuratie zonder dat de publieke ingang van
/// de cel verandert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reduction {
    /// De regeling, bij `$id`. Moet in `laws` van dezelfde cel staan.
    pub regulation: String,
    /// De uitkomst van die regeling die de reductie moet opleveren.
    ///
    /// Dit stuurt de evaluatie aan; het begrenst het antwoord niet. Een
    /// lexostatus is een rechtstoestand, en de engine levert alle uitkomsten die
    /// ze onderweg naar deze berekende. Een scenario mag daar dus ook op
    /// controleren.
    pub output: String,
    /// De parameters voor de regeling. Een waarde `$naam` verwijst naar een
    /// gedocumenteerde parameter van de lexostatus; elke andere waarde is een
    /// letterlijke tekst.
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

impl LexostatusDefinition {
    /// Controleer de definitie tegen de cel waarin ze staat.
    ///
    /// Twee dingen moeten kloppen voordat een consument er ooit bij kan: de
    /// reductie mag alleen een eigen regeling van de cel raken, en elke
    /// `$`-verwijzing moet een gedocumenteerde parameter zijn.
    pub(crate) fn validate(&self, cell: &str, own_laws: &[String]) -> Result<()> {
        if !own_laws.contains(&self.reduction.regulation) {
            return Err(SimulatorError::ForeignRegulation {
                cell: cell.to_string(),
                lexostatus: self.name.clone(),
                regulation: self.reduction.regulation.clone(),
            });
        }

        for reference in self
            .reduction
            .parameters
            .values()
            .filter_map(|binding| binding_name(binding))
        {
            if !self.inputs.iter().any(|input| input.name == reference) {
                return Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    lexostatus: self.name.clone(),
                    reference: reference.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Zet de vraag van een consument om in parameters voor de engine.
    ///
    /// Weigert een ontbrekende parameter, een niet-gedocumenteerde parameter en
    /// een parameter van het verkeerde type. Dat is wat "gedocumenteerde
    /// parameters" waard maakt: de cel accepteert precies wat ze publiceert.
    pub(crate) fn bind(
        &self,
        cell: &str,
        params: &BTreeMap<String, Value>,
    ) -> Result<BTreeMap<String, Value>> {
        for supplied in params.keys() {
            if !self.inputs.iter().any(|input| &input.name == supplied) {
                return Err(SimulatorError::UndocumentedParameter {
                    cell: cell.to_string(),
                    lexostatus: self.name.clone(),
                    parameter: supplied.clone(),
                    documented: self.documented_parameters(),
                });
            }
        }

        for input in &self.inputs {
            let Some(value) = params.get(&input.name) else {
                return Err(SimulatorError::MissingParameter {
                    cell: cell.to_string(),
                    lexostatus: self.name.clone(),
                    parameter: input.name.clone(),
                });
            };
            if !input.value_type.accepts(value) {
                return Err(SimulatorError::ParameterType {
                    cell: cell.to_string(),
                    lexostatus: self.name.clone(),
                    parameter: input.name.clone(),
                    expected: input.value_type.label(),
                    actual: value.type_name(),
                });
            }
        }

        let mut bound = BTreeMap::new();
        for (name, binding) in &self.reduction.parameters {
            let value = match binding_name(binding) {
                Some(reference) => params.get(reference).cloned().unwrap_or(Value::Null),
                None => Value::String(binding.clone()),
            };
            bound.insert(name.clone(), value);
        }
        Ok(bound)
    }

    /// Komma-gescheiden lijst van gedocumenteerde parameters, voor foutmeldingen.
    fn documented_parameters(&self) -> String {
        self.inputs
            .iter()
            .map(|input| input.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// `"$bsn"` → `Some("bsn")`; alles zonder `$` is een letterlijke waarde.
fn binding_name(binding: &str) -> Option<&str> {
    binding.strip_prefix('$')
}
