//! Het gram: een feit zoals een cel het vastlegt
//! (`schema/chronolex/v0.1.0/gram.json`), met wat een besluit erbij draagt.
//! Hoe een gram uit een stroom en een indiening ontstaat, staat in
//! [`crate::stroom`].
//!
//! Een gram heeft twee tijden (paper P:46: "op 3 april heeft de ambtenaar
//! vastgesteld dat ... per 2 april"):
//!
//! - `op_moment`: wanneer het feit rechtens geldt of plaatsvond. Standaard is
//!   dat het moment van vastleggen; een event kan het aan een ingediende
//!   waarde binden, met grondslag (`op_moment_grondslag`), zoals de dag van
//!   ontvangst van een aanvraag die langs een andere weg binnenkwam (Awb 4:1,
//!   4:13). In een startstand is het met de hand gezet (de datum van een
//!   besluit, of van een vaststelling).
//! - `vastgelegd_op`: wanneer de cel het vastlegde, altijd haar eigen klok;
//!   bij een startstand de laadtijd. Een gram van voor dit veld heeft het
//!   niet: dan geldt het `op_moment` (zie [`Gram::vul_vastgelegd_op`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::datum;
use crate::schema::{self, Soort};
use crate::stroom::{Besluit, Zaak};

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
    /// Alleen bij een besluit dat een proces nam: wie handelde (zie
    /// [`HandelendeActor`]). Naast `recording_actor` (wie vastlegt) en
    /// `competent_authority` (wie de wet bevoegd maakt) de derde as van
    /// RFC-022 §2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handelende_actor: Option<HandelendeActor>,
    /// Wanneer het feit rechtens geldt of plaatsvond.
    pub op_moment: String,
    /// Alleen als het event `op_moment` aan een ingediende waarde bond en die
    /// waarde er was: de grondslag daarvan, uit de stroom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_moment_grondslag: Option<Vec<String>>,
    /// Wanneer de cel het gram vastlegde: haar eigen klok. Leeg bij een gram
    /// van voor dit veld, tot [`Gram::vul_vastgelegd_op`] het invult.
    #[serde(default)]
    pub vastgelegd_op: String,
    /// Uit de stroom: of het gram een zaak opent of volgt. Weggelaten als
    /// het event geen zaak heeft.
    #[serde(default, skip_serializing_if = "zonder_zaak")]
    pub zaak: Zaak,
    /// Alleen bij een event met een zaak (`zaak: opent` of `volgt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zaakkenmerk: Option<String>,
    /// Uit de stroom: of het gram in de zaak een besluit opent, volgt of
    /// wijzigt. Weggelaten als het bij geen besluit hoort (zoals de
    /// aanvraag, die de zaak opent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besluit: Option<Besluit>,
    /// Alleen bij een gram met `besluit`: het kenmerk van het besluit. Bij
    /// `opent` en `wijzigt` geeft de cel het (`<zaakkenmerk>/<volgnummer>`),
    /// bij `volgt` is het dat van het besluit dat het gram volgt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besluitkenmerk: Option<String>,
    /// Alleen bij `besluit: wijzigt`: het kenmerk van het besluit dat dit
    /// besluit wijzigt (RFC-022 par. 3.1 `is_wijziging_van`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wijzigt: Option<String>,
    pub stroom: StroomVerwijzing,
    /// Alleen als de cel het gram niet zelf vaststelde: `startstand` is bij
    /// het starten in een lege kroniek geplaatst (zie [`crate::startstand`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herkomst: Option<String>,
    pub fields: Map<String, Value>,
    /// Bij elke handeling die de engine uitrekende (een besluit, een vervolg
    /// of een feit met uitkomsten): elke parameter die meedeed, met haar
    /// waarde en haar herkomst (RFC-013 `accepted_values`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Invoer>,
    /// Bij elke handeling die de engine uitrekende: wat er meedeed, met de
    /// hash erover (RFC-013, RFC-022 par. 1.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
    /// `op_moment` en `vastgelegd_op`, gelezen: een keer per gram, niet bij
    /// elke reductie of elk peil. Geen deel van het gram.
    #[serde(skip)]
    pub tijden: Tijden,
}

