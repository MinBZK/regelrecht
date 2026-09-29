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
    /// Naar welke grammen dit gram verwijst, per naam uit de wettekst
    /// (`op_aanvraag`, `besluit`, `wijzigt`, ...): het id van dat gram.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub verwijst: BTreeMap<String, String>,
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
    /// De wortel van het gram, uit de index van de kroniek: het id van het
    /// gram zonder verwijzing waarop het via zijn verwijzingen uitkomt (een
    /// gram zonder verwijzing is zijn eigen wortel). Geen deel van het gram:
    /// de kroniek vult het in bij het laden en het vastleggen.
    #[serde(skip)]
    pub wortel: Option<String>,
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

/// Welke stroomdefinitie een gram bouwde, en welke versie ervan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StroomVerwijzing {
    pub id: String,
    pub sha256: String,
}

/// Het voorvoegsel van een filtersleutel op een verwijzing:
/// `verwijst.<naam>` is het id waarnaar het gram onder die naam verwijst.
pub const VERWIJST: &str = "verwijst.";

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

/// Het vaste id dat een gram uit een kroniek van voor v0.2.0 bij het laden
/// krijgt: een uuid v5 over `sleutel`, zodat elke lezing hetzelfde id geeft.
pub fn vast_id(sleutel: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, sleutel.as_bytes()).to_string()
}

/// Maak een gram uit een kroniek van voor v0.2.0 (met `zaak`,
/// `zaakkenmerk`, `besluit`, `besluitkenmerk`, `wijzigt`) leesbaar: het
/// krijgt een vast id en verwijzingen. Een gram dat een zaak opende (de
/// aanvraag), krijgt zijn zaakkenmerk als id; een besluit (opent of wijzigt)
/// een id uit zijn besluitkenmerk; een ander gram een id uit `bron` (de
/// kroniek en de regel zelf). Een gram dat een besluit volgde, verwijst er
/// met `besluit` naar, een wijziging met `wijzigt`, en een ander gram in een
/// zaak met `zaak` naar het gram dat de zaak opende. Welke naam de wet voor
/// die laatste verwijzing heeft, weet het oude gram niet; de groep (de
/// wortel) klopt wel. Waar als er iets te migreren viel.
pub fn migreer(doc: &mut Value, bron: &str) -> bool {
    let Some(o) = doc.as_object_mut() else {
        return false;
    };
    if o.contains_key("id") {
        return false;
    }
    let tekst = |o: &mut Map<String, Value>, k: &str| {
        o.remove(k).and_then(|v| v.as_str().map(str::to_string))
    };
    let zaak = tekst(o, "zaak");
    let zaakkenmerk = tekst(o, "zaakkenmerk");
    let besluit = tekst(o, "besluit");
    let besluitkenmerk = tekst(o, "besluitkenmerk");
    let wijzigt = tekst(o, "wijzigt");
    let besluit_id = |k: &str| vast_id(&format!("urn:regelrecht:cel:besluitkenmerk:{k}"));
    let is_besluit = matches!(besluit.as_deref(), Some("opent" | "wijzigt"));
    let id = match (&zaak, &zaakkenmerk, &besluitkenmerk) {
        (Some(z), Some(k), _) if z == "opent" => k.clone(),
        (_, _, Some(k)) if is_besluit => besluit_id(k),
        _ => vast_id(&format!("urn:regelrecht:cel:gram:{bron}")),
    };
    let mut verwijst = Map::new();
    match (besluit.as_deref(), &besluitkenmerk, &wijzigt) {
        (Some("volgt"), Some(k), _) => {
            verwijst.insert("besluit".into(), Value::String(besluit_id(k)));
        }
        (Some("wijzigt"), _, Some(w)) => {
            verwijst.insert("wijzigt".into(), Value::String(besluit_id(w)));
        }
        _ => {}
    }
    if verwijst.is_empty() && zaak.as_deref() == Some("volgt") {
        if let Some(z) = zaakkenmerk {
            verwijst.insert("zaak".into(), Value::String(z));
        }
    }
    o.insert("id".into(), Value::String(id));
    if !verwijst.is_empty() {
        o.insert("verwijst".into(), Value::Object(verwijst));
    }
    true
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

    /// Een veld van het gram zelf waarop een filter selecteert (een sleutel
    /// uit [`crate::reductie::GRAM_SLEUTELS`]), als tekst. `None` als
    /// `sleutel` geen zo'n veld is: dan is het een veldpad onder `fields`.
    /// `Some(None)` als het gram het veld niet heeft.
    pub fn kenmerk(&self, sleutel: &str) -> Option<Option<&str>> {
        if let Some(naam) = sleutel.strip_prefix(VERWIJST) {
            return Some(self.verwijst.get(naam).map(String::as_str));
        }
        Some(match sleutel {
            "id" => Some(self.id.as_str()),
            "wortel" => self.wortel.as_deref(),
            "name" => Some(self.name.as_str()),
            "type" => Some(self.type_.as_str()),
            "soort" => self.soort.as_deref(),
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

/// Een gram voor tests: een melding in `test_kroniek` met dit id, zonder
/// verwijzing (dus zijn eigen wortel).
#[cfg(test)]
pub(crate) fn testgram(id: &str) -> Gram {
    Gram {
        kind: "chronolexogram".into(),
        id: id.into(),
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
        verwijst: BTreeMap::new(),
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
        wortel: Some(id.into()),
    }
}

/// Een gram voor tests dat met `naam` naar `doel` verwijst, met een nieuw
/// id; de wortel is die van het doel.
#[cfg(test)]
pub(crate) fn testvolger(naam: &str, doel: &Gram) -> Gram {
    let mut g = testgram(&uuid::Uuid::now_v7().to_string());
    g.verwijst.insert(naam.into(), doel.id.clone());
    g.wortel = doel.wortel.clone();
    g
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Een gram uit een kroniek van voor v0.2.0 krijgt een vast id en
    /// verwijzingen: de aanvraag haar zaakkenmerk, een besluit een id uit
    /// zijn besluitkenmerk, en wie het besluit volgt, verwijst ernaar.
    #[test]
    fn een_oud_gram_wordt_gemigreerd() {
        let z = "00000000-0000-4000-8000-000000000001";
        let mut aanvraag = json!({"name": "a", "zaak": "opent", "zaakkenmerk": z});
        assert!(migreer(&mut aanvraag, "k:1"));
        assert_eq!(aanvraag["id"], z);
        assert!(aanvraag.get("zaak").is_none() && aanvraag.get("verwijst").is_none());
        let mut besluit = json!({"name": "b", "zaak": "volgt", "zaakkenmerk": z, "besluit": "opent", "besluitkenmerk": format!("{z}/1")});
        migreer(&mut besluit, "k:2");
        let mut betaling = json!({"name": "c", "zaak": "volgt", "zaakkenmerk": z, "besluit": "volgt", "besluitkenmerk": format!("{z}/1")});
        migreer(&mut betaling, "k:3");
        assert_eq!(betaling["verwijst"]["besluit"], besluit["id"]);
        let mut verloop = json!({"name": "d", "zaak": "volgt", "zaakkenmerk": z});
        migreer(&mut verloop, "k:4");
        assert_eq!(verloop["verwijst"]["zaak"], z);
        // Een tweede lezing geeft hetzelfde id; een nieuw gram blijft zoals het is.
        let mut nog = json!({"name": "d", "zaak": "volgt", "zaakkenmerk": z});
        migreer(&mut nog, "k:4");
        assert_eq!(nog["id"], verloop["id"]);
        assert!(!migreer(&mut nog, "k:4"));
    }

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
