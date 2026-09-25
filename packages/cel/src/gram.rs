//! Het gram: een feit zoals een cel het vastlegt
//! (`schema/chronolex/v0.1.0/gram.json`), met wat een besluit erbij draagt.
//! Hoe een gram uit een stroom en een indiening ontstaat, staat in
//! [`crate::stroom`].

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::datum;
use crate::schema::{self, Soort};
use crate::stroom::Zaak;

/// Het vastgelegde gram (`schema/chronolex/v0.1.0/gram.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gram {
    pub kind: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub name: String,
    pub chronicle: String,
    pub recording_actor: String,
    pub grondslag: Vec<String>,
    /// Alleen bij een besluit dat een proces nam: het rechtskarakter en de
    /// soort beslissing uit `produces` van het artikel (RFC-008).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_character: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_type: Option<String>,
    /// De regeling waarop het besluit rust, met de versie ervan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_valid_from: Option<String>,
    /// Het bevoegd gezag volgens de wet. Ontbreekt het in de regeling, dan
    /// staat het er niet: de cel verzint geen gezag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competent_authority: Option<String>,
    pub op_moment: String,
    /// Uit de stroom: of het gram een zaak opent of volgt. Weggelaten als
    /// het event geen zaak heeft.
    #[serde(default, skip_serializing_if = "zonder_zaak")]
    pub zaak: Zaak,
    /// Alleen bij een event met een zaak (`zaak: opent` of `volgt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zaakkenmerk: Option<String>,
    pub stroom: StroomVerwijzing,
    /// Alleen als de cel het gram niet zelf vaststelde: `startstand` is bij
    /// het starten in een lege kroniek geplaatst (zie [`crate::startstand`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herkomst: Option<String>,
    pub fields: Map<String, Value>,
    /// Alleen bij een besluit: elke parameter die meedeed, met haar waarde en
    /// haar herkomst (RFC-013 `accepted_values`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Invoer>,
    /// Alleen bij een besluit: wat er meedeed, met de hash erover (RFC-013,
    /// RFC-022 par. 1.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
}

/// Een geaccepteerde invoer van een besluit: een waarde met haar herkomst.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invoer {
    pub waarde: Value,
    pub herkomst: crate::synthese::Herkomst,
}

/// Wat er bij een besluit meedeed, zodat het te herhalen is: de geladen
/// regelingen en de stroomdefinities, met een hash over beide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub regelingen: Vec<GeladenRegeling>,
    pub stromen: Vec<StroomVerwijzing>,
    /// SHA-256 over de twee lijsten hierboven, als canonieke JSON.
    pub sha256: String,
}

/// Een regeling zoals de runtime haar laadde.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GeladenRegeling {
    pub id: String,
    pub valid_from: String,
    pub sha256: String,
}

impl Receipt {
    /// Bouw het receipt en reken de hash uit.
    pub fn nieuw(regelingen: Vec<GeladenRegeling>, stromen: Vec<StroomVerwijzing>) -> Self {
        let canoniek =
            serde_json::json!({"regelingen": regelingen, "stromen": stromen}).to_string();
        Self {
            regelingen,
            stromen,
            sha256: hex::encode(Sha256::digest(canoniek.as_bytes())),
        }
    }
}

fn zonder_zaak(z: &Zaak) -> bool {
    !z.heeft_kenmerk()
}

/// Welke stroomdefinitie een gram bouwde, en welke versie ervan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StroomVerwijzing {
    pub id: String,
    pub sha256: String,
}

impl Gram {
    /// Het gram als JSON-waarde.
    pub fn als_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Valideer het gram tegen `gram.json`, en zijn `op_moment` als moment
    /// met tijdzone (het schema zegt alleen dat het tekst is).
    pub fn valideer(&self) -> Result<(), Vec<String>> {
        let json = serde_json::to_value(self).map_err(|e| vec![e.to_string()])?;
        let mut fouten = schema::valideer(Soort::Gram, &json)
            .err()
            .unwrap_or_default();
        if let Err(f) = datum::moment(&self.op_moment) {
            fouten.push(f);
        }
        if fouten.is_empty() {
            Ok(())
        } else {
            Err(fouten)
        }
    }

    /// De waarde op een pad onder `fields`.
    pub fn veld(&self, pad: &str) -> Option<&Value> {
        op_pad(&self.fields, pad)
    }

    /// Het `op_moment`, gelezen; een ongeldig moment is een fout.
    pub fn moment(&self) -> Result<DateTime<FixedOffset>, String> {
        datum::moment(&self.op_moment).map_err(|e| format!("gram '{}': {e}", self.name))
    }
}

/// De waarde op een veldpad met punten (`inhoud.organen`) in een object;
/// `None` als een deel van het pad er niet is of geen object is.
pub fn op_pad<'v>(velden: &'v Map<String, Value>, pad: &str) -> Option<&'v Value> {
    let mut delen = pad.split('.');
    let mut huidig = velden.get(delen.next()?)?;
    for deel in delen {
        huidig = huidig.as_object()?.get(deel)?;
    }
    Some(huidig)
}