/// De gelezen tijden van een gram, elk met de tekst waaruit het gelezen is.
/// Verandert de tekst (het stempel zet `vastgelegd_op`), dan leest het gram
/// haar opnieuw; de cache telt niet mee in een vergelijking.
#[derive(Debug, Clone, Default)]
pub struct Tijden {
    moment: OnceLock<(String, DateTime<FixedOffset>)>,
    vastgelegd: OnceLock<(String, DateTime<FixedOffset>)>,
}

impl PartialEq for Tijden {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Een tijd uit `cache` als die uit `tekst` gelezen is, anders `lees`, en
/// dat bewaard als er nog niets lag.
fn gelezen(
    cache: &OnceLock<(String, DateTime<FixedOffset>)>,
    tekst: &str,
    lees: impl FnOnce() -> Result<DateTime<FixedOffset>, String>,
) -> Result<DateTime<FixedOffset>, String> {
    if let Some((t, m)) = cache.get() {
        if t == tekst {
            return Ok(*m);
        }
    }
    let m = lees()?;
    // Lag er al een tijd van een eerdere tekst, dan blijft die liggen en
    // leest deze tekst elke keer opnieuw: juist, alleen niet gecachet.
    let _ = cache.set((tekst.to_string(), m));
    Ok(m)
}

/// Wie een besluit nam: de rol en het kanaal waarlangs de gebruiker inlogde,
/// met de waarden van de identificatievelden, en namens welk gezag. Handelt
/// het proces in mandaat (Awb 10:1), dan noemt `mandaat` de grondslag. Het
/// kanaal is nagebootst: de identiteit is wat de gebruiker invulde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandelendeActor {
    pub rol: String,
    pub kanaal: String,
    pub identiteit: BTreeMap<String, String>,
    /// De grondslag van de rol, als de configuratie er een noemt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grondslag: Option<String>,
    /// Het gezag in wiens naam is gehandeld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namens: Option<String>,
    /// De grondslag van het mandaat, als het gezag niet het eigen gezag van
    /// het proces is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandaat: Option<String>,
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

