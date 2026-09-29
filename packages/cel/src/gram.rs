//! Het gram: een feit zoals een cel het vastlegt
//! (`schema/chronolex/v0.2.0/gram.json`), met wat een besluit erbij draagt.
//!
//! Een gram heeft een eigen id (een uuid v7, dat de cel bij het vastleggen
//! geeft) en verwijst met een naam uit de wettekst naar het gram waar het bij
//! hoort (`verwijst`: een besluit `op_aanvraag`, een betaling naar het
//! `besluit`). Er is geen zaak- of besluitkenmerk: een groep volgt uit de
//! verwijzingen. De wortel van een gram (het gram zonder verwijzing waar het
//! via zijn verwijzingen op uitkomt, zoals de aanvraag) houdt de kroniek bij in
//! een index; zij staat niet in het gram.
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
//!   bij een startstand de laadtijd.
//!
//! Een kroniek van voor chronolex v0.2.0 (zonder id, met zaak- en
//! besluitkenmerk) wordt niet omgezet: de cel weigert haar te laden.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::datum;
use crate::schema::{self, Soort};

/// Het vastgelegde gram (`schema/chronolex/v0.2.0/gram.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gram {
    pub kind: String,
    /// Het eigen id: een uuid v7, dat de cel bij het vastleggen geeft (onder
    /// hetzelfde slot als `vastgelegd_op`).
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub name: String,
    pub chronicle: String,
    pub recording_actor: String,
    pub legal_basis: Vec<String>,
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
    pub acting_actor: Option<HandelendeActor>,
    /// Wanneer het feit rechtens geldt of plaatsvond.
    pub effective_at: String,
    /// Alleen als het event `op_moment` aan een ingediende waarde bond en die
    /// waarde er was: de grondslag daarvan, uit de stroom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at_legal_basis: Option<Vec<String>>,
    /// Wanneer de cel het gram vastlegde: haar eigen klok.
    pub recorded_at: String,
    /// Naar welke grammen dit gram verwijst, per naam uit de wettekst
    /// (`op_aanvraag`, `besluit`, `wijzigt`, ...): het id van dat gram.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refers_to: BTreeMap<String, String>,
    pub stream: StroomVerwijzing,
    /// Alleen als de cel het gram niet zelf vaststelde: `startstand` is bij
    /// het starten in een lege kroniek geplaatst (zie [`crate::startstand`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
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
    /// De wortel van het gram, uit de index van de kroniek: het id van het
    /// gram zonder verwijzing waarop het via zijn verwijzingen uitkomt (een
    /// gram zonder verwijzing is zijn eigen wortel). Geen deel van het gram:
    /// de kroniek vult het in bij het laden en het vastleggen.
    #[serde(skip)]
    pub root: Option<String>,
}

/// De gelezen tijden van een gram, elk met de tekst waaruit het gelezen is.
/// Verandert de tekst (het stempel zet `vastgelegd_op`), dan leest het gram
/// haar opnieuw; de cache telt niet mee in een vergelijking.
#[derive(Debug, Clone, Default)]
pub struct Tijden {
    moment: OnceLock<(String, DateTime<FixedOffset>)>,
    recorded: OnceLock<(String, DateTime<FixedOffset>)>,
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
    pub role: String,
    pub channel: String,
    pub identity: BTreeMap<String, String>,
    /// De grondslag van de rol, als de configuratie er een noemt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<String>,
    /// Het gezag in wiens naam is gehandeld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<String>,
    /// De grondslag van het mandaat, als het gezag niet het eigen gezag van
    /// het proces is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandate: Option<String>,
}

/// Een geaccepteerde invoer van een besluit: een waarde met haar herkomst.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invoer {
    pub value: Value,
    pub provenance: crate::synthese::Herkomst,
}

