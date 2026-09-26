//! Het peil van een reductie: op welk moment de kroniek gelezen wordt
//! ("tijdreizen", paper P:94). Feiten worden nooit verwijderd, dus elke
//! eerdere stand is terug te halen.
//!
//! Een gram heeft twee tijden (zie [`crate::gram`]), en een reductie kan op
//! elk van beide peilen, of op beide (bitemporeel):
//!
//! - `peilmoment` (geldigheidstijd): de stand zoals die rechtens gold op T,
//!   met alles wat de cel nu weet. Een gram telt als zijn `op_moment` op of
//!   voor T ligt. Een feit dat later werd vastgelegd maar eerder gold (een
//!   papieren aanvraag, ontvangen op dag 1 en ingevoerd op dag 5) telt dus
//!   mee voor T = dag 3; een feit van na T (een schrapping op dag 4) niet.
//!   Een `op_moment` ligt nooit na het vastleggen: wat nog moet gebeuren,
//!   is geen feit. Een besluit met werking vanaf een latere dag staat er op
//!   de dag waarop het genomen is, met de dag van ingang als veld; op die dag
//!   van ingang peilen kan een reductie (nog) niet.
//! - `bekend_op` (registratietijd): de stand zoals de cel haar kende op T.
//!   Een gram telt als zijn `vastgelegd_op` op of voor T ligt. Zo is terug
//!   te zien waarop een eerder besluit rustte.
//!
//! Zonder peil telt elk gram: de stand zoals die nu geldt en nu bekend is,
//! inclusief wat pas later ingaat. Een proces dat een datum bedoelt (de
//! peildatum van een besluit, het begin van een tijdvak) geeft die dus mee.
//! Een peil is een datum (`JJJJ-MM-DD`, de hele dag telt) of een moment met
//! tijdzone. Over HTTP zijn het de query-parameters `peilmoment` en
//! `bekend_op` van `GET lexostatus`; die namen zijn daarom geen input van
//! een lexostatus (het schema weert ze).

use serde_json::{Map, Value};

use crate::datum::Tijdpunt;
use crate::gram::Gram;

/// Waarop een reductie peilt. De standaard is geen peil: elk gram telt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Peil {
    /// Geldigheidstijd: alleen grammen met `op_moment` op of voor dit punt.
    pub peilmoment: Option<Tijdpunt>,
    /// Registratietijd: alleen grammen met `vastgelegd_op` op of voor dit
    /// punt.
    pub bekend_op: Option<Tijdpunt>,
}

impl Peil {
    /// De stand zoals die rechtens gold op `t`, met wat nu bekend is.
    pub fn op(t: Tijdpunt) -> Self {
        Self {
            peilmoment: Some(t),
            bekend_op: None,
        }
    }

    /// Lees het peil uit de query van een vraag, en haal de sleutels eruit:
    /// wat overblijft zijn de inputs van de lexostatus.
    pub fn uit_query(query: &mut Map<String, Value>) -> Result<Self, String> {
        let mut lees = |sleutel: &str| -> Result<Option<Tijdpunt>, String> {
            match query.remove(sleutel) {
                None => Ok(None),
                Some(Value::String(t)) => Tijdpunt::lees(sleutel, &t).map(Some),
                Some(w) => Err(format!("ongeldig {sleutel} '{w}'")),
            }
        };
        Ok(Self {
            peilmoment: lees("peilmoment")?,
            bekend_op: lees("bekend_op")?,
        })
    }

    /// Het peil als query-parameters: `peilmoment`, dan `bekend_op`. Het
    /// schema van `lexostatus.json` weert die namen als input.
    pub fn query(&self) -> Vec<(&'static str, String)> {
        [
            ("peilmoment", self.peilmoment),
            ("bekend_op", self.bekend_op),
        ]
        .into_iter()
        .filter_map(|(k, t)| t.map(|t| (k, t.to_string())))
        .collect()
    }

    /// Of een gram bij dit peil telt. Een ongeldig moment is een fout, geen
    /// stille uitsluiting.
    pub fn laat_door(&self, gram: &Gram) -> Result<bool, String> {
        if let Some(t) = &self.peilmoment {
            if !t.omvat(&gram.moment()?) {
                return Ok(false);
            }
        }
        if let Some(t) = &self.bekend_op {
            if !t.omvat(&gram.vastgelegd()?) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