    /// Valideer het gram tegen `gram.json`, en zijn `op_moment` en
    /// `vastgelegd_op` als moment met tijdzone (het schema toetst alleen de
    /// vorm).
    pub fn valideer(&self) -> Result<(), Vec<String>> {
        let json = serde_json::to_value(self).map_err(|e| vec![e.to_string()])?;
        let mut fouten = schema::valideer(Soort::Gram, &json)
            .err()
            .unwrap_or_default();
        if let Err(f) = datum::moment(&self.op_moment) {
            fouten.push(f);
        }
        if let Err(f) = datum::moment_van("vastgelegd_op", &self.vastgelegd_op) {
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
        gelezen(&self.tijden.moment, &self.op_moment, || {
            datum::moment(&self.op_moment).map_err(|e| format!("gram '{}': {e}", self.name))
        })
    }

    /// Het `vastgelegd_op`, gelezen; een ongeldig moment is een fout.
    pub fn vastgelegd(&self) -> Result<DateTime<FixedOffset>, String> {
        gelezen(&self.tijden.vastgelegd, &self.vastgelegd_op, || {
            datum::moment_van("vastgelegd_op", &self.vastgelegd_op)
                .map_err(|e| format!("gram '{}': {e}", self.name))
        })
    }

    /// Zet het moment van vastleggen: de cel doet dat onder haar schrijfslot,
    /// zodat de volgorde van de regels in de kroniek die van `vastgelegd_op`
    /// is. `niet_voor` is het `vastgelegd_op` van de laatste regel van de
    /// kroniek: loopt de klok terug, dan krijgt het gram dat moment, niet een
    /// eerder. Een `op_moment` dat het event niet aan een waarde bond
    /// (zonder `op_moment_grondslag`), is het moment van vastleggen en
    /// schuift mee; een gebonden `op_moment` mag er niet na liggen.
    pub fn stempel(
        &mut self,
        nu: DateTime<FixedOffset>,
        niet_voor: Option<DateTime<FixedOffset>>,
    ) -> Result<(), String> {
        let moment = match niet_voor {
            Some(v) if v > nu => v,
            _ => nu,
        };
        let tekst = datum::als_op_moment(&moment);
        if self.op_moment_grondslag.is_none() && self.herkomst.is_none() {
            self.op_moment = tekst.clone();
        } else if self.moment()? > moment {
            return Err(format!(
                "op_moment {} ligt na het vastleggen ({tekst}): wat nog moet gebeuren, wordt niet vastgelegd",
                self.op_moment
            ));
        }
        self.vastgelegd_op = tekst;
        Ok(())
    }

    /// Een gram van voor `vastgelegd_op` (gelezen uit een oudere kroniek)
    /// krijgt zijn `op_moment` als registratietijd: iets beters is er niet.
    /// Waar als het ontbrak, zodat de lezer het kan melden.
    pub fn vul_vastgelegd_op(&mut self) -> bool {
        if !self.vastgelegd_op.is_empty() {
            return false;
        }
        self.vastgelegd_op = self.op_moment.clone();
        true
    }

    /// De volgorde van twee grammen in de tijd: eerst op `op_moment`, bij
    /// gelijk moment op `vastgelegd_op`. `Equal` laat de volgorde in de
    /// kroniek beslissen.
    pub fn tijdvolgorde(&self, ander: &Gram) -> Result<std::cmp::Ordering, String> {
        Ok(self
            .moment()?
            .cmp(&ander.moment()?)
            .then(self.vastgelegd()?.cmp(&ander.vastgelegd()?)))
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
/// Zet een waarde op een veldpad met punten, en maak de tussenliggende
/// objecten; wat op de weg geen object is, wordt er een.
pub fn zet_pad(doel: &mut Map<String, Value>, pad: &str, waarde: Value) {
    let mut delen = pad.split('.').peekable();
    let mut hier = doel;
    while let Some(deel) = delen.next() {
        if delen.peek().is_none() {
            hier.insert(deel.to_string(), waarde);
            return;
        }
        let volgend = hier
            .entry(deel.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !volgend.is_object() {
            *volgend = Value::Object(Map::new());
        }
        let Value::Object(m) = volgend else {
            return;
        };
        hier = m;
    }
}

/// Een gram voor tests: een melding in `test_kroniek` die een zaak opent.
#[cfg(test)]
pub(crate) fn testgram(zaak: &str) -> Gram {
    Gram {
        kind: "chronolexogram".into(),
        type_: "indiening".into(),
        soort: Some("melding".into()),
        stage: None,
        name: "melding_ontvangen".into(),
        chronicle: "test_kroniek".into(),
        recording_actor: "test_instantie".into(),
        grondslag: vec!["testregeling_aanvraag#1".into()],
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        handelende_actor: None,
        op_moment: "2025-03-12T10:14:03+01:00".into(),
        op_moment_grondslag: None,
        vastgelegd_op: "2025-03-12T10:14:05+01:00".into(),
        zaak: Zaak::Opent,
        zaakkenmerk: Some(zaak.into()),
        besluit: None,
        besluitkenmerk: None,
        wijzigt: None,
        stroom: StroomVerwijzing {
            id: "test".into(),
            sha256: "a".repeat(64),
        },
        herkomst: None,
        fields: serde_json::json!({"x": 1})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        inputs: BTreeMap::new(),
        receipt: None,
        tijden: Tijden::default(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn zet_pad_maakt_de_objecten() {
        let mut m = Map::new();
        zet_pad(&mut m, "a.b.c", json!(1));
        zet_pad(&mut m, "a.d", json!(2));
        assert_eq!(op_pad(&m, "a.b.c"), Some(&json!(1)));
        assert_eq!(Value::Object(m), json!({"a": {"b": {"c": 1}, "d": 2}}));
    }

    /// De tijden van een gram worden een keer gelezen, en opnieuw als de
    /// tekst verandert (zoals bij het stempel).
    #[test]
    fn de_gelezen_tijd_volgt_de_tekst() {
        let mut g = testgram("z");
        let eerst = g.vastgelegd().unwrap();
        assert_eq!(g.vastgelegd().unwrap(), eerst);
        let later = DateTime::parse_from_rfc3339("2025-03-13T09:00:00+01:00").unwrap();
        g.stempel(later, None).unwrap();
        assert_eq!(g.vastgelegd().unwrap(), later);
        assert_eq!(
            g.moment().unwrap(),
            later,
            "een ongebonden op_moment schuift mee"
        );
        g.op_moment = "geen moment".into();
        assert!(g.moment().is_err());
    }
}