/// Wat er bij een besluit meedeed, zodat het te herhalen is: de geladen
/// regelingen en de stroomdefinities, met een hash over beide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub regulations: Vec<GeladenRegeling>,
    pub streams: Vec<StroomVerwijzing>,
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
    pub fn nieuw(regulations: Vec<GeladenRegeling>, streams: Vec<StroomVerwijzing>) -> Self {
        let canoniek =
            serde_json::json!({"regulations": regulations, "streams": streams}).to_string();
        Self {
            regulations,
            streams,
            sha256: hex::encode(Sha256::digest(canoniek.as_bytes())),
        }
    }
}

/// Welke stroomdefinitie een gram bouwde, en welke versie ervan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StroomVerwijzing {
    pub id: String,
    pub sha256: String,
}

/// Het voorvoegsel van een filtersleutel op een verwijzing:
/// `verwijst.<naam>` is het id waarnaar het gram onder die naam verwijst.
pub const VERWIJST: &str = "refers_to.";

/// Een nieuw id voor een gram: een uuid v7 op het moment `nu` (de klok van
/// de cel), zodat ids in de tijd oplopen.
pub fn nieuw_id(nu: DateTime<FixedOffset>) -> String {
    let ts = uuid::Timestamp::from_unix(
        uuid::NoContext,
        u64::try_from(nu.timestamp()).unwrap_or(0),
        nu.timestamp_subsec_nanos(),
    );
    uuid::Uuid::new_v7(ts).to_string()
}

/// Een vast id: een uuid v5 over `sleutel`, zodat elke lezing hetzelfde id
/// geeft (voor een regel van een startstand zonder eigen id).
pub fn vast_id(sleutel: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, sleutel.as_bytes()).to_string()
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
        if let Err(f) = datum::moment(&self.effective_at) {
            fouten.push(f);
        }
        if let Err(f) = datum::moment_van("recorded_at", &self.recorded_at) {
            fouten.push(f);
        }
        if fouten.is_empty() {
            Ok(())
        } else {
            Err(fouten)
        }
    }

    /// Een veld van het gram zelf waarop een filter selecteert (een sleutel
    /// uit [`crate::reductie::GRAM_SLEUTELS`]), als tekst. `None` als
    /// `sleutel` geen zo'n veld is: dan is het een veldpad onder `fields`.
    /// `Some(None)` als het gram het veld niet heeft.
    pub fn kenmerk(&self, sleutel: &str) -> Option<Option<&str>> {
        if let Some(name) = sleutel.strip_prefix(VERWIJST) {
            return Some(self.refers_to.get(name).map(String::as_str));
        }
        Some(match sleutel {
            "id" => Some(self.id.as_str()),
            "root" => self.root.as_deref(),
            "name" => Some(self.name.as_str()),
            "type" => Some(self.type_.as_str()),
            "subtype" => self.subtype.as_deref(),
            "stage" => self.stage.as_deref(),
            "recording_actor" => Some(self.recording_actor.as_str()),
            "chronicle" => Some(self.chronicle.as_str()),
            "legal_character" => self.legal_character.as_deref(),
            "decision_type" => self.decision_type.as_deref(),
            "regulation" => self.regulation.as_deref(),
            "competent_authority" => self.competent_authority.as_deref(),
            _ => return None,
        })
    }

    /// De waarde op een pad onder `fields`.
    pub fn field(&self, path: &str) -> Option<&Value> {
        op_pad(&self.fields, path)
    }

    /// Het `op_moment`, gelezen; een ongeldig moment is een fout.
    pub fn moment(&self) -> Result<DateTime<FixedOffset>, String> {
        gelezen(&self.tijden.moment, &self.effective_at, || {
            datum::moment(&self.effective_at).map_err(|e| format!("gram '{}': {e}", self.name))
        })
    }

    /// Het `vastgelegd_op`, gelezen; een ongeldig moment is een fout.
    pub fn recorded(&self) -> Result<DateTime<FixedOffset>, String> {
        gelezen(&self.tijden.recorded, &self.recorded_at, || {
            datum::moment_van("recorded_at", &self.recorded_at)
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
        if self.effective_at_legal_basis.is_none() && self.provenance.is_none() {
            self.effective_at = tekst.clone();
        } else if self.moment()? > moment {
            return Err(format!(
                "op_moment {} ligt na het vastleggen ({tekst}): wat nog moet gebeuren, wordt niet vastgelegd",
                self.effective_at
            ));
        }
        self.recorded_at = tekst;
        Ok(())
    }

    /// De volgorde van twee grammen in de tijd: eerst op `op_moment`, bij
    /// gelijk moment op `vastgelegd_op`. `Equal` laat de volgorde in de
    /// kroniek beslissen.
    pub fn tijdvolgorde(&self, ander: &Gram) -> Result<std::cmp::Ordering, String> {
        Ok(self
            .moment()?
            .cmp(&ander.moment()?)
            .then(self.recorded()?.cmp(&ander.recorded()?)))
    }
}

/// De waarde op een veldpad met punten (`inhoud.organen`) in een object;
/// `None` als een deel van het pad er niet is of geen object is.
pub fn op_pad<'v>(fields: &'v Map<String, Value>, path: &str) -> Option<&'v Value> {
    let mut delen = path.split('.');
    let mut huidig = fields.get(delen.next()?)?;
    for deel in delen {
        huidig = huidig.as_object()?.get(deel)?;
    }
    Some(huidig)
}
/// Zet een waarde op een veldpad met punten, en maak de tussenliggende
/// objecten; wat op de weg geen object is, wordt er een.
pub fn zet_pad(doel: &mut Map<String, Value>, path: &str, value: Value) {
    let mut delen = path.split('.').peekable();
    let mut hier = doel;
    while let Some(deel) = delen.next() {
        if delen.peek().is_none() {
            hier.insert(deel.to_string(), value);
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

/// Een gram voor tests: een melding in `test_kroniek` met dit id, zonder
/// verwijzing (dus zijn eigen wortel).
#[cfg(test)]
pub(crate) fn testgram(id: &str) -> Gram {
    Gram {
        kind: "chronolexogram".into(),
        id: id.into(),
        type_: "submission".into(),
        subtype: Some("melding".into()),
        stage: None,
        name: "melding_ontvangen".into(),
        chronicle: "test_kroniek".into(),
        recording_actor: "test_instantie".into(),
        legal_basis: vec!["testregeling_aanvraag#1".into()],
        legal_character: None,
        decision_type: None,
        regulation: None,
        regulation_valid_from: None,
        competent_authority: None,
        acting_actor: None,
        effective_at: "2025-03-12T10:14:03+01:00".into(),
        effective_at_legal_basis: None,
        recorded_at: "2025-03-12T10:14:05+01:00".into(),
        refers_to: BTreeMap::new(),
        stream: StroomVerwijzing {
            id: "test".into(),
            sha256: "a".repeat(64),
        },
        provenance: None,
        fields: serde_json::json!({"x": 1})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        inputs: BTreeMap::new(),
        receipt: None,
        tijden: Tijden::default(),
        root: Some(id.into()),
    }
}

/// Een gram voor tests dat met `naam` naar `doel` verwijst, met een nieuw
/// id; de wortel is die van het doel.
#[cfg(test)]
pub(crate) fn testvolger(name: &str, doel: &Gram) -> Gram {
    let mut g = testgram(&uuid::Uuid::now_v7().to_string());
    g.refers_to.insert(name.into(), doel.id.clone());
    g.root = doel.root.clone();
    g
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
        let eerst = g.recorded().unwrap();
        assert_eq!(g.recorded().unwrap(), eerst);
        let later = DateTime::parse_from_rfc3339("2025-03-13T09:00:00+01:00").unwrap();
        g.stempel(later, None).unwrap();
        assert_eq!(g.recorded().unwrap(), later);
        assert_eq!(
            g.moment().unwrap(),
            later,
            "een ongebonden op_moment schuift mee"
        );
        g.effective_at = "geen moment".into();
        assert!(g.moment().is_err());
    }
}
